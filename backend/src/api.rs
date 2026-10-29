use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use axum::extract::DefaultBodyLimit;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router, middleware};
use log::warn;
use serde_json::Value;
use tokio::sync::watch;
use tower_http::cors::CorsLayer;

use crate::accounts::{Access, Account, Accounts, Page, Permission, Platform};
use crate::app_attest::AppAttest;
use crate::auth::Auth;
use crate::changes::Change;
use crate::counters::Counters;
use crate::database::Database;
use crate::deliveries::Deliveries;
use crate::pages::Pages;
use crate::payments::Orders;
use crate::paypal::PayPal;
use crate::remote_control::Streamers;
use crate::requests::Requests;
use crate::stripe::Stripe;
use crate::subscribers::Allowances;

mod account;
mod admin;
mod banner;
mod devices;
mod effects;
mod events;
mod moblin;
mod moderators;
mod orders;
mod providers;
mod public;
mod recent;
mod settings;
mod sign_in;
mod spins;
mod subscriber;
mod test_sends;
#[cfg(test)]
mod test_utils;
mod tips;

use account::*;
use admin::*;
use banner::*;
use devices::*;
use effects::*;
use events::*;
use moblin::*;
use moderators::*;
pub use orders::settle_orders;
use orders::*;
use providers::*;
use public::*;
use recent::*;
use settings::*;
use sign_in::*;
use spins::*;
use subscriber::*;
use test_sends::*;
use tips::*;

pub struct Api {
    pub app_attest: Option<AppAttest>,
    pub require_attested_moblin: bool,
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
    pub requests: Requests,
    pub stopping: watch::Sender<bool>,
}

pub fn router(api: Arc<Api>) -> Router {
    let origin = reqwest::Url::parse(&api.auth.frontend_url)
        .expect("the frontend URL is not a URL")
        .origin()
        .ascii_serialization();
    let cors = CorsLayer::new()
        .allow_origin(HeaderValue::from_str(&origin).expect("the frontend URL is not an origin"))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE])
        .max_age(Duration::from_secs(7200));
    Router::new()
        .route(
            "/remote-control/streamer/{key}",
            get(handle_remote_control_streamer),
        )
        .route("/pages/twitch/{login}", get(handle_twitch_page))
        .route("/pages/kick/{login}", get(handle_kick_page))
        .route("/pages/addresses", get(handle_page_addresses))
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
        .route(
            "/account/devices/{device}/remove",
            post(handle_account_device_remove),
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
        .route("/account/test/tip", post(handle_account_test_tip))
        .route("/account/test/spin", post(handle_account_test_spin))
        .route("/account/test/effect", post(handle_account_test_effect))
        .route(
            "/account/moderators/{platform}",
            post(handle_account_moderator_add),
        )
        .route(
            "/account/moderators/{platform}/{moderator}/remove",
            post(handle_account_moderator_remove),
        )
        .route(
            "/account/moderators/{platform}/{moderator}/preview",
            post(handle_account_moderator_preview),
        )
        .route(
            "/account/moderators/{platform}/{moderator}/go-live",
            post(handle_account_moderator_go_live),
        )
        .route(
            "/account/moderators/{platform}/{moderator}/edit-tips",
            post(handle_account_moderator_edit_tips),
        )
        .route(
            "/account/moderators/{platform}/{moderator}/remote-control",
            post(handle_account_moderator_remote_control),
        )
        .route("/account/pages/{id}/recent", get(handle_moderator_recent))
        .route(
            "/account/pages/{id}/recent/{order}/resend",
            post(handle_moderator_resend),
        )
        .route("/account/pages/{id}", get(handle_moderator_account))
        .route("/account/pages/{id}/events", get(handle_page_events))
        .route(
            "/account/pages/{id}/devices/{device}/events",
            get(handle_device_events),
        )
        .route(
            "/account/pages/{id}/devices/{device}/preview",
            get(handle_device_preview),
        )
        .route(
            "/account/pages/{id}/devices/{device}/control",
            post(handle_device_control),
        )
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
        .route(
            "/account/pages/{id}/test/effect",
            post(handle_moderator_test_effect),
        )
        .route("/admin/stats", get(handle_admin_stats))
        .route("/admin/stats/events", get(handle_admin_stats_events))
        .route("/pages/{id}", get(handle_page))
        .route("/pages/{id}/image", get(handle_page_image))
        .route("/pages/{id}/events", get(handle_public_page_events))
        .route(
            "/pages/{id}/spin/subscriber",
            get(handle_subscriber_spin_allowance).post(handle_subscriber_spin),
        )
        .route("/pages/{id}/tips", get(handle_tips))
        .route("/pages/{id}/tips/orders", post(handle_tip_order))
        .route(
            "/pages/{id}/tips/orders/{order_id}/capture",
            post(handle_tip_capture),
        )
        .route("/pages/{id}/effects", get(handle_effects))
        .route(
            "/pages/{id}/effects/subscriber",
            get(handle_subscriber_effect_allowance).post(handle_subscriber_effect),
        )
        .route("/pages/{id}/effects/orders", post(handle_effect_order))
        .route(
            "/pages/{id}/effects/orders/{order_id}/capture",
            post(handle_effect_capture),
        )
        .route("/effects/{id}", get(handle_effect_media))
        .route("/icons/{id}", get(handle_icon))
        .route("/pages/{id}/spin", get(handle_spin))
        .route("/pages/{id}/spin/orders", post(handle_spin_order))
        .route(
            "/pages/{id}/spin/orders/{order_id}/capture",
            post(handle_spin_capture),
        )
        .layer(cors)
        .layer(middleware::from_fn_with_state(
            api.clone(),
            crate::requests::count,
        ))
        .with_state(api)
}

pub(super) fn page_of(api: &Api, id: &str) -> Option<Page> {
    Some(api.accounts.of_page(id)?.page)
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
    let (id, account) = match api.accounts.get(&session.account) {
        Some(account) => (session.account, account),
        None => {
            let id = api
                .accounts
                .account_of_identity(Platform::Twitch, &session.account)
                .ok_or_else(not_signed_in)?;
            let account = api.accounts.get(&id).ok_or_else(not_signed_in)?;
            (id, account)
        }
    };
    if account.sessions != session.generation {
        return Err(not_signed_in());
    }
    Ok(SignedIn { fresh, id, account })
}

pub(super) fn account_changed(api: &Api, id: &str, change: Change) {
    if let Some(account) = api.accounts.get(id) {
        api.remote_control.changes.notify(&account.page.id, change);
    }
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
    let allowed = signed_in.account.page.id == id
        || listed(
            api,
            &signed_in.account,
            id,
            |platform, moderator, broadcaster| {
                api.accounts.moderates(platform, moderator, broadcaster)
            },
        );
    if !allowed {
        return Err(failure(
            "not-moderator",
            StatusCode::FORBIDDEN,
            "Only the streamer and the moderators they turned on can do this.",
        ));
    }
    Ok(signed_in)
}

pub(super) fn may(api: &Api, account: &Account, id: &str, permission: Permission) -> bool {
    account.page.id == id
        || listed(api, account, id, |platform, moderator, broadcaster| {
            api.accounts
                .permits(platform, moderator, broadcaster, permission)
        })
}

pub(super) fn access(api: &Api, account: &Account, id: &str) -> Access {
    if account.page.id == id {
        return Access::Control;
    }
    [Access::Control, Access::View]
        .into_iter()
        .find(|&access| {
            listed(api, account, id, |platform, moderator, broadcaster| {
                api.accounts
                    .grants(platform, moderator, broadcaster, access)
            })
        })
        .unwrap_or(Access::None)
}

pub(super) fn watcher(api: &Api, headers: &HeaderMap, id: &str) -> Result<SignedIn, Failure> {
    let signed_in = moderator(api, headers, id)?;
    if access(api, &signed_in.account, id) < Access::View {
        return Err(failure(
            "no-remote-control",
            StatusCode::FORBIDDEN,
            "The streamer has not let you use the remote control.",
        ));
    }
    Ok(signed_in)
}

pub(super) fn previewer(api: &Api, headers: &HeaderMap, id: &str) -> Result<SignedIn, Failure> {
    let signed_in = watcher(api, headers, id)?;
    if !may(api, &signed_in.account, id, Permission::Preview) {
        return Err(failure(
            "no-preview",
            StatusCode::FORBIDDEN,
            "The streamer has not let you see the stream preview.",
        ));
    }
    Ok(signed_in)
}

pub(super) fn editor(api: &Api, headers: &HeaderMap, id: &str) -> Result<SignedIn, Failure> {
    let signed_in = moderator(api, headers, id)?;
    if !may(api, &signed_in.account, id, Permission::EditTips) {
        return Err(failure(
            "no-edit-tips",
            StatusCode::FORBIDDEN,
            "The streamer has not let you edit tips.",
        ));
    }
    Ok(signed_in)
}

fn listed(
    api: &Api,
    account: &Account,
    id: &str,
    check: impl Fn(Platform, &str, &str) -> bool,
) -> bool {
    let streamer = api.accounts.of_page(id);
    [Platform::Twitch, Platform::Kick]
        .into_iter()
        .any(|platform| {
            account.identity(platform).is_some_and(|moderator| {
                streamer
                    .as_ref()
                    .and_then(|streamer| streamer.identity(platform))
                    .is_some_and(|broadcaster| {
                        check(platform, &moderator.user_id, &broadcaster.user_id)
                    })
            })
        })
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
