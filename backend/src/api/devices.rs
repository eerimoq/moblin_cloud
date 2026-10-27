use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::{iter, unfold};
use futures_util::{Stream, StreamExt};
use log::info;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::watch;
use tokio::time::{Instant, sleep_until};

use crate::remote_control::{Control, RequestError, Snapshot, Watch};

use super::*;

#[derive(Serialize)]
struct SnapshotResponse {
    name: String,
    #[serde(flatten)]
    snapshot: Snapshot,
}

fn known_device(api: &Api, page: &str, device: &str) -> Result<DeviceResponse, Failure> {
    devices(api, page)
        .into_iter()
        .find(|known| known.id == device)
        .ok_or(failure(
            "not-found",
            StatusCode::NOT_FOUND,
            "This Moblin is not in the list.",
        ))
}

fn request_failure(error: RequestError) -> Failure {
    match error {
        RequestError::Offline => failure(
            "offline",
            StatusCode::CONFLICT,
            "This Moblin is not connected.",
        ),
        RequestError::Timeout => failure(
            "moblin-timeout",
            StatusCode::GATEWAY_TIMEOUT,
            "Moblin did not answer. Try again.",
        ),
        RequestError::Failed => failure(
            "moblin-error",
            StatusCode::BAD_GATEWAY,
            "Moblin could not answer. Try again.",
        ),
    }
}

struct Streaming {
    api: Arc<Api>,
    headers: HeaderMap,
    page: String,
    watch: Watch,
    check_at: Instant,
    stopping: watch::Receiver<bool>,
}

pub(super) async fn handle_device_events(
    State(api): State<Arc<Api>>,
    Path((id, device)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, Failure> {
    moderator(&api, &headers, &id)?;
    let known = known_device(&api, &id, &device)?;
    let watch = api.remote_control.watch((&id, &device));
    let first = Event::default()
        .event("snapshot")
        .json_data(SnapshotResponse {
            name: known.name,
            snapshot: watch.snapshot.clone(),
        })
        .ok();
    let stopping = api.stopping.subscribe();
    let streaming = Streaming {
        api,
        headers,
        page: id,
        watch,
        check_at: Instant::now() + Duration::from_secs(60),
        stopping,
    };
    let stream = unfold(streaming, |mut streaming| async move {
        let mut stopping = streaming.stopping.clone();
        loop {
            tokio::select! {
                received = streaming.watch.events.recv() => {
                    let update = match received {
                        Ok(update) => update,
                        Err(RecvError::Closed | RecvError::Lagged(_)) => return None,
                    };
                    let event = Event::default().event(update.name()).json_data(&*update).ok()?;
                    return Some((Ok(event), streaming));
                }
                _ = stopping.wait_for(|stopping| *stopping) => return None,
                _ = sleep_until(streaming.check_at) => {
                    if moderator(&streaming.api, &streaming.headers, &streaming.page).is_err() {
                        return None;
                    }
                    streaming.check_at = Instant::now() + Duration::from_secs(60);
                }
            }
        }
    });
    Ok(Sse::new(iter(first.map(Ok)).chain(stream)).keep_alive(KeepAlive::default()))
}

pub(super) async fn handle_device_control(
    State(api): State<Arc<Api>>,
    Path((id, device)): Path<(String, String)>,
    headers: HeaderMap,
    control: Result<Json<Control>, JsonRejection>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let result = async {
        let signed_in = moderator(&api, &headers, &id)?;
        let Json(control) = control.map_err(|_| {
            failure(
                "invalid-control",
                StatusCode::UNPROCESSABLE_ENTITY,
                "unknown control",
            )
        })?;
        known_device(&api, &id, &device)?;
        api.remote_control
            .control((&id, &device), control.clone())
            .await
            .map_err(request_failure)?;
        info!(
            "{} sent {} to device {device} of page {id}",
            signed_in.id,
            serde_json::to_string(&control).unwrap_or_default()
        );
        Ok(accepted())
    }
    .await;
    counted(&api, "deviceControls", result)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::http::StatusCode;
    use futures_util::SinkExt;
    use serde_json::{Value, json};
    use tokio::time::timeout;
    use tokio_tungstenite::tungstenite::Message;
    use tokio_tungstenite::tungstenite::protocol::CloseFrame;
    use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

    use crate::accounts::Platform;
    use crate::api::test_utils::*;

    struct Events {
        response: reqwest::Response,
        buffer: String,
    }

    impl Events {
        async fn open(client: &reqwest::Client, url: &str, session: &str) -> Self {
            let response = client.get(url).bearer_auth(session).send().await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers()["content-type"].to_str().unwrap(),
                "text/event-stream"
            );
            Self {
                response,
                buffer: String::new(),
            }
        }

        async fn next(&mut self) -> (String, Value) {
            loop {
                if let Some(end) = self.buffer.find("\n\n") {
                    let block: String = self.buffer.drain(..end + 2).collect();
                    let mut name = String::new();
                    let mut data = None;
                    for line in block.lines() {
                        if let Some(value) = line.strip_prefix("event:") {
                            name = value.trim().to_string();
                        } else if let Some(value) = line.strip_prefix("data:") {
                            data = Some(serde_json::from_str(value.trim()).unwrap());
                        }
                    }
                    if let Some(data) = data {
                        return (name, data);
                    }
                    continue;
                }
                let chunk = timeout(Duration::from_secs(5), self.response.chunk())
                    .await
                    .expect("an event")
                    .unwrap()
                    .expect("an open stream");
                self.buffer.push_str(std::str::from_utf8(&chunk).unwrap());
            }
        }
    }

    fn settings() -> Value {
        json!({
            "streams": [{"id": "S1", "name": "Twitch"}],
            "scenes": [{"id": "C1", "name": "Main"}, {"id": "C2", "name": "BRB"}],
            "bitratePresets": [{"id": "B1", "bitrate": 5000000}],
            "mics": [{"id": "built-in", "name": "Built-in"}],
            "srt": {"connectionPrioritiesEnabled": false, "connectionPriorities": []},
        })
    }

    async fn answer_monitoring(socket: &mut Socket, started: bool) {
        if started {
            let request = receive(socket).await;
            assert_eq!(
                request["request"]["data"],
                json!({"startStatus": {"interval": 1, "filter": {"topRight": true}}})
            );
            answer(socket, &request, Value::Null).await;
            let request = receive(socket).await;
            assert_eq!(request["request"]["data"], json!({"getStatus": {}}));
            answer(socket, &request, Value::Null).await;
        }
        let request = receive(socket).await;
        assert_eq!(request["request"]["data"], json!({"getSettings": {}}));
        answer(
            socket,
            &request,
            json!({"getSettings": {"data": settings()}}),
        )
        .await;
    }

    async fn send_event(socket: &mut Socket, event: Value) {
        socket
            .send(Message::text(json!({"event": {"data": event}}).to_string()))
            .await
            .unwrap();
    }

    fn counter(api: &crate::api::Api, name: &str) -> u64 {
        api.counters
            .all()
            .into_iter()
            .find(|counter| counter.name == name)
            .map(|counter| counter.total)
            .unwrap_or(0)
    }

    #[tokio::test]
    async fn watchers_get_the_status_moblin_pushes() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, page) = streamer(&address, &api).await;
        let url = format!("http://{address}/account/pages/{page}/devices/A/events");
        let mut socket = connect(&address, &key).await;
        let mut events = Events::open(&client, &url, &session).await;
        answer_monitoring(&mut socket, true).await;
        let (name, snapshot) = events.next().await;
        assert_eq!(name, "snapshot");
        assert_eq!(
            snapshot,
            json!({
                "name": "Moblin 1",
                "connected": true,
                "status": {"general": {}, "topLeft": {}, "topRight": {}},
                "state": {},
                "settings": null,
                "history": [],
            })
        );
        assert_eq!(
            events.next().await,
            ("settings".to_string(), json!({"data": settings()}))
        );
        send_event(
            &mut socket,
            json!({"status": {
                "general": {"isLive": true, "isMuted": false},
                "topRight": {
                    "bitrate": {"message": "5.2 Mbps (1.2 GB)", "ok": true},
                    "bitrateInfo": {"bitsPerSecond": 5200000, "totalBytes": 1200000000u64},
                    "bondingInfo": [{"name": "Cellular", "bytesPerSecond": 400000, "rtt": 45}],
                },
            }}),
        )
        .await;
        let (name, status) = events.next().await;
        assert_eq!(name, "status");
        assert_eq!(status["general"], json!({"isLive": true, "isMuted": false}));
        assert_eq!(status["topLeft"], Value::Null);
        assert_eq!(
            status["topRight"]["bitrate"]["message"],
            "5.2 Mbps (1.2 GB)"
        );
        assert_eq!(status["sample"]["bitrate"], 5200000);
        assert_eq!(status["sample"]["totalBytes"], 1200000000u64);
        assert_eq!(
            status["sample"]["bonding"],
            json!([{"name": "Cellular", "bytesPerSecond": 400000, "rtt": 45}])
        );
        assert!(status["sample"]["at"].as_u64().unwrap() > 1_700_000_000_000);
        send_event(
            &mut socket,
            json!({"state": {"data": {"muted": true, "scene": "C2"}}}),
        )
        .await;
        assert_eq!(
            events.next().await,
            (
                "state".to_string(),
                json!({"data": {"muted": true, "scene": "C2"}})
            )
        );
        send_event(&mut socket, json!({"state": {"data": {"muted": false}}})).await;
        assert_eq!(
            events.next().await,
            ("state".to_string(), json!({"data": {"muted": false}}))
        );
        socket
            .send(Message::text(
                json!({"response": {"id": 999, "result": {"ok": {}}, "data": {"getStatus": {
                    "general": {"batteryLevel": 50},
                    "topLeft": {"camera": {"message": "Front", "ok": true}},
                    "topRight": {"bitrateInfo": {"bitsPerSecond": 1}},
                }}}})
                .to_string(),
            ))
            .await
            .unwrap();
        let (name, polled) = events.next().await;
        assert_eq!(name, "status");
        assert_eq!(polled["general"], json!({"batteryLevel": 50}));
        assert_eq!(polled["topLeft"]["camera"]["message"], "Front");
        assert_eq!(polled["sample"], Value::Null);
        create_account(&api, "21", "Mod 21", "mod21");
        api.accounts
            .add_moderator(
                Platform::Twitch,
                "1",
                &twitch_moderator("21", "mod21", "Mod 21"),
            )
            .unwrap();
        let mut second = Events::open(&client, &url, &api.auth.session("21", 0)).await;
        answer_monitoring(&mut socket, false).await;
        let (name, snapshot) = events.next().await;
        assert_eq!(name, "settings");
        let (name, snapshot2) = second.next().await;
        assert_eq!(name, "snapshot");
        assert_eq!(snapshot2["connected"], true);
        assert_eq!(snapshot2["status"]["general"], json!({"batteryLevel": 50}));
        assert_eq!(snapshot2["status"]["topLeft"]["camera"]["message"], "Front");
        assert_eq!(snapshot2["state"], json!({"muted": false, "scene": "C2"}));
        assert_eq!(snapshot2["settings"], settings());
        assert_eq!(snapshot2["history"].as_array().unwrap().len(), 1);
        assert_eq!(snapshot2["history"][0]["bitrate"], 5200000);
        let _ = snapshot;
        assert_eq!(second.next().await.0, "settings");
        drop(events);
        drop(second);
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], json!({"stopStatus": {}}));
        answer(&mut socket, &request, Value::Null).await;
        let response = client.get(&url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        create_account(&api, "22", "Viewer", "viewer");
        let response = client
            .get(&url)
            .bearer_auth(api.auth.session("22", 0))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = client
            .get(format!(
                "http://{address}/account/pages/{page}/devices/B/events"
            ))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response.text().await.unwrap(),
            "This Moblin is not in the list."
        );
    }

    #[tokio::test]
    async fn watchers_of_an_offline_device_wait_for_it() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, page) = streamer(&address, &api).await;
        api.accounts.seen_device(&page, "B");
        let url = format!("http://{address}/account/pages/{page}/devices/B/events");
        let mut events = Events::open(&client, &url, &session).await;
        let (name, snapshot) = events.next().await;
        assert_eq!(name, "snapshot");
        assert_eq!(snapshot["connected"], false);
        assert_eq!(snapshot["settings"], Value::Null);
        let mut socket = connect_as(&address, &key, "B").await;
        answer_monitoring(&mut socket, true).await;
        assert_eq!(
            events.next().await,
            ("connection".to_string(), json!({"connected": true}))
        );
        assert_eq!(
            events.next().await,
            ("settings".to_string(), json!({"data": settings()}))
        );
        socket
            .send(Message::Close(Some(CloseFrame {
                code: CloseCode::Normal,
                reason: "".into(),
            })))
            .await
            .unwrap();
        assert_eq!(
            events.next().await,
            ("connection".to_string(), json!({"connected": false}))
        );
        let mut second = Events::open(&client, &url, &session).await;
        let (name, snapshot) = second.next().await;
        assert_eq!(name, "snapshot");
        assert_eq!(snapshot["connected"], false);
        assert_eq!(snapshot["settings"], settings());
        let mut socket = connect_as(&address, &key, "B").await;
        answer_monitoring(&mut socket, true).await;
        assert_eq!(events.next().await.0, "connection");
        assert_eq!(second.next().await.0, "connection");
    }

    #[tokio::test]
    async fn controls_are_sent_to_moblin() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, page) = streamer(&address, &api).await;
        let url = format!("http://{address}/account/pages/{page}/devices/A/control");
        let mut socket = connect(&address, &key).await;
        let control = |body: Value| client.post(&url).bearer_auth(&session).json(&body).send();
        let call = tokio::spawn(control(
            json!({"setScene": {"id": "3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B"}}),
        ));
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"],
            json!({"setScene": {"id": "3f2b8c1e-5a4d-4e7f-9b6a-1c2d3e4f5a6b"}})
        );
        answer(&mut socket, &request, Value::Null).await;
        let response = call.await.unwrap().unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(counter(&api, "deviceControls"), 1);
        let call = tokio::spawn(control(json!({"setMic": {"id": "bt"}})));
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], json!({"setMic": {"id": "bt"}}));
        socket
            .send(Message::text(
                json!({"response": {"id": request["request"]["id"], "result": {"error": {}}}})
                    .to_string(),
            ))
            .await
            .unwrap();
        let response = call.await.unwrap().unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            response.text().await.unwrap(),
            "Moblin could not answer. Try again."
        );
        assert_eq!(counter(&api, "deviceControlsFailed:moblin-error"), 1);
        let response = control(json!({"tip": {"name": "x"}})).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let response = control(json!({"setScene": {"id": "C2"}})).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let response = client
            .post(&url)
            .json(&json!({"tip": {"name": "x"}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let response =
            control(json!({"startStatus": {"interval": 1, "filter": {"topRight": true}}}))
                .await
                .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(counter(&api, "deviceControlsFailed:invalid-control"), 3);
        create_account(&api, "22", "Viewer", "viewer");
        let response = client
            .post(&url)
            .bearer_auth(api.auth.session("22", 0))
            .json(&json!({"setMute": {"on": true}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(counter(&api, "deviceControlsFailed:not-moderator"), 1);
        let mut events = Events::open(
            &client,
            &format!("http://{address}/account/pages/{page}/devices/A/events"),
            &session,
        )
        .await;
        answer_monitoring(&mut socket, true).await;
        assert_eq!(events.next().await.0, "snapshot");
        assert_eq!(events.next().await.0, "settings");
        let call = tokio::spawn(control(
            json!({"setSrtConnectionPrioritiesEnabled": {"enabled": true}}),
        ));
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"],
            json!({"setSrtConnectionPrioritiesEnabled": {"enabled": true}})
        );
        answer(&mut socket, &request, Value::Null).await;
        assert_eq!(call.await.unwrap().unwrap().status(), StatusCode::ACCEPTED);
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], json!({"getSettings": {}}));
        let mut changed = settings();
        changed["srt"]["connectionPrioritiesEnabled"] = json!(true);
        answer(
            &mut socket,
            &request,
            json!({"getSettings": {"data": changed}}),
        )
        .await;
        let (name, event) = events.next().await;
        assert_eq!(name, "settings");
        assert_eq!(event["data"]["srt"]["connectionPrioritiesEnabled"], true);
    }

    #[tokio::test]
    async fn streams_end_when_the_backend_stops() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, page) = streamer(&address, &api).await;
        let url = format!("http://{address}/account/pages/{page}/devices/A/events");
        let mut socket = connect(&address, &key).await;
        let mut events = Events::open(&client, &url, &session).await;
        answer_monitoring(&mut socket, true).await;
        assert_eq!(events.next().await.0, "snapshot");
        assert_eq!(events.next().await.0, "settings");
        api.stopping.send_replace(true);
        loop {
            let chunk = timeout(Duration::from_secs(5), events.response.chunk())
                .await
                .expect("the stream to end")
                .unwrap();
            if chunk.is_none() {
                break;
            }
        }
    }

    #[tokio::test]
    async fn watching_follows_a_reconnecting_device() {
        let (address, api) =
            start_api_lingering(None, None, None, None, None, Duration::from_secs(5)).await;
        let client = reqwest::Client::new();
        let (session, key, page) = streamer(&address, &api).await;
        let url = format!("http://{address}/account/pages/{page}/devices/A/events");
        let socket = connect(&address, &key).await;
        let mut events = Events::open(&client, &url, &session).await;
        let mut socket = socket;
        answer_monitoring(&mut socket, true).await;
        assert_eq!(events.next().await.0, "snapshot");
        assert_eq!(events.next().await.0, "settings");
        drop(socket);
        assert_eq!(
            events.next().await,
            ("connection".to_string(), json!({"connected": false}))
        );
        let mut socket = connect(&address, &key).await;
        answer_monitoring(&mut socket, true).await;
        assert_eq!(
            events.next().await,
            ("connection".to_string(), json!({"connected": true}))
        );
        assert_eq!(events.next().await.0, "settings");
        let mut late = Events::open(&client, &url, &session).await;
        answer_monitoring(&mut socket, false).await;
        let (name, snapshot) = late.next().await;
        assert_eq!(name, "snapshot");
        assert_eq!(snapshot["connected"], true);
        assert_eq!(snapshot["settings"], settings());
    }
}
