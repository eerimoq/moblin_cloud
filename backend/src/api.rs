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

use crate::accounts::{Account, AccountError, Accounts, Device, Page};
use crate::app_attest::AppAttest;
use crate::auth::{Auth, STATE_COOKIE};
use crate::challenges::Challenges;
use crate::deliveries::{self, Deliveries};
use crate::live::Live;
use crate::pages::{
    CURRENCIES, Pages, Paywall, Settings, SettingsError, Spin, SpinOption, Tips, format_price,
};
use crate::payments::{CaptureError, Item, Order, Orders, Payee};
use crate::paypal::{PayPal, Seller};
use crate::remote_control::{self, MusicCommand, MusicStatus, RequestError, Streamers, Wheel};
use crate::store::{Channel, Store, Streamer};
use crate::stripe::{self, Stripe};

const ASSERTION_HEADER: &str = "moblin-assertion";

pub struct Api {
    pub store: Arc<Store>,
    pub challenges: Challenges,
    pub app_attest: Option<AppAttest>,
    pub live: Option<Live>,
    pub remote_control: Streamers,
    pub pages: Pages,
    pub paypal: Option<PayPal>,
    pub stripe: Option<Stripe>,
    pub orders: Orders,
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
        .route("/account/devices", post(handle_account_devices))
        .route(
            "/account/devices/choose",
            post(handle_account_devices_choose),
        )
        .route(
            "/account/devices/{device}/key",
            post(handle_account_device_key),
        )
        .route(
            "/account/devices/{device}/remove",
            post(handle_account_device_remove),
        )
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
        .route(
            "/account/stripe/connect",
            post(handle_account_stripe_connect),
        )
        .route("/account/stripe/check", post(handle_account_stripe_check))
        .route(
            "/account/stripe/disconnect",
            post(handle_account_stripe_disconnect),
        )
        .route("/account/sign-out", post(handle_account_sign_out))
        .route("/account/test/song", post(handle_account_test_song))
        .route("/account/test/tip", post(handle_account_test_tip))
        .route("/account/test/spin", post(handle_account_test_spin))
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
        .device_of_key(&key)
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(upgrade.on_upgrade(move |socket| async move { api.remote_control.serve(id, socket).await }))
}

fn page_of(api: &Api, id: &str) -> Option<Page> {
    api.accounts.of_page(id).and_then(|account| account.page)
}

fn music_device(api: &Api, id: &str) -> String {
    page_of(api, id)
        .and_then(|page| page.music_device().map(|device| device.id.clone()))
        .unwrap_or_default()
}

fn wheel_device(api: &Api, id: &str) -> String {
    page_of(api, id)
        .and_then(|page| page.wheel_device().map(|device| device.id.clone()))
        .unwrap_or_default()
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
    stripe: bool,
    #[serde(rename = "stripeAccountId")]
    stripe_account_id: Option<String>,
    #[serde(rename = "stripePending")]
    stripe_pending: bool,
    #[serde(rename = "stripeTest")]
    stripe_test: bool,
    title: Option<String>,
    message: Option<String>,
    accent: Option<String>,
}

fn any_connected(api: &Api, account: &Account) -> bool {
    account
        .page
        .iter()
        .flat_map(|page| &page.devices)
        .any(|device| api.remote_control.is_connected(&device.id))
}

fn settings_response(api: &Api, id: String, settings: Settings) -> Json<SettingsResponse> {
    let account = api.accounts.of_page(&id).unwrap_or_default();
    Json(SettingsResponse {
        connected: any_connected(api, &account),
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
        stripe: api.stripe.is_some(),
        stripe_pending: account.stripe_account_id.is_some() && !account.stripe_ready,
        stripe_account_id: account.stripe_account_id.filter(|_| account.stripe_ready),
        stripe_test: api.stripe.as_ref().is_some_and(Stripe::is_test),
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

fn stripe_of(api: &Api, id: &str) -> Option<String> {
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
    devices: Vec<DeviceResponse>,
    music_device: Option<String>,
    wheel_device: Option<String>,
    settings: Option<SettingsResponse>,
}

#[derive(Serialize)]
struct DeviceResponse {
    id: String,
    name: String,
    key: String,
    connected: bool,
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
    let page = account.page.as_ref();
    Json(AccountResponse {
        name: account.name,
        login: account.login,
        devices: page
            .iter()
            .flat_map(|page| &page.devices)
            .map(|device| DeviceResponse {
                id: device.id.clone(),
                name: device.name.clone(),
                key: device.key.clone(),
                connected: api.remote_control.is_connected(&device.id),
            })
            .collect(),
        music_device: page
            .and_then(Page::music_device)
            .map(|device| device.id.clone()),
        wheel_device: page
            .and_then(Page::wheel_device)
            .map(|device| device.id.clone()),
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
        AccountError::TooManyDevices => {
            (StatusCode::CONFLICT, "at most 5 Moblin devices".to_string())
        }
        AccountError::UnknownDevice => (StatusCode::NOT_FOUND, "no such device".to_string()),
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
    let device = Device {
        id: page.clone(),
        name: "Moblin".to_string(),
        key,
    };
    let account = api
        .accounts
        .give_page(&id, Page::new(page, device))
        .map_err(account_error)?;
    info!("{id} got a page");
    Ok(account_response(&api, account))
}

#[derive(Deserialize)]
struct DeviceRequest {
    name: String,
}

async fn handle_account_devices(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<DeviceRequest>,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let id = signed_in(&api, &headers)?.id;
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 30 {
        return Err((
            StatusCode::BAD_REQUEST,
            "the name must be 1 to 30 characters".to_string(),
        ));
    }
    let (key, _) = new_key(&api)?;
    let device = api.remote_control.new_request_id().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "no device could be created".to_string(),
    ))?;
    let account = api
        .accounts
        .add_device(
            &id,
            Device {
                id: device.clone(),
                name: name.to_string(),
                key,
            },
        )
        .map_err(account_error)?;
    info!("{id} added Moblin device {device}");
    Ok(account_response(&api, account))
}

#[derive(Deserialize)]
struct ChooseDevicesRequest {
    music: String,
    wheel: String,
}

async fn handle_account_devices_choose(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<ChooseDevicesRequest>,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let id = signed_in(&api, &headers)?.id;
    let account = api
        .accounts
        .choose_devices(&id, request.music, request.wheel)
        .map_err(account_error)?;
    Ok(account_response(&api, account))
}

async fn handle_account_device_key(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(device): Path<String>,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let id = signed_in(&api, &headers)?.id;
    let (key, _) = new_key(&api)?;
    let account = api
        .accounts
        .replace_key(&id, &device, key)
        .map_err(account_error)?;
    api.remote_control.disconnect(&device);
    info!("{id} got a new Moblin Cloud key for device {device}");
    Ok(account_response(&api, account))
}

async fn handle_account_device_remove(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(device): Path<String>,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let id = signed_in(&api, &headers)?.id;
    let account = api
        .accounts
        .remove_device(&id, &device)
        .map_err(account_error)?;
    api.remote_control.disconnect(&device);
    info!("{id} removed Moblin device {device}");
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
struct SignUpResponse {
    url: String,
}

async fn handle_account_paypal_connect(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<SignUpResponse>, (StatusCode, String)> {
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
    Ok(Json(SignUpResponse { url }))
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

#[derive(Deserialize)]
struct StripeConnectRequest {
    country: Option<String>,
}

async fn handle_account_stripe_connect(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    request: Option<Json<StripeConnectRequest>>,
) -> Result<Json<SignUpResponse>, (StatusCode, String)> {
    let SignedIn { id, account, fresh } = signed_in(&api, &headers)?;
    if !fresh {
        return Err((
            StatusCode::FORBIDDEN,
            "sign in again to connect Stripe".to_string(),
        ));
    }
    let stripe = api.stripe.as_ref().ok_or_else(payments_unavailable)?;
    let stripe_account_id = match account.stripe_account_id {
        Some(stripe_account_id) => stripe_account_id,
        None => {
            let country = request
                .and_then(|Json(request)| request.country)
                .map(|country| country.trim().to_uppercase())
                .filter(|country| stripe::COUNTRIES.contains(&country.as_str()))
                .ok_or((
                    StatusCode::BAD_REQUEST,
                    "pick one of the countries Stripe supports".to_string(),
                ))?;
            let stripe_account_id = stripe
                .create_account(&id, &account.name, &country)
                .await
                .map_err(|error| {
                    warn!("creating a Stripe account in {country} for {id} failed: {error:#}");
                    (
                        StatusCode::BAD_GATEWAY,
                        "Stripe could not create the account".to_string(),
                    )
                })?;
            api.accounts
                .start_stripe(&id, stripe_account_id.clone())
                .map_err(account_error)?;
            info!("{id} got Stripe account {stripe_account_id}");
            stripe_account_id
        }
    };
    let url = stripe
        .onboarding_link(
            &stripe_account_id,
            &format!("{}/dashboard/?stripe", api.auth.frontend_url),
        )
        .await
        .map_err(|error| {
            warn!("creating a Stripe account link for {id} failed: {error:#}");
            stripe_unreachable()
        })?;
    info!("{id} started connecting Stripe account {stripe_account_id}");
    Ok(Json(SignUpResponse { url }))
}

fn stripe_unreachable() -> (StatusCode, String) {
    (
        StatusCode::BAD_GATEWAY,
        "Stripe could not be reached".to_string(),
    )
}

async fn handle_account_stripe_check(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let SignedIn { id, account, .. } = signed_in(&api, &headers)?;
    let page = account.page.ok_or_else(no_page)?.id;
    let stripe = api.stripe.as_ref().ok_or_else(payments_unavailable)?;
    if let Some(stripe_account_id) = account.stripe_account_id {
        let ready = stripe
            .takes_payments(&stripe_account_id)
            .await
            .map_err(|error| {
                warn!("looking up Stripe account {stripe_account_id} of {id} failed: {error:#}");
                stripe_unreachable()
            })?;
        api.accounts
            .stripe_ready(&id, &stripe_account_id, ready)
            .map_err(account_error)?;
        if ready != account.stripe_ready {
            info!("Stripe account {stripe_account_id} of {id} takes payments: {ready}");
        }
        if !ready {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                "the Stripe account can't receive payments yet".to_string(),
            ));
        }
    }
    let settings = api.pages.get(&page);
    Ok(settings_response(&api, page, settings))
}

async fn handle_account_stripe_disconnect(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let SignedIn { id, account, .. } = signed_in(&api, &headers)?;
    let page = account.page.ok_or_else(no_page)?.id;
    api.accounts.forget_stripe(&id).map_err(account_error)?;
    info!("{id} disconnected Stripe");
    let settings = api.pages.get(&page);
    Ok(settings_response(&api, page, settings))
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
        "the spin wheel needs one to five options of 1 to 10 spins for 0.50 to 1000".to_string(),
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
        "tips need one to five amounts of 0.50 to 1000".to_string(),
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

fn client_id(api: &Api, merchant_id: &Option<String>) -> Option<String> {
    api.paypal
        .as_ref()
        .filter(|_| merchant_id.is_some())
        .map(|paypal| paypal.client_id().to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicStripe {
    publishable_key: String,
    account_id: String,
}

fn public_stripe(api: &Api, id: &str) -> Option<PublicStripe> {
    Some(PublicStripe {
        publishable_key: api.stripe.as_ref()?.publishable_key().to_string(),
        account_id: stripe_of(api, id)?,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicPaywall {
    price: u32,
    currency: String,
    client_id: Option<String>,
    merchant_id: Option<String>,
    environment: Option<&'static str>,
    stripe: Option<PublicStripe>,
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
            client_id: client_id(&api, &merchant_id),
            merchant_id,
            environment: api.paypal.as_ref().map(PayPal::environment),
            stripe: public_stripe(&api, &id),
        }),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicTips {
    amounts: Vec<u32>,
    currency: String,
    client_id: Option<String>,
    merchant_id: Option<String>,
    environment: Option<&'static str>,
    stripe: Option<PublicStripe>,
}

#[derive(Serialize)]
struct TipsResponse {
    tips: Option<PublicTips>,
}

async fn handle_tips(State(api): State<Arc<Api>>, Path(id): Path<String>) -> Json<TipsResponse> {
    let settings = api.pages.get(&id);
    let currency = settings.currency().to_string();
    let merchant_id = merchant_of(&api, &id);
    let stripe = public_stripe(&api, &id);
    let payable = merchant_id.is_some() || stripe.is_some();
    Json(TipsResponse {
        tips: settings.tips.filter(|_| payable).map(|tips| PublicTips {
            amounts: tips.amounts,
            currency,
            client_id: client_id(&api, &merchant_id),
            merchant_id,
            environment: api.paypal.as_ref().map(PayPal::environment),
            stripe,
        }),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicSpin {
    options: Vec<SpinOption>,
    currency: String,
    client_id: Option<String>,
    merchant_id: Option<String>,
    environment: Option<&'static str>,
    stripe: Option<PublicStripe>,
}

#[derive(Serialize)]
struct SpinResponse {
    spin: Option<PublicSpin>,
    wheel: Option<Wheel>,
    connected: bool,
}

async fn handle_spin(State(api): State<Arc<Api>>, Path(id): Path<String>) -> Json<SpinResponse> {
    let settings = api.pages.get(&id);
    let currency = settings.currency().to_string();
    let merchant_id = merchant_of(&api, &id);
    let stripe = public_stripe(&api, &id);
    let payable = merchant_id.is_some() || stripe.is_some();
    Json(SpinResponse {
        spin: settings.spin.filter(|_| payable).map(|spin| PublicSpin {
            options: spin.options,
            currency,
            client_id: client_id(&api, &merchant_id),
            merchant_id,
            environment: api.paypal.as_ref().map(PayPal::environment),
            stripe,
        }),
        wheel: api.remote_control.wheel(&wheel_device(&api, &id)),
        connected: api.remote_control.is_connected(&wheel_device(&api, &id)),
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QueueResponse {
    #[serde(flatten)]
    status: MusicStatus,
    song_requests: bool,
}

async fn handle_queue(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
) -> Result<Json<QueueResponse>, Failure> {
    let device = music_device(&api, &id);
    let status = api
        .remote_control
        .music(&device, MusicCommand::Status {})
        .await
        .map_err(music_error)?;
    Ok(Json(QueueResponse {
        status,
        song_requests: api.remote_control.takes_song_requests(&device),
    }))
}

#[derive(Deserialize)]
struct SongRequest {
    song: String,
    user: Option<String>,
    #[serde(default)]
    provider: Option<String>,
}

struct RequestedSong {
    link: String,
    user: String,
    name: String,
    duration: Option<u32>,
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
    let device = music_device(api, id);
    if api.remote_control.is_connected(&device) && !api.remote_control.takes_song_requests(&device)
    {
        return Err(failure(
            StatusCode::CONFLICT,
            "The streamer is not taking song requests right now.",
        ));
    }
    let song = api
        .remote_control
        .find(&device, link.clone())
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
        duration: song.duration,
    })
}

fn song_price(price: u32, duration: Option<u32>) -> u32 {
    price.saturating_mul(duration.unwrap_or(0).div_ceil(300).max(1))
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
    #[serde(flatten)]
    stripe: Option<StripePayment>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StripePayment {
    client_secret: String,
    price: u32,
    currency: String,
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
    let payee = payee(&api, &id, request.provider.as_deref())?;
    let song = requested_song(&api, &id, song_and_user(request)?).await?;
    let order = Order {
        streamer: id.clone(),
        item: Item::Song {
            link: song.link,
            name: song.name,
        },
        user: song.user,
        payee,
        price: song_price(paywall.price, song.duration),
        currency,
    };
    Ok(Json(start_order(&api, order).await?))
}

async fn handle_song_capture(
    State(api): State<Arc<Api>>,
    Path((id, order_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    tokio::spawn(capture_song(api, id, order_id))
        .await
        .unwrap_or_else(|_| Err(capture_crashed()))
}

fn payee(api: &Api, id: &str, provider: Option<&str>) -> Result<Payee, Failure> {
    match provider.unwrap_or("paypal") {
        "paypal" if api.paypal.is_some() => merchant_of(api, id).map(Payee::PayPal),
        "stripe" => stripe_of(api, id).map(Payee::Stripe),
        _ => None,
    }
    .ok_or_else(payment_unavailable)
}

async fn start_order(api: &Api, order: Order) -> Result<OrderResponse, Failure> {
    let provider = order.payee.provider();
    let created = async {
        api.orders.make_room()?;
        match &order.payee {
            Payee::PayPal(merchant_id) => {
                let paypal = api.paypal.as_ref().context("no PayPal")?;
                let order_id = paypal.create_order(&order, merchant_id).await?;
                anyhow::Ok((order_id, None))
            }
            Payee::Stripe(account) => {
                let stripe = api.stripe.as_ref().context("no Stripe")?;
                let (order_id, client_secret) = stripe.create_payment(&order, account).await?;
                Ok((
                    order_id,
                    Some(StripePayment {
                        client_secret,
                        price: order.price,
                        currency: order.currency.clone(),
                    }),
                ))
            }
        }
    };
    let (order_id, stripe) = created.await.map_err(|error| {
        warn!(
            "creating a {provider} {} order for remote control streamer {} failed: {error:#}",
            order.item.kind(),
            order.streamer
        );
        failure(
            StatusCode::BAD_GATEWAY,
            &format!("{provider} could not start the payment. Try again."),
        )
    })?;
    api.orders.insert(order_id.clone(), order);
    Ok(OrderResponse { order_id, stripe })
}

fn capture_crashed() -> Failure {
    failure(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Something went wrong with the payment.",
    )
}

fn payment_unavailable() -> Failure {
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        "Payments are unavailable right now. Try again later.",
    )
}

async fn capture(api: &Api, id: &str, order_id: &str, kind: &str) -> Result<Order, Failure> {
    let order = api
        .orders
        .get(order_id)
        .filter(|order| order.streamer == id && order.item.kind() == kind)
        .ok_or(failure(
            StatusCode::GONE,
            "This payment is unknown or expired, so no money was taken.",
        ))?;
    let music = music_device(api, id);
    let refusal = if kind == "song" && !api.remote_control.is_connected(&music) {
        Some((
            StatusCode::NOT_FOUND,
            "The streamer went offline, so your payment was not taken.",
        ))
    } else if kind == "song" && !api.remote_control.takes_song_requests(&music) {
        Some((
            StatusCode::CONFLICT,
            "The streamer stopped taking song requests, so your payment was not taken.",
        ))
    } else if kind == "spin" && api.remote_control.wheel(&wheel_device(api, id)).is_none() {
        Some((
            StatusCode::NOT_FOUND,
            "The streamer's wheel is gone, so your payment was not taken.",
        ))
    } else {
        None
    };
    if let Some((status, refusal)) = refusal {
        if let Payee::Stripe(account) = &order.payee
            && let Some(stripe) = &api.stripe
        {
            api.orders.remove(order_id);
            if let Err(error) = stripe.cancel(order_id, account).await {
                warn!("cancelling Stripe payment {order_id} failed: {error:#}");
            }
        }
        return Err(failure(status, refusal));
    }
    let provider = order.payee.provider();
    let captured = match &order.payee {
        Payee::PayPal(merchant_id) => {
            let paypal = api.paypal.as_ref().ok_or_else(payment_unavailable)?;
            paypal.capture(order_id, &order, merchant_id).await
        }
        Payee::Stripe(account) => {
            let stripe = api.stripe.as_ref().ok_or_else(payment_unavailable)?;
            stripe.capture(order_id, &order, account).await
        }
    };
    if !matches!(
        captured,
        Err(CaptureError::NotApproved | CaptureError::Failed)
    ) {
        api.orders.remove(order_id);
    }
    captured.map_err(|error| match error {
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
            &format!("{provider} is still reviewing the payment, so the wheel was not spun. If the money is taken, ask the streamer for a refund."),
        ),
        CaptureError::Pending if kind == "tip" => failure(
            StatusCode::PAYMENT_REQUIRED,
            &format!("{provider} is still reviewing the payment, so the tip was not shown. If {provider} approves it, the money still reaches the streamer."),
        ),
        CaptureError::Pending => failure(
            StatusCode::PAYMENT_REQUIRED,
            &format!("{provider} is still reviewing the payment, so no song was added. If the money is taken, ask the streamer for a refund."),
        ),
        CaptureError::Failed | CaptureError::Unexpected => failure(
            StatusCode::BAD_GATEWAY,
            &format!("{provider} could not complete the payment."),
        ),
    })?;
    info!(
        "{:?} paid {} {} in {provider} order {order_id}",
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
        duration: None,
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
    #[serde(default)]
    provider: Option<String>,
}

async fn handle_tip_order(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    Json(request): Json<TipRequest>,
) -> Result<Json<OrderResponse>, Failure> {
    let settings = api.pages.get(&id);
    let currency = settings.currency().to_string();
    let tips = settings.tips.ok_or(failure(
        StatusCode::BAD_REQUEST,
        "The streamer no longer takes tips. Reload the page.",
    ))?;
    let payee = payee(&api, &id, request.provider.as_deref())?;
    let minimum = tips.amounts.iter().copied().min().unwrap_or(50);
    if request.amount < minimum {
        return Err(failure(
            StatusCode::BAD_REQUEST,
            &format!("Tip at least {}.", format_price(minimum)),
        ));
    }
    let message = viewer_message(request.message)?;
    let order = Order {
        streamer: id.clone(),
        item: Item::Tip { message },
        user: user_name(request.user, "Someone"),
        payee,
        price: request.amount,
        currency,
    };
    Ok(Json(start_order(&api, order).await?))
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
        "queued a tip of {} {} by {:?} in order {order_id} for remote control streamer {id}",
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
    #[serde(default)]
    provider: Option<String>,
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
    let device = wheel_device(&api, &id);
    if api.remote_control.wheel(&device).is_none() {
        return Err(if api.remote_control.is_connected(&device) {
            failure(
                StatusCode::CONFLICT,
                "The streamer is not taking wheel spins right now.",
            )
        } else {
            failure(
                StatusCode::NOT_FOUND,
                "The streamer's wheel is not available right now. Try again when they are live.",
            )
        });
    }
    let payee = payee(&api, &id, request.provider.as_deref())?;
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
        payee,
        price: option.price,
        currency,
    };
    Ok(Json(start_order(&api, order).await?))
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
        "queued {spins} wheel spins by {:?} in order {order_id} for remote control streamer {id}",
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

#[derive(Deserialize)]
struct TestRequest {
    #[serde(default)]
    song: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

fn test_page(api: &Api, headers: &HeaderMap) -> Result<(String, Account, String), Failure> {
    let account = signed_in(api, headers)?.account;
    let id = account.page.as_ref().ok_or_else(no_page)?.id.clone();
    let request_id = api.remote_control.new_request_id().ok_or(failure(
        StatusCode::SERVICE_UNAVAILABLE,
        "The test could not be sent. Try again.",
    ))?;
    Ok((id, account, request_id))
}

fn moblin_not_connected() -> Failure {
    failure(StatusCode::CONFLICT, "Connect Moblin first.")
}

async fn handle_account_test_song(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<TestRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let (id, account, request_id) = test_page(&api, &headers)?;
    if !api.remote_control.is_connected(&music_device(&api, &id)) {
        return Err(moblin_not_connected());
    }
    let request = SongRequest {
        song: request.song.unwrap_or_default(),
        user: Some(account.name),
        provider: None,
    };
    let song = requested_song(&api, &id, song_and_user(request)?).await?;
    Ok(queue(&api, id, request_id, song))
}

async fn handle_account_test_tip(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<TestRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let (id, account, request_id) = test_page(&api, &headers)?;
    if !any_connected(&api, &account) {
        return Err(moblin_not_connected());
    }
    let settings = api.pages.get(&id);
    let amount = settings
        .tips
        .as_ref()
        .and_then(|tips| tips.amounts.first().copied())
        .unwrap_or(500);
    info!("queued a test tip in request {request_id} for remote control streamer {id}");
    api.deliveries.add(
        id,
        request_id,
        deliveries::Item::Tip {
            user: account.name,
            message: viewer_message(request.message)?.unwrap_or_default(),
            amount,
            currency: settings.currency().to_string(),
        },
    );
    Ok(accepted())
}

async fn handle_account_test_spin(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<TestRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let (id, account, request_id) = test_page(&api, &headers)?;
    let device = wheel_device(&api, &id);
    if api.remote_control.wheel(&device).is_none() {
        return Err(if api.remote_control.is_connected(&device) {
            failure(
                StatusCode::CONFLICT,
                "Pick a wheel of luck in Moblin's Moblin Cloud settings first.",
            )
        } else {
            moblin_not_connected()
        });
    }
    let spins = api
        .pages
        .get(&id)
        .spin
        .and_then(|spin| spin.options.first().map(|option| option.spins))
        .unwrap_or(1);
    info!("queued a test spin in request {request_id} for remote control streamer {id}");
    api.deliveries.add(
        id,
        request_id,
        deliveries::Item::Spin {
            user: account.name,
            message: viewer_message(request.message)?.unwrap_or_default(),
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
    use crate::database::Database;

    async fn start() -> String {
        start_with(None).await
    }

    async fn start_with(paypal: Option<PayPal>) -> String {
        start_api(paypal).await.0
    }

    async fn start_api(paypal: Option<PayPal>) -> (String, Arc<Api>) {
        start_api_with(paypal, None, None, Pages::new(Database::memory())).await
    }

    async fn start_api_with(
        paypal: Option<PayPal>,
        stripe: Option<Stripe>,
        twitch: Option<OAuthApp>,
        pages: Pages,
    ) -> (String, Arc<Api>) {
        let api = Arc::new(Api {
            store: Arc::new(Store::new(6, Database::memory())),
            challenges: Challenges::new(),
            app_attest: None,
            live: None,
            remote_control: Streamers::new(),
            pages,
            paypal,
            stripe,
            orders: Orders::new(),
            deliveries: Deliveries::new(Database::memory()),
            auth: Auth::new(
                reqwest::Client::new(),
                "http://localhost".to_string(),
                "https://moblin.app".to_string(),
                twitch,
                false,
                Some("secret"),
            )
            .unwrap(),
            accounts: Accounts::new(Database::memory()),
        });
        tokio::spawn({
            let api = api.clone();
            async move { api.deliveries.run(&api.remote_control, &api.accounts).await }
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

    #[derive(Clone, Default)]
    struct FakeStripe {
        accounts: Arc<std::sync::Mutex<Vec<Value>>>,
        link: Arc<std::sync::Mutex<Option<Value>>>,
        active: Arc<std::sync::Mutex<bool>>,
        payment: Arc<std::sync::Mutex<Option<Value>>>,
        state: Arc<std::sync::Mutex<&'static str>>,
        cancelled: Arc<std::sync::Mutex<Vec<String>>>,
    }

    fn new_stripe(url: String) -> Stripe {
        Stripe::new(
            reqwest::Client::new(),
            url,
            "sk_test_secret".to_string(),
            "pk_test_public".to_string(),
        )
    }

    async fn start_stripe() -> (String, FakeStripe) {
        let fake = FakeStripe::default();
        *fake.state.lock().unwrap() = "requires_payment_method";
        let router = Router::new()
            .route(
                "/v2/core/accounts",
                post({
                    let accounts = fake.accounts.clone();
                    move |headers: HeaderMap, Json(body): Json<Value>| async move {
                        assert!(headers.contains_key("stripe-version"));
                        accounts.lock().unwrap().push(body);
                        Json(json!({"id": "acct_1", "object": "v2.core.account"}))
                    }
                }),
            )
            .route(
                "/v2/core/account_links",
                post({
                    let link = fake.link.clone();
                    move |Json(body): Json<Value>| async move {
                        *link.lock().unwrap() = Some(body);
                        Json(json!({"url": "https://connect.stripe.com/setup/x"}))
                    }
                }),
            )
            .route(
                "/v2/core/accounts/{account}",
                get({
                    let active = fake.active.clone();
                    move |Path(account): Path<String>| async move {
                        assert_eq!(account, "acct_1");
                        let status = if *active.lock().unwrap() {
                            "active"
                        } else {
                            "pending"
                        };
                        Json(json!({"id": account, "configuration": {"merchant": {
                            "capabilities": {"card_payments": {"status": status}},
                        }}}))
                    }
                }),
            )
            .route(
                "/v1/payment_intents",
                post({
                    let payment = fake.payment.clone();
                    move |headers: HeaderMap,
                          axum::Form(form): axum::Form<
                        std::collections::HashMap<String, String>,
                    >| async move {
                        *payment.lock().unwrap() = Some(json!({
                            "account": headers["stripe-account"].to_str().unwrap(),
                            "form": form,
                        }));
                        Json(json!({
                            "id": "pi_1",
                            "client_secret": "pi_1_secret_2",
                            "status": "requires_payment_method",
                        }))
                    }
                }),
            )
            .route(
                "/v1/payment_intents/{id}/capture",
                post({
                    let payment = fake.payment.clone();
                    let state = fake.state.clone();
                    move |headers: HeaderMap, Path(id): Path<String>| async move {
                        let payment = payment.lock().unwrap().clone().unwrap();
                        assert_eq!(headers["stripe-account"], "acct_1");
                        let mut state = state.lock().unwrap();
                        if id != "pi_1" || *state != "requires_capture" {
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(json!({"error": {
                                    "code": "payment_intent_unexpected_state",
                                    "payment_intent": {"id": id, "status": *state},
                                }})),
                            );
                        }
                        *state = "succeeded";
                        (
                            StatusCode::OK,
                            Json(json!({
                                "id": id,
                                "status": "succeeded",
                                "amount_received": payment["form"]["amount"]
                                    .as_str()
                                    .unwrap()
                                    .parse::<u32>()
                                    .unwrap(),
                                "currency": payment["form"]["currency"],
                            })),
                        )
                    }
                }),
            )
            .route(
                "/v1/payment_intents/{id}/cancel",
                post({
                    let cancelled = fake.cancelled.clone();
                    move |Path(id): Path<String>| async move {
                        cancelled.lock().unwrap().push(id);
                        Json(json!({"status": "canceled"}))
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await });
        (format!("http://{address}"), fake)
    }

    async fn start_stripe_api() -> (String, Arc<Api>, FakeStripe) {
        let (stripe_url, fake) = start_stripe().await;
        let (address, api) = start_api_with(
            None,
            Some(new_stripe(stripe_url)),
            None,
            Pages::new(Database::memory()),
        )
        .await;
        (address, api, fake)
    }

    fn connect_stripe(api: &Api) {
        api.accounts
            .start_stripe("1", "acct_1".to_string())
            .unwrap();
        api.accounts.stripe_ready("1", "acct_1", true).unwrap();
    }

    #[tokio::test]
    async fn stripe_connects_and_takes_tips() {
        let (address, api, fake) = start_stripe_api().await;
        let client = reqwest::Client::new();
        let (session, _, id) = streamer(&address, &api).await;
        let post = |path: &str| {
            client
                .post(format!("http://{address}{path}"))
                .bearer_auth(&session)
        };
        let settings: Value = post("/account/stripe/check")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["stripe"], true);
        assert_eq!(settings["stripeAccountId"], Value::Null);
        assert_eq!(settings["stripePending"], false);
        assert_eq!(settings["stripeTest"], true);
        assert_eq!(settings["paypal"], false);
        for request in [json!({}), json!({"country": "XX"})] {
            let response = post("/account/stripe/connect")
                .json(&request)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
        let response = post("/account/stripe/connect").send().await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        for request in [Some(json!({"country": " se "})), None] {
            let mut connect = post("/account/stripe/connect");
            if let Some(request) = request {
                connect = connect.json(&request);
            }
            let signed_up: Value = connect.send().await.unwrap().json().await.unwrap();
            assert_eq!(
                signed_up,
                json!({"url": "https://connect.stripe.com/setup/x"})
            );
        }
        let accounts = fake.accounts.lock().unwrap().clone();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0]["display_name"], "Erik");
        assert_eq!(accounts[0]["dashboard"], "full");
        assert_eq!(accounts[0]["identity"], json!({"country": "se"}));
        assert_eq!(accounts[0]["metadata"], json!({"twitch_user_id": "1"}));
        let link = fake.link.lock().unwrap().clone().unwrap();
        assert_eq!(link["account"], "acct_1");
        assert_eq!(
            link["use_case"]["account_onboarding"]["return_url"],
            "https://moblin.app/dashboard/?stripe"
        );
        let response = post("/account/stripe/check").send().await.unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let settings_request = json!({"currency": "eur", "tips": {"amounts": [500]}});
        let response = post("/account/settings")
            .json(&settings_request)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        *fake.active.lock().unwrap() = true;
        let settings: Value = post("/account/stripe/check")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["stripeAccountId"], "acct_1");
        assert_eq!(settings["stripePending"], false);
        let response = post("/account/settings")
            .json(&settings_request)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let tips_url = format!("http://{address}/pages/{id}/tips");
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
                "amounts": [500],
                "currency": "EUR",
                "clientId": null,
                "merchantId": null,
                "environment": null,
                "stripe": {"publishableKey": "pk_test_public", "accountId": "acct_1"},
            }})
        );
        let orders_url = format!("{tips_url}/orders");
        let response = client
            .post(&orders_url)
            .json(&json!({"amount": 700}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let order: Value = client
            .post(&orders_url)
            .json(&json!({"amount": 700, "user": "Viewer", "message": "Great stream!", "provider": "stripe"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            order,
            json!({
                "orderId": "pi_1",
                "clientSecret": "pi_1_secret_2",
                "price": 700,
                "currency": "EUR",
            })
        );
        let payment = fake.payment.lock().unwrap().clone().unwrap();
        assert_eq!(payment["account"], "acct_1");
        assert_eq!(payment["form"]["amount"], "700");
        assert_eq!(payment["form"]["currency"], "eur");
        assert_eq!(payment["form"]["capture_method"], "manual");
        assert_eq!(
            payment["form"]["description"],
            "Tip from Viewer: Great stream!"
        );
        let capture_url = format!("{orders_url}/pi_1/capture");
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
        *fake.state.lock().unwrap() = "requires_capture";
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::GONE);
        assert_eq!(api.deliveries.pending().len(), 1);
        let settings: Value = post("/account/stripe/disconnect")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["stripeAccountId"], Value::Null);
        let public: Value = client
            .get(&tips_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(public, json!({"tips": null}));
    }

    #[tokio::test]
    async fn stripe_holds_are_released_when_the_streamer_goes_offline() {
        let (address, api, fake) = start_stripe_api().await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        connect_stripe(&api);
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"currency": "SEK", "paywall": {"price": 250}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut socket = connect(&address, &key).await;
        let orders_url = format!("http://{address}/pages/{id}/songs/orders");
        let created = tokio::spawn({
            let orders_url = orders_url.clone();
            async move {
                reqwest::Client::new()
                    .post(orders_url)
                    .json(&json!({"song": LINK, "provider": "stripe"}))
                    .send()
                    .await
                    .unwrap()
                    .json::<Value>()
                    .await
                    .unwrap()
            }
        });
        let request = receive(&mut socket).await;
        answer(
            &mut socket,
            &request,
            json!({"musicFind": {"song": {"id": "1440818839", "title": "Song", "duration": 400}}}),
        )
        .await;
        assert_eq!(created.await.unwrap()["orderId"], "pi_1");
        let payment = fake.payment.lock().unwrap().clone().unwrap();
        assert_eq!(payment["form"]["amount"], "500");
        assert_eq!(payment["form"]["currency"], "sek");
        socket.close(None).await.unwrap();
        for _ in 0..100 {
            if !api.remote_control.is_connected(&id) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        *fake.state.lock().unwrap() = "requires_capture";
        let capture_url = format!("{orders_url}/pi_1/capture");
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(*fake.cancelled.lock().unwrap(), ["pi_1"]);
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::GONE);
        assert!(api.deliveries.pending().is_empty());
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
        let pages = Pages::new(Database::memory());
        pages
            .set(
                "p",
                Settings {
                    currency: Some("SEK".to_string()),
                    paywall: Some(Paywall { price: 250 }),
                    ..Settings::default()
                },
            )
            .unwrap();
        let (paypal_url, _) = start_paypal().await;
        let (address, _) = start_api_with(Some(new_paypal(paypal_url)), None, None, pages).await;
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
                "stripe": null,
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
        account["devices"][0]["key"].as_str().unwrap().to_string()
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
    async fn long_songs_cost_more() {
        let (address, api, paypal_order) = start_paid().await;
        let (key, id) = paid_streamer(&address, &api).await;
        let mut socket = connect(&address, &key).await;
        let orders_url = format!("http://{address}/pages/{id}/songs/orders");
        for (duration, value) in [
            (None, "2.50"),
            (Some(300), "2.50"),
            (Some(301), "5.00"),
            (Some(601), "7.50"),
        ] {
            let created = tokio::spawn({
                let orders_url = orders_url.clone();
                async move {
                    reqwest::Client::new()
                        .post(orders_url)
                        .json(&json!({"song": LINK}))
                        .send()
                        .await
                        .unwrap()
                        .status()
                }
            });
            let request = receive(&mut socket).await;
            answer(
                &mut socket,
                &request,
                json!({"musicFind": {"song": {"id": "1440818839", "title": "Song", "duration": duration}}}),
            )
            .await;
            assert_eq!(created.await.unwrap(), StatusCode::OK);
            let sent = paypal_order.lock().unwrap().clone().unwrap();
            assert_eq!(
                sent["purchase_units"][0]["amount"],
                json!({"currency_code": "SEK", "value": value})
            );
        }
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
                "stripe": null,
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
        assert_eq!(
            public,
            json!({"spin": null, "wheel": null, "connected": false})
        );
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
                "stripe": null,
            }, "wheel": null, "connected": false})
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
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        let public: Value = client
            .get(&spin_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(public["wheel"], Value::Null);
        assert_eq!(public["connected"], true);
        let response = client
            .post(&orders_url)
            .json(&json!({"user": "Viewer", "spins": 2}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.text().await.unwrap(),
            "The streamer is not taking wheel spins right now."
        );
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
                "stripe": null,
            }})
        );
        for request in [
            json!({"amount": 199}),
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
    async fn streamers_send_themselves_free_tests() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, _) = streamer(&address, &api).await;
        let test = |kind: &str, body: Value| {
            client
                .post(format!("http://{address}/account/test/{kind}"))
                .bearer_auth(&session)
                .json(&body)
                .send()
        };
        let response = client
            .post(format!("http://{address}/account/test/tip"))
            .json(&json!({}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        for kind in ["song", "tip", "spin"] {
            let response = test(kind, json!({"song": LINK})).await.unwrap();
            assert_eq!(response.status(), StatusCode::CONFLICT);
            assert_eq!(response.text().await.unwrap(), "Connect Moblin first.");
        }
        let mut socket = connect(&address, &key).await;
        let response = test("tip", json!({"message": " Testing "})).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let request = receive(&mut socket).await;
        let tip = &request["request"]["data"]["tip"];
        assert_eq!(tip["name"], "Erik");
        assert_eq!(tip["message"], "Testing");
        assert_eq!(tip["amount"], "5.00");
        assert_eq!(tip["currency"], "USD");
        answer(&mut socket, &request, Value::Null).await;
        wait_until_added(&api).await;
        let response = test("spin", json!({})).await.unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        socket
            .send(Message::text(
                json!({"event": {"data": {"wheelOfLuck": {"wheel": {"options": [{"text": "A", "weight": 1}]}}}}})
                    .to_string(),
            ))
            .await
            .unwrap();
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        let response = test("spin", json!({})).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let request = receive(&mut socket).await;
        let spin = &request["request"]["data"]["spinWheelOfLuck"];
        assert_eq!(spin["name"], "Erik");
        assert_eq!(spin["count"], 1);
        answer(&mut socket, &request, Value::Null).await;
        wait_until_added(&api).await;
        let response = test("song", json!({"song": "nope"})).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let created = tokio::spawn(test("song", json!({"song": LINK})));
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"],
            json!({"music": {"command": {"find": {"title": LINK}}}})
        );
        answer(
            &mut socket,
            &request,
            json!({"musicFind": {"song": {"id": "1440818839", "title": "Song", "artist": "Band"}}}),
        )
        .await;
        assert_eq!(
            created.await.unwrap().unwrap().status(),
            StatusCode::ACCEPTED
        );
        let request = receive(&mut socket).await;
        let add = &request["request"]["data"]["music"]["command"]["add"];
        assert_eq!(add["title"], LINK);
        assert_eq!(add["user"], "Erik");
    }

    #[tokio::test]
    async fn devices_take_music_and_wheel_as_chosen_and_all_get_tips() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        let devices_url = format!("http://{address}/account/devices");
        let response = client
            .post(&devices_url)
            .bearer_auth(&session)
            .json(&json!({"name": " "}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let account: Value = client
            .post(&devices_url)
            .bearer_auth(&session)
            .json(&json!({"name": " Backup "}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let backup = account["devices"][1].clone();
        assert_eq!(backup["name"], "Backup");
        assert_eq!(account["musicDevice"], id);
        let backup_id = backup["id"].as_str().unwrap().to_string();
        let account: Value = client
            .post(format!("{devices_url}/choose"))
            .bearer_auth(&session)
            .json(&json!({"music": backup_id, "wheel": id}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(account["musicDevice"], backup_id);
        assert_eq!(account["wheelDevice"], id);
        let mut main = connect(&address, &key).await;
        let mut other = connect(&address, backup["key"].as_str().unwrap()).await;
        main.send(Message::text(
            json!({"event": {"data": {"wheelOfLuck": {"wheel": {"options": [{"text": "A", "weight": 1}]}}}}})
                .to_string(),
        ))
        .await
        .unwrap();
        other
            .send(Message::text(
                json!({"event": {"data": {"songRequests": {"enabled": false}}}}).to_string(),
            ))
            .await
            .unwrap();
        for socket in [&mut main, &mut other] {
            socket
                .send(Message::text(json!({"ping": {}}).to_string()))
                .await
                .unwrap();
            assert_eq!(receive(socket).await, json!({"pong": {}}));
        }
        let spin: Value = client
            .get(format!("http://{address}/pages/{id}/spin"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(spin["wheel"]["options"][0]["text"], "A");
        let queue = tokio::spawn({
            let url = format!("http://{address}/pages/{id}/songs/queue");
            async move {
                reqwest::get(url)
                    .await
                    .unwrap()
                    .json::<Value>()
                    .await
                    .unwrap()
            }
        });
        let request = receive(&mut other).await;
        answer(&mut other, &request, music(json!([]))).await;
        assert_eq!(queue.await.unwrap()["songRequests"], false);
        api.deliveries.add(
            id.clone(),
            "TIP1".to_string(),
            deliveries::Item::Tip {
                user: "Viewer".to_string(),
                message: String::new(),
                amount: 500,
                currency: "USD".to_string(),
            },
        );
        for socket in [&mut main, &mut other] {
            let request = receive(socket).await;
            assert_eq!(request["request"]["data"]["tip"]["requestId"], "TIP1");
            answer(socket, &request, Value::Null).await;
        }
        wait_until_added(&api).await;
        let account: Value = client
            .post(format!("{devices_url}/{backup_id}/remove"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(account["devices"].as_array().unwrap().len(), 1);
        assert_eq!(account["musicDevice"], id);
        assert!(!api.remote_control.is_connected(&backup_id));
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
        let status = status.await.unwrap();
        assert_eq!(status["songRequests"], true);
        assert_eq!(
            status["songs"],
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
    async fn song_requests_can_be_turned_off_by_the_streamer() {
        let (address, api, _) = start_paid().await;
        let (key, id) = paid_streamer(&address, &api).await;
        let client = reqwest::Client::new();
        let orders_url = format!("http://{address}/pages/{id}/songs/orders");
        let mut socket = connect(&address, &key).await;
        assert_eq!(
            request_song(&mut socket, &orders_url).await,
            json!({"orderId": "ORDER1"})
        );
        socket
            .send(Message::text(
                json!({"event": {"data": {"songRequests": {"enabled": false}}}}).to_string(),
            ))
            .await
            .unwrap();
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        let response = client
            .post(&orders_url)
            .json(&json!({"song": LINK}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.text().await.unwrap(),
            "The streamer is not taking song requests right now."
        );
        let response = client
            .post(format!("{orders_url}/ORDER1/capture"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.text().await.unwrap(),
            "The streamer stopped taking song requests, so your payment was not taken."
        );
        let status = tokio::spawn({
            let music_url = format!("http://{address}/pages/{id}/songs/queue");
            async move {
                reqwest::get(music_url)
                    .await
                    .unwrap()
                    .json::<Value>()
                    .await
                    .unwrap()
            }
        });
        let request = receive(&mut socket).await;
        answer(
            &mut socket,
            &request,
            music(json!([{"id": "q1", "title": "Song"}])),
        )
        .await;
        let status = status.await.unwrap();
        assert_eq!(status["songRequests"], false);
        assert_eq!(status["songs"][0]["title"], "Song");
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
            None,
            Some(OAuthApp {
                client_id: "id".to_string(),
                client_secret: "secret".to_string(),
                authorize_url: format!("{twitch}/authorize"),
                token_url: format!("{twitch}/token"),
                users_url: format!("{twitch}/users"),
            }),
            Pages::new(Database::memory()),
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
            json!({
                "name": "Erik",
                "login": "erik",
                "devices": [],
                "musicDevice": null,
                "wheelDevice": null,
                "settings": null,
            })
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
    async fn new_moblin_cloud_keys_keep_the_page() {
        let (address, api) = start_api(None).await;
        let (session, key, id) = streamer(&address, &api).await;
        let mut socket = connect(&address, &key).await;
        assert!(api.remote_control.is_connected(&id));
        let account: Value = reqwest::Client::new()
            .post(format!("http://{address}/account/devices/{id}/key"))
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
            ("POST", "/account/devices"),
            ("POST", "/account/devices/choose"),
            ("POST", "/account/devices/d/key"),
            ("POST", "/account/devices/d/remove"),
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
                    .json(&json!({"paywall": null, "name": "Moblin", "music": "d", "wheel": "d"}));
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
