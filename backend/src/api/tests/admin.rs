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
                {"method": "GET", "path": "/account", "total": 2, "statuses": {"200": 2}},
                {"method": "GET", "path": "/admin/stats", "total": 2, "statuses": {"401": 1, "403": 1}},
                {"method": "GET", "path": "/remote-control/streamer/{key}", "total": 1, "statuses": {"101": 1}},
            ],
        })
    );
    assert_eq!(
        stats,
        json!({
            "accounts": {"total": 2, "paypal": 0, "stripe": 0, "premium": 0},
            "moblin": {"connectedDevices": 1},
            "pending": {"orders": 0, "deliveries": 0},
            "database": {"bytes": 0, "files": []},
            "counters": [
                {"name": "tipOrders", "today": 2, "week": 2, "total": 2},
            ],
            "signIn": {"restricted": false, "allowed": [], "requests": []},
            "premium": [],
        })
    );
}

#[tokio::test]
async fn admins_make_accounts_premium() {
    let (address, api) = start_api(None).await;
    let client = reqwest::Client::new();
    let (session, _, page_id) = streamer(&api);
    create_account(&api, "admin", "Admin", "admin");
    let admin = api.auth.session("admin", 0);
    let search = |session: String, text: &'static str| {
        client
            .get(format!("http://{address}/admin/accounts"))
            .query(&[("search", text)])
            .bearer_auth(session)
            .send()
    };
    let premium = |session: String, id: &'static str, premium: bool| {
        client
            .post(format!("http://{address}/admin/accounts/{id}/premium"))
            .bearer_auth(session)
            .json(&json!({"premium": premium}))
            .send()
    };
    let save_font = |font: &'static str| {
        client
            .post(format!("http://{address}/account/pages/{page_id}/settings"))
            .bearer_auth(&session)
            .json(&json!({"font": font}))
            .send()
    };
    let response = search(session.clone(), "erik").await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = premium(session.clone(), "1", true).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = premium(admin.clone(), "2", true).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(response.text().await.unwrap(), "No such account.");
    let found: Value = search(admin.clone(), "ERI")
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        found,
        json!([{
            "id": "1",
            "name": "Erik",
            "avatar": null,
            "identities": [{"platform": "twitch", "userId": "1", "login": "erik"}],
            "premium": false,
        }])
    );
    let found: Value = search(admin.clone(), "admin")
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(found[0]["premium"], false);
    let response = save_font("handwritten").await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let url = format!("http://{address}/account/pages/{page_id}/events");
    let mut events = Events::open(&client, &url, &session).await;
    let listed: Value = premium(admin.clone(), "1", true)
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listed[0]["id"], "1");
    assert_eq!(listed[0]["premium"], true);
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(events.next().await.0, "account");
    assert_eq!(account_of(&address, &session).await["premium"], true);
    let stats: Value = client
        .get(format!("http://{address}/admin/stats"))
        .bearer_auth(&admin)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(stats["premium"], listed);
    assert_eq!(stats["accounts"]["premium"], 1);
    let response = save_font("handwritten").await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(events.next().await.0, "account");
    let listed: Value = premium(admin, "1", false)
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listed, json!([]));
    assert_eq!(events.next().await.0, "account");
    let account = account_of(&address, &session).await;
    assert_eq!(account["premium"], false);
    assert_eq!(account["settings"]["font"], "handwritten");
    let response = save_font("serif").await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
