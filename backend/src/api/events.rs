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
    page: String,
    changes: Receiver<Change>,
    lagged: bool,
    check_at: Instant,
}

pub(super) async fn handle_page_events(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, Failure> {
    moderator(&api, &headers, &id)?;
    let watching = Watching {
        api: api.clone(),
        headers,
        changes: api.remote_control.changes.subscribe(&id),
        page: id,
        lagged: false,
        check_at: Instant::now() + Duration::from_secs(60),
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
                    if moderator(&watching.api, &watching.headers, &watching.page).is_err() {
                        return None;
                    }
                    watching.check_at = Instant::now() + Duration::from_secs(60);
                }
            }
        }
    });
    Ok(until_stopping(&api, stream))
}

pub(super) async fn handle_public_page_events(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    page_of(&api, &id).ok_or(StatusCode::NOT_FOUND)?;
    let changes = api.remote_control.changes.subscribe(&id);
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

pub(super) async fn handle_admin_stats_events(
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
mod tests {
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

    fn captured_tip(api: &crate::api::Api, page: &str) {
        let order = Order {
            streamer: page.to_string(),
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
        let (session, key, page) = streamer(&address, &api).await;
        let url = format!("http://{address}/account/pages/{page}/events");
        let mut events = Events::open(&client, &url, &session).await;
        let mut socket = connect(&address, &key).await;
        assert_eq!(events.next().await, ("account".to_string(), json!({})));
        let response = client
            .post(format!("http://{address}/account/settings"))
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
        captured_tip(&api, &page);
        let response = client
            .post(format!("http://{address}/account/recent/TIP1/resend"))
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
    async fn moderators_hear_about_their_channels() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, _, page) = streamer(&address, &api).await;
        let moderator = create_account(&api, "21", "Mod 21", "mod21");
        let moderator_session = api.auth.session("21", 0);
        let url = format!("http://{address}/account/pages/{page}/events");
        let response = client
            .get(&url)
            .bearer_auth(&moderator_session)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = client.get(&url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        api.accounts
            .add_moderator(
                Platform::Twitch,
                "1",
                &twitch_moderator("21", "mod21", "Mod 21"),
            )
            .unwrap();
        let mut moderated = Events::open(&client, &url, &moderator_session).await;
        let mut own = Events::open(
            &client,
            &format!(
                "http://{address}/account/pages/{}/events",
                moderator.page.id
            ),
            &moderator_session,
        )
        .await;
        let response = client
            .post(format!(
                "http://{address}/account/moderators/twitch/21/remove"
            ))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(moderated.next().await.0, "account");
        assert_eq!(own.next().await.0, "account");
    }

    #[tokio::test]
    async fn tip_pages_hear_about_changes() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, page) = streamer(&address, &api).await;
        let mut events = Events::open(
            &client,
            &format!("http://{address}/pages/{page}/events"),
            "",
        )
        .await;
        let mut socket = connect(&address, &key).await;
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
        captured_tip(&api, &page);
        let response = client
            .post(format!("http://{address}/account/recent/TIP1/resend"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        quiet(&mut events).await;
        let response = client
            .post(format!("http://{address}/account/settings"))
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

    #[tokio::test]
    async fn admins_get_the_stats_streamed() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, _, _) = streamer(&address, &api).await;
        let url = format!("http://{address}/admin/stats/events");
        let response = client.get(&url).bearer_auth(&session).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        create_account(&api, "admin", "Admin", "admin");
        let mut events = Events::open(&client, &url, &api.auth.session("admin", 0)).await;
        let (name, stats) = events.next().await;
        assert_eq!(name, "stats");
        assert_eq!(stats["accounts"]["total"], 2);
        assert_eq!(stats["server"]["version"], "dev");
    }
}
