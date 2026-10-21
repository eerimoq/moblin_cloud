use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State, WebSocketUpgrade};
use axum::http::header::{
    AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, COOKIE, SET_COOKIE, X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower_http::cors::{Any, CorsLayer};

use crate::accounts::{
    Account, AccountError, AccountStats, Accounts, ChannelModerator, Device, Page,
};
use crate::app_attest::AppAttest;
use crate::auth::{Auth, STATE_COOKIE, SignIn, StateError, Viewer};
use crate::challenges::Challenges;
use crate::counters::{self, Counter, Counters};
use crate::database::Database;
use crate::deliveries::{self, Deliveries};
use crate::images;
use crate::live::Live;
use crate::pages::{
    CURRENCIES, FONTS, Pages, Paywall, Settings, SettingsError, Spin, SpinOption, Tiers, Tips,
    format_price, is_color,
};
use crate::payments::{
    CaptureError, Item, Order, Orders, Payee, PaymentStatus, Stage, insert_captured,
};
use crate::paypal::{PayPal, Seller};
use crate::remote_control::{self, MusicCommand, MusicStatus, RequestError, Streamers, Wheel};
use crate::store::{Channel, Store, Streamer};
use crate::stripe::{self, Stripe};
use crate::subscribers::Allowances;

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
    pub allowances: Allowances,
    pub deliveries: Deliveries,
    pub auth: Auth,
    pub accounts: Accounts,
    pub counters: Counters,
    pub database: Arc<Database>,
    pub admins: Vec<String>,
    pub premium_accounts: Vec<String>,
    pub tests: Mutex<HashMap<String, Vec<Instant>>>,
    pub song_finds: Mutex<HashMap<String, Vec<Instant>>>,
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
        .route(
            "/account/devices/{device}/enabled",
            post(handle_account_device_enabled),
        )
        .route("/account/settings", post(handle_account_settings))
        .route(
            "/account/image",
            post(handle_account_image).layer(DefaultBodyLimit::max(2 * 1024 * 1024)),
        )
        .route("/account/image/remove", post(handle_account_image_remove))
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
        .route("/account/delete", post(handle_account_delete))
        .route("/account/recent", get(handle_account_recent))
        .route(
            "/account/recent/{order}/resend",
            post(handle_account_resend),
        )
        .route("/account/test/song", post(handle_account_test_song))
        .route("/account/test/tip", post(handle_account_test_tip))
        .route("/account/test/spin", post(handle_account_test_spin))
        .route(
            "/account/moderators/{moderator}",
            post(handle_account_moderator),
        )
        .route("/account/pages/{id}/music", post(handle_moderator_music))
        .route("/account/pages/{id}/songs", post(handle_moderator_song))
        .route("/account/pages/{id}/recent", get(handle_moderator_recent))
        .route(
            "/account/pages/{id}/recent/{order}/resend",
            post(handle_moderator_resend),
        )
        .route("/account/pages/{id}", get(handle_moderator_account))
        .route(
            "/account/pages/{id}/settings",
            post(handle_moderator_settings),
        )
        .route(
            "/account/pages/{id}/image",
            post(handle_moderator_image).layer(DefaultBodyLimit::max(2 * 1024 * 1024)),
        )
        .route(
            "/account/pages/{id}/image/remove",
            post(handle_moderator_image_remove),
        )
        .route(
            "/account/pages/{id}/test/tip",
            post(handle_moderator_test_tip),
        )
        .route(
            "/account/pages/{id}/test/spin",
            post(handle_moderator_test_spin),
        )
        .route("/admin/stats", get(handle_admin_stats))
        .route("/pages/{id}/songs", get(handle_songs))
        .route("/pages/{id}", get(handle_page))
        .route("/pages/{id}/image", get(handle_page_image))
        .route("/pages/{id}/songs/queue", get(handle_queue))
        .route(
            "/pages/{id}/songs/subscriber",
            get(handle_subscriber_song_allowance).post(handle_subscriber_song),
        )
        .route(
            "/pages/{id}/spin/subscriber",
            get(handle_subscriber_spin_allowance).post(handle_subscriber_spin),
        )
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
    let Some(id) = api.accounts.device_of_key(&key) else {
        api.counters.bump("moblinConnectionsFailed:unknown-key");
        return Err(StatusCode::NOT_FOUND);
    };
    api.counters.bump("moblinConnections");
    Ok(upgrade.on_upgrade(move |socket| async move { api.remote_control.serve(id, socket).await }))
}

fn page_of(api: &Api, id: &str) -> Option<Page> {
    Some(api.accounts.of_page(id)?.page)
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
    #[serde(rename = "subSongs")]
    sub_songs: Option<Tiers>,
    #[serde(rename = "subSpins")]
    sub_spins: Option<Tiers>,
    #[serde(rename = "maxSongMinutes")]
    max_song_minutes: Option<u32>,
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

fn any_connected(api: &Api, account: &Account) -> bool {
    account
        .page
        .enabled_devices()
        .any(|device| api.remote_control.is_connected(&device.id))
}

fn settings_response(api: &Api, id: String, settings: Settings) -> Json<SettingsResponse> {
    let account = api.accounts.of_page(&id).unwrap_or_default();
    Json(SettingsResponse {
        connected: any_connected(api, &account),
        currency: settings.currency().to_string(),
        paywall: settings.paywall,
        tips: settings.tips,
        spin: settings.spin,
        sub_songs: settings.sub_songs,
        sub_spins: settings.sub_spins,
        max_song_minutes: settings.max_song_minutes,
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
        payable: merchant_of(api, &id).is_some() || stripe_of(api, &id).is_some(),
        message: settings.message,
        accent: settings.accent,
        font: settings.font,
        image: api.pages.image_version(&id),
        id,
    })
}

fn without_payments(mut response: Json<SettingsResponse>) -> Json<SettingsResponse> {
    response.paypal = false;
    response.merchant_id = None;
    response.paypal_pending = false;
    response.stripe = false;
    response.stripe_account_id = None;
    response.stripe_pending = false;
    response
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
    #[serde(default, rename = "subSongs")]
    sub_songs: Option<Tiers>,
    #[serde(default, rename = "subSpins")]
    sub_spins: Option<Tiers>,
    #[serde(default, rename = "maxSongMinutes")]
    max_song_minutes: Option<u32>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    accent: Option<String>,
    #[serde(default)]
    font: Option<String>,
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
                sub_songs: request.sub_songs,
                sub_spins: request.sub_spins,
                max_song_minutes: request.max_song_minutes,
                message: request.message,
                accent,
                font,
            },
        )
        .map_err(|error| match error {
            SettingsError::InvalidCurrency => invalid_currency(),
            SettingsError::InvalidPaywall => invalid_paywall(),
            SettingsError::InvalidTips => invalid_tips(),
            SettingsError::InvalidSpin => invalid_spin(),
            SettingsError::InvalidTiers => (
                StatusCode::BAD_REQUEST,
                "subscribers' songs and spins per month must be 0 to 100 per tier, with at least one above 0"
                    .to_string(),
            ),
            SettingsError::InvalidSongLength => (
                StatusCode::BAD_REQUEST,
                "the longest song must be 1 to 60 minutes".to_string(),
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
        "remote control streamer {id} set currency {}, paywall {:?}, tips {:?}, spin {:?}, sub songs {:?}, sub spins {:?}, max song minutes {:?}, message {:?}, accent {:?} and font {:?}",
        settings.currency(),
        settings.paywall,
        settings.tips,
        settings.spin,
        settings.sub_songs,
        settings.sub_spins,
        settings.max_song_minutes,
        settings.message,
        settings.accent,
        settings.font
    );
    Ok(settings_response(api, id, settings))
}

#[derive(Deserialize)]
struct LoginQuery {
    page: Option<String>,
}

async fn handle_login(
    State(api): State<Arc<Api>>,
    Query(query): Query<LoginQuery>,
) -> Result<([(HeaderName, String); 1], Redirect), StatusCode> {
    if let Some(page) = &query.page
        && api.accounts.streamer_of_page(page).is_none()
    {
        return Err(StatusCode::NOT_FOUND);
    }
    let (url, state) = api
        .auth
        .login_url(query.page.as_deref())
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
    error: Option<String>,
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
    let Some(state) = query.state else {
        return Redirect::to(&format!("{dashboard}#signInFailed"));
    };
    let page = match api.auth.consume_state(&state, browser_state) {
        Ok(page) => page,
        Err(StateError {
            page: Some(page),
            error,
        }) => return viewer_redirect(api, &page, Err(error)).await,
        Err(StateError { error, .. }) => {
            warn!("signing in with Twitch failed: {error:#}");
            return Redirect::to(&format!("{dashboard}#signInFailed"));
        }
    };
    let sign_in = match query.code {
        Some(code) => api.auth.callback(&code).await,
        None => Err(anyhow::anyhow!(
            "Twitch answered {}",
            query.error.as_deref().unwrap_or("no code")
        )),
    };
    if let Some(page) = page {
        return viewer_redirect(api, &page, sign_in).await;
    }
    let sign_in = match sign_in {
        Ok(sign_in) => sign_in,
        Err(error) => {
            warn!("signing in with Twitch failed: {error:#}");
            return Redirect::to(&format!("{dashboard}#signInFailed"));
        }
    };
    let user = &sign_in.user;
    let Some((key, page)) = api.remote_control.new_key() else {
        warn!("no page could be created for {}", user.id);
        return Redirect::to(&format!("{dashboard}#signInFailed"));
    };
    let device = Device {
        id: page.clone(),
        name: "Moblin".to_string(),
        key,
        enabled: true,
    };
    let account = match api.accounts.signed_in(
        &user.id,
        &user.name,
        &user.login,
        user.avatar.as_deref(),
        Page::new(page, device),
    ) {
        Ok(account) => account,
        Err(error) => {
            warn!("saving account {} failed: {error:?}", user.id);
            return Redirect::to(&format!("{dashboard}#signInFailed"));
        }
    };
    match api.auth.channel_moderators(&sign_in).await {
        Ok(moderators) => api.accounts.set_channel_moderators(&user.id, &moderators),
        Err(error) => warn!("looking up the moderators of {} failed: {error:#}", user.id),
    }
    info!("Twitch user {} ({}) signed in", user.id, user.login);
    Redirect::to(&format!(
        "{dashboard}#session={}",
        api.auth.session(&user.id, account.sessions)
    ))
}

async fn viewer_redirect(api: &Api, page: &str, sign_in: Result<SignIn>) -> Redirect {
    let streamer = api.accounts.streamer_of_page(page);
    let url = match &streamer {
        Some((_, login)) => format!("{}/twitch/{login}", api.auth.frontend_url),
        None => format!("{}/page/?streamer={page}", api.auth.frontend_url),
    };
    let tier = async {
        let sign_in = sign_in?;
        let (broadcaster, _) = streamer.context("the page has no streamer")?;
        let tier = api.auth.subscription_tier(&sign_in, &broadcaster).await?;
        anyhow::Ok((sign_in.user, tier))
    };
    match tier.await {
        Ok((user, tier)) => {
            info!(
                "Twitch user {} ({}) signed in as a viewer of page {page} with subscription tier {tier:?}",
                user.id, user.login
            );
            Redirect::to(&format!(
                "{url}#viewer={}",
                api.auth
                    .viewer_session(user.id, user.name, page.to_string(), tier)
            ))
        }
        Err(error) => {
            warn!("signing in as a viewer of page {page} failed: {error:#}");
            Redirect::to(&format!("{url}#viewerSignInFailed"))
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountResponse {
    name: String,
    login: String,
    avatar: Option<String>,
    devices: Vec<DeviceResponse>,
    music_device: Option<String>,
    wheel_device: Option<String>,
    settings: SettingsResponse,
    admin: bool,
    premium: bool,
    moderating: Vec<ModeratingResponse>,
    moderators: Vec<ChannelModerator>,
}

#[derive(Serialize)]
struct ModeratingResponse {
    id: String,
    name: String,
    login: String,
    avatar: Option<String>,
}

#[derive(Serialize)]
struct DeviceResponse {
    id: String,
    name: String,
    key: String,
    enabled: bool,
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

fn is_admin(api: &Api, id: &str) -> bool {
    api.admins.iter().any(|admin| admin == id)
}

fn is_premium(api: &Api, id: &str) -> bool {
    is_admin(api, id) || api.premium_accounts.iter().any(|premium| premium == id)
}

fn admin(api: &Api, headers: &HeaderMap) -> Result<SignedIn, (StatusCode, String)> {
    let signed_in = signed_in(api, headers)?;
    if !is_admin(api, &signed_in.id) {
        return Err((StatusCode::FORBIDDEN, "not an admin".to_string()));
    }
    Ok(signed_in)
}

fn account_response(api: &Api, id: &str, account: Account) -> Json<AccountResponse> {
    let page = &account.page;
    let settings = api.pages.get(&page.id);
    Json(AccountResponse {
        name: account.name,
        login: account.login,
        avatar: account.avatar,
        devices: page
            .devices
            .iter()
            .map(|device| DeviceResponse {
                id: device.id.clone(),
                name: device.name.clone(),
                key: device.key.clone(),
                enabled: device.enabled,
                connected: api.remote_control.is_connected(&device.id),
            })
            .collect(),
        music_device: page.music_device().map(|device| device.id.clone()),
        wheel_device: page.wheel_device().map(|device| device.id.clone()),
        settings: settings_response(api, page.id.clone(), settings).0,
        admin: is_admin(api, id),
        premium: is_premium(api, id),
        moderators: api.accounts.channel_moderators(id),
        moderating: api
            .accounts
            .moderated(id)
            .into_iter()
            .map(|streamer| ModeratingResponse {
                id: streamer.page.id,
                name: streamer.name,
                login: streamer.login,
                avatar: streamer.avatar,
            })
            .collect(),
    })
}

#[derive(Deserialize)]
struct ModeratorRequest {
    enabled: bool,
}

async fn handle_account_moderator(
    State(api): State<Arc<Api>>,
    Path(moderator): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ModeratorRequest>,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let SignedIn { id, account, .. } = signed_in(&api, &headers)?;
    api.accounts
        .enable_moderator(&id, &moderator, request.enabled)
        .map_err(account_error)?;
    info!(
        "{id} turned moderator {moderator} {}",
        if request.enabled { "on" } else { "off" }
    );
    Ok(account_response(&api, &id, account))
}

fn account_error(error: AccountError) -> (StatusCode, String) {
    match error {
        AccountError::Unknown => not_signed_in(),
        AccountError::Full => (
            StatusCode::SERVICE_UNAVAILABLE,
            "too many accounts".to_string(),
        ),
        AccountError::TooManyDevices => {
            (StatusCode::CONFLICT, "at most 5 Moblin devices".to_string())
        }
        AccountError::UnknownDevice => (StatusCode::NOT_FOUND, "no such device".to_string()),
        AccountError::DeviceOff => (StatusCode::CONFLICT, "the device is off".to_string()),
        AccountError::UnknownModerator => (StatusCode::NOT_FOUND, "no such moderator".to_string()),
        AccountError::NotSaved => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "the account could not be saved".to_string(),
        ),
    }
}

fn new_key(api: &Api) -> Result<(String, String), (StatusCode, String)> {
    api.remote_control.new_key().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "no streamer could be created".to_string(),
    ))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminStatsResponse {
    accounts: AccountStats,
    moblin: MoblinStats,
    pending: PendingStats,
    database: DatabaseStats,
    counters: Vec<Counter>,
}

#[derive(Serialize)]
struct DatabaseStats {
    bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MoblinStats {
    connected_devices: usize,
}

#[derive(Serialize)]
struct PendingStats {
    orders: usize,
    deliveries: usize,
}

async fn handle_admin_stats(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<AdminStatsResponse>, (StatusCode, String)> {
    admin(&api, &headers)?;
    Ok(Json(AdminStatsResponse {
        accounts: api.accounts.stats(
            &api.admins
                .iter()
                .chain(&api.premium_accounts)
                .map(String::as_str)
                .collect::<Vec<_>>(),
        ),
        moblin: MoblinStats {
            connected_devices: api.remote_control.connected_count(),
        },
        pending: PendingStats {
            orders: api.orders.unfinished(),
            deliveries: api.deliveries.count(),
        },
        database: DatabaseStats {
            bytes: api.database.size(),
        },
        counters: api.counters.all(),
    }))
}

async fn handle_account(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let SignedIn { id, account, .. } = signed_in(&api, &headers)?;
    Ok(account_response(&api, &id, account))
}

#[derive(Deserialize)]
struct RecentQuery {
    kind: String,
    before: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize)]
struct RecentResponse {
    items: Vec<Recent>,
    more: bool,
}

#[derive(Serialize)]
struct Recent {
    id: String,
    at: u64,
    user: String,
    amount: u32,
    currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    spins: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tier: Option<u8>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    moderator: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<&'static str>,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    devices: Option<Vec<RecentDevice>>,
}

#[derive(Serialize)]
struct RecentDevice {
    name: String,
    received: bool,
}

async fn handle_account_recent(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Query(query): Query<RecentQuery>,
) -> Result<Json<RecentResponse>, (StatusCode, String)> {
    let SignedIn { account, .. } = signed_in(&api, &headers)?;
    recent(&api, account.page, query, true)
}

fn recent(
    api: &Api,
    page: Page,
    query: RecentQuery,
    owner: bool,
) -> Result<Json<RecentResponse>, (StatusCode, String)> {
    if !["song", "tip", "spin"].contains(&query.kind.as_str()) {
        return Err((StatusCode::BAD_REQUEST, "Unknown kind.".to_string()));
    }
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let mut orders = api
        .orders
        .recent(&page.id, &query.kind, query.before.as_deref(), limit + 1);
    let more = orders.len() > limit;
    orders.truncate(limit);
    let items = orders
        .into_iter()
        .map(|(id, at, order)| {
            let delivery = api.deliveries.get(&id);
            let devices = delivery
                .as_ref()
                .map(|delivery| recent_devices(&page, &order.item, delivery));
            let tier = match order.payee {
                Payee::Twitch(tier) => Some(tier),
                _ => None,
            };
            let moderator = order.payee == Payee::Moderator;
            let provider = match order.payee {
                Payee::PayPal(_) | Payee::Stripe(_) if owner => Some(order.payee.slug()),
                _ => None,
            };
            let (name, link, message, spins) = match order.item {
                Item::Song {
                    link,
                    name,
                    message,
                } => (Some(name), Some(link), message, None),
                Item::Tip { message } => (None, None, message, None),
                Item::Spin { spins, message } => (None, None, message, Some(spins)),
            };
            Recent {
                id,
                at,
                user: order.user,
                amount: order.price,
                currency: order.currency,
                name,
                link,
                message,
                spins,
                tier,
                moderator,
                provider,
                status: match &delivery {
                    None => "delivered",
                    Some(delivery) if delivery.finished => "delivered",
                    Some(delivery) if delivery.failure.as_deref() == Some("not-found") => {
                        "notAdded"
                    }
                    Some(_) => "waiting",
                },
                devices,
            }
        })
        .collect();
    Ok(Json(RecentResponse { items, more }))
}

fn recent_devices(page: &Page, item: &Item, delivery: &deliveries::Delivery) -> Vec<RecentDevice> {
    let target = match item {
        Item::Song { .. } => page.music_device(),
        Item::Spin { .. } => page.wheel_device(),
        Item::Tip { .. } => None,
    }
    .map(|device| device.id.clone());
    page.devices
        .iter()
        .filter_map(|device| {
            let received = delivery.delivered.contains(&device.id);
            let waiting = match item {
                Item::Tip { .. } => device.enabled,
                _ => target.as_ref() == Some(&device.id),
            };
            (received || (waiting && !delivery.finished)).then(|| RecentDevice {
                name: device.name.clone(),
                received,
            })
        })
        .collect()
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
                enabled: true,
            },
        )
        .map_err(account_error)?;
    info!("{id} added Moblin device {device}");
    Ok(account_response(&api, &id, account))
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
    Ok(account_response(&api, &id, account))
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
    Ok(account_response(&api, &id, account))
}

#[derive(Deserialize)]
struct EnabledRequest {
    enabled: bool,
}

async fn handle_account_device_enabled(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(device): Path<String>,
    Json(request): Json<EnabledRequest>,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let id = signed_in(&api, &headers)?.id;
    let account = api
        .accounts
        .enable_device(&id, &device, request.enabled)
        .map_err(account_error)?;
    api.deliveries.notify();
    info!(
        "{id} turned Moblin device {device} {}",
        if request.enabled { "on" } else { "off" }
    );
    Ok(account_response(&api, &id, account))
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
    api.deliveries.notify();
    api.remote_control.disconnect(&device);
    info!("{id} removed Moblin device {device}");
    Ok(account_response(&api, &id, account))
}

async fn handle_account_image(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SettingsResponse>, Failure> {
    let result = account_image(&api, &headers, body).await;
    counted(&api, "imageUploads", result)
}

async fn account_image(
    api: &Api,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<Json<SettingsResponse>, Failure> {
    let SignedIn { id, account, .. } = signed_in(api, headers)?;
    let premium = is_premium(api, &id);
    save_image(api, &id, account.page.id, premium, body).await
}

async fn handle_moderator_image(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SettingsResponse>, Failure> {
    let result = moderator_image(&api, &id, &headers, body).await;
    counted(&api, "imageUploads", result)
}

async fn moderator_image(
    api: &Api,
    page: &str,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<Json<SettingsResponse>, Failure> {
    let user = moderator(api, headers, page)?.id;
    let premium = streamer_is_premium(api, page);
    save_image(api, &user, page.to_string(), premium, body)
        .await
        .map(without_payments)
}

fn streamer_is_premium(api: &Api, page: &str) -> bool {
    api.accounts
        .streamer_of_page(page)
        .is_some_and(|(streamer, _)| is_premium(api, &streamer))
}

async fn save_image(
    api: &Api,
    id: &str,
    page: String,
    premium: bool,
    body: Bytes,
) -> Result<Json<SettingsResponse>, Failure> {
    if !premium {
        return Err(Failure {
            reason: "not-premium",
            status: StatusCode::FORBIDDEN,
            text: "Banner images are only for premium accounts.".to_string(),
        });
    }
    let image = tokio::task::spawn_blocking(move || images::banner(&body))
        .await
        .ok()
        .flatten()
        .ok_or_else(|| Failure {
            reason: "invalid-image",
            status: StatusCode::BAD_REQUEST,
            text: "That image could not be read. Use a JPEG, PNG or WebP at least 600 pixels wide and at most 4000 pixels on each side."
                .to_string(),
        })?;
    let version = api.pages.set_image(&page, &image).map_err(|_| Failure {
        reason: "not-saved",
        status: StatusCode::INTERNAL_SERVER_ERROR,
        text: "The image could not be saved. Try again.".to_string(),
    })?;
    info!(
        "{id} uploaded banner image {version} of {} bytes for page {page}",
        image.len()
    );
    let settings = api.pages.get(&page);
    Ok(settings_response(api, page, settings))
}

async fn handle_account_image_remove(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let SignedIn { id, account, .. } = signed_in(&api, &headers)?;
    remove_image(&api, &id, account.page.id)
}

async fn handle_moderator_image_remove(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, Failure> {
    let user = moderator(&api, &headers, &id)?.id;
    remove_image(&api, &user, id)
        .map(without_payments)
        .map_err(Failure::from)
}

fn remove_image(
    api: &Api,
    id: &str,
    page: String,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    api.pages.remove_image(&page).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "the image could not be removed".to_string(),
        )
    })?;
    info!("{id} removed the banner image of page {page}");
    let settings = api.pages.get(&page);
    Ok(settings_response(api, page, settings))
}

async fn handle_account_settings(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<SettingsRequest>,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let SignedIn { id, account, .. } = signed_in(&api, &headers)?;
    let page = account.page.id;
    update_settings(&api, page, is_premium(&api, &id), request).await
}

async fn handle_moderator_settings(
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

async fn handle_moderator_account(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<AccountResponse>, Failure> {
    moderator(&api, &headers, &id)?;
    let account = api.accounts.of_page(&id).ok_or(failure(
        "not-found",
        StatusCode::NOT_FOUND,
        "This page does not exist.",
    ))?;
    let settings = api.pages.get(&id);
    Ok(Json(AccountResponse {
        name: account.name,
        login: account.login,
        avatar: account.avatar,
        devices: Vec::new(),
        music_device: None,
        wheel_device: None,
        settings: without_payments(settings_response(&api, id.clone(), settings)).0,
        admin: false,
        premium: streamer_is_premium(&api, &id),
        moderators: Vec::new(),
        moderating: Vec::new(),
    }))
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
    let page = account.page.id;
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
    let page = account.page.id;
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
    let page = account.page.id;
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
    let page = account.page.id;
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

async fn handle_account_delete(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, String)> {
    let id = signed_in(&api, &headers)?.id;
    let account = api.accounts.delete(&id).map_err(account_error)?;
    for device in &account.page.devices {
        api.remote_control.disconnect(&device.id);
    }
    info!("{id} deleted their account");
    Ok(StatusCode::NO_CONTENT)
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

fn invalid_paywall() -> (StatusCode, String) {
    (
        StatusCode::BAD_REQUEST,
        "the paywall needs a price of 0.50 to 100000".to_string(),
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
    message: Option<String>,
    accent: Option<String>,
    font: Option<String>,
    image: Option<String>,
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
        message: settings.message,
        accent: settings.accent,
        font: settings.font,
        image: api.pages.image_version(&id),
    }))
}

async fn handle_page_image(
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
#[serde(rename_all = "camelCase")]
struct PaywallResponse {
    paywall: Option<PublicPaywall>,
    sub_songs: Option<Tiers>,
    max_song_minutes: Option<u32>,
}

async fn handle_songs(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
) -> Json<PaywallResponse> {
    let merchant_id = merchant_of(&api, &id);
    let settings = api.pages.get(&id);
    let currency = settings.currency().to_string();
    Json(PaywallResponse {
        sub_songs: settings.sub_songs,
        max_song_minutes: settings.max_song_minutes,
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
    #[serde(rename = "subSpins")]
    sub_spins: Option<Tiers>,
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
        sub_spins: settings.sub_spins.clone(),
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

struct Failure {
    reason: &'static str,
    status: StatusCode,
    text: String,
}

impl From<(StatusCode, String)> for Failure {
    fn from((status, text): (StatusCode, String)) -> Self {
        Self {
            reason: "account",
            status,
            text,
        }
    }
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        (self.status, self.text).into_response()
    }
}

fn failure(reason: &'static str, status: StatusCode, text: &str) -> Failure {
    Failure {
        reason,
        status,
        text: text.to_string(),
    }
}

fn music_error(error: RequestError) -> Failure {
    match error {
        RequestError::Offline => failure(
            "offline",
            StatusCode::NOT_FOUND,
            "The streamer is offline right now. Try again when they are live.",
        ),
        RequestError::Timeout => failure(
            "moblin-timeout",
            StatusCode::GATEWAY_TIMEOUT,
            "The streamer's Moblin did not answer. It may need an update.",
        ),
        RequestError::Failed => failure(
            "moblin-error",
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
    message: Option<String>,
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
        "invalid-link",
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
            "requests-off",
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
            "song-not-found",
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

fn check_song_length(max_minutes: Option<u32>, song: &RequestedSong) -> Result<(), Failure> {
    if let Some(minutes) = max_minutes
        && song
            .duration
            .is_some_and(|duration| duration > minutes * 60)
    {
        return Err(failure(
            "over-max-length",
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("The streamer only takes songs up to {minutes} minutes long."),
        ));
    }
    Ok(())
}

fn song_price(price: u32, duration: Option<u32>) -> u32 {
    price.saturating_mul(duration.unwrap_or(0).div_ceil(300).max(1))
}

fn queue(
    api: &Api,
    id: String,
    request_id: String,
    song: RequestedSong,
    message: Option<String>,
) -> (StatusCode, Json<Value>) {
    info!(
        "queued {:?} by {:?} in request {request_id} for remote control streamer {id}",
        song.link, song.user
    );
    api.deliveries.add_test(
        id.clone(),
        request_id,
        deliveries::Item::Song {
            link: song.link,
            user: song.user,
            message: message.unwrap_or_default(),
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
    let result = song_order(&api, id, request).await;
    counted(&api, "songOrders", result)
}

async fn song_order(
    api: &Api,
    id: String,
    mut request: SongRequest,
) -> Result<Json<OrderResponse>, Failure> {
    let message = viewer_message(request.message.take())?;
    let settings = api.pages.get(&id);
    let currency = settings.currency().to_string();
    let paywall = settings.paywall.ok_or(failure(
        "feature-off",
        StatusCode::BAD_REQUEST,
        "The streamer no longer takes song requests. Reload the page.",
    ))?;
    let payee = payee(api, &id, request.provider.as_deref())?;
    let song_and_user = song_and_user(request)?;
    limit_song_finds(api, &id)?;
    let song = requested_song(api, &id, song_and_user).await?;
    check_song_length(settings.max_song_minutes, &song)?;
    let order = Order {
        streamer: id.clone(),
        item: Item::Song {
            link: song.link,
            name: song.name,
            message,
        },
        user: song.user,
        payee,
        price: song_price(paywall.price, song.duration),
        currency,
    };
    Ok(Json(start_order(api, order).await?))
}

fn limit_song_finds(api: &Api, id: &str) -> Result<(), Failure> {
    if !within_limit(&api.song_finds, id, 10) {
        return Err(failure(
            "too-many-requests",
            StatusCode::TOO_MANY_REQUESTS,
            "Too many song requests on this page right now. Try again in a minute.",
        ));
    }
    Ok(())
}

fn viewer(api: &Api, headers: &HeaderMap, id: &str) -> Result<Viewer, Failure> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .and_then(|session| api.auth.viewer_of(session))
        .filter(|viewer| viewer.page == id)
        .ok_or(failure(
            "not-signed-in",
            StatusCode::UNAUTHORIZED,
            "Sign in with Twitch again.",
        ))
}

#[derive(Serialize)]
struct AllowanceResponse {
    name: String,
    tier: Option<u8>,
    allowance: u32,
    left: u32,
}

fn allowance_response(
    api: &Api,
    headers: &HeaderMap,
    id: &str,
    kind: SubscriberKind,
    tiers: Option<Tiers>,
) -> Result<Json<AllowanceResponse>, Failure> {
    let viewer = viewer(api, headers, id)?;
    let allowance = tiers.map_or(0, |tiers| tiers.allowance(viewer.tier));
    Ok(Json(AllowanceResponse {
        left: api
            .allowances
            .left(id, &viewer.id, kind.name(), allowance)
            .map_err(not_saved)?,
        name: viewer.name,
        tier: viewer.tier,
        allowance,
    }))
}

async fn handle_subscriber_song_allowance(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<AllowanceResponse>, Failure> {
    allowance_response(
        &api,
        &headers,
        &id,
        SubscriberKind::Song,
        api.pages.get(&id).sub_songs,
    )
}

async fn handle_subscriber_spin_allowance(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<AllowanceResponse>, Failure> {
    allowance_response(
        &api,
        &headers,
        &id,
        SubscriberKind::Spin,
        api.pages.get(&id).sub_spins,
    )
}

#[derive(Deserialize)]
struct SubscriberSongRequest {
    song: String,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Deserialize)]
struct SubscriberSpinRequest {
    #[serde(default)]
    message: Option<String>,
}

#[derive(Serialize)]
struct LeftResponse {
    left: u32,
}

async fn handle_subscriber_song(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<SubscriberSongRequest>,
) -> Result<(StatusCode, Json<LeftResponse>), Failure> {
    let result = subscriber_song(&api, id, &headers, request).await;
    counted(&api, "subSongs", result)
}

async fn handle_subscriber_spin(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<SubscriberSpinRequest>,
) -> Result<(StatusCode, Json<LeftResponse>), Failure> {
    let result = subscriber_spin(&api, id, &headers, request);
    counted(&api, "subSpins", result)
}

#[derive(Clone, Copy)]
enum SubscriberKind {
    Song,
    Spin,
}

impl SubscriberKind {
    fn name(self) -> &'static str {
        match self {
            Self::Song => "song",
            Self::Spin => "spin",
        }
    }

    fn short(self, left: u32) -> Failure {
        match self {
            Self::Song if left > 0 => failure(
                "too-long",
                StatusCode::CONFLICT,
                "The song is longer than the minutes you have left this month.",
            ),
            Self::Song => failure(
                "no-minutes-left",
                StatusCode::CONFLICT,
                "You have used all your minutes this month.",
            ),
            Self::Spin => failure(
                "no-spins-left",
                StatusCode::CONFLICT,
                "You have used all your spins this month.",
            ),
        }
    }
}

fn not_saved(error: anyhow::Error) -> Failure {
    warn!("{error:#}");
    failure(
        "not-saved",
        StatusCode::SERVICE_UNAVAILABLE,
        "This is unavailable right now. Try again later.",
    )
}

struct Subscriber {
    viewer: Viewer,
    tier: u8,
    allowance: u32,
    left: u32,
}

fn subscriber(
    api: &Api,
    headers: &HeaderMap,
    id: &str,
    kind: SubscriberKind,
    tiers: Option<&Tiers>,
) -> Result<Subscriber, Failure> {
    let (off, not_subscribed, excluded) = match kind {
        SubscriberKind::Song => (
            "The streamer no longer takes song requests from subscribers. Reload the page.",
            "Only Twitch subscribers of this streamer can request songs.",
            "request songs",
        ),
        SubscriberKind::Spin => (
            "The streamer no longer lets subscribers spin the wheel. Reload the page.",
            "Only Twitch subscribers of this streamer can spin the wheel.",
            "spin the wheel",
        ),
    };
    let tiers = tiers.ok_or(failure("feature-off", StatusCode::BAD_REQUEST, off))?;
    let viewer = viewer(api, headers, id)?;
    let tier = viewer.tier.ok_or(failure(
        "not-subscribed",
        StatusCode::FORBIDDEN,
        not_subscribed,
    ))?;
    let allowance = tiers.allowance(Some(tier));
    if allowance == 0 {
        return Err(failure(
            "tier-excluded",
            StatusCode::FORBIDDEN,
            &format!("Tier {tier} subscribers can't {excluded} here."),
        ));
    }
    let left = api
        .allowances
        .left(id, &viewer.id, kind.name(), allowance)
        .map_err(not_saved)?;
    if left == 0 {
        return Err(kind.short(0));
    }
    Ok(Subscriber {
        viewer,
        tier,
        allowance,
        left,
    })
}

fn new_request_id(api: &Api) -> Result<String, Failure> {
    api.remote_control.new_request_id().ok_or(failure(
        "crashed",
        StatusCode::SERVICE_UNAVAILABLE,
        "Something went wrong. Try again.",
    ))
}

fn queue_for_subscriber(
    api: &Api,
    subscriber: &Subscriber,
    kind: SubscriberKind,
    units: u32,
    order: &Order,
) -> Result<u32, Failure> {
    let request_id = new_request_id(api)?;
    let item = delivery_item(order);
    let left = api
        .allowances
        .take(
            &order.streamer,
            &subscriber.viewer.id,
            kind.name(),
            subscriber.allowance,
            units,
            |connection| {
                insert_captured(connection, &request_id, order)?;
                deliveries::insert(connection, &order.streamer, &request_id, &item)
            },
        )
        .map_err(not_saved)?
        .map_err(|left| kind.short(left))?;
    api.deliveries.notify();
    info!(
        "queued {:?} for tier {} subscriber {:?} in request {request_id} for remote control streamer {}",
        order.description(),
        subscriber.tier,
        order.user,
        order.streamer
    );
    Ok(left)
}

async fn subscriber_song(
    api: &Api,
    id: String,
    headers: &HeaderMap,
    request: SubscriberSongRequest,
) -> Result<(StatusCode, Json<LeftResponse>), Failure> {
    let settings = api.pages.get(&id);
    let subscriber = subscriber(
        api,
        headers,
        &id,
        SubscriberKind::Song,
        settings.sub_songs.as_ref(),
    )?;
    let message = viewer_message(request.message)?;
    let song_and_user = song_and_user(SongRequest {
        song: request.song,
        user: Some(subscriber.viewer.name.clone()),
        message: None,
        provider: None,
    })?;
    limit_song_finds(api, &id)?;
    let song = requested_song(api, &id, song_and_user).await?;
    check_song_length(settings.max_song_minutes, &song)?;
    let units = song_price(1, song.duration);
    if units > subscriber.left {
        return Err(SubscriberKind::Song.short(subscriber.left));
    }
    let order = Order {
        streamer: id,
        item: Item::Song {
            link: song.link,
            name: song.name,
            message,
        },
        user: song.user,
        payee: Payee::Twitch(subscriber.tier),
        price: 0,
        currency: settings.currency().to_string(),
    };
    let left = queue_for_subscriber(api, &subscriber, SubscriberKind::Song, units, &order)?;
    Ok((StatusCode::ACCEPTED, Json(LeftResponse { left })))
}

fn subscriber_spin(
    api: &Api,
    id: String,
    headers: &HeaderMap,
    request: SubscriberSpinRequest,
) -> Result<(StatusCode, Json<LeftResponse>), Failure> {
    let settings = api.pages.get(&id);
    let subscriber = subscriber(
        api,
        headers,
        &id,
        SubscriberKind::Spin,
        settings.sub_spins.as_ref(),
    )?;
    wheel_available(api, &id)?;
    let order = Order {
        streamer: id,
        item: Item::Spin {
            spins: 1,
            message: viewer_message(request.message)?,
        },
        user: user_name(Some(subscriber.viewer.name.clone()), "Someone"),
        payee: Payee::Twitch(subscriber.tier),
        price: 0,
        currency: settings.currency().to_string(),
    };
    let left = queue_for_subscriber(api, &subscriber, SubscriberKind::Spin, 1, &order)?;
    Ok((StatusCode::ACCEPTED, Json(LeftResponse { left })))
}

fn moderator(api: &Api, headers: &HeaderMap, id: &str) -> Result<SignedIn, Failure> {
    let signed_in = signed_in(api, headers)?;
    let allowed = signed_in.account.page.id == id
        || api
            .accounts
            .streamer_of_page(id)
            .is_some_and(|(broadcaster, _)| api.accounts.moderates(&signed_in.id, &broadcaster));
    if !allowed {
        return Err(failure(
            "not-moderator",
            StatusCode::FORBIDDEN,
            "Only the streamer and the moderators they turned on can do this.",
        ));
    }
    Ok(signed_in)
}

#[derive(Deserialize)]
struct ModeratorMusicRequest {
    command: String,
    #[serde(default)]
    id: Option<String>,
}

async fn handle_moderator_music(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ModeratorMusicRequest>,
) -> Result<Json<QueueResponse>, Failure> {
    let result = moderator_music(&api, &id, &headers, request).await;
    counted(&api, "moderatorMusic", result)
}

async fn moderator_music(
    api: &Api,
    id: &str,
    headers: &HeaderMap,
    request: ModeratorMusicRequest,
) -> Result<Json<QueueResponse>, Failure> {
    let SignedIn {
        id: user, account, ..
    } = moderator(api, headers, id)?;
    let song = || {
        request
            .id
            .clone()
            .filter(|id| !id.is_empty() && id.len() <= 200)
            .ok_or(failure(
                "invalid-command",
                StatusCode::BAD_REQUEST,
                "Pick a song in the queue.",
            ))
    };
    let command = match request.command.as_str() {
        "play" => MusicCommand::Play {},
        "pause" => MusicCommand::Pause {},
        "clear" => MusicCommand::Clear {},
        "remove" => MusicCommand::Remove { id: song()? },
        "select" => MusicCommand::Select { id: song()? },
        _ => {
            return Err(failure(
                "invalid-command",
                StatusCode::BAD_REQUEST,
                "Unknown command.",
            ));
        }
    };
    info!(
        "{user} ({}) sent {command:?} to remote control streamer {id}",
        account.name
    );
    let device = music_device(api, id);
    let status = api
        .remote_control
        .music_uncached(&device, command)
        .await
        .map_err(music_error)?;
    Ok(Json(QueueResponse {
        status,
        song_requests: api.remote_control.takes_song_requests(&device),
    }))
}

async fn handle_moderator_song(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<SubscriberSongRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let result = moderator_song(&api, id, &headers, request).await;
    counted(&api, "modSongs", result)
}

async fn moderator_song(
    api: &Api,
    id: String,
    headers: &HeaderMap,
    request: SubscriberSongRequest,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let account = moderator(api, headers, &id)?.account;
    let message = viewer_message(request.message)?;
    let song_and_user = song_and_user(SongRequest {
        song: request.song,
        user: Some(account.name),
        message: None,
        provider: None,
    })?;
    limit_song_finds(api, &id)?;
    let song = requested_song(api, &id, song_and_user).await?;
    let order = Order {
        currency: api.pages.get(&id).currency().to_string(),
        streamer: id,
        item: Item::Song {
            link: song.link,
            name: song.name,
            message,
        },
        user: song.user,
        payee: Payee::Moderator,
        price: 0,
    };
    let request_id = new_request_id(api)?;
    let item = delivery_item(&order);
    {
        let mut database = api.database.lock();
        let transaction = database
            .transaction()
            .map_err(|error| not_saved(error.into()))?;
        insert_captured(&transaction, &request_id, &order)
            .and_then(|()| deliveries::insert(&transaction, &order.streamer, &request_id, &item))
            .and_then(|()| transaction.commit())
            .map_err(|error| not_saved(error.into()))?;
    }
    api.deliveries.notify();
    info!(
        "queued {:?} for moderator {:?} in request {request_id} for remote control streamer {}",
        order.description(),
        order.user,
        order.streamer
    );
    Ok(accepted())
}

async fn handle_moderator_recent(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Query(query): Query<RecentQuery>,
) -> Result<Json<RecentResponse>, Failure> {
    moderator(&api, &headers, &id)?;
    let page = page_of(&api, &id).ok_or(failure(
        "not-found",
        StatusCode::NOT_FOUND,
        "This page does not exist.",
    ))?;
    recent(&api, page, query, false).map_err(|(status, text)| Failure {
        reason: "invalid-kind",
        status,
        text,
    })
}

async fn handle_account_resend(
    State(api): State<Arc<Api>>,
    Path(order_id): Path<String>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let SignedIn { account, .. } = signed_in(&api, &headers)?;
    resend(&api, &account.page.id, &order_id)
}

async fn handle_moderator_resend(
    State(api): State<Arc<Api>>,
    Path((id, order_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<Value>), Failure> {
    moderator(&api, &headers, &id)?;
    resend(&api, &id, &order_id)
}

fn resend(api: &Api, page: &str, order_id: &str) -> Result<(StatusCode, Json<Value>), Failure> {
    let order = match api.orders.get(order_id) {
        Some((Stage::Captured, order)) if order.streamer == page => order,
        _ => {
            return Err(failure(
                "not-found",
                StatusCode::NOT_FOUND,
                "This request does not exist.",
            ));
        }
    };
    api.deliveries
        .resend(&order.streamer, order_id, &delivery_item(&order))
        .map_err(|error| not_saved(error.into()))?;
    info!(
        "queued {:?} in order {order_id} again for remote control streamer {}",
        order.description(),
        order.streamer
    );
    Ok(accepted())
}

async fn handle_song_capture(
    State(api): State<Arc<Api>>,
    Path((id, order_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    tokio::spawn(capture_paid(api, id, order_id, "song"))
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
    let name = format!("{}Orders", order.payee.slug());
    let result = create_order(api, order).await;
    counted(api, &name, result)
}

async fn create_order(api: &Api, order: Order) -> Result<OrderResponse, Failure> {
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
            Payee::Twitch(_) | Payee::Moderator => anyhow::bail!("free requests are not paid"),
        }
    };
    let (order_id, stripe) = created.await.map_err(|error| {
        warn!(
            "creating a {provider} {} order for remote control streamer {} failed: {error:#}",
            order.item.kind(),
            order.streamer
        );
        failure(
            "provider",
            StatusCode::BAD_GATEWAY,
            &format!("{provider} could not start the payment. Try again."),
        )
    })?;
    api.orders.insert(&order_id, &order).map_err(|error| {
        warn!("{error:#}");
        failure(
            "not-saved",
            StatusCode::SERVICE_UNAVAILABLE,
            "Payments are unavailable right now. Try again later.",
        )
    })?;
    Ok(OrderResponse { order_id, stripe })
}

fn capture_crashed() -> Failure {
    failure(
        "crashed",
        StatusCode::INTERNAL_SERVER_ERROR,
        "Something went wrong with the payment.",
    )
}

fn payment_unavailable() -> Failure {
    failure(
        "payments-unavailable",
        StatusCode::SERVICE_UNAVAILABLE,
        "Payments are unavailable right now. Try again later.",
    )
}

fn counted<T>(api: &Api, name: &str, result: Result<T, Failure>) -> Result<T, Failure> {
    api.counters.bump(&match &result {
        Ok(_) => name.to_string(),
        Err(failure) => format!("{name}Failed:{}", failure.reason),
    });
    result
}

async fn capture_paid(
    api: Arc<Api>,
    id: String,
    order_id: String,
    kind: &'static str,
) -> Result<(StatusCode, Json<Value>), Failure> {
    if let Err(failure) = capture_order(&api, &id, &order_id, kind).await {
        api.counters
            .bump(&format!("{kind}PaymentsFailed:{}", failure.reason));
        return Err(failure);
    }
    Ok(accepted())
}

fn expired() -> Failure {
    failure(
        "expired",
        StatusCode::GONE,
        "This payment is unknown or expired, so no money was taken.",
    )
}

fn provider_failed(provider: &str) -> Failure {
    failure(
        "provider",
        StatusCode::BAD_GATEWAY,
        &format!("{provider} could not complete the payment."),
    )
}

fn pending(provider: &str, kind: &str) -> Failure {
    let text = match kind {
        "song" => format!(
            "{provider} is still reviewing the payment, so no song was added yet. It will be added if {provider} approves the payment."
        ),
        "tip" => format!(
            "{provider} is still reviewing the payment. If {provider} approves it, the tip will be shown and the money reaches the streamer."
        ),
        _ => format!(
            "{provider} is still reviewing the payment, so the wheel was not spun yet. It will be spun if {provider} approves the payment."
        ),
    };
    failure("pending", StatusCode::PAYMENT_REQUIRED, &text)
}

async fn capture_order(api: &Api, id: &str, order_id: &str, kind: &str) -> Result<(), Failure> {
    let (stage, order) = api
        .orders
        .get(order_id)
        .filter(|(_, order)| order.streamer == id && order.item.kind() == kind)
        .ok_or_else(expired)?;
    match stage {
        Stage::Captured => return Ok(()),
        Stage::Capturing => {
            let status = settle_one(api, order_id, &order).await;
            return settled(&order, status);
        }
        Stage::Created => {}
    }
    let music = music_device(api, id);
    let refusal = if kind == "song" && !api.remote_control.is_connected(&music) {
        Some((
            "offline",
            StatusCode::NOT_FOUND,
            "The streamer went offline, so your payment was not taken.",
        ))
    } else if kind == "song" && !api.remote_control.takes_song_requests(&music) {
        Some((
            "requests-off",
            StatusCode::CONFLICT,
            "The streamer stopped taking song requests, so your payment was not taken.",
        ))
    } else if kind == "spin" && api.remote_control.wheel(&wheel_device(api, id)).is_none() {
        Some((
            "no-wheel",
            StatusCode::NOT_FOUND,
            "The streamer's wheel is gone, so your payment was not taken.",
        ))
    } else {
        None
    };
    if let Some((reason, status, refusal)) = refusal {
        if let Payee::Stripe(account) = &order.payee
            && let Some(stripe) = &api.stripe
        {
            api.orders.remove(order_id);
            if let Err(error) = stripe.cancel(order_id, account).await {
                warn!("cancelling Stripe payment {order_id} failed: {error:#}");
            }
        }
        return Err(failure(reason, status, refusal));
    }
    let available = match &order.payee {
        Payee::PayPal(_) => api.paypal.is_some(),
        Payee::Stripe(_) => api.stripe.is_some(),
        Payee::Twitch(_) | Payee::Moderator => false,
    };
    if !available {
        return Err(payment_unavailable());
    }
    if let Err(error) = api
        .orders
        .transition(order_id, Stage::Created, Stage::Capturing)
    {
        warn!("{error:#}");
        return Err(failure(
            "not-saved",
            StatusCode::SERVICE_UNAVAILABLE,
            "Payments are unavailable right now, so no money was taken. Try again later.",
        ));
    }
    capture_at_provider(api, order_id, &order, kind).await
}

async fn capture_at_provider(
    api: &Api,
    order_id: &str,
    order: &Order,
    kind: &str,
) -> Result<(), Failure> {
    let provider = order.payee.provider();
    let captured = match &order.payee {
        Payee::PayPal(merchant_id) => {
            let paypal = api.paypal.as_ref().ok_or_else(payment_unavailable)?;
            paypal.capture(order_id, order, merchant_id).await
        }
        Payee::Stripe(account) => {
            let stripe = api.stripe.as_ref().ok_or_else(payment_unavailable)?;
            stripe.capture(order_id, order, account).await
        }
        Payee::Twitch(_) | Payee::Moderator => return Err(payment_unavailable()),
    };
    let result = match captured {
        Ok(()) | Err(CaptureError::AlreadyCaptured) => {
            info!(
                "{:?} paid {} {} in {provider} order {order_id}",
                order.user,
                format_price(order.price),
                order.currency
            );
            record_capture(api, order_id, order);
            Ok(())
        }
        Err(CaptureError::Unknown) => {
            let status = settle_one(api, order_id, order).await;
            settled(order, status)
        }
        Err(error) => {
            if error != CaptureError::Pending
                && let Err(error) =
                    api.orders
                        .transition(order_id, Stage::Capturing, Stage::Created)
            {
                warn!("{error:#}");
            }
            Err(match error {
                CaptureError::NotApproved if kind == "song" => failure(
                    "not-approved",
                    StatusCode::PAYMENT_REQUIRED,
                    "The payment was not completed. No song was added.",
                ),
                CaptureError::NotApproved => failure(
                    "not-approved",
                    StatusCode::PAYMENT_REQUIRED,
                    "The payment was not completed.",
                ),
                CaptureError::Pending => pending(provider, kind),
                _ => provider_failed(provider),
            })
        }
    };
    if let Err(failure) = &result {
        api.counters.bump(&format!(
            "{}PaymentsFailed:{}",
            order.payee.slug(),
            failure.reason
        ));
    }
    result
}

fn settled(order: &Order, status: Option<PaymentStatus>) -> Result<(), Failure> {
    let provider = order.payee.provider();
    match status {
        Some(PaymentStatus::Captured) => Ok(()),
        Some(PaymentStatus::Pending) => Err(pending(provider, order.item.kind())),
        Some(PaymentStatus::NotCaptured) => Err(provider_failed(provider)),
        Some(PaymentStatus::Gone) => Err(expired()),
        None => Err(failure(
            "unknown",
            StatusCode::BAD_GATEWAY,
            &format!(
                "{provider} did not answer. If the money was taken, {}.",
                match order.item.kind() {
                    "song" => "the song will still be added",
                    "tip" => "the tip will still be shown",
                    _ => "the wheel will still be spun",
                }
            ),
        )),
    }
}

async fn settle_one(api: &Api, order_id: &str, order: &Order) -> Option<PaymentStatus> {
    let provider = order.payee.provider();
    let status = match &order.payee {
        Payee::PayPal(merchant_id) => {
            api.paypal
                .as_ref()?
                .status(order_id, order, merchant_id)
                .await
        }
        Payee::Stripe(account) => api.stripe.as_ref()?.status(order_id, order, account).await,
        Payee::Twitch(_) | Payee::Moderator => return None,
    };
    let status = match status {
        Ok(status) => status,
        Err(error) => {
            warn!("settling {provider} order {order_id} failed: {error:#}");
            return None;
        }
    };
    match status {
        PaymentStatus::Captured => {
            info!(
                "{:?} paid {} {} in {provider} order {order_id}, found when settling it",
                order.user,
                format_price(order.price),
                order.currency
            );
            record_capture(api, order_id, order);
        }
        PaymentStatus::Pending => {
            info!("{provider} is still reviewing order {order_id}");
        }
        PaymentStatus::NotCaptured => {
            info!("{provider} order {order_id} was not captured");
            if let Err(error) = api
                .orders
                .transition(order_id, Stage::Capturing, Stage::Created)
            {
                warn!("{error:#}");
            }
        }
        PaymentStatus::Gone => {
            info!("{provider} order {order_id} is gone");
            api.orders.remove(order_id);
        }
    }
    Some(status)
}

pub async fn settle(api: &Api, min_age: Duration) {
    for unsettled in api.orders.unsettled(min_age) {
        settle_one(api, &unsettled.id, &unsettled.order).await;
    }
}

async fn expire_orders(api: &Api) {
    for (order_id, order) in api.orders.expire() {
        if let Payee::Stripe(account) = &order.payee
            && let Some(stripe) = &api.stripe
        {
            match stripe.cancel(&order_id, account).await {
                Ok(()) => info!("cancelled expired Stripe payment {order_id}"),
                Err(error) => {
                    warn!("cancelling expired Stripe payment {order_id} failed: {error:#}")
                }
            }
        }
    }
}

pub async fn settle_orders(api: Arc<Api>) {
    loop {
        settle(&api, Duration::from_secs(60)).await;
        expire_orders(&api).await;
        api.orders.prune();
        api.allowances.prune();
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

fn delivery_item(order: &Order) -> deliveries::Item {
    match &order.item {
        Item::Song { link, message, .. } => deliveries::Item::Song {
            link: link.clone(),
            user: order.user.clone(),
            message: message.clone().unwrap_or_default(),
        },
        Item::Tip { message } => deliveries::Item::Tip {
            user: order.user.clone(),
            message: message.clone().unwrap_or_default(),
            amount: order.price,
            currency: order.currency.clone(),
        },
        Item::Spin { spins, message } => deliveries::Item::Spin {
            user: order.user.clone(),
            message: message.clone().unwrap_or_default(),
            spins: *spins,
        },
    }
}

fn record_capture(api: &Api, order_id: &str, order: &Order) {
    let item = delivery_item(order);
    let saved = api.orders.captured(order_id, |connection| {
        deliveries::insert(connection, &order.streamer, order_id, &item)?;
        counters::bump(connection, &format!("{}Payments", order.item.kind()));
        counters::bump(connection, &format!("{}Payments", order.payee.slug()));
        Ok(())
    });
    match saved {
        Ok(false) => {}
        Ok(true) => {
            api.deliveries.notify();
            info!(
                "queued {:?} in order {order_id} for remote control streamer {}",
                order.description(),
                order.streamer
            );
        }
        Err(error) => warn!("{error:#}, settling it later"),
    }
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
            "invalid-message",
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
    let result = tip_order(&api, id, request).await;
    counted(&api, "tipOrders", result)
}

async fn tip_order(
    api: &Api,
    id: String,
    request: TipRequest,
) -> Result<Json<OrderResponse>, Failure> {
    let settings = api.pages.get(&id);
    let currency = settings.currency().to_string();
    let tips = settings.tips.ok_or(failure(
        "feature-off",
        StatusCode::BAD_REQUEST,
        "The streamer no longer takes tips. Reload the page.",
    ))?;
    let payee = payee(api, &id, request.provider.as_deref())?;
    let minimum = tips.amounts.iter().copied().min().unwrap_or(50);
    if request.amount < minimum {
        return Err(failure(
            "amount-too-low",
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
    Ok(Json(start_order(api, order).await?))
}

async fn handle_tip_capture(
    State(api): State<Arc<Api>>,
    Path((id, order_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    tokio::spawn(capture_paid(api, id, order_id, "tip"))
        .await
        .unwrap_or_else(|_| Err(capture_crashed()))
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
    #[serde(default)]
    price: Option<u32>,
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
    let result = spin_order(&api, id, request).await;
    counted(&api, "spinOrders", result)
}

async fn spin_order(
    api: &Api,
    id: String,
    request: SpinRequest,
) -> Result<Json<OrderResponse>, Failure> {
    let settings = api.pages.get(&id);
    let currency = settings.currency().to_string();
    let spin = settings.spin.ok_or(failure(
        "feature-off",
        StatusCode::BAD_REQUEST,
        "The streamer turned the wheel off. Reload the page.",
    ))?;
    wheel_available(api, &id)?;
    let payee = payee(api, &id, request.provider.as_deref())?;
    let option = spin
        .options
        .iter()
        .find(|option| {
            option.spins == request.spins && request.price.is_none_or(|price| price == option.price)
        })
        .ok_or(failure(
            "invalid-option",
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
    Ok(Json(start_order(api, order).await?))
}

fn wheel_available(api: &Api, id: &str) -> Result<(), Failure> {
    let device = wheel_device(api, id);
    if api.remote_control.wheel(&device).is_none() {
        return Err(if api.remote_control.is_connected(&device) {
            failure(
                "no-wheel",
                StatusCode::CONFLICT,
                "The streamer is not taking wheel spins right now.",
            )
        } else {
            failure(
                "offline",
                StatusCode::NOT_FOUND,
                "The streamer's wheel is not available right now. Try again when they are live.",
            )
        });
    }
    Ok(())
}

async fn handle_spin_capture(
    State(api): State<Arc<Api>>,
    Path((id, order_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    tokio::spawn(capture_paid(api, id, order_id, "spin"))
        .await
        .unwrap_or_else(|_| Err(capture_crashed()))
}

#[derive(Deserialize)]
struct TestRequest {
    #[serde(default)]
    song: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

fn within_limit(limits: &Mutex<HashMap<String, Vec<Instant>>>, id: &str, max: usize) -> bool {
    let mut limits = limits.lock().unwrap();
    limits.retain(|_, sent| {
        sent.retain(|at| at.elapsed() < Duration::from_secs(60));
        !sent.is_empty()
    });
    let sent = limits.entry(id.to_string()).or_default();
    if sent.len() >= max {
        return false;
    }
    sent.push(Instant::now());
    true
}

fn test_page(
    api: &Api,
    headers: &HeaderMap,
    page: Option<&str>,
) -> Result<(String, Account, String), Failure> {
    let (id, account) = match page {
        Some(page) => (page.to_string(), moderator(api, headers, page)?.account),
        None => {
            let account = signed_in(api, headers)?.account;
            (account.page.id.clone(), account)
        }
    };
    if !within_limit(&api.tests, &id, 10) {
        return Err(failure(
            "too-many-tests",
            StatusCode::TOO_MANY_REQUESTS,
            "Too many tests. Try again in a minute.",
        ));
    }
    let request_id = api.remote_control.new_request_id().ok_or(failure(
        "crashed",
        StatusCode::SERVICE_UNAVAILABLE,
        "The test could not be sent. Try again.",
    ))?;
    Ok((id, account, request_id))
}

fn moblin_not_connected() -> Failure {
    failure("offline", StatusCode::CONFLICT, "Connect Moblin first.")
}

async fn handle_account_test_song(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<TestRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let result = account_test_song(&api, &headers, request).await;
    counted(&api, "songTests", result)
}

async fn account_test_song(
    api: &Api,
    headers: &HeaderMap,
    request: TestRequest,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let (id, account, request_id) = test_page(api, headers, None)?;
    if !api.remote_control.is_connected(&music_device(api, &id)) {
        return Err(moblin_not_connected());
    }
    let message = viewer_message(request.message)?;
    let request = SongRequest {
        song: request.song.unwrap_or_default(),
        user: Some(account.name),
        message: None,
        provider: None,
    };
    let song = requested_song(api, &id, song_and_user(request)?).await?;
    Ok(queue(api, id, request_id, song, message))
}

async fn handle_account_test_tip(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<TestRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let result = account_test_tip(&api, &headers, None, request).await;
    counted(&api, "tipTests", result)
}

async fn handle_moderator_test_tip(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<TestRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let result = account_test_tip(&api, &headers, Some(&id), request).await;
    counted(&api, "tipTests", result)
}

async fn account_test_tip(
    api: &Api,
    headers: &HeaderMap,
    page: Option<&str>,
    request: TestRequest,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let (id, account, request_id) = test_page(api, headers, page)?;
    let settings = api.pages.get(&id);
    let amount = settings
        .tips
        .as_ref()
        .and_then(|tips| tips.amounts.first().copied())
        .unwrap_or(500);
    info!("queued a test tip in request {request_id} for remote control streamer {id}");
    api.deliveries.add_test(
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
    let result = account_test_spin(&api, &headers, None, request).await;
    counted(&api, "spinTests", result)
}

async fn handle_moderator_test_spin(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<TestRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let result = account_test_spin(&api, &headers, Some(&id), request).await;
    counted(&api, "spinTests", result)
}

async fn account_test_spin(
    api: &Api,
    headers: &HeaderMap,
    page: Option<&str>,
    request: TestRequest,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let (id, account, request_id) = test_page(api, headers, page)?;
    let device = wheel_device(api, &id);
    if api.remote_control.wheel(&device).is_none() {
        return Err(if api.remote_control.is_connected(&device) {
            failure(
                "no-wheel",
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
    api.deliveries.add_test(
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
    use std::sync::atomic::{AtomicUsize, Ordering};
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
        start_api_with(paypal, None, None, None).await
    }

    async fn start_api_with(
        paypal: Option<PayPal>,
        stripe: Option<Stripe>,
        twitch: Option<OAuthApp>,
        pages: Option<Pages>,
    ) -> (String, Arc<Api>) {
        let database = Database::memory();
        let pages = pages.unwrap_or_else(|| Pages::new(database.clone()));
        let api = Arc::new(Api {
            store: Arc::new(Store::new(6, Database::memory())),
            challenges: Challenges::new(),
            app_attest: None,
            live: None,
            remote_control: Streamers::new(),
            pages,
            paypal,
            stripe,
            orders: Orders::new(database.clone()),
            allowances: Allowances::new(database.clone()),
            deliveries: Deliveries::new(database.clone()),
            auth: Auth::new(
                reqwest::Client::new(),
                "http://localhost".to_string(),
                "https://moblin.app".to_string(),
                twitch,
                Some("secret"),
            )
            .unwrap(),
            accounts: Accounts::new(database.clone()),
            counters: Counters::new(database.clone()),
            database,
            admins: vec!["admin".to_string()],
            premium_accounts: vec!["premium".to_string()],
            tests: Mutex::default(),
            song_finds: Mutex::default(),
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
        captured: Arc<std::sync::Mutex<bool>>,
        lose_capture: Arc<std::sync::Mutex<bool>>,
        fail_capture: Arc<std::sync::Mutex<bool>>,
        lookups_fail: Arc<std::sync::Mutex<bool>>,
        approved: Arc<std::sync::Mutex<bool>>,
        refuse_capture: Arc<std::sync::Mutex<bool>>,
        gone: Arc<std::sync::Mutex<bool>>,
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
                    let captured = fake.captured.clone();
                    move |headers: HeaderMap, Json(mut request): Json<Value>| async move {
                        request["assertion"] =
                            json!(headers["paypal-auth-assertion"].to_str().unwrap());
                        *order.lock().unwrap() = Some(request);
                        *captured.lock().unwrap() = false;
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
                    let captured = fake.captured.clone();
                    let lose_capture = fake.lose_capture.clone();
                    let fail_capture = fake.fail_capture.clone();
                    let refuse_capture = fake.refuse_capture.clone();
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
                        if *fail_capture.lock().unwrap() {
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({"name": "INTERNAL_SERVER_ERROR"})),
                            );
                        }
                        if *refuse_capture.lock().unwrap() {
                            return (
                                StatusCode::UNPROCESSABLE_ENTITY,
                                Json(json!({"details": [{"issue": "INSTRUMENT_DECLINED"}]})),
                            );
                        }
                        if *captured.lock().unwrap() {
                            return (
                                StatusCode::UNPROCESSABLE_ENTITY,
                                Json(json!({"details": [{"issue": "ORDER_ALREADY_CAPTURED"}]})),
                            );
                        }
                        *captured.lock().unwrap() = true;
                        if *lose_capture.lock().unwrap() {
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({"name": "INTERNAL_SERVER_ERROR"})),
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
            )
            .route(
                "/v2/checkout/orders/{id}",
                get({
                    let order = order.clone();
                    let captured = fake.captured.clone();
                    let lookups_fail = fake.lookups_fail.clone();
                    let approved = fake.approved.clone();
                    let gone = fake.gone.clone();
                    move |Path(id): Path<String>| async move {
                        if *lookups_fail.lock().unwrap() {
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({"name": "INTERNAL_SERVER_ERROR"})),
                            );
                        }
                        if id != "ORDER1" || *gone.lock().unwrap() {
                            return (
                                StatusCode::NOT_FOUND,
                                Json(json!({"name": "RESOURCE_NOT_FOUND"})),
                            );
                        }
                        let amount =
                            order.lock().unwrap().as_ref().unwrap()["purchase_units"][0]["amount"]
                                .clone();
                        if !*captured.lock().unwrap() {
                            return (StatusCode::OK, Json(json!({"id": id, "status": "APPROVED"})));
                        }
                        let status = if amount["currency_code"] == "EUR" && !*approved.lock().unwrap() {
                            "PENDING"
                        } else {
                            "COMPLETED"
                        };
                        (
                            StatusCode::OK,
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
        lose_capture: Arc<std::sync::Mutex<bool>>,
        processing: Arc<std::sync::Mutex<bool>>,
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
                    let lose_capture = fake.lose_capture.clone();
                    let processing = fake.processing.clone();
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
                        if *processing.lock().unwrap() {
                            *state = "processing";
                            return (
                                StatusCode::OK,
                                Json(json!({
                                    "id": id,
                                    "status": "processing",
                                    "amount_received": 0,
                                    "currency": payment["form"]["currency"],
                                })),
                            );
                        }
                        *state = "succeeded";
                        if *lose_capture.lock().unwrap() {
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({"error": {"type": "api_error"}})),
                            );
                        }
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
                "/v1/payment_intents/{id}",
                get({
                    let payment = fake.payment.clone();
                    let state = fake.state.clone();
                    move |headers: HeaderMap, Path(id): Path<String>| async move {
                        assert_eq!(headers["stripe-account"], "acct_1");
                        if id != "pi_1" {
                            return (
                                StatusCode::NOT_FOUND,
                                Json(json!({"error": {"code": "resource_missing"}})),
                            );
                        }
                        let payment = payment.lock().unwrap().clone().unwrap();
                        let state = *state.lock().unwrap();
                        let amount = payment["form"]["amount"]
                            .as_str()
                            .unwrap()
                            .parse::<u32>()
                            .unwrap();
                        (
                            StatusCode::OK,
                            Json(json!({
                                "id": id,
                                "status": state,
                                "amount_received": if state == "succeeded" { amount } else { 0 },
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
        let (address, api) = start_api_with(None, Some(new_stripe(stripe_url)), None, None).await;
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
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(api.deliveries.pending().len(), 1);
        let counters = api.counters.all();
        let count = |name: &str| {
            counters
                .iter()
                .find(|counter| counter.name == name)
                .map_or(0, |counter| counter.total)
        };
        assert_eq!(count("stripeOrders"), 1);
        assert_eq!(count("stripePaymentsFailed:not-approved"), 1);
        assert_eq!(count("stripePayments"), 1);
        assert_eq!(count("tipPayments"), 1);
        assert!(
            !counters
                .iter()
                .any(|counter| counter.name.starts_with("paypal"))
        );
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
    async fn stripe_holds_of_expired_orders_are_released() {
        let (_, api, fake) = start_stripe_api().await;
        let order = |payee: Payee| Order {
            streamer: "p".to_string(),
            item: Item::Tip { message: None },
            user: "Viewer".to_string(),
            payee,
            price: 700,
            currency: "USD".to_string(),
        };
        for (id, payee) in [
            ("pi_1", Payee::Stripe("acct_1".to_string())),
            ("pi_2", Payee::Stripe("acct_1".to_string())),
            ("ORDER1", Payee::PayPal("MERCHANT00001".to_string())),
        ] {
            api.orders.insert(id, &order(payee)).unwrap();
        }
        api.orders.age("pi_1", 4 * 60 * 60);
        api.orders.age("ORDER1", 4 * 60 * 60);
        expire_orders(&api).await;
        assert_eq!(*fake.cancelled.lock().unwrap(), ["pi_1"]);
        assert_eq!(api.orders.get("pi_1"), None);
        assert_eq!(api.orders.get("ORDER1"), None);
        assert_eq!(api.orders.get("pi_2").unwrap().0, Stage::Created);
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

    type Socket = tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >;

    async fn connect(address: &str, key: &str) -> Socket {
        let (mut socket, _) = tokio_tungstenite::connect_async(format!(
            "ws://{address}/remote-control/streamer/{key}"
        ))
        .await
        .unwrap();
        identify(&mut socket).await;
        socket
    }

    async fn start_paid() -> (String, Arc<Api>, Arc<std::sync::Mutex<Option<Value>>>) {
        let (address, api, fake) = start_paid_fake().await;
        (address, api, fake.order)
    }

    async fn start_paid_fake() -> (String, Arc<Api>, FakePayPal) {
        let (paypal_url, fake) = start_paypal().await;
        let (address, api) = start_api(Some(new_paypal(paypal_url))).await;
        (address, api, fake)
    }

    async fn paypal_tip_order(address: &str, api: &Api) -> String {
        paypal_tip_order_as(address, api).await.1
    }

    async fn paypal_tip_order_as(address: &str, api: &Api) -> (String, String) {
        paypal_tip_order_in(address, api, "USD").await
    }

    async fn paypal_tip_order_in(address: &str, api: &Api, currency: &str) -> (String, String) {
        let (session, _, id) = streamer(address, api).await;
        connect_paypal(api);
        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"currency": currency, "tips": {"amounts": [500]}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let orders_url = format!("http://{address}/pages/{id}/tips/orders");
        let order: Value = client
            .post(&orders_url)
            .json(&json!({"amount": 700, "user": "Viewer"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(order, json!({"orderId": "ORDER1"}));
        (session, format!("{orders_url}/ORDER1/capture"))
    }

    #[tokio::test]
    async fn recent_payments_are_listed_to_the_streamer() {
        let (address, api, _) = start_paid_fake().await;
        let (session, capture_url) = paypal_tip_order_as(&address, &api).await;
        let client = reqwest::Client::new();
        let recent = || async {
            client
                .get(format!("http://{address}/account/recent?kind=tip"))
                .bearer_auth(&session)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        };
        assert_eq!(recent().await, json!({"items": [], "more": false}));
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let listed = recent().await;
        assert_eq!(listed["more"], false);
        assert_eq!(listed["items"].as_array().unwrap().len(), 1);
        let tip = &listed["items"][0];
        assert_eq!(tip["id"], "ORDER1");
        assert_eq!(tip["user"], "Viewer");
        assert_eq!(tip["amount"], 700);
        assert_eq!(tip["currency"], "USD");
        assert_eq!(tip["provider"], "paypal");
        assert_eq!(tip["status"], "waiting");
        assert!(tip["at"].as_u64().unwrap() > 0);
        assert!(tip.get("message").is_none());
        let id = api.accounts.get("1").unwrap().page.id;
        let moderated: Value = client
            .get(format!(
                "http://{address}/account/pages/{id}/recent?kind=tip"
            ))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(moderated["items"][0]["id"], "ORDER1");
        assert!(moderated["items"][0].get("provider").is_none());
        let devices_url = format!("http://{address}/account/devices");
        let account: Value = client
            .post(&devices_url)
            .bearer_auth(&session)
            .json(&json!({"name": "Backup"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let main_name = account["devices"][0]["name"].clone();
        let backup_id = account["devices"][1]["id"].as_str().unwrap().to_string();
        assert_eq!(
            recent().await["items"][0]["devices"],
            json!([
                {"name": main_name, "received": false},
                {"name": "Backup", "received": false},
            ])
        );
        let key = api.accounts.get("1").unwrap().page.devices[0].key.clone();
        let mut socket = connect(&address, &key).await;
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"]["tip"]["requestId"], "ORDER1");
        socket
            .send(Message::text(
                json!({"response": {"id": request["request"]["id"], "result": {"ok": {}}}})
                    .to_string(),
            ))
            .await
            .unwrap();
        for _ in 0..100 {
            if api.deliveries.get("ORDER1").unwrap().delivered.len() == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let tip = &recent().await["items"][0];
        assert_eq!(tip["status"], "waiting");
        assert_eq!(
            tip["devices"],
            json!([
                {"name": main_name, "received": true},
                {"name": "Backup", "received": false},
            ])
        );
        let response = client
            .post(format!("{devices_url}/{backup_id}/remove"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        wait_until_added(&api).await;
        let tip = &recent().await["items"][0];
        assert_eq!(tip["status"], "delivered");
        assert_eq!(
            tip["devices"],
            json!([{"name": main_name, "received": true}])
        );
        let response = client
            .get(format!("http://{address}/account/recent?kind=tip"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn recent_payments_can_be_sent_again() {
        let (address, api, _) = start_paid_fake().await;
        let (session, capture_url) = paypal_tip_order_as(&address, &api).await;
        let client = reqwest::Client::new();
        let status = || async {
            client
                .get(format!("http://{address}/account/recent?kind=tip"))
                .bearer_auth(&session)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()["items"][0]["status"]
                .clone()
        };
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let key = api.accounts.get("1").unwrap().page.devices[0].key.clone();
        let mut socket = connect(&address, &key).await;
        let acknowledge = async |socket: &mut Socket, request_id: &str| {
            let request = receive(socket).await;
            assert_eq!(request["request"]["data"]["tip"]["requestId"], request_id);
            socket
                .send(Message::text(
                    json!({"response": {"id": request["request"]["id"], "result": {"ok": {}}}})
                        .to_string(),
                ))
                .await
                .unwrap();
            wait_until_added(&api).await;
        };
        acknowledge(&mut socket, "ORDER1").await;
        assert_eq!(status().await, "delivered");
        let id = api.accounts.get("1").unwrap().page.id;
        for (n, url) in [
            format!("http://{address}/account/recent/ORDER1/resend"),
            format!("http://{address}/account/pages/{id}/recent/ORDER1/resend"),
        ]
        .into_iter()
        .enumerate()
        {
            let response = client
                .post(&url)
                .bearer_auth(&session)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::ACCEPTED);
            acknowledge(&mut socket, &format!("ORDER1-{}", n + 1)).await;
            assert_eq!(status().await, "delivered");
        }
        for url in [
            format!("http://{address}/account/recent/ORDER2/resend"),
            format!("http://{address}/account/pages/OTHER/recent/ORDER1/resend"),
        ] {
            let response = client
                .post(&url)
                .bearer_auth(&session)
                .send()
                .await
                .unwrap();
            assert!(
                [StatusCode::NOT_FOUND, StatusCode::FORBIDDEN].contains(&response.status()),
                "{url}"
            );
        }
        let response = client
            .post(format!("http://{address}/account/recent/ORDER1/resend"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(status().await, "delivered");
    }

    #[derive(Clone, Copy, Debug)]
    enum Fault {
        None,
        LostAnswer,
        LostAnswerAndLookups,
        ProviderDown,
        Refused,
        PendingReview,
        AlreadyCaptured,
        DatabaseReadOnly,
        CrashedAfterCapture,
    }

    async fn tip_delivered_once(
        mut socket: tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        address: &str,
        key: &str,
        api: &Api,
        currency: &str,
    ) {
        let tip = json!({"tip": {
            "name": "Viewer",
            "message": "",
            "amount": "7.00",
            "currency": currency,
            "requestId": "ORDER1",
        }});
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
        assert_eq!(api.deliveries.pending().len(), 1);
        let mut socket = connect(address, key).await;
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], tip);
        answer(&mut socket, &request, Value::Null).await;
        wait_until_added(api).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(200), receive(&mut socket))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn paid_tips_always_reach_the_queue_exactly_once() {
        for fault in [
            Fault::None,
            Fault::LostAnswer,
            Fault::LostAnswerAndLookups,
            Fault::ProviderDown,
            Fault::Refused,
            Fault::PendingReview,
            Fault::AlreadyCaptured,
            Fault::DatabaseReadOnly,
            Fault::CrashedAfterCapture,
        ] {
            let (address, api, fake) = start_paid_fake().await;
            let currency = match fault {
                Fault::PendingReview => "EUR",
                _ => "USD",
            };
            let (session, capture_url) = paypal_tip_order_in(&address, &api, currency).await;
            let client = reqwest::Client::new();
            let capture = || async { client.post(&capture_url).send().await.unwrap() };
            let not_paid = |api: &Api, fake: &FakePayPal| {
                assert!(!*fake.captured.lock().unwrap(), "{fault:?}");
                assert!(api.deliveries.pending().is_empty(), "{fault:?}");
            };
            match fault {
                Fault::None => {
                    assert_eq!(capture().await.status(), StatusCode::ACCEPTED, "{fault:?}");
                }
                Fault::LostAnswer => {
                    *fake.lose_capture.lock().unwrap() = true;
                    assert_eq!(capture().await.status(), StatusCode::ACCEPTED, "{fault:?}");
                }
                Fault::LostAnswerAndLookups => {
                    *fake.lose_capture.lock().unwrap() = true;
                    *fake.lookups_fail.lock().unwrap() = true;
                    assert_eq!(
                        capture().await.status(),
                        StatusCode::BAD_GATEWAY,
                        "{fault:?}"
                    );
                    assert!(*fake.captured.lock().unwrap());
                    assert!(api.deliveries.pending().is_empty());
                    *fake.lookups_fail.lock().unwrap() = false;
                    settle(&api, Duration::ZERO).await;
                }
                Fault::ProviderDown => {
                    *fake.fail_capture.lock().unwrap() = true;
                    assert_eq!(
                        capture().await.status(),
                        StatusCode::BAD_GATEWAY,
                        "{fault:?}"
                    );
                    not_paid(&api, &fake);
                    *fake.fail_capture.lock().unwrap() = false;
                    assert_eq!(capture().await.status(), StatusCode::ACCEPTED, "{fault:?}");
                }
                Fault::Refused => {
                    *fake.refuse_capture.lock().unwrap() = true;
                    assert_eq!(
                        capture().await.status(),
                        StatusCode::BAD_GATEWAY,
                        "{fault:?}"
                    );
                    not_paid(&api, &fake);
                    *fake.refuse_capture.lock().unwrap() = false;
                    assert_eq!(capture().await.status(), StatusCode::ACCEPTED, "{fault:?}");
                }
                Fault::PendingReview => {
                    assert_eq!(
                        capture().await.status(),
                        StatusCode::PAYMENT_REQUIRED,
                        "{fault:?}"
                    );
                    assert!(api.deliveries.pending().is_empty());
                    settle(&api, Duration::ZERO).await;
                    assert!(api.deliveries.pending().is_empty());
                    *fake.approved.lock().unwrap() = true;
                    settle(&api, Duration::ZERO).await;
                }
                Fault::AlreadyCaptured => {
                    *fake.captured.lock().unwrap() = true;
                    assert_eq!(capture().await.status(), StatusCode::ACCEPTED, "{fault:?}");
                }
                Fault::DatabaseReadOnly => {
                    api.orders.read_only(true);
                    assert_eq!(
                        capture().await.status(),
                        StatusCode::SERVICE_UNAVAILABLE,
                        "{fault:?}"
                    );
                    not_paid(&api, &fake);
                    api.orders.read_only(false);
                    assert_eq!(capture().await.status(), StatusCode::ACCEPTED, "{fault:?}");
                }
                Fault::CrashedAfterCapture => {
                    assert!(
                        api.orders
                            .transition("ORDER1", Stage::Created, Stage::Capturing)
                            .unwrap()
                    );
                    *fake.captured.lock().unwrap() = true;
                    assert!(api.deliveries.pending().is_empty());
                    settle(&api, Duration::ZERO).await;
                }
            }
            assert!(*fake.captured.lock().unwrap(), "{fault:?}");
            let pending = api.deliveries.pending();
            assert_eq!(pending.len(), 1, "{fault:?}");
            assert_eq!(pending[0].request_id, "ORDER1", "{fault:?}");
            assert_eq!(
                pending[0].item,
                deliveries::Item::Tip {
                    user: "Viewer".to_string(),
                    message: String::new(),
                    amount: 700,
                    currency: currency.to_string(),
                },
                "{fault:?}"
            );
            assert_eq!(
                api.orders.get("ORDER1").unwrap().0,
                Stage::Captured,
                "{fault:?}"
            );
            assert_eq!(capture().await.status(), StatusCode::ACCEPTED, "{fault:?}");
            settle(&api, Duration::ZERO).await;
            assert_eq!(api.deliveries.pending().len(), 1, "{fault:?}");
            let key = api.accounts.get("1").unwrap().page.devices[0].key.clone();
            let socket = connect(&address, &key).await;
            tip_delivered_once(socket, &address, &key, &api, currency).await;
            let recent: Value = client
                .get(format!("http://{address}/account/recent?kind=tip"))
                .bearer_auth(&session)
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            assert_eq!(recent["items"][0]["status"], "delivered", "{fault:?}");
        }
    }

    #[tokio::test]
    async fn tips_reach_every_device_but_stop_waiting_for_removed_ones() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        let devices_url = format!("http://{address}/account/devices");
        let account: Value = client
            .post(&devices_url)
            .bearer_auth(&session)
            .json(&json!({"name": "Backup"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let backup_id = account["devices"][1]["id"].as_str().unwrap().to_string();
        for request_id in ["TIP1", "TIP2"] {
            api.deliveries.add_test(
                id.clone(),
                request_id.to_string(),
                deliveries::Item::Tip {
                    user: "Viewer".to_string(),
                    message: String::new(),
                    amount: 500,
                    currency: "USD".to_string(),
                },
            );
        }
        let mut main = connect(&address, &key).await;
        for request_id in ["TIP1", "TIP2"] {
            let request = receive(&mut main).await;
            assert_eq!(request["request"]["data"]["tip"]["requestId"], request_id);
            answer(&mut main, &request, Value::Null).await;
        }
        for _ in 0..100 {
            if api
                .deliveries
                .pending()
                .iter()
                .all(|delivery| delivery.delivered == vec![id.clone()])
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(api.deliveries.pending().len(), 2);
        assert!(
            tokio::time::timeout(Duration::from_millis(200), receive(&mut main))
                .await
                .is_err()
        );
        let response = client
            .post(format!("{devices_url}/{backup_id}/remove"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        wait_until_added(&api).await;
    }

    #[tokio::test]
    async fn nothing_is_sent_to_a_disabled_device() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        let devices_url = format!("http://{address}/account/devices");
        let account: Value = client
            .post(&devices_url)
            .bearer_auth(&session)
            .json(&json!({"name": "Backup"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let backup_id = account["devices"][1]["id"].as_str().unwrap().to_string();
        let backup_key = account["devices"][1]["key"].as_str().unwrap().to_string();
        let response = client
            .post(format!("{devices_url}/{backup_id}/enabled"))
            .bearer_auth(&session)
            .json(&json!({"enabled": false}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let account: Value = response.json().await.unwrap();
        assert_eq!(account["devices"][1]["enabled"], false);
        let response = client
            .post(format!("{devices_url}/x/enabled"))
            .bearer_auth(&session)
            .json(&json!({"enabled": false}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let mut backup = connect(&address, &backup_key).await;
        let settings: Value = client
            .get(format!("http://{address}/pages/{id}/spin"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["connected"], false);
        api.deliveries.add_test(
            id.clone(),
            "TIP1".to_string(),
            deliveries::Item::Tip {
                user: "Viewer".to_string(),
                message: String::new(),
                amount: 500,
                currency: "USD".to_string(),
            },
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(200), receive(&mut backup))
                .await
                .is_err()
        );
        let mut main = connect(&address, &key).await;
        let request = receive(&mut main).await;
        assert_eq!(request["request"]["data"]["tip"]["requestId"], "TIP1");
        answer(&mut main, &request, Value::Null).await;
        wait_until_added(&api).await;
        let response = client
            .post(format!("{devices_url}/{backup_id}/enabled"))
            .bearer_auth(&session)
            .json(&json!({"enabled": true}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            tokio::time::timeout(Duration::from_millis(200), receive(&mut backup))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn lost_capture_answers_are_settled_right_away() {
        let (address, api, fake) = start_paid_fake().await;
        let capture_url = paypal_tip_order(&address, &api).await;
        *fake.lose_capture.lock().unwrap() = true;
        let client = reqwest::Client::new();
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(api.deliveries.pending().len(), 1);
        assert_eq!(api.orders.get("ORDER1").unwrap().0, Stage::Captured);
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(api.deliveries.pending().len(), 1);
    }

    #[tokio::test]
    async fn lost_capture_answers_are_settled_later() {
        let (address, api, fake) = start_paid_fake().await;
        let capture_url = paypal_tip_order(&address, &api).await;
        *fake.lose_capture.lock().unwrap() = true;
        *fake.lookups_fail.lock().unwrap() = true;
        let client = reqwest::Client::new();
        let unknown = "PayPal did not answer. If the money was taken, the tip will still be shown.";
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(response.text().await.unwrap(), unknown);
        assert!(api.deliveries.pending().is_empty());
        assert_eq!(api.orders.get("ORDER1").unwrap().0, Stage::Capturing);
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(response.text().await.unwrap(), unknown);
        settle(&api, Duration::ZERO).await;
        assert!(api.deliveries.pending().is_empty());
        *fake.lookups_fail.lock().unwrap() = false;
        settle(&api, Duration::from_secs(60)).await;
        assert!(api.deliveries.pending().is_empty());
        settle(&api, Duration::ZERO).await;
        assert_eq!(api.deliveries.pending().len(), 1);
        assert_eq!(api.orders.get("ORDER1").unwrap().0, Stage::Captured);
        assert!(
            api.counters
                .all()
                .iter()
                .any(|counter| counter.name == "tipPayments" && counter.total == 1)
        );
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(api.deliveries.pending().len(), 1);
        let counters = api.counters.all();
        let count = |name: &str| {
            counters
                .iter()
                .find(|counter| counter.name == name)
                .map_or(0, |counter| counter.total)
        };
        assert_eq!(count("tipPayments"), 1);
        assert_eq!(count("paypalOrders"), 1);
        assert_eq!(count("paypalPaymentsFailed:unknown"), 1);
        assert_eq!(count("paypalPayments"), 1);
    }

    #[tokio::test]
    async fn failed_captures_can_be_retried() {
        let (address, api, fake) = start_paid_fake().await;
        let capture_url = paypal_tip_order(&address, &api).await;
        *fake.fail_capture.lock().unwrap() = true;
        let client = reqwest::Client::new();
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            response.text().await.unwrap(),
            "PayPal could not complete the payment."
        );
        assert!(api.deliveries.pending().is_empty());
        assert_eq!(api.orders.get("ORDER1").unwrap().0, Stage::Created);
        *fake.fail_capture.lock().unwrap() = false;
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(api.deliveries.pending().len(), 1);
    }

    #[tokio::test]
    async fn captures_need_a_writable_database_before_taking_money() {
        let (address, api, fake) = start_paid_fake().await;
        let capture_url = paypal_tip_order(&address, &api).await;
        api.orders.read_only(true);
        let client = reqwest::Client::new();
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response.text().await.unwrap(),
            "Payments are unavailable right now, so no money was taken. Try again later."
        );
        assert!(!*fake.captured.lock().unwrap());
        assert_eq!(api.orders.get("ORDER1").unwrap().0, Stage::Created);
        api.orders.read_only(false);
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert!(*fake.captured.lock().unwrap());
        assert_eq!(api.deliveries.pending().len(), 1);
    }

    #[tokio::test]
    async fn refused_captures_go_back_to_created() {
        let (address, api, fake) = start_paid_fake().await;
        let capture_url = paypal_tip_order(&address, &api).await;
        *fake.refuse_capture.lock().unwrap() = true;
        let client = reqwest::Client::new();
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            response.text().await.unwrap(),
            "PayPal could not complete the payment."
        );
        assert!(api.deliveries.pending().is_empty());
        assert_eq!(api.orders.get("ORDER1").unwrap().0, Stage::Created);
        assert!(!*fake.captured.lock().unwrap());
        *fake.refuse_capture.lock().unwrap() = false;
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(api.deliveries.pending().len(), 1);
    }

    #[tokio::test]
    async fn gone_orders_are_forgotten_when_settling() {
        let (address, api, fake) = start_paid_fake().await;
        let capture_url = paypal_tip_order(&address, &api).await;
        *fake.lose_capture.lock().unwrap() = true;
        *fake.lookups_fail.lock().unwrap() = true;
        let client = reqwest::Client::new();
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(api.orders.get("ORDER1").unwrap().0, Stage::Capturing);
        *fake.lookups_fail.lock().unwrap() = false;
        *fake.gone.lock().unwrap() = true;
        settle(&api, Duration::ZERO).await;
        assert_eq!(api.orders.get("ORDER1"), None);
        assert!(api.deliveries.pending().is_empty());
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::GONE);
    }

    #[tokio::test]
    async fn spin_captures_need_the_wheel() {
        let (address, api, _) = start_paid().await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        connect_paypal(&api);
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"currency": "usd", "paywall": null, "spin": {
                "options": [{"spins": 2, "price": 800}],
            }}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let wheel = json!({"event": {"data": {"wheelOfLuck": {"wheel": {"options": [
            {"text": "Yes", "weight": 1},
        ]}}}}});
        let mut socket = connect(&address, &key).await;
        socket.send(Message::text(wheel.to_string())).await.unwrap();
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        let orders_url = format!("http://{address}/pages/{id}/spin/orders");
        let order: Value = client
            .post(&orders_url)
            .json(&json!({"user": "Viewer", "spins": 2}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(order, json!({"orderId": "ORDER1"}));
        socket.close(None).await.unwrap();
        for _ in 0..100 {
            if !api.remote_control.is_connected(&id) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let capture_url = format!("{orders_url}/ORDER1/capture");
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response.text().await.unwrap(),
            "The streamer's wheel is gone, so your payment was not taken."
        );
        assert_eq!(api.orders.get("ORDER1").unwrap().0, Stage::Created);
        assert!(api.deliveries.pending().is_empty());
        let mut socket = connect(&address, &key).await;
        socket.send(Message::text(wheel.to_string())).await.unwrap();
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"]["spinWheelOfLuck"]["requestId"],
            "ORDER1"
        );
    }

    #[tokio::test]
    async fn recent_lists_are_paged_per_kind_and_show_songs_not_added() {
        let (address, api) = start_api(None).await;
        let (session, _, id) = streamer(&address, &api).await;
        let order = |item: Item, price: u32| Order {
            streamer: id.clone(),
            item,
            user: "Viewer".to_string(),
            payee: Payee::PayPal("MERCHANT00001".to_string()),
            price,
            currency: "USD".to_string(),
        };
        for n in 0..45 {
            let order_id = format!("TIP{n}");
            api.orders
                .insert(
                    &order_id,
                    &order(
                        Item::Tip {
                            message: Some(format!("m{n}")),
                        },
                        100 + n,
                    ),
                )
                .unwrap();
            api.orders.captured(&order_id, |_| Ok(())).unwrap();
        }
        api.orders
            .insert(
                "SONG1",
                &order(
                    Item::Song {
                        link: LINK.to_string(),
                        name: "Song".to_string(),
                        message: Some("Play it loud!".to_string()),
                    },
                    250,
                ),
            )
            .unwrap();
        api.orders
            .captured("SONG1", |connection| {
                deliveries::insert(
                    connection,
                    &id,
                    "SONG1",
                    &deliveries::Item::Song {
                        link: LINK.to_string(),
                        user: "Viewer".to_string(),
                        message: "Play it loud!".to_string(),
                    },
                )?;
                connection.execute(
                    "UPDATE deliveries SET failure = 'not-found' WHERE request_id = 'SONG1'",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        api.orders
            .insert(
                "SPIN1",
                &order(
                    Item::Spin {
                        spins: 2,
                        message: Some("Go!".to_string()),
                    },
                    800,
                ),
            )
            .unwrap();
        api.orders
            .captured("SPIN1", |connection| {
                deliveries::insert(
                    connection,
                    &id,
                    "SPIN1",
                    &deliveries::Item::Spin {
                        user: "Viewer".to_string(),
                        message: "Go!".to_string(),
                        spins: 2,
                    },
                )
            })
            .unwrap();
        let client = reqwest::Client::new();
        let recent = |query: String| {
            let request = client
                .get(format!("http://{address}/account/recent?{query}"))
                .bearer_auth(&session);
            async move { request.send().await.unwrap() }
        };
        let response = recent("kind=gift".to_string()).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let mut ids = Vec::new();
        let mut before = String::new();
        for (expected, more) in [(20, true), (20, true), (5, false)] {
            let listed: Value = recent(format!("kind=tip{before}"))
                .await
                .json()
                .await
                .unwrap();
            let items = listed["items"].as_array().unwrap();
            assert_eq!(items.len(), expected);
            assert_eq!(listed["more"], more);
            ids.extend(
                items
                    .iter()
                    .map(|item| item["id"].as_str().unwrap().to_string()),
            );
            before = format!("&before={}", ids.last().unwrap());
        }
        assert_eq!(
            ids,
            (0..45).rev().map(|n| format!("TIP{n}")).collect::<Vec<_>>()
        );
        let listed: Value = recent("kind=tip&limit=5".to_string())
            .await
            .json()
            .await
            .unwrap();
        let tips = listed["items"].as_array().unwrap();
        assert_eq!(tips.len(), 5);
        assert_eq!(listed["more"], true);
        assert_eq!(tips[0]["id"], "TIP44");
        assert_eq!(tips[0]["message"], "m44");
        assert_eq!(tips[0]["amount"], 144);
        assert_eq!(tips[0]["status"], "delivered");
        let listed: Value = recent("kind=song".to_string()).await.json().await.unwrap();
        assert_eq!(listed["items"].as_array().unwrap().len(), 1);
        let song = &listed["items"][0];
        assert_eq!(song["name"], "Song");
        assert_eq!(song["link"], LINK);
        assert_eq!(song["message"], "Play it loud!");
        assert_eq!(song["amount"], 250);
        assert_eq!(song["status"], "notAdded");
        let device = api.accounts.of_page(&id).unwrap().page.devices[0]
            .name
            .clone();
        assert_eq!(
            song["devices"],
            json!([{"name": device, "received": false}])
        );
        let listed: Value = recent("kind=spin".to_string()).await.json().await.unwrap();
        assert_eq!(listed["items"].as_array().unwrap().len(), 1);
        let spin = &listed["items"][0];
        assert_eq!(spin["spins"], 2);
        assert_eq!(spin["message"], "Go!");
        assert_eq!(spin["status"], "waiting");
        assert_eq!(
            spin["devices"],
            json!([{"name": device, "received": false}])
        );
        api.orders
            .insert(
                "OTHER",
                &Order {
                    streamer: "other".to_string(),
                    ..order(Item::Tip { message: None }, 100)
                },
            )
            .unwrap();
        api.orders.captured("OTHER", |_| Ok(())).unwrap();
        let listed: Value = recent("kind=tip&before=OTHER".to_string())
            .await
            .json()
            .await
            .unwrap();
        assert_eq!(listed, json!({"items": [], "more": false}));
        let listed: Value = recent("kind=tip".to_string()).await.json().await.unwrap();
        assert_eq!(listed["items"][0]["id"], "TIP44");
    }

    #[tokio::test]
    async fn processing_stripe_captures_are_settled_once_succeeded() {
        let (address, api, fake) = start_stripe_api().await;
        let (session, _, id) = streamer(&address, &api).await;
        connect_stripe(&api);
        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"currency": "eur", "tips": {"amounts": [500]}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let orders_url = format!("http://{address}/pages/{id}/tips/orders");
        let order: Value = client
            .post(&orders_url)
            .json(&json!({"amount": 700, "user": "Viewer", "provider": "stripe"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(order["orderId"], "pi_1");
        *fake.state.lock().unwrap() = "requires_capture";
        *fake.processing.lock().unwrap() = true;
        let capture_url = format!("{orders_url}/pi_1/capture");
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
        assert_eq!(
            response.text().await.unwrap(),
            "Stripe is still reviewing the payment. If Stripe approves it, the tip will be shown and the money reaches the streamer."
        );
        assert_eq!(api.orders.get("pi_1").unwrap().0, Stage::Capturing);
        settle(&api, Duration::ZERO).await;
        assert!(api.deliveries.pending().is_empty());
        *fake.state.lock().unwrap() = "succeeded";
        settle(&api, Duration::ZERO).await;
        assert_eq!(api.deliveries.pending().len(), 1);
        assert_eq!(api.orders.get("pi_1").unwrap().0, Stage::Captured);
    }

    #[tokio::test]
    async fn already_captured_orders_are_delivered() {
        let (address, api, fake) = start_paid_fake().await;
        let capture_url = paypal_tip_order(&address, &api).await;
        *fake.captured.lock().unwrap() = true;
        let response = reqwest::Client::new()
            .post(&capture_url)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(api.deliveries.pending().len(), 1);
        assert_eq!(api.orders.get("ORDER1").unwrap().0, Stage::Captured);
    }

    #[tokio::test]
    async fn lost_stripe_capture_answers_are_settled() {
        let (address, api, fake) = start_stripe_api().await;
        let (session, _, id) = streamer(&address, &api).await;
        connect_stripe(&api);
        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"currency": "eur", "tips": {"amounts": [500]}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let orders_url = format!("http://{address}/pages/{id}/tips/orders");
        let order: Value = client
            .post(&orders_url)
            .json(&json!({"amount": 700, "user": "Viewer", "provider": "stripe"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(order["orderId"], "pi_1");
        let capture_url = format!("{orders_url}/pi_1/capture");
        *fake.lose_capture.lock().unwrap() = true;
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
        assert_eq!(api.orders.get("pi_1").unwrap().0, Stage::Created);
        *fake.state.lock().unwrap() = "requires_capture";
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(api.deliveries.pending().len(), 1);
        assert_eq!(api.orders.get("pi_1").unwrap().0, Stage::Captured);
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
        let (address, _) =
            start_api_with(Some(new_paypal(paypal_url)), None, None, Some(pages)).await;
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
            }, "subSongs": null, "maxSongMinutes": null})
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

    fn create_account(api: &Api, id: &str, name: &str, login: &str) -> Account {
        let (key, page) = api.remote_control.new_key().unwrap();
        let device = Device {
            id: page.clone(),
            name: "Moblin".to_string(),
            key,
            enabled: true,
        };
        api.accounts
            .signed_in(id, name, login, None, Page::new(page, device))
            .unwrap()
    }

    fn channel_moderator(id: &str, login: &str, name: &str) -> ChannelModerator {
        ChannelModerator {
            id: id.to_string(),
            login: login.to_string(),
            name: name.to_string(),
            enabled: false,
        }
    }

    async fn streamer(_address: &str, api: &Api) -> (String, String, String) {
        let account = create_account(api, "1", "Erik", "erik");
        (
            api.auth.session("1", 0),
            account.page.devices[0].key.clone(),
            account.page.id,
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
                    .json(&json!({"song": "https://music.apple.com/us/song/x/1440818839", "user": "Viewer", "message": " Play it loud! "}))
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

    async fn song_requests<S>(socket: &mut S, enabled: bool)
    where
        S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
            + SinkExt<Message>
            + Unpin,
        <S as futures_util::Sink<Message>>::Error: std::fmt::Debug,
    {
        socket
            .send(Message::text(
                json!({"event": {"data": {"songRequests": {"enabled": enabled}}}}).to_string(),
            ))
            .await
            .unwrap();
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(socket).await, json!({"pong": {}}));
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
    async fn songs_longer_than_the_max_are_refused() {
        let (address, api, _) = start_paid().await;
        let (session, key, id) = streamer(&address, &api).await;
        connect_paypal(&api);
        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"paywall": {"price": 250}, "maxSongMinutes": 61}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let settings: Value = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"paywall": {"price": 250}, "maxSongMinutes": 5}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["maxSongMinutes"], 5);
        let songs: Value = client
            .get(format!("http://{address}/pages/{id}/songs"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(songs["maxSongMinutes"], 5);
        let mut socket = connect(&address, &key).await;
        let orders_url = format!("http://{address}/pages/{id}/songs/orders");
        for (duration, status) in [
            (None, StatusCode::OK),
            (Some(300), StatusCode::OK),
            (Some(301), StatusCode::UNPROCESSABLE_ENTITY),
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
                }
            });
            let request = receive(&mut socket).await;
            answer(
                &mut socket,
                &request,
                json!({"musicFind": {"song": {"id": "1440818839", "title": "Song", "duration": duration}}}),
            )
            .await;
            let response = created.await.unwrap();
            assert_eq!(response.status(), status);
            if status != StatusCode::OK {
                assert_eq!(
                    response.text().await.unwrap(),
                    "The streamer only takes songs up to 5 minutes long."
                );
            }
        }
        assert!(api.counters.all().iter().any(|counter| counter.name
            == "songOrdersFailed:over-max-length"
            && counter.total == 1));
    }

    #[tokio::test]
    async fn song_orders_are_rate_limited_per_page() {
        let (address, api, _) = start_paid().await;
        let (_, id) = paid_streamer(&address, &api).await;
        let client = reqwest::Client::new();
        let orders_url = format!("http://{address}/pages/{id}/songs/orders");
        for _ in 0..10 {
            let response = client
                .post(&orders_url)
                .json(&json!({"song": LINK}))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
        let response = client
            .post(&orders_url)
            .json(&json!({"song": LINK}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response.text().await.unwrap(),
            "Too many song requests on this page right now. Try again in a minute."
        );
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
            }, "subSongs": null, "maxSongMinutes": null})
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
            "Song request: Band - Song: Play it loud!"
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
                "message": "Play it loud!",
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
        assert_eq!(response.status(), StatusCode::ACCEPTED);
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
    async fn pending_captures_add_the_song_once_approved() {
        let (address, api, fake) = start_paid_fake().await;
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
        let capture_url = format!("{orders_url}/ORDER1/capture");
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
        assert_eq!(
            response.text().await.unwrap(),
            "PayPal is still reviewing the payment, so no song was added yet. It will be added if PayPal approves the payment."
        );
        assert!(api.deliveries.pending().is_empty());
        assert_eq!(api.orders.get("ORDER1").unwrap().0, Stage::Capturing);
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
        settle(&api, Duration::ZERO).await;
        assert!(api.deliveries.pending().is_empty());
        *fake.approved.lock().unwrap() = true;
        settle(&api, Duration::ZERO).await;
        assert_eq!(api.deliveries.pending().len(), 1);
        assert_eq!(api.orders.get("ORDER1").unwrap().0, Stage::Captured);
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"]["music"]["command"]["add"]["requestId"],
            "ORDER1"
        );
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
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
            "message": "Play it loud!",
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
            json!({"spin": null, "subSpins": null, "wheel": null, "connected": false})
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
            }, "subSpins": null, "wheel": null, "connected": false})
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
        assert_eq!(response.status(), StatusCode::ACCEPTED);
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
    async fn tests_are_rate_limited_per_page() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, _, _) = streamer(&address, &api).await;
        for _ in 0..10 {
            let response = client
                .post(format!("http://{address}/account/test/spin"))
                .bearer_auth(&session)
                .json(&json!({}))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::CONFLICT);
        }
        let response = client
            .post(format!("http://{address}/account/test/spin"))
            .bearer_auth(&session)
            .json(&json!({}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response.text().await.unwrap(),
            "Too many tests. Try again in a minute."
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
        for kind in ["song", "spin"] {
            let response = test(kind, json!({"song": LINK})).await.unwrap();
            assert_eq!(response.status(), StatusCode::CONFLICT);
            assert_eq!(response.text().await.unwrap(), "Connect Moblin first.");
        }
        let response = test("tip", json!({"message": " Testing "})).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let mut socket = connect(&address, &key).await;
        let request = receive(&mut socket).await;
        let tip = &request["request"]["data"]["tip"];
        assert_eq!(tip["name"], "Erik");
        assert_eq!(tip["message"], "Testing");
        assert_eq!(tip["amount"], "5.00");
        assert_eq!(tip["currency"], "USD");
        answer(&mut socket, &request, Value::Null).await;
        wait_until_added(&api).await;
        let recent: Value = client
            .get(format!("http://{address}/account/recent?kind=tip"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(recent, json!({"items": [], "more": false}));
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
        let created = tokio::spawn(test("song", json!({"song": LINK, "message": " Testing "})));
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
        assert_eq!(add["message"], "Testing");
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
        api.deliveries.add_test(
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
    async fn songs_not_found_stay_songs_and_are_not_retried() {
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
        let delivery = api.deliveries.get("ORDER1").unwrap();
        assert_eq!(
            delivery.item,
            deliveries::Item::Song {
                link: LINK.to_string(),
                user: "Viewer".to_string(),
                message: "Play it loud!".to_string(),
            }
        );
        assert_eq!(delivery.failure.as_deref(), Some("not-found"));
        assert!(
            tokio::time::timeout(Duration::from_millis(200), receive(&mut socket))
                .await
                .is_err()
        );
        assert!(
            api.counters
                .all()
                .iter()
                .any(|counter| counter.name == "songDeliveriesFailed:not-found"
                    && counter.total == 1)
        );
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
            )
            .route(
                "/subscriptions",
                get(
                    |headers: HeaderMap, Query(query): Query<HashMap<String, String>>| async move {
                        assert_eq!(headers[AUTHORIZATION], "Bearer u");
                        assert_eq!(headers["client-id"], "id");
                        assert_eq!(query["user_id"], "42");
                        match query["broadcaster_id"].as_str() {
                            "1" => Json(json!({"data": [{"tier": "2000"}]})).into_response(),
                            "3" => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                            _ => StatusCode::NOT_FOUND.into_response(),
                        }
                    },
                ),
            )
            .route(
                "/moderators",
                get({
                    let calls = Arc::new(AtomicUsize::new(0));
                    move |headers: HeaderMap, Query(query): Query<HashMap<String, String>>| {
                        let calls = calls.clone();
                        async move {
                            assert_eq!(headers[AUTHORIZATION], "Bearer u");
                            assert_eq!(query["broadcaster_id"], "42");
                            assert_eq!(query["first"], "100");
                            if calls.fetch_add(1, Ordering::SeqCst) >= 2 {
                                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                            }
                            match query.get("after").map(String::as_str) {
                                None => Json(json!({
                                    "data": [{"user_id": "7", "user_login": "Seven", "user_name": "Seven"}],
                                    "pagination": {"cursor": "more"},
                                })),
                                Some("more") => Json(json!({
                                    "data": [{"user_id": "8", "user_login": "eight", "user_name": ""}],
                                    "pagination": {},
                                })),
                                Some(after) => panic!("unexpected cursor {after}"),
                            }
                            .into_response()
                        }
                    }
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

    fn twitch_app(twitch: &str) -> OAuthApp {
        OAuthApp {
            client_id: "id".to_string(),
            client_secret: "secret".to_string(),
            authorize_url: format!("{twitch}/authorize"),
            token_url: format!("{twitch}/token"),
            users_url: format!("{twitch}/users"),
            subscriptions_url: format!("{twitch}/subscriptions"),
            moderators_url: format!("{twitch}/moderators"),
        }
    }

    #[tokio::test]
    async fn viewers_sign_in_with_their_subscription_tier() {
        let twitch = start_twitch().await;
        let (address, api) = start_api_with(None, None, Some(twitch_app(&twitch)), None).await;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let (_, _, id) = streamer(&address, &api).await;
        let response = client
            .get(format!("http://{address}/auth/twitch/login?page=unknown"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let sign_in = async |page: &str| {
            let response = client
                .get(format!("http://{address}/auth/twitch/login?page={page}"))
                .send()
                .await
                .unwrap();
            let login = reqwest::Url::parse(&location(&response)).unwrap();
            let query: HashMap<_, _> = login.query_pairs().into_owned().collect();
            assert_eq!(query["scope"], "user:read:subscriptions");
            location(
                &client
                    .get(format!(
                        "http://{address}/auth/twitch/callback?code=c&state={}",
                        query["state"]
                    ))
                    .header("cookie", format!("moblin_sign_in_state={}", query["state"]))
                    .send()
                    .await
                    .unwrap(),
            )
        };
        let page = sign_in(&id).await;
        let session = page
            .strip_prefix("https://moblin.app/twitch/erik#viewer=")
            .unwrap();
        let viewer = api.auth.viewer_of(session).unwrap();
        assert_eq!(
            (
                viewer.id.as_str(),
                viewer.name.as_str(),
                viewer.page.as_str(),
                viewer.tier
            ),
            ("42", "Erik", id.as_str(), Some(2))
        );
        assert_eq!(api.accounts.get("42"), None);
        assert_eq!(api.auth.session_of(session), None);
        let other = create_account(&api, "2", "Other", "other").page.id;
        let other = other.as_str();
        let page = sign_in(other).await;
        let session = page
            .strip_prefix("https://moblin.app/twitch/other#viewer=")
            .unwrap();
        assert_eq!(api.auth.viewer_of(session).unwrap().tier, None);
        let third = create_account(&api, "3", "Third", "third").page.id;
        let third = third.as_str();
        assert_eq!(
            sign_in(third).await,
            "https://moblin.app/twitch/third#viewerSignInFailed"
        );
        let response = client
            .get(format!("http://{address}/auth/twitch/login?page={id}"))
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
                        "http://{address}/auth/twitch/callback?error=access_denied&state={state}"
                    ))
                    .header("cookie", format!("moblin_sign_in_state={state}"))
                    .send()
                    .await
                    .unwrap()
            ),
            "https://moblin.app/twitch/erik#viewerSignInFailed"
        );
        let response = client
            .get(format!("http://{address}/auth/twitch/login?page={id}"))
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
            "https://moblin.app/twitch/erik#viewerSignInFailed"
        );
        assert_eq!(
            location(
                &client
                    .get(format!(
                        "http://{address}/auth/twitch/callback?error=access_denied"
                    ))
                    .send()
                    .await
                    .unwrap()
            ),
            "https://moblin.app/dashboard/#signInFailed"
        );
    }

    async fn send_subscriber_song(
        address: &str,
        id: &str,
        session: &str,
        song: &str,
    ) -> reqwest::Response {
        reqwest::Client::new()
            .post(format!("http://{address}/pages/{id}/songs/subscriber"))
            .bearer_auth(session)
            .json(&json!({"song": song, "message": " For you "}))
            .send()
            .await
            .unwrap()
    }

    async fn subscriber_song_found<S>(
        socket: &mut S,
        address: &str,
        id: &str,
        session: &str,
        duration: Option<u32>,
    ) -> reqwest::Response
    where
        S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
            + SinkExt<Message>
            + Unpin,
        <S as futures_util::Sink<Message>>::Error: std::fmt::Debug,
    {
        let sent = tokio::spawn({
            let (address, id, session) = (address.to_string(), id.to_string(), session.to_string());
            async move { send_subscriber_song(&address, &id, &session, LINK).await }
        });
        let request = receive(socket).await;
        assert_eq!(
            request["request"]["data"],
            json!({"music": {"command": {"find": {"title": LINK}}}})
        );
        answer(
            socket,
            &request,
            json!({"musicFind": {"song": {"id": "1440818839", "title": "Song", "artist": "Band", "duration": duration}}}),
        )
        .await;
        sent.await.unwrap()
    }

    #[tokio::test]
    async fn subscriber_songs_longer_than_the_max_are_refused() {
        let (address, api) = start_api(None).await;
        let (session, key, id) = streamer(&address, &api).await;
        let response = reqwest::Client::new()
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({
                "subSongs": {"tier1": 0, "tier2": 0, "tier3": 3},
                "maxSongMinutes": 5,
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let tier3 = api.auth.viewer_session(
            "31".to_string(),
            "Viewer 31".to_string(),
            id.clone(),
            Some(3),
        );
        let mut socket = connect(&address, &key).await;
        let response = subscriber_song_found(&mut socket, &address, &id, &tier3, Some(301)).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            response.text().await.unwrap(),
            "The streamer only takes songs up to 5 minutes long."
        );
        let response = subscriber_song_found(&mut socket, &address, &id, &tier3, Some(300)).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(response.json::<Value>().await.unwrap(), json!({"left": 2}));
        let request = receive(&mut socket).await;
        answer(&mut socket, &request, music(json!([]))).await;
        wait_until_added(&api).await;
        assert!(
            api.counters
                .all()
                .iter()
                .any(|counter| counter.name == "subSongsFailed:over-max-length"
                    && counter.total == 1)
        );
    }

    #[tokio::test]
    async fn twitch_subscribers_request_songs_every_month() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        let settings = |body: Value| {
            client
                .post(format!("http://{address}/account/settings"))
                .bearer_auth(&session)
                .json(&body)
                .send()
        };
        let viewer = |viewer: &str, page: &str, tier: Option<u8>| {
            api.auth.viewer_session(
                viewer.to_string(),
                format!("Viewer {viewer}"),
                page.to_string(),
                tier,
            )
        };
        let tier1 = viewer("11", &id, Some(1));
        let response = send_subscriber_song(&address, &id, &tier1, LINK).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        for sub_songs in [
            json!({"tier1": 0, "tier2": 0, "tier3": 0}),
            json!({"tier1": 101, "tier2": 0, "tier3": 0}),
        ] {
            let response = settings(json!({"subSongs": sub_songs})).await.unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
        let response = settings(json!({"subSongs": {"tier1": 1, "tier2": 0, "tier3": 3}}))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.json::<Value>().await.unwrap()["subSongs"],
            json!({"tier1": 1, "tier2": 0, "tier3": 3})
        );
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
            json!({"paywall": null, "subSongs": {"tier1": 1, "tier2": 0, "tier3": 3}, "maxSongMinutes": null})
        );
        let status = async |session: &str| {
            let response = client
                .get(format!("http://{address}/pages/{id}/songs/subscriber"))
                .bearer_auth(session)
                .send()
                .await
                .unwrap();
            (
                response.status(),
                response.json::<Value>().await.unwrap_or_default(),
            )
        };
        assert_eq!(
            status(&tier1).await,
            (
                StatusCode::OK,
                json!({"name": "Viewer 11", "tier": 1, "allowance": 1, "left": 1})
            )
        );
        assert_eq!(status("garbage").await.0, StatusCode::UNAUTHORIZED);
        assert_eq!(
            status(&viewer("11", "other", Some(1))).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status(&api.auth.session("1", 0)).await.0,
            StatusCode::UNAUTHORIZED
        );
        let response = send_subscriber_song(&address, &id, &viewer("12", &id, None), LINK).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = send_subscriber_song(&address, &id, &viewer("13", &id, Some(2)), LINK).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response.text().await.unwrap(),
            "Tier 2 subscribers can't request songs here."
        );
        let response = send_subscriber_song(&address, &id, &tier1, "nope").await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let response = send_subscriber_song(&address, &id, &tier1, LINK).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let mut socket = connect(&address, &key).await;
        let response = subscriber_song_found(&mut socket, &address, &id, &tier1, None).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(response.json::<Value>().await.unwrap(), json!({"left": 0}));
        let request = receive(&mut socket).await;
        let add = &request["request"]["data"]["music"]["command"]["add"];
        assert_eq!(add["title"], LINK);
        assert_eq!(add["user"], "Viewer 11");
        assert_eq!(add["message"], "For you");
        let request_id = add["requestId"].as_str().unwrap().to_string();
        answer(&mut socket, &request, music(json!([]))).await;
        wait_until_added(&api).await;
        let response = send_subscriber_song(&address, &id, &tier1, LINK).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.text().await.unwrap(),
            "You have used all your minutes this month."
        );
        let tier3 = viewer("31", &id, Some(3));
        let response = subscriber_song_found(&mut socket, &address, &id, &tier3, Some(301)).await;
        assert_eq!(response.json::<Value>().await.unwrap(), json!({"left": 1}));
        let request = receive(&mut socket).await;
        answer(&mut socket, &request, music(json!([]))).await;
        let response = subscriber_song_found(&mut socket, &address, &id, &tier3, Some(301)).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.text().await.unwrap(),
            "The song is longer than the minutes you have left this month."
        );
        let response = subscriber_song_found(&mut socket, &address, &id, &tier3, Some(300)).await;
        assert_eq!(response.json::<Value>().await.unwrap(), json!({"left": 0}));
        let request = receive(&mut socket).await;
        answer(&mut socket, &request, music(json!([]))).await;
        wait_until_added(&api).await;
        let response = send_subscriber_song(&address, &id, &tier3, LINK).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let response = settings(json!({"subSongs": {"tier1": 1, "tier2": 0, "tier3": 4}}))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(status(&tier3).await.1["left"], 1);
        let response = settings(json!({"subSongs": {"tier1": 1, "tier2": 0, "tier3": 2}}))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(status(&tier3).await.1["left"], 0);
        api.allowances.last_month();
        assert_eq!(status(&tier1).await.1["left"], 1);
        assert_eq!(status(&tier3).await.1["left"], 2);
        let recent: Value = client
            .get(format!("http://{address}/account/recent?kind=song"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let items = recent["items"].as_array().unwrap();
        assert_eq!(items.len(), 3);
        let first = items.last().unwrap();
        assert_eq!(first["id"], request_id.as_str());
        assert_eq!(first["user"], "Viewer 11");
        assert_eq!(first["tier"], 1);
        assert_eq!(first["amount"], 0);
        assert_eq!(first["status"], "delivered");
        assert_eq!(items[0]["tier"], 3);
        let response = settings(json!({"subSongs": null})).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = send_subscriber_song(&address, &id, &tier1, LINK).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let counters = api.counters.all();
        let count = |name: &str| {
            counters
                .iter()
                .find(|counter| counter.name == name)
                .map_or(0, |counter| counter.total)
        };
        assert_eq!(count("subSongs"), 3);
        assert_eq!(count("subSongsFailed:no-minutes-left"), 2);
        assert_eq!(count("subSongsFailed:tier-excluded"), 1);
        assert_eq!(count("subSongsFailed:not-subscribed"), 1);
        assert_eq!(count("subSongsFailed:feature-off"), 2);
        assert_eq!(count("subSongsFailed:offline"), 1);
        assert_eq!(count("songDeliveries"), 3);
        assert_eq!(count("subSongsFailed:too-long"), 1);
    }

    #[tokio::test]
    async fn twitch_subscribers_spin_the_wheel_every_month() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        let viewer = |viewer: &str, tier: Option<u8>| {
            api.auth.viewer_session(
                viewer.to_string(),
                format!("Viewer {viewer}"),
                id.clone(),
                tier,
            )
        };
        let spin = async |session: &str| {
            client
                .post(format!("http://{address}/pages/{id}/spin/subscriber"))
                .bearer_auth(session)
                .json(&json!({"message": " Big one "}))
                .send()
                .await
                .unwrap()
        };
        let tier1 = viewer("11", Some(1));
        assert_eq!(spin(&tier1).await.status(), StatusCode::BAD_REQUEST);
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({
                "subSpins": {"tier1": 1, "tier2": 0, "tier3": 2},
                "subSongs": {"tier1": 1, "tier2": 0, "tier3": 2},
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let public: Value = client
            .get(format!("http://{address}/pages/{id}/spin"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            public["subSpins"],
            json!({"tier1": 1, "tier2": 0, "tier3": 2})
        );
        assert_eq!(public["spin"], Value::Null);
        assert_eq!(spin(&tier1).await.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            spin(&viewer("12", None)).await.status(),
            StatusCode::FORBIDDEN
        );
        let mut socket = connect(&address, &key).await;
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
        let response = spin(&tier1).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(response.json::<Value>().await.unwrap(), json!({"left": 0}));
        let request = receive(&mut socket).await;
        let sent = &request["request"]["data"]["spinWheelOfLuck"];
        assert_eq!(sent["name"], "Viewer 11");
        assert_eq!(sent["message"], "Big one");
        assert_eq!(sent["count"], 1);
        answer(&mut socket, &request, Value::Null).await;
        wait_until_added(&api).await;
        let response = spin(&tier1).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.text().await.unwrap(),
            "You have used all your spins this month."
        );
        let upgraded: Value = client
            .get(format!("http://{address}/pages/{id}/spin/subscriber"))
            .bearer_auth(viewer("11", Some(3)))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            upgraded,
            json!({"name": "Viewer 11", "tier": 3, "allowance": 2, "left": 1})
        );
        let status: Value = client
            .get(format!("http://{address}/pages/{id}/spin/subscriber"))
            .bearer_auth(viewer("31", Some(3)))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            status,
            json!({"name": "Viewer 31", "tier": 3, "allowance": 2, "left": 2})
        );
        let recent: Value = client
            .get(format!("http://{address}/account/recent?kind=spin"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(recent["items"][0]["user"], "Viewer 11");
        assert_eq!(recent["items"][0]["tier"], 1);
        assert_eq!(recent["items"][0]["spins"], 1);
        assert_eq!(recent["items"][0]["status"], "delivered");
        let counters = api.counters.all();
        let count = |name: &str| {
            counters
                .iter()
                .find(|counter| counter.name == name)
                .map_or(0, |counter| counter.total)
        };
        assert_eq!(count("subSpins"), 1);
        assert_eq!(count("subSpinsFailed:no-spins-left"), 1);
        assert_eq!(count("subSpinsFailed:offline"), 1);
        assert_eq!(count("spinDeliveries"), 1);
        let songs: Value = client
            .get(format!("http://{address}/pages/{id}/songs/subscriber"))
            .bearer_auth(&tier1)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(songs["left"], 1);
        let racer = viewer("13", Some(1));
        let (first, second) = tokio::join!(spin(&racer), spin(&racer));
        let mut statuses = [first.status(), second.status()];
        statuses.sort();
        assert_eq!(statuses, [StatusCode::ACCEPTED, StatusCode::CONFLICT]);
        let request = receive(&mut socket).await;
        answer(&mut socket, &request, Value::Null).await;
        wait_until_added(&api).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(200), receive(&mut socket))
                .await
                .is_err()
        );
        let tier3 = viewer("31", Some(3));
        assert_eq!(spin(&tier3).await.status(), StatusCode::ACCEPTED);
        let request = receive(&mut socket).await;
        let request_id = request["request"]["data"]["spinWheelOfLuck"]["requestId"].clone();
        drop(socket);
        let mut socket = connect(&address, &key).await;
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"]["spinWheelOfLuck"]["requestId"],
            request_id
        );
        answer(&mut socket, &request, Value::Null).await;
        wait_until_added(&api).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(200), receive(&mut socket))
                .await
                .is_err()
        );
        let recent: Value = client
            .get(format!("http://{address}/account/recent?kind=spin"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(recent["items"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn streamers_sign_in_and_manage_their_page() {
        let twitch = start_twitch().await;
        let (address, api) = start_api_with(None, None, Some(twitch_app(&twitch)), None).await;
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
            api.accounts.channel_moderators("42"),
            [
                ChannelModerator {
                    id: "8".to_string(),
                    login: "eight".to_string(),
                    name: "eight".to_string(),
                    enabled: false,
                },
                ChannelModerator {
                    id: "7".to_string(),
                    login: "seven".to_string(),
                    name: "Seven".to_string(),
                    enabled: false,
                },
            ]
        );
        create_account(&api, "1", "One", "one");
        api.accounts
            .set_channel_moderators("1", &[channel_moderator("42", "fortytwo", "42")]);
        api.accounts.enable_moderator("1", "42", true).unwrap();
        assert!(api.accounts.moderates("42", "1"));
        assert!(!api.accounts.moderates("42", "9"));
        let response = client
            .get(format!("http://{address}/auth/twitch/login"))
            .send()
            .await
            .unwrap();
        let again = reqwest::Url::parse(&location(&response)).unwrap();
        let again_cookie = response.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        let again_state = again
            .query_pairs()
            .find(|(name, _)| name == "state")
            .unwrap()
            .1
            .into_owned();
        let again = location(
            &client
                .get(format!(
                    "http://{address}/auth/twitch/callback?code=c&state={again_state}"
                ))
                .header("cookie", &again_cookie)
                .send()
                .await
                .unwrap(),
        );
        assert!(
            again.starts_with("https://moblin.app/dashboard/#session="),
            "{again}"
        );
        assert_eq!(api.accounts.channel_moderators("42").len(), 2);
        assert!(api.accounts.moderates("42", "1"));
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
        let key = key_of(&account);
        let page = account["settings"]["id"].as_str().unwrap();
        assert_eq!(remote_control::id_of(&key).as_deref(), Some(page));
        assert_eq!(account["name"], "Erik");
        assert_eq!(account["login"], "erik");
        assert_eq!(account["avatar"], "https://static-cdn.jtvnw.net/erik.png");
        assert_eq!(account["devices"][0]["id"], page);
        assert_eq!(account["devices"][0]["name"], "Moblin");
        assert_eq!(account["devices"][0]["enabled"], true);
        assert_eq!(account["devices"][0]["connected"], false);
        assert_eq!(account["devices"].as_array().unwrap().len(), 1);
        assert_eq!(account["musicDevice"], page);
        assert_eq!(account["wheelDevice"], page);
        assert_eq!(account["admin"], false);
        assert_eq!(account["premium"], false);
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
                "message": "Thanks for hanging out!",
                "accent": "pink",
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["message"], "Thanks for hanging out!");
        assert_eq!(settings["accent"], "pink");
        assert_eq!(settings["font"], Value::Null);
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
                "message": "Thanks for hanging out!",
                "accent": "pink",
                "font": null,
                "image": null,
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
            .get(format!("http://{address}/account"))
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
    async fn deleting_an_account_removes_everything_it_owns() {
        let (address, api) = start_api(None).await;
        let (session, key, id) = streamer(&address, &api).await;
        let moderator = create_account(&api, "21", "Mod 21", "mod21");
        api.accounts
            .set_channel_moderators("1", &[channel_moderator("21", "mod21", "Mod 21")]);
        api.accounts.enable_moderator("1", "21", true).unwrap();
        api.accounts
            .set_channel_moderators("21", &[channel_moderator("1", "erik", "Erik")]);
        api.accounts.enable_moderator("21", "1", true).unwrap();
        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"message": "Hi"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        api.deliveries.add_test(
            id.clone(),
            "r".to_string(),
            crate::deliveries::Item::Tip {
                user: "Erik".to_string(),
                message: String::new(),
                amount: 500,
                currency: "USD".to_string(),
            },
        );
        let socket = connect(&address, &key).await;
        assert!(api.remote_control.is_connected(&id));
        assert_eq!(api.deliveries.count(), 1);
        let response = client
            .post(format!("http://{address}/account/delete"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        drop(socket);
        assert!(!api.remote_control.is_connected(&id));
        assert_eq!(api.deliveries.count(), 0);
        assert!(api.accounts.get("1").is_none());
        assert!(api.accounts.page_of_login("erik").is_none());
        assert!(api.accounts.device_of_key(&key).is_none());
        assert!(api.accounts.channel_moderators("21").is_empty());
        assert!(api.accounts.moderated("21").is_empty());
        assert_eq!(api.accounts.get("21").unwrap(), moderator);
        for (table, column, value) in [
            ("pages", "id", id.as_str()),
            ("channel_moderators", "broadcaster", "1"),
            ("channel_moderators", "moderator", "1"),
            ("enabled_moderators", "broadcaster", "1"),
            ("enabled_moderators", "moderator", "1"),
        ] {
            let count: u64 = api
                .database
                .lock()
                .query_row(
                    &format!("SELECT count(*) FROM {table} WHERE {column} = ?1"),
                    [value],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0, "{table}");
        }
        for path in ["/account", "/account/delete"] {
            let response = client
                .request(
                    if path == "/account" {
                        Method::GET
                    } else {
                        Method::POST
                    },
                    format!("http://{address}{path}"),
                )
                .bearer_auth(&session)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
        for path in ["/pages/twitch/erik".to_string(), format!("/pages/{id}")] {
            let response = client
                .get(format!("http://{address}{path}"))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
        assert!(
            tokio_tungstenite::connect_async(format!(
                "ws://{address}/remote-control/streamer/{key}"
            ))
            .await
            .is_err()
        );
        let again = create_account(&api, "1", "Erik", "erik");
        assert_ne!(again.page.id, id);
        assert_eq!(api.accounts.page_of_login("erik"), Some(again.page.id));
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
            ("POST", "/account/devices"),
            ("POST", "/account/devices/choose"),
            ("POST", "/account/devices/d/key"),
            ("POST", "/account/devices/d/remove"),
            ("POST", "/account/devices/d/enabled"),
            ("POST", "/account/settings"),
            ("POST", "/account/paypal/connect"),
            ("POST", "/account/paypal/check"),
            ("POST", "/account/sign-out"),
            ("POST", "/account/delete"),
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
                    .json(&json!({"paywall": null, "name": "Moblin", "music": "d", "wheel": "d", "enabled": false}));
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
        assert_eq!(
            api.counters.all(),
            [Counter {
                name: "moblinConnectionsFailed:unknown-key".to_string(),
                today: 1,
                week: 1,
                total: 1,
            }]
        );
    }

    #[tokio::test]
    async fn banner_images() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let upload = |session: String, body: Vec<u8>| {
            client
                .post(format!("http://{address}/account/image"))
                .bearer_auth(session)
                .header("content-type", "image/png")
                .body(body)
                .send()
        };
        let (session, _, id) = streamer(&address, &api).await;
        let response = upload(session, images::tests::png(1200, 400))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
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
        assert_eq!(account["premium"], true);
        assert_eq!(account["settings"]["image"], Value::Null);
        let page = account["settings"]["id"].as_str().unwrap().to_string();
        let image_url = format!("http://{address}/pages/{page}/image");
        assert_eq!(
            client.get(&image_url).send().await.unwrap().status(),
            StatusCode::NOT_FOUND
        );
        let response = upload(premium.clone(), b"not an image".to_vec())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let response = upload(premium.clone(), vec![0; 3 * 1024 * 1024])
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let settings: Value = upload(premium.clone(), images::tests::png(1200, 400))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let first = settings["image"].as_str().unwrap().to_string();
        let profile: Value = client
            .get(format!("http://{address}/pages/{page}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(profile["image"], first);
        let response = client.get(&image_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "image/jpeg");
        assert!(!response.bytes().await.unwrap().is_empty());
        let settings: Value = upload(premium.clone(), images::tests::png(2400, 800))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_ne!(settings["image"], first);
        let other: Value = client
            .get(format!("http://{address}/pages/{id}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(other["image"], Value::Null);
        let settings: Value = client
            .post(format!("http://{address}/account/image/remove"))
            .bearer_auth(&premium)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["image"], Value::Null);
        assert_eq!(
            client.get(&image_url).send().await.unwrap().status(),
            StatusCode::NOT_FOUND
        );
        let counters = api.counters.all();
        let count = |name: &str| {
            counters
                .iter()
                .find(|counter| counter.name == name)
                .map(|counter| counter.total)
        };
        assert_eq!(count("imageUploads"), Some(2));
        assert_eq!(count("imageUploadsFailed:not-premium"), Some(1));
        assert_eq!(count("imageUploadsFailed:invalid-image"), Some(1));
    }

    #[tokio::test]
    async fn custom_colors_and_fonts_are_for_pro_streamers() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let save = |session: String, accent: &'static str| {
            client
                .post(format!("http://{address}/account/settings"))
                .bearer_auth(session)
                .json(&json!({"paywall": null, "accent": accent}))
                .send()
        };
        let save_font = |session: String, font: &'static str| {
            client
                .post(format!("http://{address}/account/settings"))
                .bearer_auth(session)
                .json(&json!({"paywall": null, "font": font}))
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
    async fn admin_stats() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let url = format!("http://{address}/admin/stats");
        let response = client.get(&url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let (session, _, _) = streamer(&address, &api).await;
        let response = client.get(&url).bearer_auth(&session).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let account: Value = client
            .get(format!("http://{address}/account"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(account["admin"], false);
        create_account(&api, "admin", "Admin", "admin");
        let admin = api.auth.session("admin", 0);
        let account: Value = client
            .get(format!("http://{address}/account"))
            .bearer_auth(&admin)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(account["admin"], true);
        api.counters.bump("songOrders");
        api.counters.bump("songOrders");
        let response = client.get(&url).bearer_auth(&admin).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let stats: Value = response.json().await.unwrap();
        assert_eq!(
            stats,
            json!({
                "accounts": {"total": 2, "paypal": 0, "stripe": 0, "premium": 1},
                "moblin": {"connectedDevices": 0},
                "pending": {"orders": 0, "deliveries": 0},
                "database": { "bytes": 0 },
                "counters": [{"name": "songOrders", "today": 2, "week": 2, "total": 2}],
            })
        );
    }

    #[tokio::test]
    async fn moderators_manage_the_queue() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        create_account(&api, "21", "Mod 21", "mod21");
        create_account(&api, "22", "Viewer 22", "viewer22");
        api.accounts
            .set_channel_moderators("1", &[channel_moderator("21", "mod21", "Mod 21")]);
        api.accounts
            .set_channel_moderators("9", &[channel_moderator("22", "viewer22", "Viewer 22")]);
        let (mod1, viewer1) = (api.auth.session("21", 0), api.auth.session("22", 0));
        let moderating = async |session: &str| {
            client
                .get(format!("http://{address}/account"))
                .bearer_auth(session)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()["moderating"]
                .clone()
        };
        let command = async |session: &str, body: Value| {
            client
                .post(format!("http://{address}/account/pages/{id}/music"))
                .bearer_auth(session)
                .json(&body)
                .send()
                .await
                .unwrap()
        };
        let toggle = async |moderator: &str, enabled: bool| {
            client
                .post(format!("http://{address}/account/moderators/{moderator}"))
                .bearer_auth(&session)
                .json(&json!({"enabled": enabled}))
                .send()
                .await
                .unwrap()
        };
        let settings = async |enabled: bool| {
            let response = toggle("21", enabled).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.json::<Value>().await.unwrap()["moderators"],
                json!([{"id": "21", "login": "mod21", "name": "Mod 21", "enabled": enabled}])
            );
        };
        assert_eq!(toggle("22", true).await.status(), StatusCode::NOT_FOUND);
        assert_eq!(moderating(&mod1).await, json!([]));
        let response = command(&mod1, json!({"command": "play"})).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        settings(true).await;
        assert_eq!(
            moderating(&mod1).await,
            json!([{"id": id, "name": "Erik", "login": "erik", "avatar": null}])
        );
        assert_eq!(moderating(&viewer1).await, json!([]));
        let response = command(&viewer1, json!({"command": "clear"})).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = command("garbage", json!({"command": "clear"})).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let response = command(&mod1, json!({"command": "remove"})).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let response = command(&mod1, json!({"command": "shuffle"})).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let response = command(&mod1, json!({"command": "play"})).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let mut socket = connect(&address, &key).await;
        for (body, command) in [
            (json!({"command": "play"}), json!({"play": {}})),
            (json!({"command": "pause"}), json!({"pause": {}})),
            (json!({"command": "clear"}), json!({"clear": {}})),
            (
                json!({"command": "remove", "id": "e1"}),
                json!({"remove": {"id": "e1"}}),
            ),
            (
                json!({"command": "select", "id": "e2"}),
                json!({"select": {"id": "e2"}}),
            ),
        ] {
            let sent = tokio::spawn({
                let (address, id, mod1) = (address.clone(), id.clone(), mod1.clone());
                async move {
                    reqwest::Client::new()
                        .post(format!("http://{address}/account/pages/{id}/music"))
                        .bearer_auth(mod1)
                        .json(&body)
                        .send()
                        .await
                        .unwrap()
                }
            });
            let request = receive(&mut socket).await;
            assert_eq!(
                request["request"]["data"],
                json!({"music": {"command": command}})
            );
            answer(
                &mut socket,
                &request,
                music(json!([{"id": "e2", "title": "Song", "user": "Viewer"}])),
            )
            .await;
            let response = sent.await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let queue: Value = response.json().await.unwrap();
            assert_eq!(queue["songs"][0]["id"], "e2");
            assert_eq!(queue["songRequests"], true);
        }
        let sent = tokio::spawn({
            let (address, id, mod1) = (address.clone(), id.clone(), mod1.clone());
            async move {
                reqwest::Client::new()
                    .post(format!("http://{address}/account/pages/{id}/songs"))
                    .bearer_auth(mod1)
                    .json(&json!({"song": LINK, "message": " Hi "}))
                    .send()
                    .await
                    .unwrap()
            }
        });
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"],
            json!({"music": {"command": {"find": {"title": LINK}}}})
        );
        answer(
            &mut socket,
            &request,
            json!({"musicFind": {"song": {"id": "1440818839", "title": "Song", "artist": "Band", "duration": 900}}}),
        )
        .await;
        assert_eq!(sent.await.unwrap().status(), StatusCode::ACCEPTED);
        let request = receive(&mut socket).await;
        let add = &request["request"]["data"]["music"]["command"]["add"];
        assert_eq!(add["title"], LINK);
        assert_eq!(add["user"], "Mod 21");
        assert_eq!(add["message"], "Hi");
        answer(&mut socket, &request, music(json!([]))).await;
        wait_until_added(&api).await;
        let recent = async |session: &str| {
            client
                .get(format!(
                    "http://{address}/account/pages/{id}/recent?kind=song"
                ))
                .bearer_auth(session)
                .send()
                .await
                .unwrap()
        };
        assert_eq!(recent(&viewer1).await.status(), StatusCode::FORBIDDEN);
        let recent: Value = recent(&mod1).await.json().await.unwrap();
        let items = recent["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["user"], "Mod 21");
        assert_eq!(items[0]["name"], "Band - Song");
        assert_eq!(items[0]["amount"], 0);
        assert_eq!(items[0]["moderator"], true);
        assert_eq!(items[0]["status"], "delivered");
        settings(false).await;
        assert_eq!(moderating(&mod1).await, json!([]));
        let response = command(&mod1, json!({"command": "play"})).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let sent = tokio::spawn({
            let (address, id, session) = (address.clone(), id.clone(), session.clone());
            async move {
                reqwest::Client::new()
                    .post(format!("http://{address}/account/pages/{id}/music"))
                    .bearer_auth(session)
                    .json(&json!({"command": "pause"}))
                    .send()
                    .await
                    .unwrap()
            }
        });
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"],
            json!({"music": {"command": {"pause": {}}}})
        );
        answer(&mut socket, &request, music(json!([]))).await;
        assert_eq!(sent.await.unwrap().status(), StatusCode::OK);
        let counters = api.counters.all();
        let count = |name: &str| {
            counters
                .iter()
                .find(|counter| counter.name == name)
                .map_or(0, |counter| counter.total)
        };
        assert_eq!(count("moderatorMusic"), 6);
        assert_eq!(count("moderatorMusicFailed:not-moderator"), 3);
        assert_eq!(count("moderatorMusicFailed:account"), 1);
        assert_eq!(count("moderatorMusicFailed:invalid-command"), 2);
        assert_eq!(count("moderatorMusicFailed:offline"), 1);
        assert_eq!(count("modSongs"), 1);
    }

    #[tokio::test]
    async fn moderators_use_the_streamers_dashboard() {
        let (address, api, _fake) = start_paid_fake().await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        connect_paypal(&api);
        api.pages
            .set(
                &id,
                Settings {
                    currency: Some("SEK".to_string()),
                    paywall: Some(Paywall { price: 250 }),
                    tips: Some(Tips { amounts: vec![500] }),
                    ..Settings::default()
                },
            )
            .unwrap();
        create_account(&api, "21", "Mod 21", "mod21");
        create_account(&api, "22", "Viewer 22", "viewer22");
        api.accounts
            .set_channel_moderators("1", &[channel_moderator("21", "mod21", "Mod 21")]);
        let (mod1, viewer1) = (api.auth.session("21", 0), api.auth.session("22", 0));
        let get = async |session: &str, path: &str| {
            client
                .get(format!("http://{address}/account/pages/{id}{path}"))
                .bearer_auth(session)
                .send()
                .await
                .unwrap()
        };
        let post = async |session: &str, path: &str, body: Value| {
            client
                .post(format!("http://{address}/account/pages/{id}{path}"))
                .bearer_auth(session)
                .json(&body)
                .send()
                .await
                .unwrap()
        };
        for (session, status) in [
            ("", StatusCode::UNAUTHORIZED),
            (mod1.as_str(), StatusCode::FORBIDDEN),
            (viewer1.as_str(), StatusCode::FORBIDDEN),
        ] {
            assert_eq!(get(session, "").await.status(), status);
            assert_eq!(
                post(session, "/settings", json!({"message": "Hi"}))
                    .await
                    .status(),
                status
            );
            assert_eq!(post(session, "/test/tip", json!({})).await.status(), status);
            assert_eq!(
                post(session, "/test/spin", json!({})).await.status(),
                status
            );
            assert_eq!(
                post(session, "/image/remove", json!({})).await.status(),
                status
            );
        }
        api.accounts.enable_moderator("1", "21", true).unwrap();
        let own: Value = client
            .get(format!("http://{address}/account"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(own["settings"]["merchantId"], "MERCHANT00001");
        let account: Value = get(&mod1, "").await.json().await.unwrap();
        assert_eq!(account["name"], "Erik");
        assert_eq!(account["login"], "erik");
        assert_eq!(account["devices"], json!([]));
        assert_eq!(account["admin"], false);
        assert_eq!(account["premium"], false);
        assert_eq!(account["moderating"], json!([]));
        assert_eq!(account["moderators"], json!([]));
        let settings = &account["settings"];
        assert_eq!(settings["id"], id);
        assert_eq!(settings["currency"], "SEK");
        assert_eq!(settings["paywall"], json!({"price": 250}));
        assert_eq!(settings["tips"], json!({"amounts": [500]}));
        assert_eq!(settings["paypal"], false);
        assert_eq!(settings["merchantId"], Value::Null);
        assert_eq!(settings["paypalPending"], false);
        assert_eq!(settings["stripe"], false);
        assert_eq!(settings["stripeAccountId"], Value::Null);
        assert_eq!(settings["stripePending"], false);
        assert_eq!(settings["payable"], true);
        assert_eq!(get(&viewer1, "").await.status(), StatusCode::FORBIDDEN);
        let response = post(
            &mod1,
            "/settings",
            json!({
                "currency": "USD",
                "paywall": null,
                "tips": {"amounts": [300, 700]},
                "spin": {"options": [{"spins": 1, "price": 100}]},
                "message": " Hi ",
                "maxSongMinutes": 7,
                "subSongs": {"tier1": 1, "tier2": 2, "tier3": 3},
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let settings: Value = response.json().await.unwrap();
        assert_eq!(settings["message"], "Hi");
        assert_eq!(settings["maxSongMinutes"], 7);
        assert_eq!(
            settings["subSongs"],
            json!({"tier1": 1, "tier2": 2, "tier3": 3})
        );
        assert_eq!(settings["currency"], "SEK");
        assert_eq!(settings["paywall"], Value::Null);
        assert_eq!(settings["tips"], json!({"amounts": [300, 700]}));
        assert_eq!(
            settings["spin"],
            json!({"options": [{"spins": 1, "price": 100}]})
        );
        assert_eq!(settings["merchantId"], Value::Null);
        assert_eq!(settings["paypal"], false);
        assert_eq!(settings["payable"], true);
        let saved = api.pages.get(&id);
        assert_eq!(saved.message.as_deref(), Some("Hi"));
        assert_eq!(saved.currency(), "SEK");
        assert_eq!(saved.paywall, None);
        assert_eq!(
            saved.tips,
            Some(Tips {
                amounts: vec![300, 700]
            })
        );
        assert_eq!(
            saved.spin,
            Some(Spin {
                options: vec![SpinOption {
                    spins: 1,
                    price: 100
                }]
            })
        );
        let response = post(&mod1, "/settings", json!({"paywall": {"price": 100}})).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(api.pages.get(&id).paywall, Some(Paywall { price: 100 }));
        api.accounts.forget_paypal("1").unwrap();
        let account: Value = get(&mod1, "").await.json().await.unwrap();
        assert_eq!(account["settings"]["payable"], false);
        let response = post(&mod1, "/settings", json!({"paywall": {"price": 150}})).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.text().await.unwrap(),
            "connect PayPal or Stripe first"
        );
        assert_eq!(api.pages.get(&id).paywall, Some(Paywall { price: 100 }));
        let response = post(&mod1, "/settings", json!({"paywall": {"price": 100}})).await;
        assert_eq!(response.status(), StatusCode::OK);
        let response = post(&mod1, "/settings", json!({"message": "Hi"})).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(api.pages.get(&id).paywall, None);
        connect_paypal(&api);
        let response = post(&mod1, "/settings", json!({"font": "Poppins"})).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = client
            .post(format!("http://{address}/account/pages/{id}/image"))
            .bearer_auth(&mod1)
            .header("content-type", "image/png")
            .body(images::tests::png(1200, 400))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = post(&mod1, "/image/remove", json!({})).await;
        assert_eq!(response.status(), StatusCode::OK);
        let response = post(&mod1, "/test/spin", json!({})).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(response.text().await.unwrap(), "Connect Moblin first.");
        let response = post(&mod1, "/test/tip", json!({"message": " From a mod "})).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let mut socket = connect(&address, &key).await;
        let request = receive(&mut socket).await;
        let tip = &request["request"]["data"]["tip"];
        assert_eq!(tip["name"], "Mod 21");
        assert_eq!(tip["message"], "From a mod");
        assert_eq!(tip["amount"], "5.00");
        assert_eq!(tip["currency"], "SEK");
        answer(&mut socket, &request, Value::Null).await;
        wait_until_added(&api).await;
        let counters = api.counters.all();
        let count = |name: &str| {
            counters
                .iter()
                .find(|counter| counter.name == name)
                .map_or(0, |counter| counter.total)
        };
        assert_eq!(count("tipTests"), 1);
        assert_eq!(count("tipTestsFailed:not-moderator"), 2);
        assert_eq!(count("spinTestsFailed:offline"), 1);
        assert_eq!(count("imageUploadsFailed:not-premium"), 1);
        assert_eq!(count("imageUploadsFailed:not-moderator"), 0);
    }

    #[tokio::test]
    async fn moderators_add_songs_of_any_length_for_free() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        create_account(&api, "21", "Mod 21", "mod21");
        create_account(&api, "22", "Viewer 22", "viewer22");
        api.accounts
            .set_channel_moderators("1", &[channel_moderator("21", "mod21", "Mod 21")]);
        let (mod1, viewer1) = (api.auth.session("21", 0), api.auth.session("22", 0));
        let send = |session: String, body: Value| {
            let (address, id) = (address.clone(), id.clone());
            tokio::spawn(async move {
                reqwest::Client::new()
                    .post(format!("http://{address}/account/pages/{id}/songs"))
                    .bearer_auth(session)
                    .json(&body)
                    .send()
                    .await
                    .unwrap()
            })
        };
        let response = send(mod1.clone(), json!({"song": LINK})).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        api.accounts.enable_moderator("1", "21", true).unwrap();
        let response = send(viewer1.clone(), json!({"song": LINK})).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = send(mod1.clone(), json!({"song": "nope"})).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let response = send(mod1.clone(), json!({"song": LINK})).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"paywall": null, "maxSongMinutes": 1}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut socket = connect(&address, &key).await;
        song_requests(&mut socket, false).await;
        let response = send(mod1.clone(), json!({"song": LINK})).await.unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        song_requests(&mut socket, true).await;
        let sent = send(mod1.clone(), json!({"song": LINK}));
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"],
            json!({"music": {"command": {"find": {"title": LINK}}}})
        );
        answer(&mut socket, &request, json!({"musicFind": {}})).await;
        assert_eq!(
            sent.await.unwrap().status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        for (who, name) in [(&mod1, "Mod 21"), (&session, "Erik")] {
            let sent = send(who.clone(), json!({"song": LINK, "message": " Long one "}));
            let request = receive(&mut socket).await;
            answer(
                &mut socket,
                &request,
                json!({"musicFind": {"song": {"id": "1440818839", "title": "Song", "artist": "Band", "duration": 900}}}),
            )
            .await;
            assert_eq!(sent.await.unwrap().status(), StatusCode::ACCEPTED);
            let request = receive(&mut socket).await;
            let add = &request["request"]["data"]["music"]["command"]["add"];
            assert_eq!(add["title"], LINK);
            assert_eq!(add["user"], name);
            assert_eq!(add["message"], "Long one");
            answer(&mut socket, &request, music(json!([]))).await;
            wait_until_added(&api).await;
        }
        let recent = async |kind: &str| {
            client
                .get(format!(
                    "http://{address}/account/pages/{id}/recent?kind={kind}"
                ))
                .bearer_auth(&mod1)
                .send()
                .await
                .unwrap()
        };
        let songs: Value = recent("song").await.json().await.unwrap();
        let items = songs["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["user"], "Erik");
        assert_eq!(items[1]["user"], "Mod 21");
        for item in items {
            assert_eq!(item["moderator"], true);
            assert_eq!(item["amount"], 0);
            assert_eq!(item["message"], "Long one");
            assert_eq!(item["status"], "delivered");
            assert!(item.get("provider").is_none());
        }
        for kind in ["tip", "spin"] {
            let response = recent(kind).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.json::<Value>().await.unwrap()["items"], json!([]));
        }
        assert_eq!(recent("bogus").await.status(), StatusCode::BAD_REQUEST);
        let own: Value = client
            .get(format!("http://{address}/account/recent?kind=song"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(own["items"][1]["user"], "Mod 21");
        assert_eq!(own["items"][1]["moderator"], true);
        assert!(own["items"][1].get("provider").is_none());
        socket.close(None).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        for _ in 0..5 {
            let response = send(mod1.clone(), json!({"song": LINK})).await.unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
        let response = send(mod1.clone(), json!({"song": LINK})).await.unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        let counters = api.counters.all();
        let count = |name: &str| {
            counters
                .iter()
                .find(|counter| counter.name == name)
                .map_or(0, |counter| counter.total)
        };
        assert_eq!(count("modSongs"), 2);
        assert_eq!(count("modSongsFailed:not-moderator"), 2);
        assert_eq!(count("modSongsFailed:invalid-link"), 1);
        assert_eq!(count("modSongsFailed:offline"), 6);
        assert_eq!(count("modSongsFailed:requests-off"), 1);
        assert_eq!(count("modSongsFailed:song-not-found"), 1);
        assert_eq!(count("modSongsFailed:too-many-requests"), 1);
    }
}
