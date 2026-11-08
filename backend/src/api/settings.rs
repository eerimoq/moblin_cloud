use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use log::info;
use serde::{Deserialize, Serialize};

use crate::pages::{
    CURRENCIES, Effects, FONTS, Feature, Look, Settings, SettingsError, Spin, Tiers, Tips, is_color,
};
use crate::stripe::Stripe;

use super::*;

#[derive(Serialize)]
pub(super) struct SettingsResponse {
    id: String,
    connected: bool,
    currency: String,
    tips: Option<Feature<Tips>>,
    spin: Option<Feature<Spin>>,
    effects: Option<Feature<Effects>>,
    #[serde(rename = "subSpins")]
    sub_spins: Option<Feature<Tiers>>,
    #[serde(rename = "subEffects")]
    sub_effects: Option<Feature<Tiers>>,
    paypal: bool,
    #[serde(rename = "merchantId")]
    merchant_id: Option<String>,
    #[serde(rename = "paypalPending")]
    paypal_pending: bool,
    #[serde(rename = "paypalSandbox")]
    paypal_sandbox: bool,
    stripe: bool,
    #[serde(rename = "stripeAccountId")]
    stripe_account_id: Option<String>,
    #[serde(rename = "stripePending")]
    stripe_pending: bool,
    #[serde(rename = "stripeTest")]
    stripe_test: bool,
    payable: bool,
    message: Option<String>,
    accent: Option<String>,
    font: Option<String>,
    image: Option<String>,
}

pub(super) fn settings_response(
    api: &Api,
    account: &Account,
    look: Look,
) -> Json<SettingsResponse> {
    let page_id = account.page_id.clone();
    let settings = look.settings;
    let (merchant_id, stripe_account_id) = payment_targets(api, account);
    let payable = merchant_id.is_some() || stripe_account_id.is_some();
    Json(SettingsResponse {
        connected: api.remote_control.is_connected(&page_id),
        currency: settings.currency().to_string(),
        tips: settings.tips,
        spin: settings.spin,
        effects: settings.effects,
        sub_spins: settings.sub_spins,
        sub_effects: settings.sub_effects,
        paypal: api.paypal.is_some(),
        merchant_id: account.merchant_id.clone(),
        paypal_pending: account.paypal_tracking_id.is_some(),
        paypal_sandbox: api
            .paypal
            .as_ref()
            .is_some_and(|paypal| paypal.environment() == "sandbox"),
        stripe: api.stripe.is_some(),
        stripe_pending: account.stripe_account_id.is_some() && !account.stripe_ready,
        stripe_account_id: account
            .stripe_account_id
            .clone()
            .filter(|_| account.stripe_ready),
        stripe_test: api.stripe.as_ref().is_some_and(Stripe::is_test),
        payable,
        message: settings.message,
        accent: settings.accent,
        font: settings.font,
        image: look.image,
        id: page_id,
    })
}

pub(super) fn settings_for(
    user: &PageUser,
    mut response: Json<SettingsResponse>,
) -> Json<SettingsResponse> {
    if user.owner() {
        return response;
    }
    response.paypal = false;
    response.merchant_id = None;
    response.paypal_pending = false;
    response.stripe = false;
    response.stripe_account_id = None;
    response.stripe_pending = false;
    response
}

#[derive(Deserialize)]
pub(super) struct SettingsRequest {
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    tips: Option<Feature<Tips>>,
    #[serde(default)]
    spin: Option<Feature<Spin>>,
    #[serde(default)]
    effects: Option<Feature<Effects>>,
    #[serde(default, rename = "subSpins")]
    sub_spins: Option<Feature<Tiers>>,
    #[serde(default, rename = "subEffects")]
    sub_effects: Option<Feature<Tiers>>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    accent: Option<String>,
    #[serde(default)]
    font: Option<String>,
}

pub(super) fn payment_targets(api: &Api, account: &Account) -> (Option<String>, Option<String>) {
    (
        account.merchant_id.clone().filter(|_| api.paypal.is_some()),
        account
            .stripe_account_id
            .clone()
            .filter(|_| api.stripe.is_some() && account.stripe_ready),
    )
}

fn check_payable(api: &Api, account: &Account) -> Result<(), (StatusCode, String)> {
    if api.paypal.is_none() && api.stripe.is_none() {
        return Err(payments_unavailable());
    }
    if payment_targets(api, account) == (None, None) {
        return Err((
            StatusCode::CONFLICT,
            "connect PayPal or Stripe first".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn payments_unavailable() -> (StatusCode, String) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        "payments are unavailable".to_string(),
    )
}

async fn update_settings(
    api: &Api,
    account: &Account,
    premium: bool,
    request: SettingsRequest,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let page_id = account.page_id.as_str();
    let look = api.pages.look(page_id);
    let current = &look.settings;
    let accent = request.accent.map(|accent| accent.to_lowercase());
    if !premium && accent.as_deref().is_some_and(is_color) && accent != current.accent {
        return Err((
            StatusCode::FORBIDDEN,
            "custom colors are only for premium accounts".to_string(),
        ));
    }
    let font = request.font.filter(|font| font != FONTS[0]);
    if !premium && font.is_some() && font != current.font {
        return Err((
            StatusCode::FORBIDDEN,
            "fonts are only for premium accounts".to_string(),
        ));
    }
    let currency = request
        .currency
        .map(|currency| currency.trim().to_uppercase())
        .unwrap_or_else(|| current.currency().to_string());
    if !CURRENCIES.contains(&currency.as_str()) {
        return Err(invalid_currency());
    }
    let tips = match request.tips {
        Some(tips) => {
            let tips = tips.normalize(Tips::normalize).ok_or_else(invalid_tips)?;
            if tips.enabled && current.tips.as_ref() != Some(&tips) {
                check_payable(api, account)?;
            }
            Some(tips)
        }
        None => None,
    };
    let spin = match request.spin {
        Some(spin) => {
            let spin = spin.normalize(Spin::normalize).ok_or_else(invalid_spin)?;
            if spin.enabled && current.spin.as_ref() != Some(&spin) {
                check_payable(api, account)?;
            }
            Some(spin)
        }
        None => None,
    };
    let effects = match request.effects {
        Some(effects) => {
            let effects = effects
                .normalize(Effects::normalize)
                .ok_or_else(invalid_effects)?;
            if effects.enabled && current.effects.as_ref() != Some(&effects) {
                check_payable(api, account)?;
            }
            Some(effects)
        }
        None => None,
    };
    if currency != current.currency()
        && (current.tips().is_some() || current.spin().is_some() || current.effects().is_some())
    {
        return Err((
            StatusCode::CONFLICT,
            "turn off all tips before changing the currency".to_string(),
        ));
    }
    let settings = api
        .pages
        .set(
            page_id,
            Settings {
                currency: Some(currency),
                tips,
                spin,
                effects,
                sub_spins: request.sub_spins,
                sub_effects: request.sub_effects,
                message: request.message,
                accent,
                font,
            },
        )
        .map_err(|error| match error {
            SettingsError::InvalidCurrency => invalid_currency(),
            SettingsError::InvalidTips => invalid_tips(),
            SettingsError::InvalidSpin => invalid_spin(),
            SettingsError::InvalidEffects => invalid_effects(),
            SettingsError::InvalidTiers => (
                StatusCode::BAD_REQUEST,
                "subscribers' spins and effects per month must be 0 to 100 per tier, with at least one above 0"
                    .to_string(),
            ),
            SettingsError::InvalidPage => (
                StatusCode::BAD_REQUEST,
                "the message can be at most 300 characters and the accent and font among the supported ones".to_string(),
            ),
            SettingsError::Full => (
                StatusCode::SERVICE_UNAVAILABLE,
                "too many streamers".to_string(),
            ),
            SettingsError::NotSaved => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "the settings could not be saved".to_string(),
            ),
        })?;
    info!(
        "remote control streamer {page_id} set currency {}, tips {:?}, spin {:?}, effects {:?}, sub spins {:?}, sub effects {:?}, message {:?}, accent {:?} and font {:?}",
        settings.currency(),
        settings.tips,
        settings.spin,
        settings.effects,
        settings.sub_spins,
        settings.sub_effects,
        settings.message,
        settings.accent,
        settings.font
    );
    api.remote_control.changes.notify(page_id, Change::Page);
    Ok(settings_response(api, account, Look { settings, ..look }))
}

pub(super) async fn handle_settings(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<SettingsRequest>,
) -> Result<Json<SettingsResponse>, Failure> {
    let user = editor(&api, &headers, &page_id)?;
    let request = if user.owner() {
        request
    } else {
        SettingsRequest {
            currency: None,
            ..request
        }
    };
    let premium = is_premium(&api, &user.streamer);
    let response = update_settings(&api, &user.streamer, premium, request).await?;
    Ok(settings_for(&user, response))
}

fn invalid_spin() -> (StatusCode, String) {
    (
        StatusCode::BAD_REQUEST,
        "the spin wheel needs one to five options of 1 to 10 spins for 0.50 to 100000".to_string(),
    )
}

fn invalid_effects() -> (StatusCode, String) {
    (
        StatusCode::BAD_REQUEST,
        "effects need a price of 0.50 to 100000".to_string(),
    )
}

fn invalid_currency() -> (StatusCode, String) {
    (
        StatusCode::BAD_REQUEST,
        "the currency is not supported".to_string(),
    )
}

fn invalid_tips() -> (StatusCode, String) {
    (
        StatusCode::BAD_REQUEST,
        "tips need one to five amounts of 0.50 to 100000".to_string(),
    )
}

#[cfg(test)]
#[path = "tests/settings.rs"]
mod tests;
