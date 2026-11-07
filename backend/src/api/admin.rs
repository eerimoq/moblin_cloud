use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use serde::Serialize;

use crate::accounts::{AccountStats, Platform};
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
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
    let kilobytes = line.split_whitespace().nth(1)?.parse::<u64>().ok()?;
    Some(kilobytes * 1024)
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

pub(super) async fn handle_admin_stats(
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
        accounts: api.accounts.stats(
            &api.admins
                .iter()
                .chain(&api.premium_accounts)
                .map(String::as_str)
                .collect::<Vec<_>>(),
        ),
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
    }
}

#[cfg(test)]
#[path = "tests/admin.rs"]
mod tests;
