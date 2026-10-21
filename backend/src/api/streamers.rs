use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::app_attest::AppAttest;
use crate::challenges::Challenges;
use crate::remote_control;
use crate::store::{Channel, Streamer};

use super::*;

const ASSERTION_HEADER: &str = "moblin-assertion";

#[derive(Serialize)]
pub(super) struct StreamersResponse {
    streamers: Vec<Streamer>,
}

pub(super) async fn handle_streamers(State(api): State<Arc<Api>>) -> Json<StreamersResponse> {
    Json(StreamersResponse {
        streamers: api.store.streamers(),
    })
}

#[derive(Serialize)]
pub(super) struct TwitchLiveResponse {
    channel: String,
    live: bool,
}

pub(super) async fn handle_twitch_live(
    State(api): State<Arc<Api>>,
) -> Result<Json<TwitchLiveResponse>, (StatusCode, String)> {
    let Some(live) = &api.live else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "no Twitch client ID and secret".to_string(),
        ));
    };
    let is_live = live.is_live().await.map_err(|error| {
        warn!("checking if {} is live failed: {error:#}", live.login());
        (StatusCode::SERVICE_UNAVAILABLE, format!("{error:#}"))
    })?;
    Ok(Json(TwitchLiveResponse {
        channel: live.login().to_string(),
        live: is_live,
    }))
}

#[derive(Serialize)]
pub(super) struct ChallengeResponse {
    challenge: String,
}

pub(super) async fn handle_challenge(
    State(api): State<Arc<Api>>,
) -> Result<Json<ChallengeResponse>, StatusCode> {
    let challenge = api
        .challenges
        .issue()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(ChallengeResponse {
        challenge: BASE64_STANDARD.encode(challenge),
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LiveRequest {
    channels: Vec<Channel>,
    challenge: Option<String>,
    key_id: Option<String>,
    attestation: Option<String>,
    attestation_challenge: Option<String>,
}

pub(super) async fn handle_streamers_live(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    let request: LiveRequest = serde_json::from_slice(&body)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    if let Some(app_attest) = &api.app_attest
        && let Err(error) = verify(app_attest, &api.challenges, &request, &headers, &body)
    {
        warn!("rejected live post: {error:#}");
        return Err((StatusCode::UNAUTHORIZED, format!("{error:#}")));
    }
    let channels = request.channels;
    Channel::validate_all(&channels)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    info!(
        "{} went live",
        channels
            .iter()
            .map(Channel::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    );
    api.store.streamer_live(channels);
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn handle_remote_control_streamer(
    State(api): State<Arc<Api>>,
    Path(key): Path<String>,
    upgrade: WebSocketUpgrade,
) -> Result<Response, StatusCode> {
    let Some(page) = api.accounts.page_of_key(&key) else {
        api.counters.bump("moblinConnectionsFailed:unknown-key");
        return Err(StatusCode::NOT_FOUND);
    };
    Ok(upgrade.on_upgrade(move |mut socket| async move {
        let device = match api.remote_control.identify(&mut socket).await {
            Ok(Some(device)) => device,
            Ok(None) => {
                api.counters.bump("moblinConnectionsFailed:no-device-id");
                remote_control::refuse(socket).await;
                return;
            }
            Err(error) => {
                info!("remote control streamer {page} failed to identify: {error:#}");
                return;
            }
        };
        let Some(connection) = api.remote_control.connect(&page, &device) else {
            info!("remote control streamer {page} device {device} rejected, too many devices");
            api.counters
                .bump("moblinConnectionsFailed:too-many-devices");
            remote_control::refuse(socket).await;
            return;
        };
        api.accounts.seen_device(&page, &device);
        api.counters.bump("moblinConnections");
        api.remote_control.serve(connection, socket).await;
        api.accounts.left_device(&page, &device);
    }))
}

fn verify(
    app_attest: &AppAttest,
    challenges: &Challenges,
    request: &LiveRequest,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<()> {
    let assertion = headers
        .get(ASSERTION_HEADER)
        .context("no assertion")?
        .to_str()
        .ok()
        .context("assertion is not text")?;
    let assertion = base64(assertion).context("assertion")?;
    let key_id = base64(request.key_id.as_deref().context("no key id")?).context("key id")?;
    let attestation =
        base64(request.attestation.as_deref().context("no attestation")?).context("attestation")?;
    let attestation_challenge = base64(
        request
            .attestation_challenge
            .as_deref()
            .context("no attestation challenge")?,
    )
    .context("attestation challenge")?;
    let challenge =
        base64(request.challenge.as_deref().context("no challenge")?).context("challenge")?;
    let public_key = app_attest
        .verify_attestation(&attestation, &key_id, &attestation_challenge)
        .context("bad attestation")?;
    app_attest
        .verify_assertion(&assertion, body, &public_key)
        .context("bad assertion")?;
    anyhow::ensure!(
        challenges.consume(&challenge),
        "the challenge is unknown, used or expired"
    );
    Ok(())
}

pub(super) fn base64(text: &str) -> Result<Vec<u8>> {
    BASE64_STANDARD.decode(text).context("not base64")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::http::StatusCode;
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{Value, json};
    use tokio_tungstenite::tungstenite::Message;

    use crate::api::test_utils::*;
    use crate::deliveries;

    async fn refused(address: &str, key: &str, identify: Value) {
        let (mut socket, _) = tokio_tungstenite::connect_async(format!(
            "ws://{address}/remote-control/streamer/{key}"
        ))
        .await
        .unwrap();
        assert_eq!(receive(&mut socket).await["hello"]["apiVersion"], "0.1");
        socket
            .send(Message::text(identify.to_string()))
            .await
            .unwrap();
        assert_eq!(
            receive(&mut socket).await,
            json!({"identified": {"result": {"error": {}}}})
        );
        assert!(matches!(
            socket.next().await,
            None | Some(Err(_)) | Some(Ok(Message::Close(_)))
        ));
    }

    #[tokio::test]
    async fn up_to_5_devices_connect_with_one_key() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        let mut sockets = Vec::new();
        for device in ["a", "b", "c", "d", "e"] {
            sockets.push(connect_as(&address, &key, device).await);
        }
        refused(
            &address,
            &key,
            json!({"identify": {"streamerId": "f", "authentication": "x"}}),
        )
        .await;
        refused(&address, &key, json!({"identify": {"authentication": "x"}})).await;
        sockets.push(connect_as(&address, &key, "a").await);
        let account: Value = client
            .get(format!("http://{address}/account"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(account["key"], key);
        assert_eq!(
            account["devices"],
            json!([
                {"id": "b", "name": "Moblin 2", "connected": true, "music": true, "wheel": false, "lastSeen": null},
                {"id": "c", "name": "Moblin 3", "connected": true, "music": false, "wheel": false, "lastSeen": null},
                {"id": "d", "name": "Moblin 4", "connected": true, "music": false, "wheel": false, "lastSeen": null},
                {"id": "e", "name": "Moblin 5", "connected": true, "music": false, "wheel": false, "lastSeen": null},
                {"id": "a", "name": "Moblin 1", "connected": true, "music": false, "wheel": false, "lastSeen": null},
            ])
        );
        assert_eq!(api.remote_control.connected_count(), 5);
        let counters = api.counters.all();
        let count = |name: &str| {
            counters
                .iter()
                .find(|counter| counter.name == name)
                .map(|counter| counter.total)
        };
        assert_eq!(count("moblinConnections"), Some(6));
        assert_eq!(count("moblinConnectionsFailed:too-many-devices"), Some(1));
        assert_eq!(count("moblinConnectionsFailed:no-device-id"), Some(1));
        let rename = |device: &str, name: &str| {
            client
                .post(format!("http://{address}/account/devices/{device}/name"))
                .bearer_auth(&session)
                .json(&json!({"name": name}))
                .send()
        };
        let response = rename("x", "Backup").await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let response = rename("b", &"x".repeat(31)).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let account: Value = rename("b", " Backup ").await.unwrap().json().await.unwrap();
        assert_eq!(account["devices"][0]["name"], "Backup");
        let response = rename("b", " ").await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let response = client
            .post(format!("http://{address}/account/key"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let account: Value = response.json().await.unwrap();
        assert_ne!(account["key"], key);
        assert!(!api.remote_control.is_connected(&id));
        let devices = account["devices"].as_array().unwrap();
        assert_eq!(devices.len(), 5);
        assert!(devices.iter().all(|device| device["connected"] == false
            && device["music"] == false
            && device["lastSeen"].is_u64()));
        assert!(
            devices
                .iter()
                .any(|device| device["id"] == "b" && device["name"] == "Backup")
        );
    }

    async fn send_event(socket: &mut Socket, event: Value) {
        socket
            .send(Message::text(json!({"event": {"data": event}}).to_string()))
            .await
            .unwrap();
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(socket).await, json!({"pong": {}}));
    }

    #[tokio::test]
    async fn songs_and_spins_go_to_the_first_device_taking_them_and_tips_to_all() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        let mut main = connect_as(&address, &key, "main").await;
        let mut other = connect_as(&address, &key, "other").await;
        send_event(&mut main, json!({"songRequests": {"enabled": false}})).await;
        send_event(
            &mut other,
            json!({"wheelOfLuck": {"wheel": {"options": [{"text": "A", "weight": 1}]}}}),
        )
        .await;
        let account: Value = client
            .get(format!("http://{address}/account"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            account["devices"],
            json!([
                {"id": "main", "name": "Moblin 1", "connected": true, "music": false, "wheel": false, "lastSeen": null},
                {"id": "other", "name": "Moblin 2", "connected": true, "music": true, "wheel": true, "lastSeen": null},
            ])
        );
        let spin_url = format!("http://{address}/pages/{id}/spin");
        let spin: Value = client
            .get(&spin_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(spin["wheel"]["options"][0]["text"], "A");
        let queue_url = format!("http://{address}/pages/{id}/songs/queue");
        let queue = |url: String| {
            tokio::spawn(async move {
                reqwest::get(url)
                    .await
                    .unwrap()
                    .json::<Value>()
                    .await
                    .unwrap()
            })
        };
        let status = queue(queue_url.clone());
        let request = receive(&mut other).await;
        answer(&mut other, &request, music(json!([]))).await;
        assert_eq!(status.await.unwrap()["songRequests"], true);
        send_event(&mut other, json!({"songRequests": {"enabled": false}})).await;
        let status = queue(queue_url.clone());
        let request = receive(&mut main).await;
        answer(&mut main, &request, music(json!([]))).await;
        assert_eq!(status.await.unwrap()["songRequests"], false);
        let account: Value = client
            .get(format!("http://{address}/account"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let devices = account["devices"].as_array().unwrap();
        assert!(devices.iter().all(|device| device["music"] == false));
        api.deliveries.add_test(
            id.clone(),
            "TIP1".to_string(),
            deliveries::Item::Tip {
                user: "Viewer".to_string(),
                message: String::new(),
                amount: 500,
                currency: "USD".to_string(),
            },
        );
        for socket in [&mut main, &mut other] {
            let request = receive(socket).await;
            assert_eq!(request["request"]["data"]["tip"]["requestId"], "TIP1");
            answer(socket, &request, Value::Null).await;
        }
        wait_until_added(&api).await;
        other.close(None).await.unwrap();
        for _ in 0..100 {
            if api.remote_control.devices(&id).len() == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let spin: Value = client
            .get(&spin_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(spin["wheel"], Value::Null);
        assert_eq!(spin["connected"], true);
    }
}
