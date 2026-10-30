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

use crate::accounts::{Access, Permission};
use crate::remote_control::{Control, PreviewWatch, RequestError, Snapshot, Watch};

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
        RequestError::Unsupported => failure(
            "moblin-unsupported",
            StatusCode::CONFLICT,
            "This Moblin can't do that. Update Moblin on the device and try again.",
        ),
    }
}

fn controller(api: &Api, headers: &HeaderMap, page: &str) -> Result<SignedIn, Failure> {
    let signed_in = watcher(api, headers, page)?;
    if access(api, &signed_in.account, page) < Access::Control {
        return Err(failure(
            "no-control",
            StatusCode::FORBIDDEN,
            "The streamer has not let you control Moblin.",
        ));
    }
    Ok(signed_in)
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
    watcher(&api, &headers, &id)?;
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
                    if watcher(&streaming.api, &streaming.headers, &streaming.page).is_err() {
                        return None;
                    }
                    streaming.check_at = Instant::now() + Duration::from_secs(60);
                }
            }
        }
    });
    Ok(Sse::new(iter(first.map(Ok)).chain(stream)).keep_alive(KeepAlive::default()))
}

#[derive(Serialize)]
struct Frame<'a> {
    image: &'a str,
}

fn frame_event(image: &str) -> Option<Event> {
    Event::default()
        .event("frame")
        .json_data(Frame { image })
        .ok()
}

struct Previewing {
    api: Arc<Api>,
    headers: HeaderMap,
    page: String,
    watch: PreviewWatch,
    check_at: Instant,
    stopping: watch::Receiver<bool>,
}

pub(super) async fn handle_device_preview(
    State(api): State<Arc<Api>>,
    Path((id, device)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, Failure> {
    previewer(&api, &headers, &id)?;
    known_device(&api, &id, &device)?;
    let watch = api.remote_control.watch_preview((&id, &device));
    let first = watch.frame.as_deref().and_then(frame_event);
    let stopping = api.stopping.subscribe();
    let previewing = Previewing {
        api,
        headers,
        page: id,
        watch,
        check_at: Instant::now() + Duration::from_secs(60),
        stopping,
    };
    let stream = unfold(previewing, |mut previewing| async move {
        let mut stopping = previewing.stopping.clone();
        loop {
            tokio::select! {
                received = previewing.watch.frames.recv() => {
                    match received {
                        Ok(frame) => return Some((Ok(frame_event(&frame)?), previewing)),
                        Err(RecvError::Lagged(_)) => {}
                        Err(RecvError::Closed) => return None,
                    }
                }
                _ = stopping.wait_for(|stopping| *stopping) => return None,
                _ = sleep_until(previewing.check_at) => {
                    if previewer(&previewing.api, &previewing.headers, &previewing.page).is_err() {
                        return None;
                    }
                    previewing.check_at = Instant::now() + Duration::from_secs(60);
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
    let signed_in = watcher(&api, &headers, &id)?;
    if !matches!(control, Ok(Json(Control::SendMessage { .. })))
        && access(&api, &signed_in.account, &id) < Access::Control
    {
        return Err(failure(
            "no-control",
            StatusCode::FORBIDDEN,
            "The streamer has not let you control Moblin.",
        ));
    }
    let Some(Json(mut control)) = control.ok().filter(|Json(control)| control.valid()) else {
        return Err(failure(
            "invalid-control",
            StatusCode::UNPROCESSABLE_ENTITY,
            "unknown control",
        ));
    };
    if matches!(control, Control::SetLive { on: true })
        && !may(&api, &signed_in.account, &id, Permission::GoLive)
    {
        return Err(failure(
            "no-go-live",
            StatusCode::FORBIDDEN,
            "The streamer has not let you go live.",
        ));
    }
    if let Control::SendMessage { name, .. } = &mut control {
        *name = Some(signed_in.account.name().to_string()).filter(|name| !name.is_empty());
    }
    known_device(&api, &id, &device)?;
    api.remote_control
        .control((&id, &device), control.clone())
        .await
        .map_err(request_failure)?;
    info!(
        "{} sent {} to device {device} of page {id}",
        signed_in.id,
        control.logged()
    );
    Ok(accepted())
}

pub(super) async fn handle_device_scenes(
    State(api): State<Arc<Api>>,
    Path((id, device)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    controller(&api, &headers, &id)?;
    known_device(&api, &id, &device)?;
    let scenes = api
        .remote_control
        .scenes((&id, &device))
        .await
        .map_err(request_failure)?;
    Ok(Json(scenes))
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

    use crate::accounts::{Access, Platform};
    use crate::api::test_utils::*;

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

    #[tokio::test]
    async fn watchers_get_the_status_moblin_pushes() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, page) = streamer(&api);
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
            .add_assistant(
                Platform::Twitch,
                "1",
                &twitch_assistant("21", "mod21", "Mod 21"),
            )
            .unwrap();
        api.accounts
            .set_assistant_access(Platform::Twitch, "1", "21", Access::View)
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
        let (session, key, page) = streamer(&api);
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
        let (session, key, page) = streamer(&api);
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
        let call = tokio::spawn(control(
            json!({"sendMessage": {"text": "Hi!", "name": "Someone else"}}),
        ));
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"],
            json!({"sendMessage": {"text": "Hi!", "name": "Erik"}})
        );
        answer(&mut socket, &request, Value::Null).await;
        let response = call.await.unwrap().unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
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
        let response = control(json!({"sendMessage": {"text": " "}}))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        create_account(&api, "22", "Viewer", "viewer");
        let response = client
            .post(&url)
            .bearer_auth(api.auth.session("22", 0))
            .json(&json!({"setMute": {"on": true}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
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
    async fn scenes_are_read_and_edited_on_moblin() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, page) = streamer(&api);
        let mut socket = connect(&address, &key).await;
        let base = format!("http://{address}/account/pages/{page}/devices/A");
        let url = format!("{base}/control");
        let control = |body: Value| client.post(&url).bearer_auth(&session).json(&body).send();
        let read = tokio::spawn(
            client
                .get(format!("{base}/scenes"))
                .bearer_auth(&session)
                .send(),
        );
        for _ in 0..3 {
            let request = receive(&mut socket).await;
            let data = &request["request"]["data"];
            let answered = if data.get("getScenes").is_some() {
                json!({"getScenes": {"scenes": [{"id": "C1", "name": "Main", "widgets": []}]}})
            } else if data.get("getWidgets").is_some() {
                json!({"getWidgets": {"widgets": [{"id": "W1", "name": "Clock", "type": "Text"}]}})
            } else {
                assert_eq!(data, &json!({"getVideoSources": {}}));
                json!({"getVideoSources": {"videoSources": [{"id": "N", "name": "None", "position": "None"}]}})
            };
            answer(&mut socket, &request, answered).await;
        }
        let response = read.await.unwrap().unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.json::<Value>().await.unwrap(),
            json!({
                "scenes": [{"id": "C1", "name": "Main", "widgets": []}],
                "widgets": [{"id": "W1", "name": "Clock", "type": "Text"}],
                "cameras": [{"id": "N", "name": "None", "position": "None"}],
            })
        );
        let mut events = Events::open(&client, &format!("{base}/events"), &session).await;
        answer_monitoring(&mut socket, true).await;
        assert_eq!(events.next().await.0, "snapshot");
        assert_eq!(events.next().await.0, "settings");
        let scene =
            json!({"id": "3f2b8c1e-5a4d-4e7f-9b6a-1c2d3e4f5a6b", "name": "New", "unknown": [1]});
        let call = tokio::spawn(control(json!({"createScene": {"scene": scene}})));
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"],
            json!({"createScene": {"scene": scene}})
        );
        answer(&mut socket, &request, Value::Null).await;
        assert_eq!(call.await.unwrap().unwrap().status(), StatusCode::ACCEPTED);
        assert_eq!(events.next().await, ("scenes".to_string(), json!({})));
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], json!({"getSettings": {}}));
        answer(
            &mut socket,
            &request,
            json!({"getSettings": {"data": settings()}}),
        )
        .await;
        assert_eq!(events.next().await.0, "settings");
        let call = tokio::spawn(control(
            json!({"setWidgetImage": {"id": "3f2b8c1e-5a4d-4e7f-9b6a-1c2d3e4f5a6b", "image": "iVBORw0K"}}),
        ));
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"]["setWidgetImage"]["image"],
            "iVBORw0K"
        );
        socket
            .send(Message::text(
                json!({"response": {"id": request["request"]["id"], "result": {"unknownRequest": {}}}})
                    .to_string(),
            ))
            .await
            .unwrap();
        let response = call.await.unwrap().unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.text().await.unwrap(),
            "This Moblin can't do that. Update Moblin on the device and try again."
        );
        for body in [
            json!({"createScene": {"scene": {"name": "No id"}}}),
            json!({"updateWidget": {"widget": {"id": "W1"}}}),
            json!({"setWidgetImage": {"id": "3f2b8c1e-5a4d-4e7f-9b6a-1c2d3e4f5a6b", "image": "not base64!"}}),
            json!({"deleteScene": {"id": "C1"}}),
        ] {
            let response = control(body).await.unwrap();
            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        }
    }

    #[tokio::test]
    async fn assistants_edit_scenes_only_with_control() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, _key, page) = streamer(&api);
        create_account(&api, "21", "Mod 21", "mod21");
        api.accounts
            .add_assistant(
                Platform::Twitch,
                "1",
                &twitch_assistant("21", "mod21", "Mod 21"),
            )
            .unwrap();
        api.accounts
            .set_assistant_access(Platform::Twitch, "1", "21", Access::View)
            .unwrap();
        let assistant = api.auth.session("21", 0);
        let scenes = format!("http://{address}/account/pages/{page}/devices/A/scenes");
        let response = client
            .get(&scenes)
            .bearer_auth(&assistant)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response.text().await.unwrap(),
            "The streamer has not let you control Moblin."
        );
        let response = client
            .get(&scenes)
            .bearer_auth(&session)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn watchers_get_the_preview() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, page) = streamer(&api);
        let mut socket = connect(&address, &key).await;
        let url = format!("http://{address}/account/pages/{page}/devices/A/preview");
        let mut preview = Events::open(&client, &url, &session).await;
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], json!({"startPreview": {}}));
        answer(&mut socket, &request, Value::Null).await;
        socket
            .send(Message::text(
                json!({"preview": {"preview": "/9j/1"}}).to_string(),
            ))
            .await
            .unwrap();
        assert_eq!(
            preview.next().await,
            ("frame".to_string(), json!({"image": "/9j/1"}))
        );
        let mut second = Events::open(&client, &url, &session).await;
        assert_eq!(
            second.next().await,
            ("frame".to_string(), json!({"image": "/9j/1"}))
        );
        drop(preview);
        drop(second);
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], json!({"stopPreview": {}}));
    }

    #[tokio::test]
    async fn assistants_view_and_send_messages_but_control_the_device_only_when_allowed() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, page) = streamer(&api);
        let mut socket = connect(&address, &key).await;
        create_account(&api, "21", "Mod 21", "mod21");
        api.accounts
            .add_assistant(
                Platform::Twitch,
                "1",
                &twitch_assistant("21", "mod21", "Mod 21"),
            )
            .unwrap();
        let assistant = api.auth.session("21", 0);
        let events = format!("http://{address}/account/pages/{page}/devices/A/events");
        let url = format!("http://{address}/account/pages/{page}/devices/A/control");
        let granted = async |session: &str| {
            client
                .get(format!("http://{address}/account/pages/{page}"))
                .bearer_auth(session)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()["remoteControl"]
                .clone()
        };
        let grant = async |access: &str| {
            client
                .post(format!(
                    "http://{address}/account/assistants/twitch/21/remote-control"
                ))
                .bearer_auth(&session)
                .json(&json!({"access": access}))
                .send()
                .await
                .unwrap()
        };
        let watch = async || {
            client
                .get(&events)
                .bearer_auth(&assistant)
                .send()
                .await
                .unwrap()
        };
        let control =
            |session: &str, body: Value| client.post(&url).bearer_auth(session).json(&body).send();
        let refused = async |response: reqwest::Response, text: &str| {
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            assert_eq!(response.text().await.unwrap(), text);
        };
        assert_eq!(granted(&session).await, "control");
        assert_eq!(granted(&assistant).await, "none");
        refused(
            watch().await,
            "The streamer has not let you use the remote control.",
        )
        .await;
        refused(
            control(&assistant, json!({"setTorch": {"on": true}}))
                .await
                .unwrap(),
            "The streamer has not let you use the remote control.",
        )
        .await;
        assert_eq!(
            grant("everything").await.status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        let response = grant("view").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.json::<Value>().await.unwrap()["twitchAssistants"],
            json!([{"id": "21", "login": "mod21", "name": "Mod 21", "avatar": null, "preview": false, "goLive": false, "editTips": false, "remoteControl": "view"}])
        );
        assert_eq!(granted(&assistant).await, "view");
        let mut watching = Events::open(&client, &events, &assistant).await;
        answer_monitoring(&mut socket, true).await;
        assert_eq!(watching.next().await.0, "snapshot");
        for body in [
            json!({"setTorch": {"on": true}}),
            json!({"setLive": {"on": false}}),
        ] {
            refused(
                control(&assistant, body).await.unwrap(),
                "The streamer has not let you control Moblin.",
            )
            .await;
        }
        let call = tokio::spawn(control(&assistant, json!({"sendMessage": {"text": "Hi!"}})));
        let request = loop {
            let request = receive(&mut socket).await;
            if request["request"]["data"].get("sendMessage").is_some() {
                break request;
            }
            answer(&mut socket, &request, Value::Null).await;
        };
        assert_eq!(
            request["request"]["data"],
            json!({"sendMessage": {"text": "Hi!", "name": "Mod 21"}})
        );
        answer(&mut socket, &request, Value::Null).await;
        assert_eq!(call.await.unwrap().unwrap().status(), StatusCode::ACCEPTED);
        assert_eq!(grant("control").await.status(), StatusCode::OK);
        assert_eq!(granted(&assistant).await, "control");
        let call = tokio::spawn(control(&assistant, json!({"setTorch": {"on": true}})));
        let request = loop {
            let request = receive(&mut socket).await;
            if request["request"]["data"].get("setTorch").is_some() {
                break request;
            }
            answer(&mut socket, &request, Value::Null).await;
        };
        answer(&mut socket, &request, Value::Null).await;
        assert_eq!(call.await.unwrap().unwrap().status(), StatusCode::ACCEPTED);
        drop(watching);
        assert_eq!(grant("none").await.status(), StatusCode::OK);
        assert_eq!(granted(&assistant).await, "none");
        refused(
            watch().await,
            "The streamer has not let you use the remote control.",
        )
        .await;
    }

    #[tokio::test]
    async fn assistants_go_live_only_when_allowed_but_always_end_the_stream() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, page) = streamer(&api);
        let mut socket = connect(&address, &key).await;
        create_account(&api, "21", "Mod 21", "mod21");
        api.accounts
            .add_assistant(
                Platform::Twitch,
                "1",
                &twitch_assistant("21", "mod21", "Mod 21"),
            )
            .unwrap();
        api.accounts
            .set_assistant_access(Platform::Twitch, "1", "21", Access::Control)
            .unwrap();
        let assistant = api.auth.session("21", 0);
        let url = format!("http://{address}/account/pages/{page}/devices/A/control");
        let allowed = async |session: &str| {
            client
                .get(format!("http://{address}/account/pages/{page}"))
                .bearer_auth(session)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()["goLive"]
                .clone()
        };
        let allow = async |on: bool| {
            client
                .post(format!(
                    "http://{address}/account/assistants/twitch/21/go-live"
                ))
                .bearer_auth(&session)
                .json(&json!({"on": on}))
                .send()
                .await
                .unwrap()
        };
        let control =
            |session: &str, body: Value| client.post(&url).bearer_auth(session).json(&body).send();
        assert_eq!(allowed(&session).await, true);
        assert_eq!(allowed(&assistant).await, false);
        let response = control(&assistant, json!({"setLive": {"on": true}}))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response.text().await.unwrap(),
            "The streamer has not let you go live."
        );
        let call = tokio::spawn(control(&assistant, json!({"setLive": {"on": false}})));
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"],
            json!({"setLive": {"on": false}})
        );
        answer(&mut socket, &request, Value::Null).await;
        assert_eq!(call.await.unwrap().unwrap().status(), StatusCode::ACCEPTED);
        let call = tokio::spawn(control(&session, json!({"setLive": {"on": true}})));
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], json!({"setLive": {"on": true}}));
        answer(&mut socket, &request, Value::Null).await;
        assert_eq!(call.await.unwrap().unwrap().status(), StatusCode::ACCEPTED);
        let response = allow(true).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.json::<Value>().await.unwrap()["twitchAssistants"],
            json!([{"id": "21", "login": "mod21", "name": "Mod 21", "avatar": null, "preview": false, "goLive": true, "editTips": false, "remoteControl": "control"}])
        );
        assert_eq!(allowed(&assistant).await, true);
        let call = tokio::spawn(control(&assistant, json!({"setLive": {"on": true}})));
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], json!({"setLive": {"on": true}}));
        answer(&mut socket, &request, Value::Null).await;
        assert_eq!(call.await.unwrap().unwrap().status(), StatusCode::ACCEPTED);
        assert_eq!(allow(false).await.status(), StatusCode::OK);
        assert_eq!(allowed(&assistant).await, false);
        let response = control(&assistant, json!({"setLive": {"on": true}}))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn assistants_get_the_preview_only_when_allowed() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, page) = streamer(&api);
        let mut socket = connect(&address, &key).await;
        create_account(&api, "21", "Mod 21", "mod21");
        api.accounts
            .add_assistant(
                Platform::Twitch,
                "1",
                &twitch_assistant("21", "mod21", "Mod 21"),
            )
            .unwrap();
        api.accounts
            .set_assistant_access(Platform::Twitch, "1", "21", Access::View)
            .unwrap();
        let assistant = api.auth.session("21", 0);
        let url = format!("http://{address}/account/pages/{page}/devices/A/preview");
        let allowed = async |session: &str| {
            client
                .get(format!("http://{address}/account/pages/{page}"))
                .bearer_auth(session)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()["preview"]
                .clone()
        };
        let allow = async |assistant: &str, on: bool| {
            client
                .post(format!(
                    "http://{address}/account/assistants/twitch/{assistant}/preview"
                ))
                .bearer_auth(&session)
                .json(&json!({"on": on}))
                .send()
                .await
                .unwrap()
        };
        assert_eq!(allowed(&session).await, true);
        assert_eq!(allowed(&assistant).await, false);
        let response = client
            .get(&url)
            .bearer_auth(&assistant)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response.text().await.unwrap(),
            "The streamer has not let you see the stream preview."
        );
        let response = client
            .post(format!(
                "http://{address}/account/assistants/twitch/21/preview"
            ))
            .bearer_auth(&assistant)
            .json(&json!({"on": true}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(allow("22", true).await.status(), StatusCode::NOT_FOUND);
        let response = allow("21", true).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.json::<Value>().await.unwrap()["twitchAssistants"],
            json!([{"id": "21", "login": "mod21", "name": "Mod 21", "avatar": null, "preview": true, "goLive": false, "editTips": false, "remoteControl": "view"}])
        );
        assert_eq!(allowed(&assistant).await, true);
        let mut preview = Events::open(&client, &url, &assistant).await;
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], json!({"startPreview": {}}));
        answer(&mut socket, &request, Value::Null).await;
        socket
            .send(Message::text(
                json!({"preview": {"preview": "/9j/1"}}).to_string(),
            ))
            .await
            .unwrap();
        assert_eq!(
            preview.next().await,
            ("frame".to_string(), json!({"image": "/9j/1"}))
        );
        drop(preview);
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], json!({"stopPreview": {}}));
        assert_eq!(allow("21", false).await.status(), StatusCode::OK);
        assert_eq!(allowed(&assistant).await, false);
        let response = client
            .get(&url)
            .bearer_auth(&assistant)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn streams_end_when_the_backend_stops() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, page) = streamer(&api);
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
        let (address, api) = start_api_lingering(Duration::from_secs(5)).await;
        let client = reqwest::Client::new();
        let (session, key, page) = streamer(&api);
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
