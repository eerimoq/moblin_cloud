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

use super::*;

#[derive(Deserialize)]
pub(super) struct TestRequest {
    #[serde(default)]
    song: Option<String>,
    #[serde(default)]
    message: Option<String>,
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

pub(super) async fn handle_account_test_song(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<TestRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let result = account_test_song(&api, &headers, request).await;
    counted(&api, "songTests", result)
}

async fn account_test_song(
    api: &Api,
    headers: &HeaderMap,
    request: TestRequest,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let (id, account, request_id) = test_page(api, headers, None)?;
    if !api.remote_control.is_connected(&id) {
        return Err(moblin_not_connected());
    }
    let message = viewer_message(request.message)?;
    let request = SongRequest {
        song: request.song.unwrap_or_default(),
        user: Some(account.name().to_string()),
        message: None,
        provider: None,
    };
    let song = requested_song(api, &id, song_and_user(request)?).await?;
    Ok(queue(api, id, request_id, song, message))
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
        for kind in ["song", "spin"] {
            let response = test(kind, json!({"song": LINK})).await.unwrap();
            assert_eq!(response.status(), StatusCode::CONFLICT);
            assert_eq!(response.text().await.unwrap(), "Connect Moblin first.");
        }
        let response = test("tip", json!({"message": " Testing "})).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let mut socket = connect(&address, &key).await;
        let request = receive(&mut socket).await;
        let tip = &request["request"]["data"]["tip"];
        assert_eq!(tip["name"], "Erik");
        assert_eq!(tip["message"], "Testing");
        assert_eq!(tip["amount"], "5.00");
        assert_eq!(tip["currency"], "USD");
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
        let spin = &request["request"]["data"]["spinWheelOfLuck"];
        assert_eq!(spin["name"], "Erik");
        assert_eq!(spin["count"], 1);
        answer(&mut socket, &request, Value::Null).await;
        wait_until_added(&api).await;
        let response = test("song", json!({"song": "nope"})).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let created = tokio::spawn(test("song", json!({"song": LINK, "message": " Testing "})));
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"],
            json!({"music": {"command": {"find": {"title": LINK}}}})
        );
        answer(
            &mut socket,
            &request,
            json!({"musicFind": {"song": {"id": "1440818839", "title": "Song", "artist": "Band"}}}),
        )
        .await;
        assert_eq!(
            created.await.unwrap().unwrap().status(),
            StatusCode::ACCEPTED
        );
        let request = receive(&mut socket).await;
        let add = &request["request"]["data"]["music"]["command"]["add"];
        assert_eq!(add["title"], LINK);
        assert_eq!(add["user"], "Erik");
        assert_eq!(add["message"], "Testing");
    }
}
