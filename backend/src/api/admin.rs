use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::access::SignInState;
use crate::accounts::{Account, AccountError, AccountStats, Platform};
use crate::counters::Counter;
use crate::deliveries;
use crate::requests::RequestStats;
use crate::traffic::TrafficStats;

use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AdminStatsResponse {
    server: ServerStats,
    accounts: AccountStats,
    moblin: MoblinStats,
    pending: PendingStats,
    database: DatabaseStats,
    counters: Vec<Counter>,
    sign_in: SignInState,
    premium: Vec<AccountSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct IdentitySummary {
    platform: Platform,
    user_id: String,
    login: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AccountSummary {
    id: String,
    name: String,
    avatar: Option<String>,
    identities: Vec<IdentitySummary>,
    premium: bool,
}

fn account_summary(account: Account) -> AccountSummary {
    AccountSummary {
        premium: account.premium,
        name: account.name().to_string(),
        avatar: account.avatar().map(str::to_string),
        identities: account
            .identities
            .into_iter()
            .map(|identity| IdentitySummary {
                platform: identity.platform,
                user_id: identity.user_id,
                login: identity.login,
            })
            .collect(),
        id: account.id,
    }
}

fn premium_accounts(api: &Api) -> Vec<AccountSummary> {
    api.accounts
        .premium()
        .into_iter()
        .map(account_summary)
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ServerStats {
    version: &'static str,
    started: u64,
    now: u64,
    memory: Option<u64>,
    integrations: Integrations,
    requests: RequestStats,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Integrations {
    twitch: bool,
    kick: bool,
    app_attest: bool,
    paypal: bool,
    stripe: bool,
}

fn resident_memory() -> Option<u64> {
    memory_stats::memory_stats().map(|stats| stats.physical_mem as u64)
}

#[derive(Serialize)]
pub(super) struct DatabaseStats {
    bytes: u64,
    files: Vec<DatabaseFile>,
}

#[derive(Serialize)]
pub(super) struct DatabaseFile {
    name: String,
    bytes: u64,
}

fn database_stats(database: &Database) -> DatabaseStats {
    let files: Vec<DatabaseFile> = database
        .files()
        .into_iter()
        .map(|(name, bytes)| DatabaseFile { name, bytes })
        .collect();
    DatabaseStats {
        bytes: files.iter().map(|file| file.bytes).sum(),
        files,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MoblinStats {
    connected_devices: usize,
    messages: TrafficStats,
}

#[derive(Serialize)]
pub(super) struct PendingStats {
    orders: usize,
    deliveries: usize,
}

pub(super) async fn handle_admin_overview(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<AdminStatsResponse>, (StatusCode, String)> {
    admin(&api, &headers)?;
    Ok(Json(admin_stats(&api)))
}

pub(super) fn admin_stats(api: &Api) -> AdminStatsResponse {
    AdminStatsResponse {
        server: ServerStats {
            version: option_env!("MOBLIN_CLOUD_VERSION").unwrap_or("dev"),
            started: api.started,
            now: deliveries::now(),
            memory: resident_memory(),
            integrations: Integrations {
                twitch: api.auth.has(Platform::Twitch),
                kick: api.auth.has(Platform::Kick),
                app_attest: api.app_attest.is_some(),
                paypal: api.paypal.is_some(),
                stripe: api.stripe.is_some(),
            },
            requests: api.requests.stats(),
        },
        accounts: api.accounts.stats(),
        moblin: MoblinStats {
            connected_devices: api.remote_control.connected_count(),
            messages: api.remote_control.traffic.stats(),
        },
        pending: PendingStats {
            orders: api.orders.unfinished(),
            deliveries: api.deliveries.count(),
        },
        database: database_stats(&api.database),
        counters: api.counters.all(),
        sign_in: api.access.state(),
        premium: premium_accounts(api),
    }
}

fn changed(
    result: rusqlite::Result<()>,
    api: &Api,
) -> Result<Json<SignInState>, (StatusCode, String)> {
    result.map_err(|error| {
        warn!("changing who may sign in failed: {error:#}");
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "This is unavailable right now. Try again later.".to_string(),
        )
    })?;
    Ok(Json(api.access.state()))
}

#[derive(Deserialize)]
pub(super) struct RestrictRequest {
    restricted: bool,
}

pub(super) async fn handle_admin_sign_in(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<RestrictRequest>,
) -> Result<Json<SignInState>, (StatusCode, String)> {
    let admin = admin(&api, &headers)?;
    info!(
        "account {} {} sign in to allowed accounts",
        admin.account.id,
        if request.restricted {
            "limited"
        } else {
            "opened"
        }
    );
    changed(api.access.set_restricted(request.restricted), &api)
}

pub(super) async fn handle_admin_sign_in_allow(
    State(api): State<Arc<Api>>,
    Path((platform, user_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<SignInState>, (StatusCode, String)> {
    let admin = admin(&api, &headers)?;
    let platform = platform_of(&platform)?;
    let allowed = api.access.allow(platform, &user_id).map_err(|error| {
        warn!(
            "allowing {} user {user_id} failed: {error:#}",
            platform.title()
        );
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "This is unavailable right now. Try again later.".to_string(),
        )
    })?;
    if !allowed {
        return Err((
            StatusCode::NOT_FOUND,
            "No one with that ID has asked to sign in or has an account.".to_string(),
        ));
    }
    info!(
        "account {} allowed {} user {user_id} to sign in",
        admin.account.id,
        platform.title()
    );
    Ok(Json(api.access.state()))
}

pub(super) async fn handle_admin_sign_in_remove(
    State(api): State<Arc<Api>>,
    Path((platform, user_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<SignInState>, (StatusCode, String)> {
    let admin = admin(&api, &headers)?;
    let platform = platform_of(&platform)?;
    info!(
        "account {} stopped {} user {user_id} from signing in",
        admin.account.id,
        platform.title()
    );
    changed(api.access.remove(platform, &user_id), &api)
}

pub(super) async fn handle_admin_sign_in_dismiss(
    State(api): State<Arc<Api>>,
    Path((platform, user_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<SignInState>, (StatusCode, String)> {
    let admin = admin(&api, &headers)?;
    let platform = platform_of(&platform)?;
    info!(
        "account {} dismissed the sign in request of {} user {user_id}",
        admin.account.id,
        platform.title()
    );
    changed(api.access.dismiss(platform, &user_id), &api)
}

#[derive(Deserialize)]
pub(super) struct AccountsQuery {
    search: String,
}

pub(super) async fn handle_admin_accounts(
    State(api): State<Arc<Api>>,
    Query(query): Query<AccountsQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<AccountSummary>>, (StatusCode, String)> {
    admin(&api, &headers)?;
    Ok(Json(
        api.accounts
            .search(&query.search)
            .into_iter()
            .map(account_summary)
            .collect(),
    ))
}

#[derive(Deserialize)]
pub(super) struct PremiumRequest {
    premium: bool,
}

pub(super) async fn handle_admin_account_premium(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<PremiumRequest>,
) -> Result<Json<Vec<AccountSummary>>, (StatusCode, String)> {
    let admin = admin(&api, &headers)?;
    let account = api
        .accounts
        .set_premium(&id, request.premium)
        .map_err(|error| match error {
            AccountError::Unknown => (StatusCode::NOT_FOUND, "No such account.".to_string()),
            error => account_error(error),
        })?;
    info!(
        "account {} {} account {id} premium",
        admin.account.id,
        if request.premium { "made" } else { "stopped" }
    );
    api.remote_control
        .changes
        .notify(&account.page_id, Change::Account);
    Ok(Json(premium_accounts(&api)))
}

#[cfg(test)]
#[path = "tests/admin.rs"]
mod tests;
