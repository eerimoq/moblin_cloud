use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use log::info;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::deliveries;
use crate::payments::{Item, Order};
use crate::remote_control::{self, MusicCommand, MusicStatus, RequestError};

use super::*;

pub(super) fn music_error(error: RequestError) -> Failure {
    match error {
        RequestError::Offline => failure(
            "offline",
            StatusCode::NOT_FOUND,
            "The streamer is offline right now. Try again when they are live.",
        ),
        RequestError::Timeout => failure(
            "moblin-timeout",
            StatusCode::GATEWAY_TIMEOUT,
            "The streamer's Moblin did not answer. It may need an update.",
        ),
        RequestError::Failed => failure(
            "moblin-error",
            StatusCode::BAD_GATEWAY,
            "The streamer's Moblin could not do it. Try again in a moment.",
        ),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct QueueResponse {
    #[serde(flatten)]
    pub(super) status: MusicStatus,
    pub(super) song_requests: bool,
}

pub(super) async fn handle_queue(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
) -> Result<Json<QueueResponse>, Failure> {
    let device = music_device(&api, &id);
    let status = api
        .remote_control
        .music((&id, &device), MusicCommand::Status {})
        .await
        .map_err(music_error)?;
    Ok(Json(QueueResponse {
        status,
        song_requests: api.remote_control.takes_song_requests(&id),
    }))
}

#[derive(Deserialize)]
pub(super) struct SongRequest {
    pub(super) song: String,
    pub(super) user: Option<String>,
    #[serde(default)]
    pub(super) message: Option<String>,
    #[serde(default)]
    pub(super) provider: Option<String>,
}

pub(super) struct RequestedSong {
    pub(super) link: String,
    pub(super) user: String,
    pub(super) name: String,
    pub(super) duration: Option<u32>,
}

pub(super) fn song_and_user(request: SongRequest) -> Result<(String, String), Failure> {
    let link = remote_control::song_link(&request.song).ok_or(failure(
        "invalid-link",
        StatusCode::BAD_REQUEST,
        "Paste the Apple Music share link of a song.",
    ))?;
    Ok((link, user_name(request.user, "Moblin website")))
}

pub(super) async fn requested_song(
    api: &Api,
    id: &str,
    (link, user): (String, String),
) -> Result<RequestedSong, Failure> {
    if api.remote_control.is_connected(id) && !api.remote_control.takes_song_requests(id) {
        return Err(failure(
            "requests-off",
            StatusCode::CONFLICT,
            "The streamer is not taking song requests right now.",
        ));
    }
    let song = api
        .remote_control
        .find((id, &music_device(api, id)), link.clone())
        .await
        .map_err(music_error)?
        .ok_or(failure(
            "song-not-found",
            StatusCode::UNPROCESSABLE_ENTITY,
            "That song is not available on the streamer's Apple Music.",
        ))?;
    Ok(RequestedSong {
        link,
        user,
        name: song.name(),
        duration: song.duration,
    })
}

pub(super) fn check_song_length(
    max_minutes: Option<u32>,
    song: &RequestedSong,
) -> Result<(), Failure> {
    if let Some(minutes) = max_minutes
        && song
            .duration
            .is_some_and(|duration| duration > minutes * 60)
    {
        return Err(failure(
            "over-max-length",
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("The streamer only takes songs up to {minutes} minutes long."),
        ));
    }
    Ok(())
}

pub(super) fn queue(
    api: &Api,
    id: String,
    request_id: String,
    song: RequestedSong,
    message: Option<String>,
) -> (StatusCode, Json<Value>) {
    info!(
        "queued {:?} by {:?} in request {request_id} for remote control streamer {id}",
        song.link, song.user
    );
    api.deliveries.add_test(
        id.clone(),
        request_id,
        deliveries::Item::Song {
            link: song.link,
            user: song.user,
            message: message.unwrap_or_default(),
        },
    );
    (
        StatusCode::ACCEPTED,
        Json(Value::Object(Default::default())),
    )
}

pub(super) async fn handle_song_order(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    Json(request): Json<SongRequest>,
) -> Result<Json<OrderResponse>, Failure> {
    let result = song_order(&api, id, request).await;
    counted(&api, "songOrders", result)
}

async fn song_order(
    api: &Api,
    id: String,
    mut request: SongRequest,
) -> Result<Json<OrderResponse>, Failure> {
    let message = viewer_message(request.message.take())?;
    let settings = api.pages.get(&id);
    let currency = settings.currency().to_string();
    let songs = settings.songs().cloned().ok_or(failure(
        "feature-off",
        StatusCode::BAD_REQUEST,
        "The streamer no longer takes song requests. Reload the page.",
    ))?;
    let payee = payee(api, &id, request.provider.as_deref())?;
    let song_and_user = song_and_user(request)?;
    limit_song_finds(api, &id)?;
    let song = requested_song(api, &id, song_and_user).await?;
    check_song_length(settings.max_song_minutes(), &song)?;
    let order = Order {
        streamer: id.clone(),
        item: Item::Song {
            link: song.link,
            name: song.name,
            message,
        },
        user: song.user,
        payee,
        price: songs.price,
        currency,
    };
    Ok(Json(start_order(api, order).await?))
}

pub(super) fn limit_song_finds(api: &Api, id: &str) -> Result<(), Failure> {
    if !within_limit(&api.song_finds, id, 10) {
        return Err(failure(
            "too-many-requests",
            StatusCode::TOO_MANY_REQUESTS,
            "Too many song requests on this page right now. Try again in a minute.",
        ));
    }
    Ok(())
}

pub(super) async fn handle_song_capture(
    State(api): State<Arc<Api>>,
    Path((id, order_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    tokio::spawn(capture_paid(api, id, order_id, "song"))
        .await
        .unwrap_or_else(|_| Err(capture_crashed()))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::http::StatusCode;
    use futures_util::SinkExt;
    use serde_json::{Value, json};
    use tokio_tungstenite::tungstenite::Message;

    use super::*;
    use crate::api::test_utils::*;
    use crate::database::Database;
    use crate::deliveries;
    use crate::pages::{Feature, Pages, Settings, Songs};

    #[tokio::test]
    async fn songs_without_paypal_cant_be_paid() {
        let pages = Pages::new(Database::memory());
        pages
            .set(
                "p",
                Settings {
                    currency: Some("SEK".to_string()),
                    songs: Some(Feature::on(Songs { price: 250 })),
                    ..Settings::default()
                },
            )
            .unwrap();
        let (paypal_url, _) = start_paypal().await;
        let (address, _) =
            start_api_with(Some(new_paypal(paypal_url)), None, None, None, Some(pages)).await;
        let client = reqwest::Client::new();
        let public: Value = client
            .get(format!("http://{address}/pages/p/songs"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            public,
            json!({"songs": {
                "price": 250,
                "currency": "SEK",
                "clientId": null,
                "merchantId": null,
                "environment": "sandbox",
                "stripe": null,
            }, "subSongs": null, "maxSongMinutes": null})
        );
        let response = client
            .post(format!("http://{address}/pages/p/songs/orders"))
            .json(&json!({"song": LINK, "user": "Viewer"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    async fn paid_streamer(address: &str, api: &Api) -> (String, String) {
        let (session, key, id) = streamer(address, api).await;
        connect_paypal(api);
        let response = reqwest::Client::new()
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"currency": "SEK", "songs": {"price": 250}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        (key, id)
    }

    #[tokio::test]
    async fn songs_cost_the_same_whatever_their_length() {
        let (address, api, paypal_order) = start_paid().await;
        let (key, id) = paid_streamer(&address, &api).await;
        let mut socket = connect(&address, &key).await;
        let orders_url = format!("http://{address}/pages/{id}/songs/orders");
        for (duration, value) in [
            (None, "2.50"),
            (Some(300), "2.50"),
            (Some(301), "2.50"),
            (Some(601), "2.50"),
        ] {
            let created = tokio::spawn({
                let orders_url = orders_url.clone();
                async move {
                    reqwest::Client::new()
                        .post(orders_url)
                        .json(&json!({"song": LINK}))
                        .send()
                        .await
                        .unwrap()
                        .status()
                }
            });
            let request = receive(&mut socket).await;
            answer(
                &mut socket,
                &request,
                json!({"musicFind": {"song": {"id": "1440818839", "title": "Song", "duration": duration}}}),
            )
            .await;
            assert_eq!(created.await.unwrap(), StatusCode::OK);
            let sent = paypal_order.lock().unwrap().clone().unwrap();
            assert_eq!(
                sent["purchase_units"][0]["amount"],
                json!({"currency_code": "SEK", "value": value})
            );
        }
    }

    #[tokio::test]
    async fn songs_longer_than_the_max_are_refused() {
        let (address, api, _) = start_paid().await;
        let (session, key, id) = streamer(&address, &api).await;
        connect_paypal(&api);
        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"songs": {"price": 250}, "maxSongLength": {"minutes": 61}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let settings: Value = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"songs": {"price": 250}, "maxSongLength": {"minutes": 5}}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            settings["maxSongLength"],
            json!({"enabled": true, "minutes": 5})
        );
        let songs: Value = client
            .get(format!("http://{address}/pages/{id}/songs"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(songs["maxSongMinutes"], 5);
        let mut socket = connect(&address, &key).await;
        let orders_url = format!("http://{address}/pages/{id}/songs/orders");
        for (duration, status) in [
            (None, StatusCode::OK),
            (Some(300), StatusCode::OK),
            (Some(301), StatusCode::UNPROCESSABLE_ENTITY),
        ] {
            let created = tokio::spawn({
                let orders_url = orders_url.clone();
                async move {
                    reqwest::Client::new()
                        .post(orders_url)
                        .json(&json!({"song": LINK}))
                        .send()
                        .await
                        .unwrap()
                }
            });
            let request = receive(&mut socket).await;
            answer(
                &mut socket,
                &request,
                json!({"musicFind": {"song": {"id": "1440818839", "title": "Song", "duration": duration}}}),
            )
            .await;
            let response = created.await.unwrap();
            assert_eq!(response.status(), status);
            if status != StatusCode::OK {
                assert_eq!(
                    response.text().await.unwrap(),
                    "The streamer only takes songs up to 5 minutes long."
                );
            }
        }
        assert!(api.counters.all().iter().any(|counter| counter.name
            == "songOrdersFailed:over-max-length"
            && counter.total == 1));
    }

    #[tokio::test]
    async fn song_orders_are_rate_limited_per_page() {
        let (address, api, _) = start_paid().await;
        let (_, id) = paid_streamer(&address, &api).await;
        let client = reqwest::Client::new();
        let orders_url = format!("http://{address}/pages/{id}/songs/orders");
        for _ in 0..10 {
            let response = client
                .post(&orders_url)
                .json(&json!({"song": LINK}))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
        let response = client
            .post(&orders_url)
            .json(&json!({"song": LINK}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response.text().await.unwrap(),
            "Too many song requests on this page right now. Try again in a minute."
        );
    }

    #[tokio::test]
    async fn paid_songs_are_added_after_capture() {
        let (address, api, paypal_order) = start_paid().await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        let (key, id) = (key.as_str(), id.as_str());
        let settings_url = format!("http://{address}/account/settings");
        let response = client
            .post(&settings_url)
            .bearer_auth(&session)
            .json(&json!({"currency": "SEK", "songs": {"price": 250}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        connect_paypal(&api);
        let settings: Value = client
            .post(&settings_url)
            .bearer_auth(&session)
            .json(&json!({"currency": "sek", "songs": {"price": 250}}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["songs"], json!({"enabled": true, "price": 250}));
        assert_eq!(settings["currency"], "SEK");
        assert_eq!(settings["paypal"], true);
        assert_eq!(settings["merchantId"], "MERCHANT00001");
        let public: Value = client
            .get(format!("http://{address}/pages/{id}/songs"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            public,
            json!({"songs": {
                "price": 250,
                "currency": "SEK",
                "clientId": "id",
                "merchantId": "MERCHANT00001",
                "environment": "sandbox",
                "stripe": null,
            }, "subSongs": null, "maxSongMinutes": null})
        );
        let orders_url = format!("http://{address}/pages/{id}/songs/orders");
        let response = client
            .post(&orders_url)
            .json(&json!({"song": "Song", "user": "Viewer"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.text().await.unwrap(),
            "Paste the Apple Music share link of a song."
        );
        let response = client
            .post(&orders_url)
            .json(&json!({"song": LINK, "user": "Viewer"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let mut socket = connect(&address, key).await;
        *paypal_order.lock().unwrap() = None;
        let not_found = tokio::spawn({
            let client = client.clone();
            let orders_url = orders_url.clone();
            async move {
                client
                    .post(orders_url)
                    .json(&json!({"song": LINK, "user": "Viewer"}))
                    .send()
                    .await
                    .unwrap()
            }
        });
        let request = receive(&mut socket).await;
        answer(&mut socket, &request, json!({"musicFind": {}})).await;
        assert_eq!(
            not_found.await.unwrap().status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert!(paypal_order.lock().unwrap().is_none());
        assert_eq!(
            request_song(&mut socket, &orders_url).await,
            json!({"orderId": "ORDER1"})
        );
        let sent = paypal_order.lock().unwrap().clone().unwrap();
        assert_eq!(
            sent["purchase_units"][0]["amount"],
            json!({"currency_code": "SEK", "value": "2.50"})
        );
        assert_eq!(
            sent["purchase_units"][0]["payee"],
            json!({"merchant_id": "MERCHANT00001"})
        );
        assert_eq!(
            payer_of(sent["assertion"].as_str().unwrap()),
            "MERCHANT00001"
        );
        assert_eq!(
            sent["purchase_units"][0]["description"],
            "Song request: Band - Song: Play it loud!"
        );
        let response = client
            .post(format!("{orders_url}/OTHER/capture"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::GONE);
        let capture_url = format!("{orders_url}/ORDER1/capture");
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"],
            json!({"music": {"command": {"add": {
                "title": LINK,
                "user": "Viewer",
                "message": "Play it loud!",
                "requestId": "ORDER1",
            }}}})
        );
        assert!(!api.deliveries.pending().is_empty());
        answer(
            &mut socket,
            &request,
            music(json!([{"id": "q1", "title": "Song"}])),
        )
        .await;
        wait_until_added(&api).await;
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn paid_songs_are_retried_until_added_once() {
        let (address, api, _) = start_paid().await;
        let (key, id) = paid_streamer(&address, &api).await;
        let orders_url = format!("http://{address}/pages/{id}/songs/orders");
        let mut socket = connect(&address, &key).await;
        assert_eq!(
            request_song(&mut socket, &orders_url).await,
            json!({"orderId": "ORDER1"})
        );
        let response = reqwest::Client::new()
            .post(format!("{orders_url}/ORDER1/capture"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let add = json!({"music": {"command": {"add": {
            "title": LINK,
            "user": "Viewer",
            "message": "Play it loud!",
            "requestId": "ORDER1",
        }}}});
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], add);
        socket
            .send(Message::text(
                json!({"response": {"id": request["request"]["id"], "result": {"error": {}}}})
                    .to_string(),
            ))
            .await
            .unwrap();
        socket.close(None).await.unwrap();
        let mut socket = connect(&address, &key).await;
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], add);
        socket.close(None).await.unwrap();
        assert!(!api.deliveries.pending().is_empty());
        let mut socket = connect(&address, &key).await;
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], add);
        answer(
            &mut socket,
            &request,
            music(json!([{"id": "q1", "title": "Song"}])),
        )
        .await;
        wait_until_added(&api).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(200), receive(&mut socket))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn music_status_comes_from_the_streamer() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (_, key, id) = streamer(&address, &api).await;
        let music_url = format!("http://{address}/pages/{id}/songs/queue");
        let response = client.get(&music_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let mut socket = connect(&address, &key).await;
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        let status = tokio::spawn({
            let client = client.clone();
            let music_url = music_url.clone();
            async move {
                client
                    .get(music_url)
                    .send()
                    .await
                    .unwrap()
                    .json::<Value>()
                    .await
                    .unwrap()
            }
        });
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"],
            json!({"music": {"command": {"status": {}}}})
        );
        answer(
            &mut socket,
            &request,
            music(json!([{"id": "q1", "title": "Song", "duration": 213, "user": "Viewer"}])),
        )
        .await;
        let status = status.await.unwrap();
        assert_eq!(status["songRequests"], true);
        assert_eq!(
            status["songs"],
            json!([{"id": "q1", "title": "Song", "artist": null, "duration": 213, "user": "Viewer"}])
        );
        socket.close(None).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let response = client.get(&music_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn songs_need_a_price() {
        let (address, api) = start_api(None).await;
        let (_, key, id) = streamer(&address, &api).await;
        let _socket = connect(&address, &key).await;
        let response = reqwest::Client::new()
            .post(format!("http://{address}/pages/{id}/songs/orders"))
            .json(&json!({"song": LINK}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.text().await.unwrap(),
            "The streamer no longer takes song requests. Reload the page."
        );
    }

    #[tokio::test]
    async fn song_requests_can_be_turned_off_by_the_streamer() {
        let (address, api, _) = start_paid().await;
        let (key, id) = paid_streamer(&address, &api).await;
        let client = reqwest::Client::new();
        let orders_url = format!("http://{address}/pages/{id}/songs/orders");
        let mut socket = connect(&address, &key).await;
        assert_eq!(
            request_song(&mut socket, &orders_url).await,
            json!({"orderId": "ORDER1"})
        );
        socket
            .send(Message::text(
                json!({"event": {"data": {"songRequests": {"enabled": false}}}}).to_string(),
            ))
            .await
            .unwrap();
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        let response = client
            .post(&orders_url)
            .json(&json!({"song": LINK}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.text().await.unwrap(),
            "The streamer is not taking song requests right now."
        );
        let response = client
            .post(format!("{orders_url}/ORDER1/capture"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.text().await.unwrap(),
            "The streamer stopped taking song requests, so your payment was not taken."
        );
        let status = tokio::spawn({
            let music_url = format!("http://{address}/pages/{id}/songs/queue");
            async move {
                reqwest::get(music_url)
                    .await
                    .unwrap()
                    .json::<Value>()
                    .await
                    .unwrap()
            }
        });
        let request = receive(&mut socket).await;
        answer(
            &mut socket,
            &request,
            music(json!([{"id": "q1", "title": "Song"}])),
        )
        .await;
        let status = status.await.unwrap();
        assert_eq!(status["songRequests"], false);
        assert_eq!(status["songs"][0]["title"], "Song");
    }

    #[tokio::test]
    async fn songs_not_found_stay_songs_and_are_not_retried() {
        let (address, api, _) = start_paid().await;
        let (key, id) = paid_streamer(&address, &api).await;
        let orders_url = format!("http://{address}/pages/{id}/songs/orders");
        let mut socket = connect(&address, &key).await;
        assert_eq!(
            request_song(&mut socket, &orders_url).await,
            json!({"orderId": "ORDER1"})
        );
        let response = reqwest::Client::new()
            .post(format!("{orders_url}/ORDER1/capture"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let request = receive(&mut socket).await;
        let mut status = music(json!([]));
        status["music"]["status"]["songNotFound"] = json!(true);
        answer(&mut socket, &request, status).await;
        wait_until_added(&api).await;
        let delivery = api.deliveries.get("ORDER1").unwrap();
        assert_eq!(
            delivery.item,
            deliveries::Item::Song {
                link: LINK.to_string(),
                user: "Viewer".to_string(),
                message: "Play it loud!".to_string(),
            }
        );
        assert_eq!(delivery.failure.as_deref(), Some("not-found"));
        assert!(
            tokio::time::timeout(Duration::from_millis(200), receive(&mut socket))
                .await
                .is_err()
        );
        assert!(
            api.counters
                .all()
                .iter()
                .any(|counter| counter.name == "songDeliveriesFailed:not-found"
                    && counter.total == 1)
        );
    }
}
