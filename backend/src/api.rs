use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use axum::extract::DefaultBodyLimit;
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use log::warn;
use serde_json::Value;
use tower_http::cors::{Any, CorsLayer};

use crate::accounts::{Account, Accounts, Page, Platform};
use crate::app_attest::AppAttest;
use crate::auth::Auth;
use crate::challenges::Challenges;
use crate::counters::Counters;
use crate::database::Database;
use crate::deliveries::Deliveries;
use crate::live::Live;
use crate::pages::Pages;
use crate::payments::Orders;
use crate::paypal::PayPal;
use crate::remote_control::Streamers;
use crate::store::Store;
use crate::stripe::Stripe;
use crate::subscribers::Allowances;

mod account;
mod admin;
mod banner;
mod moderators;
mod orders;
mod providers;
mod public;
mod recent;
mod settings;
mod sign_in;
mod songs;
mod spins;
mod streamers;
mod subscriber;
mod test_sends;
#[cfg(test)]
mod test_utils;
mod tips;

use account::*;
use admin::*;
use banner::*;
use moderators::*;
pub use orders::settle_orders;
use orders::*;
use providers::*;
use public::*;
use recent::*;
use settings::*;
use sign_in::*;
use songs::*;
use spins::*;
use streamers::*;
use subscriber::*;
use test_sends::*;
use tips::*;

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
    pub started: u64,
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
        .route("/pages/kick/{login}", get(handle_kick_page))
        .route("/auth/twitch/login", get(handle_twitch_login))
        .route("/auth/twitch/callback", get(handle_twitch_callback))
        .route("/auth/kick/login", get(handle_kick_login))
        .route("/auth/kick/callback", get(handle_kick_callback))
        .route("/account", get(handle_account))
        .route("/account/link", post(handle_account_link))
        .route("/account/unlink/{platform}", post(handle_account_unlink))
        .route("/account/key", post(handle_account_key))
        .route(
            "/account/devices/{device}/name",
            post(handle_account_device_name),
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
            "/account/moderators/{platform}",
            post(handle_account_moderator_add),
        )
        .route(
            "/account/moderators/{platform}/{moderator}/remove",
            post(handle_account_moderator_remove),
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

pub(super) fn page_of(api: &Api, id: &str) -> Option<Page> {
    Some(api.accounts.of_page(id)?.page)
}

pub(super) fn music_device(api: &Api, id: &str) -> String {
    api.remote_control.music_device(id).unwrap_or_default()
}

pub(super) fn not_signed_in() -> (StatusCode, String) {
    (StatusCode::UNAUTHORIZED, "not signed in".to_string())
}

pub(super) struct SignedIn {
    id: String,
    account: Account,
    fresh: bool,
}

pub(super) fn signed_in(api: &Api, headers: &HeaderMap) -> Result<SignedIn, (StatusCode, String)> {
    let session = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .and_then(|session| api.auth.session_of(session))
        .ok_or_else(not_signed_in)?;
    let fresh = session.is_fresh();
    let id = match api.accounts.get(&session.account) {
        Some(_) => session.account,
        None => api
            .accounts
            .account_of_identity(Platform::Twitch, &session.account)
            .ok_or_else(not_signed_in)?,
    };
    let account = api
        .accounts
        .get(&id)
        .filter(|account| account.sessions == session.generation)
        .ok_or_else(not_signed_in)?;
    Ok(SignedIn { fresh, id, account })
}

pub(super) fn is_admin(api: &Api, account: &Account) -> bool {
    account
        .identities
        .iter()
        .any(|identity| api.admins.contains(&identity.admin_key()))
}

pub(super) fn is_premium(api: &Api, account: &Account) -> bool {
    is_admin(api, account)
        || account
            .identities
            .iter()
            .any(|identity| api.premium_accounts.contains(&identity.admin_key()))
}

pub(super) fn admin(api: &Api, headers: &HeaderMap) -> Result<SignedIn, (StatusCode, String)> {
    let signed_in = signed_in(api, headers)?;
    if !is_admin(api, &signed_in.account) {
        return Err((StatusCode::FORBIDDEN, "not an admin".to_string()));
    }
    Ok(signed_in)
}

pub(super) fn platform_of(name: &str) -> Result<Platform, (StatusCode, String)> {
    Platform::parse(name).ok_or((StatusCode::NOT_FOUND, "no such platform".to_string()))
}

pub(super) fn platform_needed(platform: Platform) -> (StatusCode, String) {
    (
        StatusCode::CONFLICT,
        format!("Link {} first.", platform.title()),
    )
}

pub(super) struct Failure {
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

pub(super) fn failure(reason: &'static str, status: StatusCode, text: &str) -> Failure {
    Failure {
        reason,
        status,
        text: text.to_string(),
    }
}

pub(super) fn user_name(user: Option<String>, anonymous: &str) -> String {
    user.map(|user| user.trim().chars().take(25).collect::<String>())
        .filter(|user| !user.is_empty())
        .unwrap_or_else(|| anonymous.to_string())
}

pub(super) fn not_saved(error: anyhow::Error) -> Failure {
    warn!("{error:#}");
    failure(
        "not-saved",
        StatusCode::SERVICE_UNAVAILABLE,
        "This is unavailable right now. Try again later.",
    )
}

pub(super) fn new_request_id(api: &Api) -> Result<String, Failure> {
    api.remote_control.new_request_id().ok_or(failure(
        "crashed",
        StatusCode::SERVICE_UNAVAILABLE,
        "Something went wrong. Try again.",
    ))
}

pub(super) fn moderator(api: &Api, headers: &HeaderMap, id: &str) -> Result<SignedIn, Failure> {
    let signed_in = signed_in(api, headers)?;
    let streamer = || api.accounts.of_page(id);
    let allowed = signed_in.account.page.id == id
        || [Platform::Twitch, Platform::Kick]
            .into_iter()
            .any(|platform| {
                signed_in
                    .account
                    .identity(platform)
                    .is_some_and(|moderator| {
                        streamer()
                            .and_then(|streamer| {
                                streamer
                                    .identity(platform)
                                    .map(|identity| identity.user_id.clone())
                            })
                            .is_some_and(|broadcaster| {
                                api.accounts
                                    .moderates(platform, &moderator.user_id, &broadcaster)
                            })
                    })
            });
    if !allowed {
        return Err(failure(
            "not-moderator",
            StatusCode::FORBIDDEN,
            "Only the streamer and the moderators they turned on can do this.",
        ));
    }
    Ok(signed_in)
}

pub(super) fn counted<T>(api: &Api, name: &str, result: Result<T, Failure>) -> Result<T, Failure> {
    api.counters.bump(&match &result {
        Ok(_) => name.to_string(),
        Err(failure) => format!("{name}Failed:{}", failure.reason),
    });
    result
}

pub(super) fn viewer_message(message: Option<String>) -> Result<Option<String>, Failure> {
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

pub(super) fn accepted() -> (StatusCode, Json<Value>) {
    (
        StatusCode::ACCEPTED,
        Json(Value::Object(Default::default())),
    )
}

pub(super) fn within_limit(
    limits: &Mutex<HashMap<String, Vec<Instant>>>,
    id: &str,
    max: usize,
) -> bool {
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
