use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use log::info;
use serde::{Deserialize, Serialize};

use crate::pages::{
    CURRENCIES, FONTS, Feature, Settings, SettingsError, Spin, Tiers, Tips, is_color,
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
    #[serde(rename = "subSpins")]
    sub_spins: Option<Feature<Tiers>>,
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
    id: String,
    settings: Settings,
) -> Json<SettingsResponse> {
    let account = api.accounts.of_page(&id).unwrap_or_default();
    let payable = account.merchant_id.is_some()
        || (api.stripe.is_some() && account.stripe_ready && account.stripe_account_id.is_some());
    Json(SettingsResponse {
        connected: api.remote_control.is_connected(&id),
        currency: settings.currency().to_string(),
        tips: settings.tips,
        spin: settings.spin,
        sub_spins: settings.sub_spins,
        paypal: api.paypal.is_some(),
        merchant_id: account.merchant_id,
        paypal_pending: account.paypal_tracking_id.is_some(),
        paypal_sandbox: api
            .paypal
            .as_ref()
            .is_some_and(|paypal| paypal.environment() == "sandbox"),
        stripe: api.stripe.is_some(),
        stripe_pending: account.stripe_account_id.is_some() && !account.stripe_ready,
        stripe_account_id: account.stripe_account_id.filter(|_| account.stripe_ready),
        stripe_test: api.stripe.as_ref().is_some_and(Stripe::is_test),
        payable,
        message: settings.message,
        accent: settings.accent,
        font: settings.font,
        image: api.pages.image_version(&id),
        id,
    })
}

pub(super) fn without_payments(mut response: Json<SettingsResponse>) -> Json<SettingsResponse> {
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
    #[serde(default, rename = "subSpins")]
    sub_spins: Option<Feature<Tiers>>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    accent: Option<String>,
    #[serde(default)]
    font: Option<String>,
}

pub(super) fn merchant_of(api: &Api, id: &str) -> Option<String> {
    api.accounts
        .of_page(id)
        .and_then(|account| account.merchant_id)
}

pub(super) fn stripe_of(api: &Api, id: &str) -> Option<String> {
    api.stripe.as_ref()?;
    api.accounts
        .of_page(id)
        .filter(|account| account.stripe_ready)
        .and_then(|account| account.stripe_account_id)
}

fn check_payable(api: &Api, id: &str) -> Result<(), (StatusCode, String)> {
    if api.paypal.is_none() && api.stripe.is_none() {
        return Err(payments_unavailable());
    }
    let paypal = api.paypal.is_some() && merchant_of(api, id).is_some();
    if !paypal && stripe_of(api, id).is_none() {
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
    id: String,
    premium: bool,
    request: SettingsRequest,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let current = api.pages.get(&id);
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
                check_payable(api, &id)?;
            }
            Some(tips)
        }
        None => None,
    };
    let spin = match request.spin {
        Some(spin) => {
            let spin = spin.normalize(Spin::normalize).ok_or_else(invalid_spin)?;
            if spin.enabled && current.spin.as_ref() != Some(&spin) {
                check_payable(api, &id)?;
            }
            Some(spin)
        }
        None => None,
    };
    if currency != current.currency() && (current.tips().is_some() || current.spin().is_some()) {
        return Err((
            StatusCode::CONFLICT,
            "turn tips and the wheel off before changing the currency".to_string(),
        ));
    }
    let settings = api
        .pages
        .set(
            &id,
            Settings {
                currency: Some(currency),
                tips,
                spin,
                sub_spins: request.sub_spins,
                message: request.message,
                accent,
                font,
            },
        )
        .map_err(|error| match error {
            SettingsError::InvalidCurrency => invalid_currency(),
            SettingsError::InvalidTips => invalid_tips(),
            SettingsError::InvalidSpin => invalid_spin(),
            SettingsError::InvalidTiers => (
                StatusCode::BAD_REQUEST,
                "subscribers' spins per month must be 0 to 100 per tier, with at least one above 0"
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
        "remote control streamer {id} set currency {}, tips {:?}, spin {:?}, sub spins {:?}, message {:?}, accent {:?} and font {:?}",
        settings.currency(),
        settings.tips,
        settings.spin,
        settings.sub_spins,
        settings.message,
        settings.accent,
        settings.font
    );
    Ok(settings_response(api, id, settings))
}

pub(super) async fn handle_account_settings(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<SettingsRequest>,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let SignedIn { account, .. } = signed_in(&api, &headers)?;
    let premium = is_premium(&api, &account);
    update_settings(&api, account.page.id, premium, request).await
}

pub(super) async fn handle_moderator_settings(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<SettingsRequest>,
) -> Result<Json<SettingsResponse>, Failure> {
    moderator(&api, &headers, &id)?;
    let request = SettingsRequest {
        currency: None,
        ..request
    };
    let premium = streamer_is_premium(&api, &id);
    update_settings(&api, id, premium, request)
        .await
        .map(without_payments)
        .map_err(Failure::from)
}

fn invalid_spin() -> (StatusCode, String) {
    (
        StatusCode::BAD_REQUEST,
        "the spin wheel needs one to five options of 1 to 10 spins for 0.50 to 100000".to_string(),
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
mod tests {
    use axum::http::StatusCode;
    use serde_json::{Value, json};

    use crate::api::test_utils::*;

    #[tokio::test]
    async fn custom_colors_and_fonts_are_for_pro_streamers() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let save = |session: String, accent: &'static str| {
            client
                .post(format!("http://{address}/account/settings"))
                .bearer_auth(session)
                .json(&json!({"accent": accent}))
                .send()
        };
        let save_font = |session: String, font: &'static str| {
            client
                .post(format!("http://{address}/account/settings"))
                .bearer_auth(session)
                .json(&json!({"font": font}))
                .send()
        };
        let (session, _, _) = streamer(&address, &api).await;
        let response = save(session.clone(), "#123abc").await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = save_font(session.clone(), "handwritten").await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = save_font(session.clone(), "rounded").await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = save(session, "pink").await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        create_account(&api, "premium", "Premium", "premium");
        let premium = api.auth.session("premium", 0);
        let account: Value = client
            .get(format!("http://{address}/account"))
            .bearer_auth(&premium)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let page = account["settings"]["id"].as_str().unwrap().to_string();
        let settings: Value = save(premium.clone(), "#123ABC")
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["accent"], "#123abc");
        let profile: Value = client
            .get(format!("http://{address}/pages/{page}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(profile["accent"], "#123abc");
        let response = save(premium.clone(), "#12345").await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let settings: Value = save_font(premium.clone(), "handwritten")
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["font"], "handwritten");
        let response = save_font(premium, "comic").await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn the_currency_only_changes_while_paid_features_are_off() {
        let (address, api, _) = start_paid().await;
        let (session, _, _) = streamer(&address, &api).await;
        connect_paypal(&api);
        let client = reqwest::Client::new();
        for (request, status) in [
            (
                json!({"currency": "SEK", "tips": {"amounts": [500]}}),
                StatusCode::OK,
            ),
            (
                json!({"currency": "EUR", "tips": {"amounts": [500]}}),
                StatusCode::CONFLICT,
            ),
            (
                json!({"currency": "EUR", "tips": null}),
                StatusCode::CONFLICT,
            ),
            (
                json!({"currency": "SEK", "tips": {"amounts": [700]}}),
                StatusCode::OK,
            ),
            (
                json!({"currency": "SEK", "tips": {"enabled": false, "amounts": [700]}}),
                StatusCode::OK,
            ),
            (
                json!({"currency": "EUR", "spin": {"options": [{"spins": 1, "price": 100}]}}),
                StatusCode::OK,
            ),
            (
                json!({"currency": "USD", "spin": {"enabled": false, "options": [{"spins": 1, "price": 100}]}}),
                StatusCode::CONFLICT,
            ),
            (json!({"currency": "EUR"}), StatusCode::OK),
            (json!({"currency": "USD"}), StatusCode::OK),
        ] {
            let response = client
                .post(format!("http://{address}/account/settings"))
                .bearer_auth(&session)
                .json(&request)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), status, "{request}");
        }
    }
}
