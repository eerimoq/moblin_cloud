use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::unfold;
use futures_util::{Stream, StreamExt};
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::{Instant, sleep, sleep_until};

use super::*;

fn signal(name: &str) -> Event {
    Event::default().event(name).data("{}")
}

fn until_stopping<S: Stream<Item = Event> + Send + 'static>(
    api: &Api,
    stream: S,
) -> Sse<impl Stream<Item = Result<Event, Infallible>> + use<S>> {
    let mut stopping = api.stopping.subscribe();
    let stream = stream.map(Ok).take_until(async move {
        let _ = stopping.wait_for(|stopping| *stopping).await;
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

struct Watching {
    api: Arc<Api>,
    headers: HeaderMap,
    page_id: String,
    changes: Receiver<Change>,
    lagged: bool,
    check_at: Instant,
}

pub(super) async fn handle_page_events(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, Failure> {
    assistant(&api, &headers, &page_id)?;
    let watching = Watching {
        api: api.clone(),
        headers,
        changes: api.remote_control.changes.subscribe(&page_id),
        page_id,
        lagged: false,
        check_at: Instant::now() + api.timing.access_recheck,
    };
    let stream = unfold(watching, |mut watching| async move {
        if watching.lagged {
            watching.lagged = false;
            return Some((signal("recent"), watching));
        }
        loop {
            tokio::select! {
                received = watching.changes.recv() => {
                    let name = match received {
                        Ok(Change::Page | Change::Account) => "account",
                        Ok(Change::Recent) => "recent",
                        Err(RecvError::Lagged(_)) => {
                            watching.lagged = true;
                            "account"
                        }
                        Err(RecvError::Closed) => return None,
                    };
                    return Some((signal(name), watching));
                }
                _ = sleep_until(watching.check_at) => {
                    if assistant(&watching.api, &watching.headers, &watching.page_id).is_err() {
                        return None;
                    }
                    watching.check_at = Instant::now() + watching.api.timing.access_recheck;
                }
            }
        }
    });
    Ok(until_stopping(&api, stream))
}

pub(super) async fn handle_public_page_events(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    api.accounts
        .of_page(&page_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    let changes = api.remote_control.changes.subscribe(&page_id);
    let stream = unfold(changes, |mut changes| async move {
        loop {
            match changes.recv().await {
                Ok(Change::Page) | Err(RecvError::Lagged(_)) => {
                    return Some((signal("page"), changes));
                }
                Ok(Change::Account | Change::Recent) => {}
                Err(RecvError::Closed) => return None,
            }
        }
    });
    Ok(until_stopping(&api, stream))
}

pub(super) async fn handle_admin_overview_events(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    admin(&api, &headers)?;
    let stream = unfold(
        (api.clone(), headers, true),
        |(api, headers, first)| async move {
            if !first {
                sleep(Duration::from_secs(2)).await;
                admin(&api, &headers).ok()?;
            }
            let event = Event::default()
                .event("stats")
                .json_data(admin_stats(&api))
                .ok()?;
            Some((event, (api, headers, false)))
        },
    );
    Ok(until_stopping(&api, stream))
}

#[cfg(test)]
#[path = "tests/events.rs"]
mod tests;
