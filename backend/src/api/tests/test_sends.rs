use axum::http::StatusCode;
use futures_util::SinkExt;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

use crate::api::test_utils::*;

#[tokio::test]
async fn tests_are_rate_limited_per_page() {
    let (address, api) = start_api(None).await;
    let client = reqwest::Client::new();
    let (session, _, _) = streamer(&api);
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
    let (session, moblin_cloud_key, _) = streamer(&api);
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
    let response = test("spin", json!({})).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(response.text().await.unwrap(), "Connect Moblin first.");
    let response = test("tip", json!({"message": " Testing "})).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let mut socket = connect(&address, &moblin_cloud_key).await;
    let request = receive(&mut socket).await;
    let tip = &request["request"]["data"]["tip"];
    assert_eq!(tip["name"], "Erik");
    assert_eq!(tip["message"], "Testing");
    assert_eq!(tip["kind"]["message"]["amount"], "5.00");
    assert_eq!(tip["kind"]["message"]["currency"], "USD");
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
    let spin = &request["request"]["data"]["tip"];
    assert_eq!(spin["name"], "Erik");
    assert_eq!(spin["kind"]["wheelOfLuck"]["count"], 1);
    answer(&mut socket, &request, Value::Null).await;
    wait_until_added(&api).await;
}
