use std::sync::Arc;

use anyhow::{Context, Result};
use axum::body::Bytes;
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::http::header::{AUTHORIZATION, COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::response::{Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower_http::cors::{Any, CorsLayer};

use crate::accounts::{Account, AccountError, Accounts, Page};
use crate::app_attest::AppAttest;
use crate::auth::{Auth, STATE_COOKIE};
use crate::challenges::Challenges;
use crate::deliveries::{self, Deliveries};
use crate::live::Live;
use crate::pages::{
    CURRENCIES, Pages, Paywall, Settings, SettingsError, Spin, SpinOption, Tips, format_price,
    valid_price,
};
use crate::paypal::{CaptureError, Item, Order, PayPal, Seller};
use crate::remote_control::{self, MusicCommand, MusicStatus, RequestError, Streamers, Wheel};
use crate::store::{Channel, Store, Streamer};

const ASSERTION_HEADER: &str = "moblin-assertion";

pub struct Api {
    pub store: Arc<Store>,
    pub challenges: Challenges,
    pub app_attest: Option<AppAttest>,
    pub live: Option<Live>,
    pub remote_control: Streamers,
    pub pages: Pages,
    pub paypal: Option<PayPal>,
    pub deliveries: Deliveries,
    pub auth: Auth,
    pub accounts: Accounts,
}

pub fn router(api: Arc<Api>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(HeaderValue::from_static("https://moblin.app"))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);
    Router::new()
        .route("/streamers", get(handle_streamers))
        .route("/streamers/live/challenge", post(handle_challenge))
        .route("/streamers/live", post(handle_streamers_live))
        .route("/twitch/live", get(handle_twitch_live))
        .route(
            "/remote-control/streamer/{key}",
            get(handle_remote_control_streamer),
        )
        .route("/pages/twitch/{login}", get(handle_twitch_page))
        .route("/auth/twitch/login", get(handle_login))
        .route("/auth/twitch/callback", get(handle_callback))
        .route("/account", get(handle_account))
        .route("/account/page", post(handle_account_page))
        .route("/account/streamer-url", post(handle_account_streamer_url))
        .route("/account/settings", post(handle_account_settings))
        .route(
            "/account/paypal/connect",
            post(handle_account_paypal_connect),
        )
        .route("/account/paypal/check", post(handle_account_paypal_check))
        .route(
            "/account/paypal/disconnect",
            post(handle_account_paypal_disconnect),
        )
        .route("/paypal/webhooks", post(handle_paypal_webhook))
        .route("/account/sign-out", post(handle_account_sign_out))
        .route("/pages/{id}/songs", get(handle_songs))
        .route("/pages/{id}", get(handle_page))
        .route("/pages/{id}/songs/queue", get(handle_queue))
        .route("/pages/{id}/songs/orders", post(handle_song_order))
        .route(
            "/pages/{id}/songs/orders/{order_id}/capture",
            post(handle_song_capture),
        )
        .route("/pages/{id}/tips", get(handle_tips))
        .route("/pages/{id}/tips/orders", post(handle_tip_order))
        .route(
            "/pages/{id}/tips/orders/{order_id}/capture",
            post(handle_tip_capture),
        )
        .route("/pages/{id}/spin", get(handle_spin))
        .route("/pages/{id}/spin/orders", post(handle_spin_order))
        .route(
            "/pages/{id}/spin/orders/{order_id}/capture",
            post(handle_spin_capture),
        )
        .layer(cors)
        .with_state(api)
}

#[derive(Serialize)]
struct StreamersResponse {
    streamers: Vec<Streamer>,
}

async fn handle_streamers(State(api): State<Arc<Api>>) -> Json<StreamersResponse> {
    Json(StreamersResponse {
        streamers: api.store.streamers(),
    })
}

#[derive(Serialize)]
struct TwitchLiveResponse {
    channel: String,
    live: bool,
}

async fn handle_twitch_live(
    State(api): State<Arc<Api>>,
) -> Result<Json<TwitchLiveResponse>, (StatusCode, String)> {
    let Some(live) = &api.live else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "no Twitch client ID and secret".to_string(),
        ));
    };
    let is_live = live.is_live().await.map_err(|error| {
        warn!("checking if {} is live failed: {error:#}", live.login());
        (StatusCode::SERVICE_UNAVAILABLE, format!("{error:#}"))
    })?;
    Ok(Json(TwitchLiveResponse {
        channel: live.login().to_string(),
        live: is_live,
    }))
}

#[derive(Serialize)]
struct ChallengeResponse {
    challenge: String,
}

async fn handle_challenge(
    State(api): State<Arc<Api>>,
) -> Result<Json<ChallengeResponse>, StatusCode> {
    let challenge = api
        .challenges
        .issue()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(ChallengeResponse {
        challenge: BASE64_STANDARD.encode(challenge),
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LiveRequest {
    channels: Vec<Channel>,
    challenge: Option<String>,
    key_id: Option<String>,
    attestation: Option<String>,
    attestation_challenge: Option<String>,
}

async fn handle_streamers_live(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    let request: LiveRequest = serde_json::from_slice(&body)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    if let Some(app_attest) = &api.app_attest
        && let Err(error) = verify(app_attest, &api.challenges, &request, &headers, &body)
    {
        warn!("rejected live post: {error:#}");
        return Err((StatusCode::UNAUTHORIZED, format!("{error:#}")));
    }
    let channels = request.channels;
    Channel::validate_all(&channels)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    info!(
        "{} went live",
        channels
            .iter()
            .map(Channel::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    );
    api.store.streamer_live(channels);
    Ok(StatusCode::NO_CONTENT)
}

async fn handle_remote_control_streamer(
    State(api): State<Arc<Api>>,
    Path(key): Path<String>,
    upgrade: WebSocketUpgrade,
) -> Result<Response, StatusCode> {
    let id = api
        .accounts
        .page_of_key(&key)
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(upgrade.on_upgrade(move |socket| async move { api.remote_control.serve(id, socket).await }))
}

#[derive(Serialize)]
struct SettingsResponse {
    id: String,
    connected: bool,
    currency: String,
    paywall: Option<Paywall>,
    tips: Option<Tips>,
    spin: Option<Spin>,
    paypal: bool,
    #[serde(rename = "merchantId")]
    merchant_id: Option<String>,
    #[serde(rename = "paypalPending")]
    paypal_pending: bool,
    #[serde(rename = "paypalSandbox")]
    paypal_sandbox: bool,
    title: Option<String>,
    message: Option<String>,
    accent: Option<String>,
}

fn settings_response(api: &Api, id: String, settings: Settings) -> Json<SettingsResponse> {
    let account = api.accounts.of_page(&id).unwrap_or_default();
    Json(SettingsResponse {
        connected: api.remote_control.is_connected(&id),
        id,
        currency: settings.currency().to_string(),
        paywall: settings.paywall,
        tips: settings.tips,
        spin: settings.spin,
        paypal: api.paypal.is_some(),
        merchant_id: account.merchant_id,
        paypal_pending: account.paypal_tracking_id.is_some(),
        paypal_sandbox: api
            .paypal
            .as_ref()
            .is_some_and(|paypal| paypal.environment() == "sandbox"),
        title: settings.title,
        message: settings.message,
        accent: settings.accent,
    })
}

#[derive(Deserialize)]
struct SettingsRequest {
    #[serde(default)]
    currency: Option<String>,
    paywall: Option<Paywall>,
    #[serde(default)]
    tips: Option<Tips>,
    #[serde(default)]
    spin: Option<Spin>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    accent: Option<String>,
}

fn merchant_of(api: &Api, id: &str) -> Option<String> {
    api.accounts
        .of_page(id)
        .and_then(|account| account.merchant_id)
}

fn check_payable(api: &Api, id: &str) -> Result<(), (StatusCode, String)> {
    if api.paypal.is_none() {
        return Err(payments_unavailable());
    }
    if merchant_of(api, id).is_none() {
        return Err((StatusCode::CONFLICT, "connect PayPal first".to_string()));
    }
    Ok(())
}

fn payments_unavailable() -> (StatusCode, String) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        "payments are unavailable".to_string(),
    )
}

async fn update_settings(
    api: &Api,
    id: String,
    request: SettingsRequest,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let current = api.pages.get(&id);
    let currency = request
        .currency
        .map(|currency| currency.trim().to_uppercase())
        .unwrap_or_else(|| current.currency().to_string());
    if !CURRENCIES.contains(&currency.as_str()) {
        return Err(invalid_currency());
    }
    let paid = currency != current.currency();
    let paywall = match request.paywall {
        Some(paywall) => {
            let paywall = paywall.normalize().ok_or_else(invalid_paywall)?;
            if paid || current.paywall.as_ref() != Some(&paywall) {
                check_payable(api, &id)?;
            }
            Some(paywall)
        }
        None => None,
    };
    let tips = match request.tips {
        Some(tips) => {
            let tips = tips.normalize().ok_or_else(invalid_tips)?;
            if paid || current.tips.as_ref() != Some(&tips) {
                check_payable(api, &id)?;
            }
            Some(tips)
        }
        None => None,
    };
    let spin = match request.spin {
        Some(spin) => {
            let spin = spin.normalize().ok_or_else(invalid_spin)?;
            if paid || current.spin.as_ref() != Some(&spin) {
                check_payable(api, &id)?;
            }
            Some(spin)
        }
        None => None,
    };
    let settings = api
        .pages
        .set(
            &id,
            Settings {
                currency: Some(currency),
                paywall,
                tips,
                spin,
                title: request.title,
                message: request.message,
                accent: request.accent,
            },
        )
        .map_err(|error| match error {
            SettingsError::InvalidCurrency => invalid_currency(),
            SettingsError::InvalidPaywall => invalid_paywall(),
            SettingsError::InvalidTips => invalid_tips(),
            SettingsError::InvalidSpin => invalid_spin(),
            SettingsError::InvalidPage => (
                StatusCode::BAD_REQUEST,
                "the title can be at most 60 characters, the message at most 300 and the accent one of the supported colors".to_string(),
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
        "remote control streamer {id} set currency {}, paywall {:?}, tips {:?}, spin {:?}, title {:?}, message {:?} and accent {:?}",
        settings.currency(),
        settings.paywall,
        settings.tips,
        settings.spin,
        settings.title,
        settings.message,
        settings.accent
    );
    Ok(settings_response(api, id, settings))
}

#[derive(Deserialize)]
struct LoginQuery {
    login: Option<String>,
}

async fn handle_login(
    State(api): State<Arc<Api>>,
    Query(query): Query<LoginQuery>,
) -> Result<([(HeaderName, String); 1], Redirect), StatusCode> {
    let (url, state) = api
        .auth
        .login_url(query.login.as_deref())
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    Ok((
        [(SET_COOKIE, api.auth.state_cookie(&state))],
        Redirect::to(&url),
    ))
}

fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value)
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
}

async fn handle_callback(
    State(api): State<Arc<Api>>,
    Query(query): Query<CallbackQuery>,
    headers: HeaderMap,
) -> ([(HeaderName, String); 1], Redirect) {
    (
        [(SET_COOKIE, api.auth.cleared_state_cookie())],
        signed_in_redirect(&api, query, cookie(&headers, STATE_COOKIE)).await,
    )
}

async fn signed_in_redirect(
    api: &Api,
    query: CallbackQuery,
    browser_state: Option<&str>,
) -> Redirect {
    let dashboard = format!("{}/dashboard/", api.auth.frontend_url);
    let (Some(code), Some(state)) = (query.code, query.state) else {
        return Redirect::to(&format!("{dashboard}#signInFailed"));
    };
    let user = match api.auth.callback(&code, &state, browser_state).await {
        Ok(user) => user,
        Err(error) => {
            warn!("signing in with Twitch failed: {error:#}");
            return Redirect::to(&format!("{dashboard}#signInFailed"));
        }
    };
    let account =
        match api
            .accounts
            .signed_in(&user.id, &user.name, &user.login, user.avatar.as_deref())
        {
            Ok(account) => account,
            Err(error) => {
                warn!("saving account {} failed: {error:?}", user.id);
                return Redirect::to(&format!("{dashboard}#signInFailed"));
            }
        };
    info!("Twitch user {} ({}) signed in", user.id, user.login);
    Redirect::to(&format!(
        "{dashboard}#session={}",
        api.auth.session(&user.id, account.sessions)
    ))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountResponse {
    name: String,
    login: String,
    streamer_url: Option<String>,
    settings: Option<SettingsResponse>,
}

fn not_signed_in() -> (StatusCode, String) {
    (StatusCode::UNAUTHORIZED, "not signed in".to_string())
}

struct SignedIn {
    id: String,
    account: Account,
    fresh: bool,
}

fn signed_in(api: &Api, headers: &HeaderMap) -> Result<SignedIn, (StatusCode, String)> {
    let session = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .and_then(|session| api.auth.session_of(session))
        .ok_or_else(not_signed_in)?;
    let account = api
        .accounts
        .get(&session.account)
        .filter(|account| account.sessions == session.generation)
        .ok_or_else(not_signed_in)?;
    Ok(SignedIn {
        fresh: session.is_fresh(),
        id: session.account,
        account,
    })
}

fn account_response(api: &Api, account: Account) -> Json<AccountResponse> {
    let settings = account.page.as_ref().map(|page| {
        let settings = api.pages.get(&page.id);
        settings_response(api, page.id.clone(), settings).0
    });
    Json(AccountResponse {
        name: account.name,
        login: account.login,
        streamer_url: account.page.map(|page| api.auth.streamer_url(&page.key)),
        settings,
    })
}

fn account_error(error: AccountError) -> (StatusCode, String) {
    match error {
        AccountError::Unknown => not_signed_in(),
        AccountError::NoPage => no_page(),
        AccountError::Full => (
            StatusCode::SERVICE_UNAVAILABLE,
            "too many accounts".to_string(),
        ),
        AccountError::NotSaved => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "the account could not be saved".to_string(),
        ),
    }
}

fn no_page() -> (StatusCode, String) {
    (StatusCode::NOT_FOUND, "no page".to_string())
}

fn new_key(api: &Api) -> Result<(String, String), (StatusCode, String)> {
    api.remote_control.new_key().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "no streamer could be created".to_string(),
    ))
}

async fn handle_account(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let signed_in = signed_in(&api, &headers)?;
    Ok(account_response(&api, signed_in.account))
}

async fn handle_account_page(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let SignedIn { id, account, .. } = signed_in(&api, &headers)?;
    if account.page.is_some() {
        return Ok(account_response(&api, account));
    }
    let (key, page) = new_key(&api)?;
    let account = api
        .accounts
        .give_page(&id, Page { id: page, key })
        .map_err(account_error)?;
    info!("{id} got a page");
    Ok(account_response(&api, account))
}

async fn handle_account_streamer_url(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let id = signed_in(&api, &headers)?.id;
    let (key, _) = new_key(&api)?;
    let account = api.accounts.replace_key(&id, key).map_err(account_error)?;
    if let Some(page) = &account.page {
        api.remote_control.disconnect(&page.id);
        info!("{id} got a new streamer URL for page {}", page.id);
    }
    Ok(account_response(&api, account))
}

async fn handle_account_settings(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<SettingsRequest>,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let id = signed_in(&api, &headers)?
        .account
        .page
        .ok_or_else(no_page)?
        .id;
    update_settings(&api, id, request).await
}

#[derive(Serialize)]
struct PayPalSignUpResponse {
    url: String,
}

async fn handle_account_paypal_connect(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<PayPalSignUpResponse>, (StatusCode, String)> {
    let signed_in = signed_in(&api, &headers)?;
    if !signed_in.fresh {
        return Err((
            StatusCode::FORBIDDEN,
            "sign in again to connect PayPal".to_string(),
        ));
    }
    let paypal = api.paypal.as_ref().ok_or_else(payments_unavailable)?;
    let tracking_id = api.remote_control.new_request_id().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "PayPal could not be connected".to_string(),
    ))?;
    let url = paypal
        .referral(
            &tracking_id,
            &format!("{}/dashboard/", api.auth.frontend_url),
        )
        .await
        .map_err(|error| {
            warn!(
                "creating a PayPal referral for {} failed: {error:#}",
                signed_in.id
            );
            paypal_unreachable()
        })?;
    api.accounts
        .start_paypal(&signed_in.id, tracking_id.clone())
        .map_err(account_error)?;
    info!(
        "{} started connecting PayPal with tracking ID {tracking_id}",
        signed_in.id
    );
    Ok(Json(PayPalSignUpResponse { url }))
}

fn paypal_unreachable() -> (StatusCode, String) {
    (
        StatusCode::BAD_GATEWAY,
        "PayPal could not be reached".to_string(),
    )
}

async fn handle_account_paypal_check(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let SignedIn { id, account, .. } = signed_in(&api, &headers)?;
    let page = account.page.ok_or_else(no_page)?.id;
    let paypal = api.paypal.as_ref().ok_or_else(payments_unavailable)?;
    if let Some(tracking_id) = account.paypal_tracking_id {
        let seller = paypal.seller(&tracking_id).await.map_err(|error| {
            warn!("looking up PayPal tracking ID {tracking_id} of {id} failed: {error:#}");
            paypal_unreachable()
        })?;
        let seller = seller.ok_or((
            StatusCode::NOT_FOUND,
            "PayPal has not connected an account yet".to_string(),
        ))?;
        finish_onboarding(&api, &id, &tracking_id, seller)?;
    }
    let settings = api.pages.get(&page);
    Ok(settings_response(&api, page, settings))
}

async fn handle_account_paypal_disconnect(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let SignedIn { id, account, .. } = signed_in(&api, &headers)?;
    let page = account.page.ok_or_else(no_page)?.id;
    api.accounts.forget_paypal(&id).map_err(account_error)?;
    info!("{id} disconnected PayPal");
    let settings = api.pages.get(&page);
    Ok(settings_response(&api, page, settings))
}

fn finish_onboarding(
    api: &Api,
    id: &str,
    tracking_id: &str,
    seller: Seller,
) -> Result<(), (StatusCode, String)> {
    if !seller.permissions_granted {
        info!(
            "PayPal merchant {} of {id} did not grant the permissions",
            seller.merchant_id
        );
        return Err((
            StatusCode::CONFLICT,
            "the PayPal account did not grant the permissions".to_string(),
        ));
    }
    if !seller.payments_receivable || !seller.email_confirmed {
        info!(
            "PayPal merchant {} of {id} can't receive payments yet (payments receivable {}, email confirmed {})",
            seller.merchant_id, seller.payments_receivable, seller.email_confirmed
        );
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "the PayPal account can't receive payments yet".to_string(),
        ));
    }
    api.accounts
        .connect_paypal(id, tracking_id, seller.merchant_id.clone())
        .map_err(account_error)?;
    info!("{id} connected PayPal merchant {}", seller.merchant_id);
    Ok(())
}

async fn handle_paypal_webhook(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    body: String,
) -> StatusCode {
    let Some(paypal) = api.paypal.as_ref().filter(|paypal| paypal.takes_webhooks()) else {
        return StatusCode::SERVICE_UNAVAILABLE;
    };
    let Ok(event) = serde_json::from_str::<Value>(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    match paypal.verify_webhook(&headers, &body).await {
        Ok(true) => {}
        Ok(false) => {
            warn!("PayPal webhook {} has a bad signature", event["id"]);
            return StatusCode::BAD_REQUEST;
        }
        Err(error) => {
            warn!("verifying PayPal webhook {} failed: {error:#}", event["id"]);
            return StatusCode::BAD_GATEWAY;
        }
    }
    let event_type = event["event_type"].as_str().unwrap_or_default();
    let merchant_id = event["resource"]["merchant_id"]
        .as_str()
        .unwrap_or_default();
    info!(
        "PayPal webhook {} {event_type} for merchant {merchant_id:?}",
        event["id"]
    );
    match event_type {
        "MERCHANT.ONBOARDING.COMPLETED" if !merchant_id.is_empty() => {
            let seller = match paypal.seller_status(merchant_id).await {
                Ok(seller) => seller,
                Err(error) => {
                    warn!("looking up PayPal merchant {merchant_id} failed: {error:#}");
                    return StatusCode::BAD_GATEWAY;
                }
            };
            let tracking_id = seller.tracking_id.clone().or_else(|| {
                event["resource"]["tracking_id"]
                    .as_str()
                    .map(str::to_string)
            });
            let Some((id, tracking_id)) = tracking_id.and_then(|tracking_id| {
                api.accounts
                    .of_paypal_tracking_id(&tracking_id)
                    .map(|id| (id, tracking_id))
            }) else {
                info!("no account is connecting PayPal merchant {merchant_id}");
                return StatusCode::OK;
            };
            let _ = finish_onboarding(&api, &id, &tracking_id, seller);
        }
        "MERCHANT.PARTNER-CONSENT.REVOKED" => {
            for id in api.accounts.disconnect_paypal(merchant_id) {
                info!("{id} was disconnected from PayPal merchant {merchant_id}");
            }
        }
        _ => {}
    }
    StatusCode::OK
}

async fn handle_account_sign_out(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, String)> {
    let id = signed_in(&api, &headers)?.id;
    api.accounts.sign_out(&id).map_err(account_error)?;
    info!("{id} signed out everywhere");
    Ok(StatusCode::NO_CONTENT)
}

fn invalid_spin() -> (StatusCode, String) {
    (
        StatusCode::BAD_REQUEST,
        "the spin wheel needs one to four options of 1 to 10 spins for 0.50 to 1000".to_string(),
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
        "tips need one to four amounts of 0.50 to 1000".to_string(),
    )
}

fn invalid_paywall() -> (StatusCode, String) {
    (
        StatusCode::BAD_REQUEST,
        "the paywall needs a price of 0.50 to 1000".to_string(),
    )
}

#[derive(Serialize)]
struct NameResponse {
    id: String,
}

#[derive(Serialize)]
struct ProfileResponse {
    name: String,
    login: String,
    avatar: Option<String>,
    title: Option<String>,
    message: Option<String>,
    accent: Option<String>,
}

async fn handle_page(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
) -> Result<Json<ProfileResponse>, StatusCode> {
    let account = api.accounts.of_page(&id).ok_or(StatusCode::NOT_FOUND)?;
    let settings = api.pages.get(&id);
    Ok(Json(ProfileResponse {
        name: account.name,
        login: account.login,
        avatar: account.avatar,
        title: settings.title,
        message: settings.message,
        accent: settings.accent,
    }))
}

async fn handle_twitch_page(
    State(api): State<Arc<Api>>,
    Path(login): Path<String>,
) -> Result<Json<NameResponse>, StatusCode> {
    let id = api
        .accounts
        .page_of_login(&login)
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(NameResponse { id }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicPaywall {
    price: u32,
    currency: String,
    client_id: Option<String>,
    merchant_id: Option<String>,
    environment: Option<&'static str>,
}

#[derive(Serialize)]
struct PaywallResponse {
    paywall: Option<PublicPaywall>,
}

async fn handle_songs(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
) -> Json<PaywallResponse> {
    let merchant_id = merchant_of(&api, &id);
    let settings = api.pages.get(&id);
    let currency = settings.currency().to_string();
    Json(PaywallResponse {
        paywall: settings.paywall.map(|paywall| PublicPaywall {
            price: paywall.price,
            currency,
            client_id: api
                .paypal
                .as_ref()
                .filter(|_| merchant_id.is_some())
                .map(|paypal| paypal.client_id().to_string()),
            merchant_id,
            environment: api.paypal.as_ref().map(PayPal::environment),
        }),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicTips {
    amounts: Vec<u32>,
    currency: String,
    client_id: Option<String>,
    merchant_id: String,
    environment: Option<&'static str>,
}

#[derive(Serialize)]
struct TipsResponse {
    tips: Option<PublicTips>,
}

async fn handle_tips(State(api): State<Arc<Api>>, Path(id): Path<String>) -> Json<TipsResponse> {
    let settings = api.pages.get(&id);
    let currency = settings.currency().to_string();
    let tips = settings.tips.zip(merchant_of(&api, &id));
    Json(TipsResponse {
        tips: tips.map(|(tips, merchant_id)| PublicTips {
            amounts: tips.amounts,
            currency,
            client_id: api
                .paypal
                .as_ref()
                .map(|paypal| paypal.client_id().to_string()),
            merchant_id,
            environment: api.paypal.as_ref().map(PayPal::environment),
        }),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicSpin {
    options: Vec<SpinOption>,
    currency: String,
    client_id: Option<String>,
    merchant_id: String,
    environment: Option<&'static str>,
}

#[derive(Serialize)]
struct SpinResponse {
    spin: Option<PublicSpin>,
    wheel: Option<Wheel>,
}

async fn handle_spin(State(api): State<Arc<Api>>, Path(id): Path<String>) -> Json<SpinResponse> {
    let settings = api.pages.get(&id);
    let currency = settings.currency().to_string();
    let spin = settings.spin.zip(merchant_of(&api, &id));
    Json(SpinResponse {
        spin: spin.map(|(spin, merchant_id)| PublicSpin {
            options: spin.options,
            currency,
            client_id: api
                .paypal
                .as_ref()
                .map(|paypal| paypal.client_id().to_string()),
            merchant_id,
            environment: api.paypal.as_ref().map(PayPal::environment),
        }),
        wheel: api.remote_control.wheel(&id),
    })
}

type Failure = (StatusCode, String);

fn failure(status: StatusCode, text: &str) -> Failure {
    (status, text.to_string())
}

fn music_error(error: RequestError) -> Failure {
    match error {
        RequestError::Offline => failure(
            StatusCode::NOT_FOUND,
            "The streamer is offline right now. Try again when they are live.",
        ),
        RequestError::Timeout => failure(
            StatusCode::GATEWAY_TIMEOUT,
            "The streamer's Moblin did not answer. It may need an update.",
        ),
        RequestError::Failed => failure(
            StatusCode::BAD_GATEWAY,
            "The streamer's Moblin could not do it. Try again in a moment.",
        ),
    }
}

async fn handle_queue(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
) -> Result<Json<MusicStatus>, Failure> {
    let status = api
        .remote_control
        .music(&id, MusicCommand::Status {})
        .await
        .map_err(music_error)?;
    Ok(Json(status))
}

#[derive(Deserialize)]
struct SongRequest {
    song: String,
    user: Option<String>,
}

struct RequestedSong {
    link: String,
    user: String,
    name: String,
}

fn song_and_user(request: SongRequest) -> Result<(String, String), Failure> {
    let link = remote_control::song_link(&request.song).ok_or(failure(
        StatusCode::BAD_REQUEST,
        "Paste the Apple Music share link of a song.",
    ))?;
    Ok((link, user_name(request.user, "Moblin website")))
}

fn user_name(user: Option<String>, anonymous: &str) -> String {
    user.map(|user| user.trim().chars().take(25).collect::<String>())
        .filter(|user| !user.is_empty())
        .unwrap_or_else(|| anonymous.to_string())
}

async fn requested_song(
    api: &Api,
    id: &str,
    (link, user): (String, String),
) -> Result<RequestedSong, Failure> {
    let song = api
        .remote_control
        .find(id, link.clone())
        .await
        .map_err(music_error)?
        .ok_or(failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            "That song is not available on the streamer's Apple Music.",
        ))?;
    Ok(RequestedSong {
        link,
        user,
        name: song.name(),
    })
}

fn queue(
    api: &Api,
    id: String,
    request_id: String,
    song: RequestedSong,
) -> (StatusCode, Json<Value>) {
    info!(
        "queued {:?} by {:?} in request {request_id} for remote control streamer {id}",
        song.link, song.user
    );
    api.deliveries.add(
        id,
        request_id,
        deliveries::Item::Song {
            link: song.link,
            user: song.user,
        },
    );
    (
        StatusCode::ACCEPTED,
        Json(Value::Object(Default::default())),
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OrderResponse {
    order_id: String,
}

async fn handle_song_order(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    Json(request): Json<SongRequest>,
) -> Result<Json<OrderResponse>, Failure> {
    let settings = api.pages.get(&id);
    let currency = settings.currency().to_string();
    let paywall = settings.paywall.ok_or(failure(
        StatusCode::BAD_REQUEST,
        "The streamer no longer takes song requests. Reload the page.",
    ))?;
    let paypal = api.paypal.as_ref().ok_or_else(paypal_unavailable)?;
    let merchant_id = merchant_of(&api, &id).ok_or_else(paypal_unavailable)?;
    let song = requested_song(&api, &id, song_and_user(request)?).await?;
    let order = Order {
        streamer: id.clone(),
        item: Item::Song {
            link: song.link,
            name: song.name,
        },
        user: song.user,
        merchant_id,
        price: paywall.price,
        currency,
    };
    let order_id = paypal.create_order(order).await.map_err(|error| {
        warn!("creating a PayPal order for remote control streamer {id} failed: {error:#}");
        failure(
            StatusCode::BAD_GATEWAY,
            "PayPal could not start the payment. Try again.",
        )
    })?;
    Ok(Json(OrderResponse { order_id }))
}

async fn handle_song_capture(
    State(api): State<Arc<Api>>,
    Path((id, order_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    tokio::spawn(capture_song(api, id, order_id))
        .await
        .unwrap_or_else(|_| Err(capture_crashed()))
}

fn capture_crashed() -> Failure {
    failure(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Something went wrong with the payment.",
    )
}

fn paypal_unavailable() -> Failure {
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        "Payments are unavailable right now. Try again later.",
    )
}

async fn capture(api: &Api, id: &str, order_id: &str, kind: &str) -> Result<Order, Failure> {
    let paypal = api.paypal.as_ref().ok_or_else(paypal_unavailable)?;
    let order = paypal
        .order(order_id)
        .filter(|order| order.streamer == id && order.item.kind() == kind)
        .ok_or(failure(
            StatusCode::GONE,
            "This payment is unknown or expired, so no money was taken.",
        ))?;
    if kind == "song" && !api.remote_control.is_connected(id) {
        return Err(failure(
            StatusCode::NOT_FOUND,
            "The streamer went offline, so your payment was not taken.",
        ));
    }
    if kind == "spin" && api.remote_control.wheel(id).is_none() {
        return Err(failure(
            StatusCode::NOT_FOUND,
            "The streamer's wheel is gone, so your payment was not taken.",
        ));
    }
    paypal
        .capture(order_id, &order)
        .await
        .map_err(|error| match error {
            CaptureError::NotApproved if kind == "song" => failure(
                StatusCode::PAYMENT_REQUIRED,
                "The payment was not completed. No song was added.",
            ),
            CaptureError::NotApproved => failure(
                StatusCode::PAYMENT_REQUIRED,
                "The payment was not completed.",
            ),
            CaptureError::AlreadyCaptured => {
                failure(StatusCode::CONFLICT, "This payment was already used.")
            }
            CaptureError::Pending if kind == "spin" => failure(
                StatusCode::PAYMENT_REQUIRED,
                "PayPal is still reviewing the payment, so the wheel was not spun. If the money is taken, ask the streamer for a refund.",
            ),
            CaptureError::Pending if kind == "tip" => failure(
                StatusCode::PAYMENT_REQUIRED,
                "PayPal is still reviewing the payment, so the tip was not shown. If PayPal approves it, the money still reaches the streamer.",
            ),
            CaptureError::Pending => failure(
                StatusCode::PAYMENT_REQUIRED,
                "PayPal is still reviewing the payment, so no song was added. If the money is taken, ask the streamer for a refund.",
            ),
            CaptureError::Failed => failure(
                StatusCode::BAD_GATEWAY,
                "PayPal could not complete the payment.",
            ),
        })?;
    info!(
        "{:?} paid {} {} in PayPal order {order_id}",
        order.user,
        format_price(order.price),
        order.currency
    );
    Ok(order)
}

async fn capture_song(
    api: Arc<Api>,
    id: String,
    order_id: String,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let order = capture(&api, &id, &order_id, "song").await?;
    let Item::Song { link, name } = order.item else {
        return Err(capture_crashed());
    };
    let song = RequestedSong {
        link,
        user: order.user,
        name,
    };
    Ok(queue(&api, id, order_id, song))
}

fn viewer_message(message: Option<String>) -> Result<Option<String>, Failure> {
    let message = message
        .map(|message| message.trim().to_string())
        .filter(|message| !message.is_empty());
    if message
        .as_ref()
        .is_some_and(|message| message.chars().count() > 200)
    {
        return Err(failure(
            StatusCode::BAD_REQUEST,
            "The message can be at most 200 characters.",
        ));
    }
    Ok(message)
}

#[derive(Deserialize)]
struct TipRequest {
    amount: u32,
    user: Option<String>,
    message: Option<String>,
}

async fn handle_tip_order(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    Json(request): Json<TipRequest>,
) -> Result<Json<OrderResponse>, Failure> {
    let settings = api.pages.get(&id);
    let currency = settings.currency().to_string();
    settings.tips.ok_or(failure(
        StatusCode::BAD_REQUEST,
        "The streamer no longer takes tips. Reload the page.",
    ))?;
    let paypal = api.paypal.as_ref().ok_or_else(paypal_unavailable)?;
    let merchant_id = merchant_of(&api, &id).ok_or_else(paypal_unavailable)?;
    if !valid_price(request.amount) {
        return Err(failure(StatusCode::BAD_REQUEST, "Tip from 0.50 to 1000."));
    }
    let message = viewer_message(request.message)?;
    let order = Order {
        streamer: id.clone(),
        item: Item::Tip { message },
        user: user_name(request.user, "Someone"),
        merchant_id,
        price: request.amount,
        currency,
    };
    let order_id = paypal.create_order(order).await.map_err(|error| {
        warn!("creating a PayPal tip order for remote control streamer {id} failed: {error:#}");
        failure(
            StatusCode::BAD_GATEWAY,
            "PayPal could not start the payment. Try again.",
        )
    })?;
    Ok(Json(OrderResponse { order_id }))
}

async fn handle_tip_capture(
    State(api): State<Arc<Api>>,
    Path((id, order_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    tokio::spawn(capture_tip(api, id, order_id))
        .await
        .unwrap_or_else(|_| Err(capture_crashed()))
}

async fn capture_tip(
    api: Arc<Api>,
    id: String,
    order_id: String,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let order = capture(&api, &id, &order_id, "tip").await?;
    let Item::Tip { message } = order.item else {
        return Err(capture_crashed());
    };
    info!(
        "queued a tip of {} {} by {:?} in PayPal order {order_id} for remote control streamer {id}",
        format_price(order.price),
        order.currency,
        order.user
    );
    api.deliveries.add(
        id,
        order_id,
        deliveries::Item::Tip {
            user: order.user,
            message: message.unwrap_or_default(),
            amount: order.price,
            currency: order.currency,
        },
    );
    Ok(accepted())
}

fn accepted() -> (StatusCode, Json<Value>) {
    (
        StatusCode::ACCEPTED,
        Json(Value::Object(Default::default())),
    )
}

#[derive(Deserialize)]
struct SpinRequest {
    spins: u32,
    user: Option<String>,
    message: Option<String>,
}

async fn handle_spin_order(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    Json(request): Json<SpinRequest>,
) -> Result<Json<OrderResponse>, Failure> {
    let settings = api.pages.get(&id);
    let currency = settings.currency().to_string();
    let spin = settings.spin.ok_or(failure(
        StatusCode::BAD_REQUEST,
        "The streamer turned the wheel off. Reload the page.",
    ))?;
    if api.remote_control.wheel(&id).is_none() {
        return Err(failure(
            StatusCode::NOT_FOUND,
            "The streamer's wheel is not available right now. Try again when they are live.",
        ));
    }
    let paypal = api.paypal.as_ref().ok_or_else(paypal_unavailable)?;
    let merchant_id = merchant_of(&api, &id).ok_or_else(paypal_unavailable)?;
    let option = spin
        .options
        .iter()
        .find(|option| option.spins == request.spins)
        .ok_or(failure(
            StatusCode::BAD_REQUEST,
            "The streamer changed the prices. Reload the page.",
        ))?;
    let order = Order {
        streamer: id.clone(),
        item: Item::Spin {
            spins: option.spins,
            message: viewer_message(request.message)?,
        },
        user: user_name(request.user, "Someone"),
        merchant_id,
        price: option.price,
        currency,
    };
    let order_id = paypal.create_order(order).await.map_err(|error| {
        warn!("creating a PayPal spin order for remote control streamer {id} failed: {error:#}");
        failure(
            StatusCode::BAD_GATEWAY,
            "PayPal could not start the payment. Try again.",
        )
    })?;
    Ok(Json(OrderResponse { order_id }))
}

async fn handle_spin_capture(
    State(api): State<Arc<Api>>,
    Path((id, order_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    tokio::spawn(capture_spin(api, id, order_id))
        .await
        .unwrap_or_else(|_| Err(capture_crashed()))
}

async fn capture_spin(
    api: Arc<Api>,
    id: String,
    order_id: String,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let order = capture(&api, &id, &order_id, "spin").await?;
    let Item::Spin { spins, message } = order.item else {
        return Err(capture_crashed());
    };
    info!(
        "queued {spins} wheel spins by {:?} in PayPal order {order_id} for remote control streamer {id}",
        order.user
    );
    api.deliveries.add(
        id,
        order_id,
        deliveries::Item::Spin {
            user: order.user,
            message: message.unwrap_or_default(),
            spins,
        },
    );
    Ok(accepted())
}

fn verify(
    app_attest: &AppAttest,
    challenges: &Challenges,
    request: &LiveRequest,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<()> {
    let assertion = headers
        .get(ASSERTION_HEADER)
        .context("no assertion")?
        .to_str()
        .ok()
        .context("assertion is not text")?;
    let assertion = base64(assertion).context("assertion")?;
    let key_id = base64(request.key_id.as_deref().context("no key id")?).context("key id")?;
    let attestation =
        base64(request.attestation.as_deref().context("no attestation")?).context("attestation")?;
    let attestation_challenge = base64(
        request
            .attestation_challenge
            .as_deref()
            .context("no attestation challenge")?,
    )
    .context("attestation challenge")?;
    let challenge =
        base64(request.challenge.as_deref().context("no challenge")?).context("challenge")?;
    let public_key = app_attest
        .verify_attestation(&attestation, &key_id, &attestation_challenge)
        .context("bad attestation")?;
    app_attest
        .verify_assertion(&assertion, body, &public_key)
        .context("bad assertion")?;
    anyhow::ensure!(
        challenges.consume(&challenge),
        "the challenge is unknown, used or expired"
    );
    Ok(())
}

fn base64(text: &str) -> Result<Vec<u8>> {
    BASE64_STANDARD.decode(text).context("not base64")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures_util::{SinkExt, StreamExt};
    use serde_json::{Value, json};
    use tokio_tungstenite::tungstenite::Message;

    use super::*;
    use crate::auth::OAuthApp;

    async fn start() -> String {
        start_with(None).await
    }

    async fn start_with(paypal: Option<PayPal>) -> String {
        start_api(paypal).await.0
    }

    async fn start_api(paypal: Option<PayPal>) -> (String, Arc<Api>) {
        start_api_with(paypal, None, Pages::load(None).unwrap()).await
    }

    async fn start_api_with(
        paypal: Option<PayPal>,
        twitch: Option<OAuthApp>,
        pages: Pages,
    ) -> (String, Arc<Api>) {
        let api = Arc::new(Api {
            store: Arc::new(Store::new(6)),
            challenges: Challenges::new(),
            app_attest: None,
            live: None,
            remote_control: Streamers::new(),
            pages,
            paypal,
            deliveries: Deliveries::load(None).unwrap(),
            auth: Auth::new(
                reqwest::Client::new(),
                "http://localhost".to_string(),
                "https://moblin.app".to_string(),
                twitch,
                false,
                Some("secret"),
            )
            .unwrap(),
            accounts: Accounts::load(None).unwrap(),
        });
        tokio::spawn({
            let api = api.clone();
            async move { api.deliveries.run(&api.remote_control).await }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn({
            let api = api.clone();
            async move { axum::serve(listener, router(api)).await }
        });
        (format!("127.0.0.1:{}", address.port()), api)
    }

    async fn receive<S>(socket: &mut S) -> Value
    where
        S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    {
        loop {
            if let Message::Text(text) = socket.next().await.unwrap().unwrap() {
                return serde_json::from_str(&text).unwrap();
            }
        }
    }

    async fn identify<S>(socket: &mut S)
    where
        S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
            + SinkExt<Message>
            + Unpin,
        <S as futures_util::Sink<Message>>::Error: std::fmt::Debug,
    {
        let hello = receive(socket).await;
        assert_eq!(hello["hello"]["apiVersion"], "0.1");
        socket
            .send(Message::text(
                json!({"identify": {"streamerId": "A", "authentication": "x"}}).to_string(),
            ))
            .await
            .unwrap();
        assert_eq!(
            receive(socket).await,
            json!({"identified": {"result": {"ok": {}}}})
        );
    }

    #[derive(Clone, Default)]
    struct FakePayPal {
        order: Arc<std::sync::Mutex<Option<Value>>>,
        referral: Arc<std::sync::Mutex<Option<Value>>>,
        seller: Arc<std::sync::Mutex<Option<Value>>>,
        list_sellers: Arc<std::sync::Mutex<bool>>,
        verification: Arc<std::sync::Mutex<Option<Value>>>,
    }

    fn new_paypal(url: String) -> PayPal {
        PayPal::new(
            reqwest::Client::new(),
            url,
            "id".to_string(),
            "secret".to_string(),
            "PARTNER000001".to_string(),
            Some("BN".to_string()),
            Some("WH".to_string()),
        )
    }

    fn payer_of(assertion: &str) -> Value {
        let payload = assertion.split('.').nth(1).unwrap();
        serde_json::from_slice::<Value>(&BASE64_STANDARD.decode(payload).unwrap()).unwrap()
            ["payer_id"]
            .clone()
    }

    async fn start_paypal() -> (String, FakePayPal) {
        let fake = FakePayPal::default();
        let order = fake.order.clone();
        let router = Router::new()
            .route(
                "/v1/notifications/verify-webhook-signature",
                post({
                    let verification = fake.verification.clone();
                    move |Json(body): Json<Value>| async move {
                        let good = body["transmission_sig"] == "good" && body["webhook_id"] == "WH";
                        *verification.lock().unwrap() = Some(body);
                        Json(json!({
                            "verification_status": if good { "SUCCESS" } else { "FAILURE" },
                        }))
                    }
                }),
            )
            .route(
                "/v2/customer/partner-referrals",
                post({
                    let referral = fake.referral.clone();
                    move |headers: HeaderMap, Json(body): Json<Value>| async move {
                        *referral.lock().unwrap() = Some(json!({
                            "attribution": headers["paypal-partner-attribution-id"].to_str().unwrap(),
                            "body": body,
                        }));
                        (
                            StatusCode::CREATED,
                            Json(json!({"links": [
                                {"href": "https://api-m.sandbox.paypal.com/v2/customer/partner-referrals/x", "rel": "self", "method": "GET"},
                                {"href": "https://www.sandbox.paypal.com/bizsignup/x", "rel": "action_url", "method": "GET"},
                            ]})),
                        )
                    }
                }),
            )
            .route(
                "/v1/customer/partners/PARTNER000001/merchant-integrations",
                get({
                    let seller = fake.seller.clone();
                    let list_sellers = fake.list_sellers.clone();
                    move |Query(query): Query<std::collections::HashMap<String, String>>| async move {
                        match seller.lock().unwrap().clone() {
                            Some(seller) if seller["tracking_id"] == query["tracking_id"].as_str() => {
                                let integration = json!({
                                    "merchant_id": seller["merchant_id"],
                                    "tracking_id": seller["tracking_id"],
                                    "links": [],
                                });
                                if *list_sellers.lock().unwrap() {
                                    (StatusCode::OK, Json(json!([integration])))
                                } else {
                                    (StatusCode::OK, Json(integration))
                                }
                            }
                            _ => (
                                StatusCode::NOT_FOUND,
                                Json(json!({"name": "RESOURCE_NOT_FOUND"})),
                            ),
                        }
                    }
                }),
            )
            .route(
                "/v1/customer/partners/PARTNER000001/merchant-integrations/{merchant}",
                get({
                    let seller = fake.seller.clone();
                    move |Path(merchant): Path<String>| async move {
                        let seller = seller.lock().unwrap().clone().unwrap();
                        assert_eq!(seller["merchant_id"], merchant.as_str());
                        Json(seller)
                    }
                }),
            )
            .route(
                "/v1/oauth2/token",
                post(|| async {
                    Json(json!({"access_token": "t", "token_type": "Bearer", "expires_in": 3600}))
                }),
            )
            .route(
                "/v2/checkout/orders",
                post({
                    let order = order.clone();
                    move |headers: HeaderMap, Json(mut request): Json<Value>| async move {
                        request["assertion"] =
                            json!(headers["paypal-auth-assertion"].to_str().unwrap());
                        *order.lock().unwrap() = Some(request);
                        (
                            StatusCode::CREATED,
                            Json(json!({
                                "id": "ORDER1",
                                "status": "CREATED",
                            })),
                        )
                    }
                }),
            )
            .route(
                "/v2/checkout/orders/{id}/capture",
                post({
                    let order = order.clone();
                    move |headers: HeaderMap, Path(id): Path<String>| async move {
                        assert_eq!(
                            payer_of(headers["paypal-auth-assertion"].to_str().unwrap()),
                            payer_of(
                                order.lock().unwrap().as_ref().unwrap()["assertion"]
                                    .as_str()
                                    .unwrap()
                            )
                        );
                        let amount =
                            order.lock().unwrap().as_ref().unwrap()["purchase_units"][0]["amount"]
                                .clone();
                        if id != "ORDER1" {
                            return (
                                StatusCode::UNPROCESSABLE_ENTITY,
                                Json(json!({"details": [{"issue": "ORDER_NOT_APPROVED"}]})),
                            );
                        }
                        let status = if amount["currency_code"] == "EUR" {
                            "PENDING"
                        } else {
                            "COMPLETED"
                        };
                        (
                            StatusCode::CREATED,
                            Json(json!({
                                "id": id,
                                "status": "COMPLETED",
                                "purchase_units": [{"payments": {"captures": [
                                    {"status": status, "amount": amount},
                                ]}}],
                            })),
                        )
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await });
        (format!("http://{address}"), fake)
    }

    const LINK: &str = "https://music.apple.com/song/1440818839?i=1440818839";

    async fn answer<S>(socket: &mut S, request: &Value, data: Value)
    where
        S: SinkExt<Message> + Unpin,
        <S as futures_util::Sink<Message>>::Error: std::fmt::Debug,
    {
        socket
            .send(Message::text(
                json!({"response": {
                    "id": request["request"]["id"],
                    "result": {"ok": {}},
                    "data": data,
                }})
                .to_string(),
            ))
            .await
            .unwrap();
    }

    fn music(songs: Value) -> Value {
        json!({"music": {"status": {
            "playing": false,
            "currentSongIndex": null,
            "songs": songs,
            "songNotFound": false,
        }}})
    }

    async fn connect(
        address: &str,
        key: &str,
    ) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>
    {
        let (mut socket, _) = tokio_tungstenite::connect_async(format!(
            "ws://{address}/remote-control/streamer/{key}"
        ))
        .await
        .unwrap();
        identify(&mut socket).await;
        socket
    }

    async fn start_paid() -> (String, Arc<Api>, Arc<std::sync::Mutex<Option<Value>>>) {
        let (paypal_url, fake) = start_paypal().await;
        let (address, api) = start_api(Some(new_paypal(paypal_url))).await;
        (address, api, fake.order)
    }

    fn connect_paypal(api: &Api) {
        api.accounts.start_paypal("1", "t".to_string()).unwrap();
        api.accounts
            .connect_paypal("1", "t", "MERCHANT00001".to_string())
            .unwrap();
    }

    #[tokio::test]
    async fn paywalls_without_paypal_cant_be_paid() {
        let path = std::env::temp_dir().join(format!("old-paywalls-{}.json", std::process::id()));
        std::fs::write(
            &path,
            r#"{"p": {"paywall": {"merchantId": "MERCHANT00001", "price": 250, "currency": "SEK"}}}"#,
        )
        .unwrap();
        let pages = Pages::load(Some(path.clone())).unwrap();
        std::fs::remove_file(&path).unwrap();
        let (paypal_url, _) = start_paypal().await;
        let (address, _) = start_api_with(Some(new_paypal(paypal_url)), None, pages).await;
        let client = reqwest::Client::new();
        let public: Value = client
            .get(format!("http://{address}/pages/p/songs"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            public,
            json!({"paywall": {
                "price": 250,
                "currency": "SEK",
                "clientId": null,
                "merchantId": null,
                "environment": "production",
            }})
        );
        let response = client
            .post(format!("http://{address}/pages/p/songs/orders"))
            .json(&json!({"song": LINK, "user": "Viewer"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    fn key_of(account: &Value) -> String {
        let url = account["streamerUrl"].as_str().unwrap();
        assert!(url.starts_with("ws://localhost/remote-control/streamer/"));
        url.rsplit('/').next().unwrap().to_string()
    }

    async fn streamer(address: &str, api: &Api) -> (String, String, String) {
        api.accounts.signed_in("1", "Erik", "erik", None).unwrap();
        let session = api.auth.session("1", 0);
        let account: Value = reqwest::Client::new()
            .post(format!("http://{address}/account/page"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        (
            session,
            key_of(&account),
            account["settings"]["id"].as_str().unwrap().to_string(),
        )
    }

    async fn paid_streamer(address: &str, api: &Api) -> (String, String) {
        let (session, key, id) = streamer(address, api).await;
        connect_paypal(api);
        let response = reqwest::Client::new()
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"currency": "SEK", "paywall": {"price": 250}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        (key, id)
    }

    async fn request_song<S>(socket: &mut S, url: &str) -> Value
    where
        S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
            + SinkExt<Message>
            + Unpin,
        <S as futures_util::Sink<Message>>::Error: std::fmt::Debug,
    {
        let created = tokio::spawn({
            let url = url.to_string();
            async move {
                reqwest::Client::new()
                    .post(url)
                    .json(&json!({"song": "https://music.apple.com/us/song/x/1440818839", "user": "Viewer"}))
                    .send()
                    .await
                    .unwrap()
                    .json::<Value>()
                    .await
                    .unwrap()
            }
        });
        let request = receive(socket).await;
        assert_eq!(
            request["request"]["data"],
            json!({"music": {"command": {"find": {"title": LINK}}}})
        );
        answer(
            socket,
            &request,
            json!({"musicFind": {"song": {"id": "1440818839", "title": "Song", "artist": "Band"}}}),
        )
        .await;
        created.await.unwrap()
    }

    async fn wait_until_added(api: &Api) {
        for _ in 0..100 {
            if api.deliveries.pending().is_empty() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("still queued");
    }

    #[tokio::test]
    async fn paid_songs_are_added_after_capture() {
        let (address, api, paypal_order) = start_paid().await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        let (key, id) = (key.as_str(), id.as_str());
        let settings_url = format!("http://{address}/account/settings");
        let response = client
            .post(&settings_url)
            .bearer_auth(&session)
            .json(&json!({"currency": "SEK", "paywall": {"price": 250}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        connect_paypal(&api);
        let settings: Value = client
            .post(&settings_url)
            .bearer_auth(&session)
            .json(&json!({"currency": "sek", "paywall": {"price": 250}}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["paywall"], json!({"price": 250}));
        assert_eq!(settings["currency"], "SEK");
        assert_eq!(settings["paypal"], true);
        assert_eq!(settings["merchantId"], "MERCHANT00001");
        let public: Value = client
            .get(format!("http://{address}/pages/{id}/songs"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            public,
            json!({"paywall": {
                "price": 250,
                "currency": "SEK",
                "clientId": "id",
                "merchantId": "MERCHANT00001",
                "environment": "production",
            }})
        );
        let orders_url = format!("http://{address}/pages/{id}/songs/orders");
        let response = client
            .post(&orders_url)
            .json(&json!({"song": "Song", "user": "Viewer"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.text().await.unwrap(),
            "Paste the Apple Music share link of a song."
        );
        let response = client
            .post(&orders_url)
            .json(&json!({"song": LINK, "user": "Viewer"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let mut socket = connect(&address, key).await;
        *paypal_order.lock().unwrap() = None;
        let not_found = tokio::spawn({
            let client = client.clone();
            let orders_url = orders_url.clone();
            async move {
                client
                    .post(orders_url)
                    .json(&json!({"song": LINK, "user": "Viewer"}))
                    .send()
                    .await
                    .unwrap()
            }
        });
        let request = receive(&mut socket).await;
        answer(&mut socket, &request, json!({"musicFind": {}})).await;
        assert_eq!(
            not_found.await.unwrap().status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert!(paypal_order.lock().unwrap().is_none());
        assert_eq!(
            request_song(&mut socket, &orders_url).await,
            json!({"orderId": "ORDER1"})
        );
        let sent = paypal_order.lock().unwrap().clone().unwrap();
        assert_eq!(
            sent["purchase_units"][0]["amount"],
            json!({"currency_code": "SEK", "value": "2.50"})
        );
        assert_eq!(
            sent["purchase_units"][0]["payee"],
            json!({"merchant_id": "MERCHANT00001"})
        );
        assert_eq!(
            payer_of(sent["assertion"].as_str().unwrap()),
            "MERCHANT00001"
        );
        assert_eq!(
            sent["purchase_units"][0]["description"],
            "Song request: Band - Song"
        );
        let response = client
            .post(format!("{orders_url}/OTHER/capture"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::GONE);
        let capture_url = format!("{orders_url}/ORDER1/capture");
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"],
            json!({"music": {"command": {"add": {
                "title": LINK,
                "user": "Viewer",
                "requestId": "ORDER1",
            }}}})
        );
        assert!(!api.deliveries.pending().is_empty());
        answer(
            &mut socket,
            &request,
            music(json!([{"id": "q1", "title": "Song"}])),
        )
        .await;
        wait_until_added(&api).await;
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::GONE);
    }

    #[tokio::test]
    async fn wheels_are_pushed_by_the_streamer() {
        let (address, api) = start_api(None).await;
        let (_, key, id) = streamer(&address, &api).await;
        let spin_url = format!("http://{address}/pages/{id}/spin");
        let wheel = || async {
            reqwest::get(&spin_url)
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()["wheel"]
                .clone()
        };
        let mut socket = connect(&address, &key).await;
        assert_eq!(wheel().await, Value::Null);
        let options = json!([{"text": "Yes", "weight": 1}, {"text": "No", "weight": 3}]);
        socket
            .send(Message::text(
                json!({"event": {"data": {"wheelOfLuck": {"wheel": {"options": options}}}}})
                    .to_string(),
            ))
            .await
            .unwrap();
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        assert_eq!(wheel().await, json!({"options": options}));
        socket
            .send(Message::text(
                json!({"event": {"data": {"wheelOfLuck": {}}}}).to_string(),
            ))
            .await
            .unwrap();
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        assert_eq!(wheel().await, Value::Null);
        socket
            .send(Message::text(
                json!({"event": {"data": {"wheelOfLuck": {"wheel": {"options": options}}}}})
                    .to_string(),
            ))
            .await
            .unwrap();
        drop(socket);
        for _ in 0..100 {
            if !api.remote_control.is_connected(&id) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(wheel().await, Value::Null);
    }

    #[tokio::test]
    async fn pending_captures_add_no_song() {
        let (address, api, _) = start_paid().await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        connect_paypal(&api);
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"currency": "EUR", "paywall": {"price": 250}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut socket = connect(&address, &key).await;
        let orders_url = format!("http://{address}/pages/{id}/songs/orders");
        assert_eq!(
            request_song(&mut socket, &orders_url).await,
            json!({"orderId": "ORDER1"})
        );
        let response = client
            .post(format!("{orders_url}/ORDER1/capture"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
        assert!(api.deliveries.pending().is_empty());
    }

    #[tokio::test]
    async fn paid_songs_are_retried_until_added_once() {
        let (address, api, _) = start_paid().await;
        let (key, id) = paid_streamer(&address, &api).await;
        let orders_url = format!("http://{address}/pages/{id}/songs/orders");
        let mut socket = connect(&address, &key).await;
        assert_eq!(
            request_song(&mut socket, &orders_url).await,
            json!({"orderId": "ORDER1"})
        );
        let response = reqwest::Client::new()
            .post(format!("{orders_url}/ORDER1/capture"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let add = json!({"music": {"command": {"add": {
            "title": LINK,
            "user": "Viewer",
            "requestId": "ORDER1",
        }}}});
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], add);
        socket
            .send(Message::text(
                json!({"response": {"id": request["request"]["id"], "result": {"error": {}}}})
                    .to_string(),
            ))
            .await
            .unwrap();
        socket.close(None).await.unwrap();
        let mut socket = connect(&address, &key).await;
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], add);
        socket.close(None).await.unwrap();
        assert!(!api.deliveries.pending().is_empty());
        let mut socket = connect(&address, &key).await;
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], add);
        answer(
            &mut socket,
            &request,
            music(json!([{"id": "q1", "title": "Song"}])),
        )
        .await;
        wait_until_added(&api).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(200), receive(&mut socket))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn spins_are_sent_to_the_streamer() {
        let (address, api, paypal_order) = start_paid().await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        connect_paypal(&api);
        let spin_url = format!("http://{address}/pages/{id}/spin");
        let public: Value = client
            .get(&spin_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(public, json!({"spin": null, "wheel": null}));
        let settings_url = format!("http://{address}/account/settings");
        let response = client
            .post(&settings_url)
            .bearer_auth(&session)
            .json(&json!({"currency": "jpy", "paywall": null, "spin": {
                "options": [{"spins": 1, "price": 500}],
            }}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let settings: Value = client
            .post(&settings_url)
            .bearer_auth(&session)
            .json(&json!({"currency": "usd", "paywall": null, "spin": {
                "options": [{"spins": 2, "price": 800}, {"spins": 1, "price": 500}],
            }}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            settings["spin"],
            json!({"options": [{"spins": 1, "price": 500}, {"spins": 2, "price": 800}]})
        );
        let public: Value = client
            .get(&spin_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            public,
            json!({"spin": {
                "options": [{"spins": 1, "price": 500}, {"spins": 2, "price": 800}],
                "currency": "USD",
                "clientId": "id",
                "merchantId": "MERCHANT00001",
                "environment": "production",
            }, "wheel": null})
        );
        let orders_url = format!("{spin_url}/orders");
        let response = client
            .post(&orders_url)
            .json(&json!({"user": "Viewer", "spins": 2}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let mut socket = connect(&address, &key).await;
        socket
            .send(Message::text(
                json!({"event": {"data": {"wheelOfLuck": {"wheel": {"options": [
                    {"text": "Yes", "weight": 1},
                ]}}}}})
                .to_string(),
            ))
            .await
            .unwrap();
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        for request in [
            json!({"user": "Viewer", "spins": 3}),
            json!({"user": "Viewer", "spins": 2, "message": "a".repeat(201)}),
        ] {
            let response = client
                .post(&orders_url)
                .json(&request)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
        let order: Value = client
            .post(&orders_url)
            .json(&json!({"user": "Viewer", "spins": 2, "message": " Go big! "}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(order, json!({"orderId": "ORDER1"}));
        let sent = paypal_order.lock().unwrap().clone().unwrap();
        assert_eq!(
            sent["purchase_units"][0]["amount"],
            json!({"currency_code": "USD", "value": "8.00"})
        );
        assert_eq!(
            sent["purchase_units"][0]["description"],
            "2 wheel spins by Viewer: Go big!"
        );
        let response = client
            .post(format!(
                "http://{address}/pages/{id}/tips/orders/ORDER1/capture"
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::GONE);
        let response = client
            .post(format!("{orders_url}/ORDER1/capture"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let request = receive(&mut socket).await;
        let spin = json!({"spinWheelOfLuck": {
            "name": "Viewer",
            "message": "Go big!",
            "count": 2,
            "requestId": "ORDER1",
        }});
        assert_eq!(request["request"]["data"], spin);
        socket.close(None).await.unwrap();
        let mut socket = connect(&address, &key).await;
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], spin);
        answer(&mut socket, &request, Value::Null).await;
        wait_until_added(&api).await;
    }

    #[tokio::test]
    async fn tips_are_sent_to_the_streamer() {
        let (address, api, paypal_order) = start_paid().await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        connect_paypal(&api);
        let tips_url = format!("http://{address}/pages/{id}/tips");
        let public: Value = client
            .get(&tips_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(public, json!({"tips": null}));
        let orders_url = format!("{tips_url}/orders");
        let response = client
            .post(&orders_url)
            .json(&json!({"amount": 500}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let settings: Value = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"currency": "usd", "paywall": null, "tips": {
                "amounts": [1000, 200, 500],
            }}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["tips"], json!({"amounts": [200, 500, 1000]}));
        let public: Value = client
            .get(&tips_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            public,
            json!({"tips": {
                "amounts": [200, 500, 1000],
                "currency": "USD",
                "clientId": "id",
                "merchantId": "MERCHANT00001",
                "environment": "production",
            }})
        );
        for request in [
            json!({"amount": 49}),
            json!({"amount": 700, "message": "a".repeat(201)}),
        ] {
            let response = client
                .post(&orders_url)
                .json(&request)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
        let order: Value = client
            .post(&orders_url)
            .json(&json!({"amount": 700, "user": " Viewer ", "message": " Great stream! "}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(order, json!({"orderId": "ORDER1"}));
        let sent = paypal_order.lock().unwrap().clone().unwrap();
        assert_eq!(
            sent["purchase_units"][0]["amount"],
            json!({"currency_code": "USD", "value": "7.00"})
        );
        assert_eq!(
            sent["purchase_units"][0]["description"],
            "Tip from Viewer: Great stream!"
        );
        let response = client
            .post(format!(
                "http://{address}/pages/{id}/songs/orders/ORDER1/capture"
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::GONE);
        let response = client
            .post(format!("{orders_url}/ORDER1/capture"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let response = client
            .post(format!("{orders_url}/ORDER1/capture"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::GONE);
        assert_eq!(api.deliveries.pending().len(), 1);
        let tip = json!({"tip": {
            "name": "Viewer",
            "message": "Great stream!",
            "amount": "7.00",
            "currency": "USD",
            "requestId": "ORDER1",
        }});
        let mut socket = connect(&address, &key).await;
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], tip);
        socket
            .send(Message::text(
                json!({"response": {"id": request["request"]["id"], "result": {"error": {}}}})
                    .to_string(),
            ))
            .await
            .unwrap();
        socket.close(None).await.unwrap();
        let mut socket = connect(&address, &key).await;
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], tip);
        answer(&mut socket, &request, Value::Null).await;
        wait_until_added(&api).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(200), receive(&mut socket))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn music_status_comes_from_the_streamer() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (_, key, id) = streamer(&address, &api).await;
        let music_url = format!("http://{address}/pages/{id}/songs/queue");
        let response = client.get(&music_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let mut socket = connect(&address, &key).await;
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        let status = tokio::spawn({
            let client = client.clone();
            let music_url = music_url.clone();
            async move {
                client
                    .get(music_url)
                    .send()
                    .await
                    .unwrap()
                    .json::<Value>()
                    .await
                    .unwrap()
            }
        });
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"],
            json!({"music": {"command": {"status": {}}}})
        );
        answer(
            &mut socket,
            &request,
            music(json!([{"id": "q1", "title": "Song", "duration": 213, "user": "Viewer"}])),
        )
        .await;
        assert_eq!(
            status.await.unwrap()["songs"],
            json!([{"id": "q1", "title": "Song", "artist": null, "duration": 213, "user": "Viewer"}])
        );
        socket.close(None).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let response = client.get(&music_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn songs_need_a_paywall() {
        let (address, api) = start_api(None).await;
        let (_, key, id) = streamer(&address, &api).await;
        let _socket = connect(&address, &key).await;
        let response = reqwest::Client::new()
            .post(format!("http://{address}/pages/{id}/songs/orders"))
            .json(&json!({"song": LINK}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.text().await.unwrap(),
            "The streamer no longer takes song requests. Reload the page."
        );
    }

    #[tokio::test]
    async fn songs_not_found_are_dropped() {
        let (address, api, _) = start_paid().await;
        let (key, id) = paid_streamer(&address, &api).await;
        let orders_url = format!("http://{address}/pages/{id}/songs/orders");
        let mut socket = connect(&address, &key).await;
        assert_eq!(
            request_song(&mut socket, &orders_url).await,
            json!({"orderId": "ORDER1"})
        );
        let response = reqwest::Client::new()
            .post(format!("{orders_url}/ORDER1/capture"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let request = receive(&mut socket).await;
        let mut status = music(json!([]));
        status["music"]["status"]["songNotFound"] = json!(true);
        answer(&mut socket, &request, status).await;
        wait_until_added(&api).await;
    }

    async fn start_twitch() -> String {
        let router = Router::new()
            .route(
                "/token",
                post(|body: String| async move {
                    assert!(body.contains("code=c"));
                    Json(json!({"access_token": "u", "token_type": "bearer"}))
                }),
            )
            .route(
                "/users",
                get(|headers: HeaderMap| async move {
                    assert_eq!(headers[AUTHORIZATION], "Bearer u");
                    assert_eq!(headers["client-id"], "id");
                    Json(json!({"data": [{
                        "id": "42",
                        "login": "erik",
                        "display_name": "Erik",
                        "profile_image_url": "https://static-cdn.jtvnw.net/erik.png",
                    }]}))
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await });
        format!("http://{address}")
    }

    fn location(response: &reqwest::Response) -> String {
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        response.headers()["location"].to_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn streamers_sign_in_and_manage_their_page() {
        let twitch = start_twitch().await;
        let (address, _) = start_api_with(
            None,
            Some(OAuthApp {
                client_id: "id".to_string(),
                client_secret: "secret".to_string(),
                authorize_url: format!("{twitch}/authorize"),
                token_url: format!("{twitch}/token"),
                users_url: format!("{twitch}/users"),
            }),
            Pages::load(None).unwrap(),
        )
        .await;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let unconfigured = start().await;
        let response = client
            .get(format!("http://{unconfigured}/auth/twitch/login"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let response = client
            .get(format!("http://{address}/auth/twitch/login"))
            .send()
            .await
            .unwrap();
        let login = location(&response);
        let cookie = response.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        assert!(login.starts_with(&format!("{twitch}/authorize?")));
        let state = reqwest::Url::parse(&login)
            .unwrap()
            .query_pairs()
            .find(|(name, _)| name == "state")
            .unwrap()
            .1
            .into_owned();
        assert_eq!(cookie, format!("moblin_sign_in_state={state}"));
        let callback = format!("http://{address}/auth/twitch/callback?code=c&state={state}");
        let page = location(
            &client
                .get(&callback)
                .header("cookie", &cookie)
                .send()
                .await
                .unwrap(),
        );
        let session = page
            .strip_prefix("https://moblin.app/dashboard/#session=")
            .unwrap()
            .to_string();
        assert_eq!(
            location(
                &client
                    .get(&callback)
                    .header("cookie", &cookie)
                    .send()
                    .await
                    .unwrap()
            ),
            "https://moblin.app/dashboard/#signInFailed"
        );
        let response = client
            .get(format!("http://{address}/auth/twitch/login"))
            .send()
            .await
            .unwrap();
        let state = reqwest::Url::parse(&location(&response))
            .unwrap()
            .query_pairs()
            .find(|(name, _)| name == "state")
            .unwrap()
            .1
            .into_owned();
        assert_eq!(
            location(
                &client
                    .get(format!(
                        "http://{address}/auth/twitch/callback?code=c&state={state}"
                    ))
                    .header("cookie", "moblin_sign_in_state=other")
                    .send()
                    .await
                    .unwrap()
            ),
            "https://moblin.app/dashboard/#signInFailed"
        );
        let response = client
            .get(format!("http://{address}/account"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let account: Value = client
            .get(format!("http://{address}/account"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            account,
            json!({"name": "Erik", "login": "erik", "streamerUrl": null, "settings": null})
        );
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"paywall": null}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let account: Value = client
            .post(format!("http://{address}/account/page"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let key = key_of(&account);
        assert_eq!(
            account["settings"]["id"].as_str(),
            remote_control::id_of(&key).as_deref()
        );
        let settings: Value = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"paywall": null}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings, account["settings"]);
        let settings: Value = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({
                "paywall": null,
                "title": "Pick a banger",
                "message": "Thanks for hanging out!",
                "accent": "pink",
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["title"], "Pick a banger");
        assert_eq!(settings["accent"], "pink");
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"paywall": null, "accent": "teal"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let name: Value = client
            .get(format!("http://{address}/pages/twitch/Erik"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(name["id"], account["settings"]["id"]);
        let profile: Value = client
            .get(format!(
                "http://{address}/pages/{}",
                name["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            profile,
            json!({
                "name": "Erik",
                "login": "erik",
                "avatar": "https://static-cdn.jtvnw.net/erik.png",
                "title": "Pick a banger",
                "message": "Thanks for hanging out!",
                "accent": "pink",
            })
        );
        let response = client
            .get(format!("http://{address}/pages/unknown"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let response = client
            .get(format!("http://{address}/pages/twitch/other"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let again: Value = client
            .post(format!("http://{address}/account/page"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(key_of(&again), key);
    }

    #[tokio::test]
    async fn new_streamer_urls_keep_the_page() {
        let (address, api) = start_api(None).await;
        let (session, key, id) = streamer(&address, &api).await;
        let mut socket = connect(&address, &key).await;
        assert!(api.remote_control.is_connected(&id));
        let account: Value = reqwest::Client::new()
            .post(format!("http://{address}/account/streamer-url"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let new_key = key_of(&account);
        assert_ne!(new_key, key);
        assert_eq!(account["settings"]["id"], id);
        assert!(!api.remote_control.is_connected(&id));
        loop {
            match socket.next().await {
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            }
        }
        assert!(
            tokio_tungstenite::connect_async(format!(
                "ws://{address}/remote-control/streamer/{key}"
            ))
            .await
            .is_err()
        );
        let _socket = connect(&address, &new_key).await;
        assert!(api.remote_control.is_connected(&id));
        let name: Value = reqwest::Client::new()
            .get(format!("http://{address}/pages/twitch/erik"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(name["id"], id);
    }

    #[tokio::test]
    async fn signing_out_ends_every_session() {
        let (address, api) = start_api(None).await;
        let (session, _, _) = streamer(&address, &api).await;
        let other = api.auth.session("1", 0);
        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://{address}/account/sign-out"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        for token in [&session, &other] {
            let response = client
                .get(format!("http://{address}/account"))
                .bearer_auth(token)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
        let response = client
            .get(format!("http://{address}/account"))
            .bearer_auth(api.auth.session("1", 1))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn paypal_is_connected_through_onboarding() {
        let (paypal_url, fake) = start_paypal().await;
        let (address, api) = start_api(Some(new_paypal(paypal_url))).await;
        let (session, _, id) = streamer(&address, &api).await;
        let old = api.auth.session_at("1", 0, crate::deliveries::now() - 601);
        let client = reqwest::Client::new();
        let paypal_url = format!("http://{address}/account/paypal");
        let settings_url = format!("http://{address}/account/settings");
        let paywall = json!({"currency": "SEK", "paywall": {"price": 250}});
        let response = client
            .post(&settings_url)
            .bearer_auth(&session)
            .json(&paywall)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let response = client
            .post(format!("{paypal_url}/connect"))
            .bearer_auth(&old)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(fake.referral.lock().unwrap().is_none());
        let sign_up: Value = client
            .post(format!("{paypal_url}/connect"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            sign_up,
            json!({"url": "https://www.sandbox.paypal.com/bizsignup/x"})
        );
        let tracking_id = api.accounts.get("1").unwrap().paypal_tracking_id.unwrap();
        let referral = fake.referral.lock().unwrap().clone().unwrap();
        assert_eq!(referral["attribution"], "BN");
        assert_eq!(referral["body"]["tracking_id"], tracking_id.as_str());
        assert_eq!(
            referral["body"]["partner_config_override"]["return_url"],
            "https://moblin.app/dashboard/"
        );
        let account: Value = client
            .get(format!("http://{address}/account"))
            .bearer_auth(&old)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(account["settings"]["merchantId"], Value::Null);
        assert_eq!(account["settings"]["paypalPending"], true);
        let check = || {
            client
                .post(format!("{paypal_url}/check"))
                .bearer_auth(&old)
                .send()
        };
        assert_eq!(check().await.unwrap().status(), StatusCode::NOT_FOUND);
        *fake.seller.lock().unwrap() = Some(json!({
            "tracking_id": tracking_id,
            "merchant_id": "SELLER0000001",
            "payments_receivable": true,
            "primary_email_confirmed": true,
            "oauth_integrations": [{
                "integration_type": "OAUTH_THIRD_PARTY",
                "integration_method": "PAYPAL",
                "oauth_third_party": [{
                    "partner_client_id": "other",
                    "merchant_client_id": "m",
                    "scopes": ["https://uri.paypal.com/services/payments/realtimepayment"],
                }],
            }],
        }));
        assert_eq!(check().await.unwrap().status(), StatusCode::CONFLICT);
        {
            let mut seller = fake.seller.lock().unwrap();
            let seller = seller.as_mut().unwrap();
            seller["oauth_integrations"][0]["oauth_third_party"][0]["partner_client_id"] =
                json!("id");
            seller["primary_email_confirmed"] = json!(false);
        }
        assert_eq!(
            check().await.unwrap().status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        fake.seller.lock().unwrap().as_mut().unwrap()["primary_email_confirmed"] = json!(true);
        *fake.list_sellers.lock().unwrap() = true;
        let settings: Value = check().await.unwrap().json().await.unwrap();
        assert_eq!(settings["merchantId"], "SELLER0000001");
        assert_eq!(settings["paypalPending"], false);
        let response = client
            .post(&settings_url)
            .bearer_auth(&old)
            .json(&paywall)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let public: Value = client
            .get(format!("http://{address}/pages/{id}/songs"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(public["paywall"]["merchantId"], "SELLER0000001");
        assert_eq!(public["paywall"]["clientId"], "id");
        let settings: Value = check().await.unwrap().json().await.unwrap();
        assert_eq!(settings["merchantId"], "SELLER0000001");
        let settings: Value = client
            .post(format!("{paypal_url}/disconnect"))
            .bearer_auth(&old)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["merchantId"], Value::Null);
        assert_eq!(settings["paypalPending"], false);
        let public: Value = client
            .get(format!("http://{address}/pages/{id}/songs"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(public["paywall"]["merchantId"], Value::Null);
        assert_eq!(public["paywall"]["clientId"], Value::Null);
        for (currency, status) in [("SEK", StatusCode::OK), ("EUR", StatusCode::CONFLICT)] {
            let response = client
                .post(&settings_url)
                .bearer_auth(&session)
                .json(&json!({"currency": currency, "paywall": {"price": 250}}))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), status);
        }
    }

    #[tokio::test]
    async fn paypal_webhooks_connect_and_disconnect_streamers() {
        let unconfigured = start().await;
        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://{unconfigured}/paypal/webhooks"))
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let (paypal_url, fake) = start_paypal().await;
        let (address, api) = start_api(Some(new_paypal(paypal_url))).await;
        let (session, _, id) = streamer(&address, &api).await;
        let response = client
            .post(format!("http://{address}/account/paypal/connect"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let tracking_id = api.accounts.get("1").unwrap().paypal_tracking_id.unwrap();
        *fake.seller.lock().unwrap() = Some(json!({
            "merchant_id": "SELLER0000001",
            "tracking_id": tracking_id,
            "payments_receivable": true,
            "primary_email_confirmed": true,
            "oauth_integrations": [{"oauth_third_party": [{
                "partner_client_id": "id",
                "merchant_client_id": "m",
                "scopes": ["https://uri.paypal.com/services/payments/realtimepayment"],
            }]}],
        }));
        let webhook = |signature: &str, event: &Value| {
            client
                .post(format!("http://{address}/paypal/webhooks"))
                .header("paypal-auth-algo", "SHA256withRSA")
                .header(
                    "paypal-cert-url",
                    "https://api.paypal.com/v1/notifications/certs/C",
                )
                .header("paypal-transmission-id", "T")
                .header("paypal-transmission-sig", signature)
                .header("paypal-transmission-time", "2026-10-16T05:19:23Z")
                .header("content-type", "application/json")
                .body(event.to_string())
                .send()
        };
        let completed = json!({
            "id": "WH-1",
            "event_type": "MERCHANT.ONBOARDING.COMPLETED",
            "resource": {"partner_client_id": "id", "merchant_id": "SELLER0000001", "links": []},
        });
        let response = webhook("bad", &completed).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(api.accounts.get("1").unwrap().merchant_id, None);
        let response = webhook("good", &completed).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let verification = fake.verification.lock().unwrap().clone().unwrap();
        assert_eq!(
            verification,
            json!({
                "auth_algo": "SHA256withRSA",
                "cert_url": "https://api.paypal.com/v1/notifications/certs/C",
                "transmission_id": "T",
                "transmission_sig": "good",
                "transmission_time": "2026-10-16T05:19:23Z",
                "webhook_id": "WH",
                "webhook_event": completed,
            })
        );
        let account = api.accounts.get("1").unwrap();
        assert_eq!(account.merchant_id.as_deref(), Some("SELLER0000001"));
        assert_eq!(account.paypal_tracking_id, None);
        let response = webhook("good", &completed).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let revoked = json!({
            "id": "WH-2",
            "event_type": "MERCHANT.PARTNER-CONSENT.REVOKED",
            "resource": {"merchant_id": "SELLER0000001", "tracking_id": tracking_id},
        });
        let response = webhook("good", &revoked).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(api.accounts.get("1").unwrap().merchant_id, None);
        let public: Value = client
            .get(format!("http://{address}/pages/{id}/tips"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(public, json!({"tips": null}));
    }

    #[tokio::test]
    async fn account_routes_need_a_session() {
        let (address, api) = start_api(None).await;
        let (session, _, _) = streamer(&address, &api).await;
        let unknown = api.auth.session("2", 0);
        let old = api.auth.session("1", 1);
        let (payload, _) = session.split_once('.').unwrap();
        let forged = format!("{payload}.AAAA");
        let client = reqwest::Client::new();
        for (method, path) in [
            ("GET", "/account"),
            ("POST", "/account/page"),
            ("POST", "/account/streamer-url"),
            ("POST", "/account/settings"),
            ("POST", "/account/paypal/connect"),
            ("POST", "/account/paypal/check"),
            ("POST", "/account/sign-out"),
        ] {
            for token in [
                None,
                Some("garbage"),
                Some(forged.as_str()),
                Some(unknown.as_str()),
                Some(old.as_str()),
            ] {
                let mut request = client
                    .request(method.parse().unwrap(), format!("http://{address}{path}"))
                    .json(&json!({"paywall": null}));
                if let Some(token) = token {
                    request = request.bearer_auth(token);
                }
                let response = request.send().await.unwrap();
                assert_eq!(
                    response.status(),
                    StatusCode::UNAUTHORIZED,
                    "{method} {path} with {token:?}"
                );
            }
        }
        let (unknown_key, _) = api.remote_control.new_key().unwrap();
        assert!(
            tokio_tungstenite::connect_async(format!(
                "ws://{address}/remote-control/streamer/{unknown_key}"
            ))
            .await
            .is_err()
        );
    }
}
