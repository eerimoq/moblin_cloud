use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::accounts::Platform;
use crate::effects::EFFECTS;
use crate::icons;
use crate::pages::{Settings, SpinOption, Tiers};
use crate::paypal::PayPal;
use crate::remote_control::Wheel;

use super::*;

#[derive(Serialize)]
pub(super) struct NameResponse {
    id: String,
}

#[derive(Serialize)]
pub(super) struct ProfileResponse {
    twitch: Option<PublicIdentity>,
    kick: Option<PublicIdentity>,
    message: Option<String>,
    accent: Option<String>,
    font: Option<String>,
    image: Option<String>,
    icon: Option<String>,
}

pub(super) async fn handle_page(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
) -> Result<Json<ProfileResponse>, StatusCode> {
    let account = api.accounts.of_page(&id).ok_or(StatusCode::NOT_FOUND)?;
    let settings = api.pages.get(&id);
    Ok(Json(ProfileResponse {
        twitch: public_identity(&account, Platform::Twitch),
        kick: public_identity(&account, Platform::Kick),
        message: settings.message,
        accent: settings.accent,
        font: settings.font,
        image: api.pages.image_version(&id),
        icon: api.pages.icon(&id),
    }))
}

pub(super) async fn handle_icon(Path(id): Path<String>) -> Result<Response, StatusCode> {
    let icon = icons::icon(&id).ok_or(StatusCode::NOT_FOUND)?;
    Ok((
        [
            (CONTENT_TYPE, "image/png"),
            (CACHE_CONTROL, "public, max-age=86400"),
            (X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        icon.data,
    )
        .into_response())
}

pub(super) async fn handle_page_image(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
) -> Result<Response, StatusCode> {
    let (_, image) = api.pages.image(&id).ok_or(StatusCode::NOT_FOUND)?;
    Ok((
        [
            (CONTENT_TYPE, "image/jpeg"),
            (CACHE_CONTROL, "public, max-age=31536000, immutable"),
            (X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        image,
    )
        .into_response())
}

pub(super) async fn handle_twitch_page(
    State(api): State<Arc<Api>>,
    Path(login): Path<String>,
) -> Result<Json<NameResponse>, StatusCode> {
    page_of_login(&api, Platform::Twitch, &login)
}

pub(super) async fn handle_kick_page(
    State(api): State<Arc<Api>>,
    Path(login): Path<String>,
) -> Result<Json<NameResponse>, StatusCode> {
    page_of_login(&api, Platform::Kick, &login)
}

fn page_of_login(
    api: &Api,
    platform: Platform,
    login: &str,
) -> Result<Json<NameResponse>, StatusCode> {
    let id = api
        .accounts
        .page_of_login(platform, login)
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(NameResponse { id }))
}

#[derive(Deserialize)]
pub(super) struct AddressesQuery {
    logins: String,
}

#[derive(Serialize)]
pub(super) struct PageAddress {
    url: String,
    icon: Option<String>,
}

pub(super) async fn handle_page_addresses(
    State(api): State<Arc<Api>>,
    Query(query): Query<AddressesQuery>,
) -> Json<HashMap<String, PageAddress>> {
    Json(
        query
            .logins
            .split(',')
            .take(100)
            .filter_map(|address| {
                let (platform, login) = address.split_once(':')?;
                let platform = Platform::parse(platform)?;
                let id = api.accounts.page_of_login(platform, login)?;
                Some((
                    address.to_string(),
                    PageAddress {
                        url: format!("{}/{}/{login}", api.auth.frontend_url, platform.name()),
                        icon: api
                            .pages
                            .icon(&id)
                            .map(|icon| format!("{}/icons/{icon}", api.auth.backend_url)),
                    },
                ))
            })
            .collect(),
    )
}

pub(super) fn sub_tiers(api: &Api, id: &str, tiers: Option<&Tiers>) -> Option<Tiers> {
    linked_tiers(api.accounts.of_page(id).as_ref(), tiers)
}

fn linked_tiers(account: Option<&Account>, tiers: Option<&Tiers>) -> Option<Tiers> {
    tiers
        .filter(|_| {
            account.is_some_and(|account| account.twitch().is_some() || account.kick().is_some())
        })
        .cloned()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PublicStripe {
    publishable_key: String,
    account_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PublicPayment {
    currency: String,
    client_id: Option<String>,
    merchant_id: Option<String>,
    environment: Option<&'static str>,
    stripe: Option<PublicStripe>,
}

fn public_payment(
    api: &Api,
    account: Option<&Account>,
    settings: &Settings,
) -> Option<PublicPayment> {
    let (merchant_id, stripe_account_id) = payment_targets(api, account?);
    let stripe = stripe_account_id.and_then(|account_id| {
        Some(PublicStripe {
            publishable_key: api.stripe.as_ref()?.publishable_key().to_string(),
            account_id,
        })
    });
    if merchant_id.is_none() && stripe.is_none() {
        return None;
    }
    Some(PublicPayment {
        currency: settings.currency().to_string(),
        client_id: api
            .paypal
            .as_ref()
            .filter(|_| merchant_id.is_some())
            .map(|paypal| paypal.client_id().to_string()),
        merchant_id,
        environment: api.paypal.as_ref().map(PayPal::environment),
        stripe,
    })
}

#[derive(Serialize)]
pub(super) struct PublicTips {
    amounts: Vec<u32>,
    #[serde(flatten)]
    payment: PublicPayment,
}

#[derive(Serialize)]
pub(super) struct TipsResponse {
    tips: Option<PublicTips>,
}

pub(super) async fn handle_tips(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
) -> Json<TipsResponse> {
    let settings = api.pages.get(&id);
    let payment = public_payment(&api, api.accounts.of_page(&id).as_ref(), &settings);
    Json(TipsResponse {
        tips: settings
            .tips()
            .zip(payment)
            .map(|(tips, payment)| PublicTips {
                amounts: tips.amounts.clone(),
                payment,
            }),
    })
}

#[derive(Serialize)]
pub(super) struct PublicEffect {
    id: &'static str,
    kind: &'static str,
}

#[derive(Serialize)]
pub(super) struct PublicEffects {
    price: u32,
    #[serde(flatten)]
    payment: PublicPayment,
}

#[derive(Serialize)]
pub(super) struct EffectsResponse {
    effects: Option<PublicEffects>,
    #[serde(rename = "subEffects")]
    sub_effects: Option<Tiers>,
    options: Vec<PublicEffect>,
}

pub(super) async fn handle_effects(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
) -> Json<EffectsResponse> {
    let account = api.accounts.of_page(&id);
    let settings = api.pages.get(&id);
    let payment = public_payment(&api, account.as_ref(), &settings);
    Json(EffectsResponse {
        effects: settings
            .effects()
            .zip(payment)
            .map(|(effects, payment)| PublicEffects {
                price: effects.price,
                payment,
            }),
        sub_effects: linked_tiers(account.as_ref(), settings.sub_effects()),
        options: EFFECTS
            .iter()
            .map(|effect| PublicEffect {
                id: effect.id,
                kind: effect.kind,
            })
            .collect(),
    })
}

#[derive(Serialize)]
pub(super) struct PublicSpin {
    options: Vec<SpinOption>,
    #[serde(flatten)]
    payment: PublicPayment,
}

#[derive(Serialize)]
pub(super) struct SpinResponse {
    spin: Option<PublicSpin>,
    #[serde(rename = "subSpins")]
    sub_spins: Option<Tiers>,
    wheel: Option<Wheel>,
    connected: bool,
}

pub(super) async fn handle_spin(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
) -> Json<SpinResponse> {
    let account = api.accounts.of_page(&id);
    let settings = api.pages.get(&id);
    let payment = public_payment(&api, account.as_ref(), &settings);
    Json(SpinResponse {
        spin: settings
            .spin()
            .zip(payment)
            .map(|(spin, payment)| PublicSpin {
                options: spin.options.clone(),
                payment,
            }),
        sub_spins: linked_tiers(account.as_ref(), settings.sub_spins()),
        wheel: api.remote_control.wheel(&id),
        connected: api.remote_control.is_connected(&id),
    })
}
