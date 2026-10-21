use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use serde::Serialize;

use crate::accounts::AccountStats;
use crate::counters::Counter;
use crate::deliveries;

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
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MoblinStats {
    connected_devices: usize,
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
    Ok(Json(AdminStatsResponse {
        server: ServerStats {
            version: option_env!("MOBLIN_WEBSITE_VERSION").unwrap_or("dev"),
            started: api.started,
            now: deliveries::now(),
            memory: resident_memory(),
            integrations: Integrations {
                twitch: api.live.is_some(),
                kick: api.auth.has_kick(),
                app_attest: api.app_attest.is_some(),
                paypal: api.paypal.is_some(),
                stripe: api.stripe.is_some(),
            },
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

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::{Value, json};

    use crate::api::test_utils::*;

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
        let mut stats: Value = response.json().await.unwrap();
        let server = stats.as_object_mut().unwrap().remove("server").unwrap();
        assert_eq!(server["version"], "dev");
        assert_eq!(server["started"], 0);
        assert!(server["now"].as_u64().unwrap() > 0);
        assert_eq!(
            server["integrations"],
            json!({"twitch": false, "kick": false, "appAttest": false, "paypal": false, "stripe": false})
        );
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
}
