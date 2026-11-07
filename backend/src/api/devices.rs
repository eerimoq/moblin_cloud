use std::convert::Infallible;
use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::{iter, unfold};
use futures_util::{Stream, StreamExt};
use log::info;
use serde::Serialize;
use serde_json::{Value, json};
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

fn known_device(api: &Api, page_id: &str, device: &str) -> Result<DeviceResponse, Failure> {
    devices(api, page_id, false)
        .into_iter()
        .find(|known| known.id == device)
        .ok_or(failure(
            "not-found",
            StatusCode::NOT_FOUND,
            "This Moblin is not in the list.",
        ))
}

fn control_failure(control: &Control, error: RequestError) -> Failure {
    if control.editor() && error == RequestError::Failed {
        return failure(
            "refused",
            StatusCode::CONFLICT,
            "Moblin refused the change. Reload the scenes and try again.",
        );
    }
    request_failure(error)
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
    page_id: String,
    watch: Watch,
    check_at: Instant,
    stopping: watch::Receiver<bool>,
}

pub(super) async fn handle_device_events(
    State(api): State<Arc<Api>>,
    Path((page_id, device)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, Failure> {
    let user = watcher(&api, &headers, &page_id)?;
    let known = known_device(&api, &page_id, &device)?;
    let watch = api
        .remote_control
        .watch((&page_id, &device), &user.signed_in.account.id);
    let first = Event::default()
        .event("snapshot")
        .json_data(SnapshotResponse {
            name: known.name,
            snapshot: watch.snapshot.clone(),
        })
        .ok();
    let stopping = api.stopping.subscribe();
    let check_at = Instant::now() + api.timing.access_recheck;
    let streaming = Streaming {
        api,
        headers,
        page_id,
        watch,
        check_at,
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
                    if watcher(&streaming.api, &streaming.headers, &streaming.page_id).is_err() {
                        return None;
                    }
                    streaming.check_at = Instant::now() + streaming.api.timing.access_recheck;
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
    page_id: String,
    watch: PreviewWatch,
    check_at: Instant,
    stopping: watch::Receiver<bool>,
}

pub(super) async fn handle_device_preview(
    State(api): State<Arc<Api>>,
    Path((page_id, device)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, Failure> {
    previewer(&api, &headers, &page_id)?;
    known_device(&api, &page_id, &device)?;
    let watch = api.remote_control.watch_preview((&page_id, &device));
    let first = watch.frame.as_deref().and_then(frame_event);
    let stopping = api.stopping.subscribe();
    let check_at = Instant::now() + api.timing.access_recheck;
    let previewing = Previewing {
        api,
        headers,
        page_id,
        watch,
        check_at,
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
                    if previewer(&previewing.api, &previewing.headers, &previewing.page_id).is_err() {
                        return None;
                    }
                    previewing.check_at = Instant::now() + previewing.api.timing.access_recheck;
                }
            }
        }
    });
    Ok(Sse::new(iter(first.map(Ok)).chain(stream)).keep_alive(KeepAlive::default()))
}

pub(super) async fn handle_device_control(
    State(api): State<Arc<Api>>,
    Path((page_id, device)): Path<(String, String)>,
    headers: HeaderMap,
    control: Result<Json<Control>, JsonRejection>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let user = watcher(&api, &headers, &page_id)?;
    if !matches!(control, Ok(Json(Control::SendMessage { .. })))
        && user.rights.remote_control < Access::Control
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
    if matches!(control, Control::SetLive { on: true }) && !user.rights.permits(Permission::GoLive)
    {
        return Err(failure(
            "no-go-live",
            StatusCode::FORBIDDEN,
            "The streamer has not let you go live.",
        ));
    }
    if let Control::SendMessage { name, .. } = &mut control {
        *name = Some(user.signed_in.account.name().to_string()).filter(|name| !name.is_empty());
    }
    known_device(&api, &page_id, &device)?;
    let name = control.name();
    let data = api
        .remote_control
        .control(
            (&page_id, &device),
            control.clone(),
            api.timing.moblin_answer,
        )
        .await
        .map_err(|error| control_failure(&control, error))?;
    info!(
        "{} sent {name} to device {device} of page {page_id}",
        user.signed_in.account.id
    );
    let Some(key) = control.answers() else {
        return Ok(accepted());
    };
    let Some(answer) = data.as_ref().and_then(|data| data.get(name)) else {
        return Err(request_failure(RequestError::Failed));
    };
    Ok((
        StatusCode::OK,
        Json(json!({ key: answer.get(key).cloned().unwrap_or(Value::Null) })),
    ))
}

#[cfg(test)]
#[path = "tests/devices.rs"]
mod tests;
