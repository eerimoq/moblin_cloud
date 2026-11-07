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
    let (session, moblin_cloud_key, _) = streamer(&api);
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
    api.counters.bump("tipOrders");
    api.counters.bump("tipOrders");
    let _socket = connect(&address, &moblin_cloud_key).await;
    let response = client.get(&url).bearer_auth(&admin).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let mut stats: Value = response.json().await.unwrap();
    let server = stats.as_object_mut().unwrap().remove("server").unwrap();
    let mut messages = stats["moblin"]
        .as_object_mut()
        .unwrap()
        .remove("messages")
        .unwrap();
    for kinds in ["sent", "received"] {
        for kind in messages[kinds].as_array_mut().unwrap() {
            assert!(
                kind.as_object_mut()
                    .unwrap()
                    .remove("bytes")
                    .unwrap()
                    .as_u64()
                    .unwrap()
                    > 0
            );
        }
    }
    assert_eq!(
        messages,
        json!({
            "sent": [
                {"kind": "hello", "messages": 1},
                {"kind": "identified", "messages": 1},
            ],
            "received": [{"kind": "identify", "messages": 1}],
        })
    );
    assert_eq!(server["version"], "dev");
    assert_eq!(server["started"], 0);
    assert!(server["now"].as_u64().unwrap() > 0);
    assert_eq!(
        server["integrations"],
        json!({"twitch": false, "kick": false, "appAttest": false, "paypal": false, "stripe": false})
    );
    assert_eq!(
        server["requests"],
        json!({
            "total": 5,
            "statuses": {"101": 1, "200": 2, "401": 1, "403": 1},
            "endpoints": [
                {"path": "/account", "total": 2, "statuses": {"200": 2}},
                {"path": "/admin/stats", "total": 2, "statuses": {"401": 1, "403": 1}},
                {"path": "/remote-control/streamer/{key}", "total": 1, "statuses": {"101": 1}},
            ],
        })
    );
    assert_eq!(
        stats,
        json!({
            "accounts": {"total": 2, "paypal": 0, "stripe": 0, "premium": 1},
            "moblin": {"connectedDevices": 1},
            "pending": {"orders": 0, "deliveries": 0},
            "database": {"bytes": 0, "files": []},
            "counters": [
                {"name": "tipOrders", "today": 2, "week": 2, "total": 2},
            ],
        })
    );
}
