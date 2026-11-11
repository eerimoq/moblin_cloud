use std::time::Duration;

use axum::http::StatusCode;
use futures_util::SinkExt;
use serde_json::{Value, json};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

use crate::accounts::Platform;
use crate::api::test_utils::*;
use crate::payments::{Item, Order, Payee};

async fn quiet(events: &mut Events) {
    assert!(
        timeout(Duration::from_millis(300), events.next())
            .await
            .is_err()
    );
}

fn captured_tip(api: &crate::api::Api, page_id: &str) {
    let order = Order {
        streamer: page_id.to_string(),
        item: Item::Tip {
            message: Some("Hi".to_string()),
        },
        user: "Viewer".to_string(),
        payee: Payee::PayPal("MERCHANT00001".to_string()),
        price: 500,
        currency: "USD".to_string(),
    };
    api.orders.insert("TIP1", &order).unwrap();
    api.orders.captured("TIP1", |_| Ok(())).unwrap();
}

#[tokio::test]
async fn dashboards_hear_about_changes() {
    let (address, api) = start_api(None).await;
    let client = reqwest::Client::new();
    let (session, moblin_cloud_key, page_id) = streamer(&api);
    let url = format!("http://{address}/account/pages/{page_id}/events");
    let mut events = Events::open(&client, &url, &session).await;
    let mut socket = connect(&address, &moblin_cloud_key).await;
    assert_eq!(events.next().await, ("account".to_string(), json!({})));
    let response = client
        .post(format!("http://{address}/account/pages/{page_id}/settings"))
        .bearer_auth(&session)
        .json(&json!({"message": "Hello"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(events.next().await.0, "account");
    let response = client
        .post(format!("http://{address}/account/devices/A/name"))
        .bearer_auth(&session)
        .json(&json!({"name": "Phone"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(events.next().await.0, "account");
    captured_tip(&api, &page_id);
    let response = client
        .post(format!(
            "http://{address}/account/pages/{page_id}/recent/TIP1/resend"
        ))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(events.next().await.0, "recent");
    let request = receive(&mut socket).await;
    assert_eq!(request["request"]["data"]["tip"]["name"], "Viewer");
    answer(&mut socket, &request, Value::Null).await;
    assert_eq!(events.next().await.0, "recent");
    drop(socket);
    assert_eq!(events.next().await.0, "account");
}

#[tokio::test]
async fn assistants_hear_about_their_channels() {
    let (address, api) = start_api(None).await;
    let client = reqwest::Client::new();
    let (session, _, page_id) = streamer(&api);
    let assistant = create_account(&api, "21", "Mod 21", "mod21");
    let assistant_session = api.auth.session("21", 0);
    let url = format!("http://{address}/account/pages/{page_id}/events");
    let response = client
        .get(&url)
        .bearer_auth(&assistant_session)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = client.get(&url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    api.accounts
        .add_assistant(
            Platform::Twitch,
            "1",
            &twitch_assistant("21", "mod21", "Mod 21"),
        )
        .unwrap();
    let mut assisted = Events::open(&client, &url, &assistant_session).await;
    let mut own = Events::open(
        &client,
        &format!(
            "http://{address}/account/pages/{}/events",
            assistant.page_id
        ),
        &assistant_session,
    )
    .await;
    let response = client
        .post(format!(
            "http://{address}/account/assistants/twitch/21/remove"
        ))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(assisted.next().await.0, "account");
    assert_eq!(own.next().await.0, "account");
}

#[tokio::test]
async fn tip_pages_hear_about_changes() {
    let (address, api) = start_api(None).await;
    let client = reqwest::Client::new();
    let (session, moblin_cloud_key, page_id) = streamer(&api);
    let mut events = Events::open(
        &client,
        &format!("http://{address}/pages/{page_id}/events"),
        "",
    )
    .await;
    let mut socket = connect(&address, &moblin_cloud_key).await;
    assert_eq!(events.next().await, ("page".to_string(), json!({})));
    socket
        .send(Message::text(
            json!({"event": {"data": {"icon": {"id": "AppIconKing"}}}}).to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(events.next().await.0, "page");
    let response = client
        .post(format!("http://{address}/account/devices/A/name"))
        .bearer_auth(&session)
        .json(&json!({"name": "Phone"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    captured_tip(&api, &page_id);
    let response = client
        .post(format!(
            "http://{address}/account/pages/{page_id}/recent/TIP1/resend"
        ))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    quiet(&mut events).await;
    let response = client
        .post(format!("http://{address}/account/pages/{page_id}/settings"))
        .bearer_auth(&session)
        .json(&json!({"message": "Hello"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(events.next().await.0, "page");
    drop(socket);
    assert_eq!(events.next().await.0, "page");
    let response = client
        .get(format!("http://{address}/pages/nobody/events"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
