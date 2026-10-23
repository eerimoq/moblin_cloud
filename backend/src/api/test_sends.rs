use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use log::info;
use serde::Deserialize;
use serde_json::Value;

use crate::accounts::Account;
use crate::deliveries;
use crate::effects::EFFECTS;

use super::*;

#[derive(Deserialize)]
pub(super) struct TestRequest {
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    effect: Option<String>,
}

fn test_page(
    api: &Api,
    headers: &HeaderMap,
    page: Option<&str>,
) -> Result<(String, Account, String), Failure> {
    let (id, account) = match page {
        Some(page) => (page.to_string(), moderator(api, headers, page)?.account),
        None => {
            let account = signed_in(api, headers)?.account;
            (account.page.id.clone(), account)
        }
    };
    if !within_limit(&api.tests, &id, 10) {
        return Err(failure(
            "too-many-tests",
            StatusCode::TOO_MANY_REQUESTS,
            "Too many tests. Try again in a minute.",
        ));
    }
    let request_id = api.remote_control.new_request_id().ok_or(failure(
        "crashed",
        StatusCode::SERVICE_UNAVAILABLE,
        "The test could not be sent. Try again.",
    ))?;
    Ok((id, account, request_id))
}

fn moblin_not_connected() -> Failure {
    failure("offline", StatusCode::CONFLICT, "Connect Moblin first.")
}

pub(super) async fn handle_account_test_tip(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<TestRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let result = account_test_tip(&api, &headers, None, request).await;
    counted(&api, "tipTests", result)
}

pub(super) async fn handle_moderator_test_tip(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<TestRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let result = account_test_tip(&api, &headers, Some(&id), request).await;
    counted(&api, "tipTests", result)
}

async fn account_test_tip(
    api: &Api,
    headers: &HeaderMap,
    page: Option<&str>,
    request: TestRequest,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let (id, account, request_id) = test_page(api, headers, page)?;
    let settings = api.pages.get(&id);
    let amount = settings
        .tips()
        .and_then(|tips| tips.amounts.first().copied())
        .unwrap_or(500);
    info!("queued a test tip in request {request_id} for remote control streamer {id}");
    api.deliveries.add_test(
        id,
        request_id,
        deliveries::Item::Tip {
            user: account.name().to_string(),
            message: viewer_message(request.message)?.unwrap_or_default(),
            amount,
            currency: settings.currency().to_string(),
        },
    );
    Ok(accepted())
}

pub(super) async fn handle_account_test_spin(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<TestRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let result = account_test_spin(&api, &headers, None, request).await;
    counted(&api, "spinTests", result)
}

pub(super) async fn handle_moderator_test_spin(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<TestRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let result = account_test_spin(&api, &headers, Some(&id), request).await;
    counted(&api, "spinTests", result)
}

async fn account_test_spin(
    api: &Api,
    headers: &HeaderMap,
    page: Option<&str>,
    request: TestRequest,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let (id, account, request_id) = test_page(api, headers, page)?;
    if api.remote_control.wheel(&id).is_none() {
        return Err(if api.remote_control.is_connected(&id) {
            failure(
                "no-wheel",
                StatusCode::CONFLICT,
                "Pick a wheel of luck in Moblin's Moblin Cloud settings first.",
            )
        } else {
            moblin_not_connected()
        });
    }
    let spins = api
        .pages
        .get(&id)
        .spin()
        .and_then(|spin| spin.options.first().map(|option| option.spins))
        .unwrap_or(1);
    info!("queued a test spin in request {request_id} for remote control streamer {id}");
    api.deliveries.add_test(
        id,
        request_id,
        deliveries::Item::Spin {
            user: account.name().to_string(),
            message: viewer_message(request.message)?.unwrap_or_default(),
            spins,
        },
    );
    Ok(accepted())
}

pub(super) async fn handle_account_test_effect(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<TestRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let result = account_test_effect(&api, &headers, None, request).await;
    counted(&api, "effectTests", result)
}

pub(super) async fn handle_moderator_test_effect(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<TestRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let result = account_test_effect(&api, &headers, Some(&id), request).await;
    counted(&api, "effectTests", result)
}

async fn account_test_effect(
    api: &Api,
    headers: &HeaderMap,
    page: Option<&str>,
    request: TestRequest,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let (id, account, request_id) = test_page(api, headers, page)?;
    let effect = viewer_effect(request.effect.as_deref().unwrap_or(EFFECTS[0].id))?;
    let message = viewer_message(request.message)?.unwrap_or_default();
    info!("queued a test effect in request {request_id} for remote control streamer {id}");
    api.deliveries.add_test(
        id,
        request_id,
        deliveries::Item::Effect {
            user: account.name().to_string(),
            message,
            effect,
        },
    );
    Ok(accepted())
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use futures_util::SinkExt;
    use serde_json::{Value, json};
    use tokio_tungstenite::tungstenite::Message;

    use crate::api::test_utils::*;

    #[tokio::test]
    async fn tests_are_rate_limited_per_page() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, _, _) = streamer(&address, &api).await;
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
        let (session, key, _) = streamer(&address, &api).await;
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
        let mut socket = connect(&address, &key).await;
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
}
