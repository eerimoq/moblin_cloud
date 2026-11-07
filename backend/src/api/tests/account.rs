use axum::http::{Method, StatusCode};
use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

use crate::accounts::Platform;
use crate::api::test_utils::*;

#[tokio::test]
async fn new_moblin_cloud_keys_keep_the_page() {
    let (address, api) = start_api(None).await;
    let (session, moblin_cloud_key, page_id) = streamer(&api);
    let mut socket = connect(&address, &moblin_cloud_key).await;
    assert!(api.remote_control.is_connected(&page_id));
    let account: Value = reqwest::Client::new()
        .post(format!("http://{address}/account/key"))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let new_moblin_cloud_key = moblin_cloud_key_of(&account);
    assert_ne!(new_moblin_cloud_key, moblin_cloud_key);
    assert_eq!(account["settings"]["id"], page_id);
    assert!(!api.remote_control.is_connected(&page_id));
    loop {
        match socket.next().await {
            Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
            Some(Ok(_)) => {}
        }
    }
    assert!(
        tokio_tungstenite::connect_async(format!(
            "ws://{address}/remote-control/streamer/{moblin_cloud_key}"
        ))
        .await
        .is_err()
    );
    let _socket = connect(&address, &new_moblin_cloud_key).await;
    assert!(api.remote_control.is_connected(&page_id));
    let name: Value = reqwest::Client::new()
        .get(format!("http://{address}/pages/twitch/erik"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(name["id"], page_id);
}

#[tokio::test]
async fn signing_out_ends_every_session() {
    let (address, api) = start_api(None).await;
    let (session, _, _) = streamer(&api);
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
    let (session, moblin_cloud_key, page_id) = streamer(&api);
    let assistant = create_account(&api, "21", "Mod 21", "mod21");
    api.accounts
        .add_assistant(
            Platform::Twitch,
            "1",
            &twitch_assistant("21", "mod21", "Mod 21"),
        )
        .unwrap();
    api.accounts
        .add_assistant(
            Platform::Twitch,
            "21",
            &twitch_assistant("1", "erik", "Erik"),
        )
        .unwrap();
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
        page_id.clone(),
        "r".to_string(),
        crate::deliveries::Item::Tip {
            user: "Erik".to_string(),
            message: String::new(),
            amount: 500,
            currency: "USD".to_string(),
        },
    );
    let socket = connect(&address, &moblin_cloud_key).await;
    assert!(api.remote_control.is_connected(&page_id));
    assert_eq!(api.deliveries.count(), 1);
    let response = client
        .post(format!("http://{address}/account/delete"))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    drop(socket);
    assert!(!api.remote_control.is_connected(&page_id));
    assert_eq!(api.deliveries.count(), 0);
    assert!(api.accounts.get("1").is_none());
    assert!(
        api.accounts
            .page_of_login(Platform::Twitch, "erik")
            .is_none()
    );
    assert!(
        api.accounts
            .page_of_moblin_cloud_key(&moblin_cloud_key)
            .is_none()
    );
    assert!(api.accounts.assistants(Platform::Twitch, "21").is_empty());
    assert!(api.accounts.assisted(Platform::Twitch, "21").is_empty());
    assert_eq!(api.accounts.get("21").unwrap(), assistant);
    for (table, column, value) in [
        ("pages", "id", page_id.as_str()),
        ("twitch_assistants", "broadcaster", "1"),
        ("twitch_assistants", "assistant", "1"),
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
    for path in [
        "/pages/twitch/erik".to_string(),
        format!("/pages/{page_id}"),
    ] {
        let response = client
            .get(format!("http://{address}{path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    assert!(
        tokio_tungstenite::connect_async(format!(
            "ws://{address}/remote-control/streamer/{moblin_cloud_key}"
        ))
        .await
        .is_err()
    );
    let again = create_account(&api, "1", "Erik", "erik");
    assert_ne!(again.page_id, page_id);
    assert_eq!(
        api.accounts.page_of_login(Platform::Twitch, "erik"),
        Some(again.page_id)
    );
}

#[tokio::test]
async fn account_routes_need_a_session() {
    let (address, api) = start_api(None).await;
    let (session, _, _) = streamer(&api);
    let unknown = api.auth.session("2", 0);
    let old = api.auth.session("1", 1);
    let (payload, _) = session.split_once('.').unwrap();
    let forged = format!("{payload}.AAAA");
    let client = reqwest::Client::new();
    for (method, path) in [
        ("GET", "/account"),
        ("POST", "/account/key"),
        ("POST", "/account/devices/d/name"),
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
                .json(&json!({"name": "Moblin", "wheel": "d", "enabled": false}));
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
    let unknown_key = api.remote_control.new_moblin_cloud_key();
    assert!(
        tokio_tungstenite::connect_async(format!(
            "ws://{address}/remote-control/streamer/{unknown_key}"
        ))
        .await
        .is_err()
    );
    assert!(api.counters.all().is_empty());
}
