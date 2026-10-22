use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use log::info;
use serde::{Deserialize, Serialize};

use crate::accounts::Platform;
use crate::auth::Viewer;
use crate::deliveries;
use crate::pages::Tiers;
use crate::payments::{Item, Order, Payee, insert_captured};

use super::*;

pub(super) fn viewer(api: &Api, headers: &HeaderMap, id: &str) -> Result<Viewer, Failure> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .and_then(|session| api.auth.viewer_of(session))
        .filter(|viewer| viewer.page == id)
        .filter(|viewer| {
            api.accounts
                .of_page(id)
                .is_some_and(|account| account.identity(viewer.platform).is_some())
        })
        .ok_or(failure(
            "not-signed-in",
            StatusCode::UNAUTHORIZED,
            "Sign in again.",
        ))
}

#[derive(Serialize)]
pub(super) struct AllowanceResponse {
    platform: Platform,
    name: String,
    tier: Option<u8>,
    allowance: u32,
    left: u32,
}

fn allowance_response(
    api: &Api,
    headers: &HeaderMap,
    id: &str,
    kind: SubscriberKind,
    tiers: Option<Tiers>,
) -> Result<Json<AllowanceResponse>, Failure> {
    let viewer = viewer(api, headers, id)?;
    let allowance = tiers.map_or(0, |tiers| tiers.allowance(viewer.tier));
    Ok(Json(AllowanceResponse {
        left: api
            .allowances
            .left(id, &viewer.key(), kind.name(), allowance)
            .map_err(not_saved)?,
        platform: viewer.platform,
        name: viewer.name,
        tier: viewer.tier,
        allowance,
    }))
}

pub(super) async fn handle_subscriber_song_allowance(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<AllowanceResponse>, Failure> {
    allowance_response(
        &api,
        &headers,
        &id,
        SubscriberKind::Song,
        sub_tiers(&api, &id, api.pages.get(&id).sub_songs()),
    )
}

pub(super) async fn handle_subscriber_spin_allowance(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<AllowanceResponse>, Failure> {
    allowance_response(
        &api,
        &headers,
        &id,
        SubscriberKind::Spin,
        sub_tiers(&api, &id, api.pages.get(&id).sub_spins()),
    )
}

#[derive(Deserialize)]
pub(super) struct SubscriberSongRequest {
    pub(super) song: String,
    #[serde(default)]
    pub(super) message: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct SubscriberSpinRequest {
    #[serde(default)]
    message: Option<String>,
}

#[derive(Serialize)]
pub(super) struct LeftResponse {
    left: u32,
}

pub(super) async fn handle_subscriber_song(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<SubscriberSongRequest>,
) -> Result<(StatusCode, Json<LeftResponse>), Failure> {
    let result = subscriber_song(&api, id, &headers, request).await;
    counted(&api, "subSongs", result)
}

pub(super) async fn handle_subscriber_spin(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<SubscriberSpinRequest>,
) -> Result<(StatusCode, Json<LeftResponse>), Failure> {
    let result = subscriber_spin(&api, id, &headers, request);
    counted(&api, "subSpins", result)
}

#[derive(Clone, Copy)]
pub(super) enum SubscriberKind {
    Song,
    Spin,
}

impl SubscriberKind {
    fn name(self) -> &'static str {
        match self {
            Self::Song => "song",
            Self::Spin => "spin",
        }
    }

    fn used_up(self) -> Failure {
        match self {
            Self::Song => failure(
                "no-songs-left",
                StatusCode::CONFLICT,
                "You have used all your songs this month.",
            ),
            Self::Spin => failure(
                "no-spins-left",
                StatusCode::CONFLICT,
                "You have used all your spins this month.",
            ),
        }
    }
}

pub(super) struct Subscriber {
    viewer: Viewer,
    tier: u8,
    allowance: u32,
}

impl Subscriber {
    fn payee(&self) -> Payee {
        match self.viewer.platform {
            Platform::Twitch => Payee::Twitch(self.tier),
            Platform::Kick => Payee::Kick,
        }
    }
}

pub(super) fn subscriber(
    api: &Api,
    headers: &HeaderMap,
    id: &str,
    kind: SubscriberKind,
    tiers: Option<&Tiers>,
) -> Result<Subscriber, Failure> {
    let (off, excluded) = match kind {
        SubscriberKind::Song => (
            "The streamer no longer takes song requests from subscribers. Reload the page.",
            "request songs",
        ),
        SubscriberKind::Spin => (
            "The streamer no longer lets subscribers spin the wheel. Reload the page.",
            "spin the wheel",
        ),
    };
    let tiers = tiers.ok_or(failure("feature-off", StatusCode::BAD_REQUEST, off))?;
    let viewer = viewer(api, headers, id)?;
    let tier = viewer.tier.ok_or(failure(
        "not-subscribed",
        StatusCode::FORBIDDEN,
        &format!(
            "Only {} subscribers of this streamer can {excluded}.",
            viewer.platform.title()
        ),
    ))?;
    let allowance = tiers.allowance(Some(tier));
    if allowance == 0 {
        return Err(failure(
            "tier-excluded",
            StatusCode::FORBIDDEN,
            &match viewer.platform {
                Platform::Twitch => format!("Tier {tier} subscribers can't {excluded} here."),
                Platform::Kick => format!("Kick subscribers can't {excluded} here."),
            },
        ));
    }
    let left = api
        .allowances
        .left(id, &viewer.key(), kind.name(), allowance)
        .map_err(not_saved)?;
    if left == 0 {
        return Err(kind.used_up());
    }
    Ok(Subscriber {
        viewer,
        tier,
        allowance,
    })
}

fn queue_for_subscriber(
    api: &Api,
    subscriber: &Subscriber,
    kind: SubscriberKind,
    order: &Order,
) -> Result<u32, Failure> {
    let request_id = new_request_id(api)?;
    let item = delivery_item(order);
    let left = api
        .allowances
        .take(
            &order.streamer,
            &subscriber.viewer.key(),
            kind.name(),
            subscriber.allowance,
            |connection| {
                insert_captured(connection, &request_id, order)?;
                deliveries::insert(connection, &order.streamer, &request_id, &item)
            },
        )
        .map_err(not_saved)?
        .ok_or(kind.used_up())?;
    api.deliveries.notify();
    info!(
        "queued {:?} for {} tier {} subscriber {:?} in request {request_id} for remote control streamer {}",
        order.description(),
        subscriber.viewer.platform.title(),
        subscriber.tier,
        order.user,
        order.streamer
    );
    Ok(left)
}

async fn subscriber_song(
    api: &Api,
    id: String,
    headers: &HeaderMap,
    request: SubscriberSongRequest,
) -> Result<(StatusCode, Json<LeftResponse>), Failure> {
    let settings = api.pages.get(&id);
    let sub_songs = sub_tiers(api, &id, settings.sub_songs());
    let subscriber = subscriber(api, headers, &id, SubscriberKind::Song, sub_songs.as_ref())?;
    let message = viewer_message(request.message)?;
    let song_and_user = song_and_user(SongRequest {
        song: request.song,
        user: Some(subscriber.viewer.name.clone()),
        message: None,
        provider: None,
    })?;
    limit_song_finds(api, &id)?;
    let song = requested_song(api, &id, song_and_user).await?;
    let order = Order {
        streamer: id,
        item: Item::Song {
            link: song.link,
            name: song.name,
            message,
        },
        user: song.user,
        payee: subscriber.payee(),
        price: 0,
        currency: settings.currency().to_string(),
    };
    let left = queue_for_subscriber(api, &subscriber, SubscriberKind::Song, &order)?;
    Ok((StatusCode::ACCEPTED, Json(LeftResponse { left })))
}

fn subscriber_spin(
    api: &Api,
    id: String,
    headers: &HeaderMap,
    request: SubscriberSpinRequest,
) -> Result<(StatusCode, Json<LeftResponse>), Failure> {
    let settings = api.pages.get(&id);
    let sub_spins = sub_tiers(api, &id, settings.sub_spins());
    let subscriber = subscriber(api, headers, &id, SubscriberKind::Spin, sub_spins.as_ref())?;
    wheel_available(api, &id)?;
    let order = Order {
        streamer: id,
        item: Item::Spin {
            spins: 1,
            message: viewer_message(request.message)?,
        },
        user: user_name(Some(subscriber.viewer.name.clone()), "Someone"),
        payee: subscriber.payee(),
        price: 0,
        currency: settings.currency().to_string(),
    };
    let left = queue_for_subscriber(api, &subscriber, SubscriberKind::Spin, &order)?;
    Ok((StatusCode::ACCEPTED, Json(LeftResponse { left })))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::Duration;

    use anyhow::Result;
    use axum::http::StatusCode;
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{Value, json};
    use tokio_tungstenite::tungstenite::Message;

    use crate::accounts::Platform;
    use crate::api::test_utils::*;

    #[tokio::test]
    async fn viewers_sign_in_with_their_subscription_tier() {
        let twitch = start_twitch().await;
        let (address, api) =
            start_api_with(None, None, Some(twitch_app(&twitch)), None, None).await;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let (_, _, id) = streamer(&address, &api).await;
        let response = client
            .get(format!("http://{address}/auth/twitch/login?page=unknown"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let sign_in = async |page: &str| {
            let response = client
                .get(format!("http://{address}/auth/twitch/login?page={page}"))
                .send()
                .await
                .unwrap();
            let login = reqwest::Url::parse(&location(&response)).unwrap();
            let query: HashMap<_, _> = login.query_pairs().into_owned().collect();
            assert_eq!(query["scope"], "user:read:subscriptions");
            location(
                &client
                    .get(format!(
                        "http://{address}/auth/twitch/callback?code=c&state={}",
                        query["state"]
                    ))
                    .header("cookie", format!("moblin_sign_in_state={}", query["state"]))
                    .send()
                    .await
                    .unwrap(),
            )
        };
        let page = sign_in(&id).await;
        let session = page
            .strip_prefix("https://moblin.app/twitch/erik#viewer=")
            .unwrap();
        let viewer = api.auth.viewer_of(session).unwrap();
        assert_eq!(
            (
                viewer.id.as_str(),
                viewer.name.as_str(),
                viewer.page.as_str(),
                viewer.tier
            ),
            ("42", "Erik", id.as_str(), Some(2))
        );
        assert_eq!(api.accounts.get("42"), None);
        assert_eq!(api.auth.session_of(session), None);
        let other = create_account(&api, "2", "Other", "other").page.id;
        let other = other.as_str();
        let page = sign_in(other).await;
        let session = page
            .strip_prefix("https://moblin.app/twitch/other#viewer=")
            .unwrap();
        assert_eq!(api.auth.viewer_of(session).unwrap().tier, None);
        let third = create_account(&api, "3", "Third", "third").page.id;
        let third = third.as_str();
        assert_eq!(
            sign_in(third).await,
            "https://moblin.app/twitch/third#viewerSignInFailed=twitch"
        );
        let response = client
            .get(format!("http://{address}/auth/twitch/login?page={id}"))
            .send()
            .await
            .unwrap();
        let state = reqwest::Url::parse(&location(&response))
            .unwrap()
            .query_pairs()
            .find(|(name, _)| name == "state")
            .unwrap()
            .1
            .into_owned();
        assert_eq!(
            location(
                &client
                    .get(format!(
                        "http://{address}/auth/twitch/callback?error=access_denied&state={state}"
                    ))
                    .header("cookie", format!("moblin_sign_in_state={state}"))
                    .send()
                    .await
                    .unwrap()
            ),
            "https://moblin.app/twitch/erik#viewerSignInFailed=twitch"
        );
        let response = client
            .get(format!("http://{address}/auth/twitch/login?page={id}"))
            .send()
            .await
            .unwrap();
        let state = reqwest::Url::parse(&location(&response))
            .unwrap()
            .query_pairs()
            .find(|(name, _)| name == "state")
            .unwrap()
            .1
            .into_owned();
        assert_eq!(
            location(
                &client
                    .get(format!(
                        "http://{address}/auth/twitch/callback?code=c&state={state}"
                    ))
                    .header("cookie", "moblin_sign_in_state=other")
                    .send()
                    .await
                    .unwrap()
            ),
            "https://moblin.app/twitch/erik#viewerSignInFailed=twitch"
        );
        assert_eq!(
            location(
                &client
                    .get(format!(
                        "http://{address}/auth/twitch/callback?error=access_denied"
                    ))
                    .send()
                    .await
                    .unwrap()
            ),
            "https://moblin.app/dashboard/#signInFailed"
        );
    }

    async fn send_subscriber_song(
        address: &str,
        id: &str,
        session: &str,
        song: &str,
    ) -> reqwest::Response {
        reqwest::Client::new()
            .post(format!("http://{address}/pages/{id}/songs/subscriber"))
            .bearer_auth(session)
            .json(&json!({"song": song, "message": " For you "}))
            .send()
            .await
            .unwrap()
    }

    async fn subscriber_song_found<S>(
        socket: &mut S,
        address: &str,
        id: &str,
        session: &str,
        duration: Option<u32>,
    ) -> reqwest::Response
    where
        S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
            + SinkExt<Message>
            + Unpin,
        <S as futures_util::Sink<Message>>::Error: std::fmt::Debug,
    {
        let sent = tokio::spawn({
            let (address, id, session) = (address.to_string(), id.to_string(), session.to_string());
            async move { send_subscriber_song(&address, &id, &session, LINK).await }
        });
        let request = receive(socket).await;
        assert_eq!(
            request["request"]["data"],
            json!({"music": {"command": {"find": {"title": LINK}}}})
        );
        answer(
            socket,
            &request,
            json!({"musicFind": {"song": {"id": "1440818839", "title": "Song", "artist": "Band", "duration": duration}}}),
        )
        .await;
        sent.await.unwrap()
    }

    #[tokio::test]
    async fn subscriber_songs_longer_than_the_max_are_refused() {
        let (address, api) = start_api(None).await;
        let (session, key, id) = streamer(&address, &api).await;
        let response = reqwest::Client::new()
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({
                "subSongs": {"tier1": 0, "tier2": 0, "tier3": 3},
                "maxSongLength": {"minutes": 5},
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let tier3 = api.auth.viewer_session(
            Platform::Twitch,
            "31".to_string(),
            "Viewer 31".to_string(),
            id.clone(),
            Some(3),
        );
        let mut socket = connect(&address, &key).await;
        let response = subscriber_song_found(&mut socket, &address, &id, &tier3, Some(301)).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            response.text().await.unwrap(),
            "The streamer only takes songs up to 5 minutes long."
        );
        let response = subscriber_song_found(&mut socket, &address, &id, &tier3, Some(300)).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(response.json::<Value>().await.unwrap(), json!({"left": 2}));
        let request = receive(&mut socket).await;
        answer(&mut socket, &request, music(json!([]))).await;
        wait_until_added(&api).await;
        assert!(
            api.counters
                .all()
                .iter()
                .any(|counter| counter.name == "subSongsFailed:over-max-length"
                    && counter.total == 1)
        );
    }

    #[tokio::test]
    async fn twitch_subscribers_request_songs_every_month() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        let settings = |body: Value| {
            client
                .post(format!("http://{address}/account/settings"))
                .bearer_auth(&session)
                .json(&body)
                .send()
        };
        let viewer = |viewer: &str, page: &str, tier: Option<u8>| {
            api.auth.viewer_session(
                Platform::Twitch,
                viewer.to_string(),
                format!("Viewer {viewer}"),
                page.to_string(),
                tier,
            )
        };
        let tier1 = viewer("11", &id, Some(1));
        let response = send_subscriber_song(&address, &id, &tier1, LINK).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        for sub_songs in [
            json!({"tier1": 0, "tier2": 0, "tier3": 0}),
            json!({"tier1": 101, "tier2": 0, "tier3": 0}),
        ] {
            let response = settings(json!({"subSongs": sub_songs})).await.unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
        let response = settings(json!({"subSongs": {"tier1": 1, "tier2": 0, "tier3": 3}}))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.json::<Value>().await.unwrap()["subSongs"],
            json!({"enabled": true, "tier1": 1, "tier2": 0, "tier3": 3})
        );
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
            json!({"songs": null, "subSongs": {"tier1": 1, "tier2": 0, "tier3": 3}, "maxSongMinutes": null})
        );
        let status = async |session: &str| {
            let response = client
                .get(format!("http://{address}/pages/{id}/songs/subscriber"))
                .bearer_auth(session)
                .send()
                .await
                .unwrap();
            (
                response.status(),
                response.json::<Value>().await.unwrap_or_default(),
            )
        };
        assert_eq!(
            status(&tier1).await,
            (
                StatusCode::OK,
                json!({"platform": "twitch", "name": "Viewer 11", "tier": 1, "allowance": 1, "left": 1})
            )
        );
        assert_eq!(status("garbage").await.0, StatusCode::UNAUTHORIZED);
        assert_eq!(
            status(&viewer("11", "other", Some(1))).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status(&api.auth.session("1", 0)).await.0,
            StatusCode::UNAUTHORIZED
        );
        let response = send_subscriber_song(&address, &id, &viewer("12", &id, None), LINK).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = send_subscriber_song(&address, &id, &viewer("13", &id, Some(2)), LINK).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response.text().await.unwrap(),
            "Tier 2 subscribers can't request songs here."
        );
        let response = send_subscriber_song(&address, &id, &tier1, "nope").await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let response = send_subscriber_song(&address, &id, &tier1, LINK).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let mut socket = connect(&address, &key).await;
        let response = subscriber_song_found(&mut socket, &address, &id, &tier1, None).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(response.json::<Value>().await.unwrap(), json!({"left": 0}));
        let request = receive(&mut socket).await;
        let add = &request["request"]["data"]["music"]["command"]["add"];
        assert_eq!(add["title"], LINK);
        assert_eq!(add["user"], "Viewer 11");
        assert_eq!(add["message"], "For you");
        let request_id = add["requestId"].as_str().unwrap().to_string();
        answer(&mut socket, &request, music(json!([]))).await;
        wait_until_added(&api).await;
        let response = send_subscriber_song(&address, &id, &tier1, LINK).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.text().await.unwrap(),
            "You have used all your songs this month."
        );
        let tier3 = viewer("31", &id, Some(3));
        for (duration, left) in [(Some(3600), 2), (Some(301), 1), (Some(300), 0)] {
            let response =
                subscriber_song_found(&mut socket, &address, &id, &tier3, duration).await;
            assert_eq!(
                response.json::<Value>().await.unwrap(),
                json!({"left": left})
            );
            let request = receive(&mut socket).await;
            answer(&mut socket, &request, music(json!([]))).await;
            wait_until_added(&api).await;
        }
        let response = send_subscriber_song(&address, &id, &tier3, LINK).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let response = settings(json!({"subSongs": {"tier1": 1, "tier2": 0, "tier3": 4}}))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(status(&tier3).await.1["left"], 1);
        let response = settings(json!({"subSongs": {"tier1": 1, "tier2": 0, "tier3": 2}}))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(status(&tier3).await.1["left"], 0);
        api.allowances.last_month();
        assert_eq!(status(&tier1).await.1["left"], 1);
        assert_eq!(status(&tier3).await.1["left"], 2);
        let recent: Value = client
            .get(format!("http://{address}/account/recent?kind=song"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let items = recent["items"].as_array().unwrap();
        assert_eq!(items.len(), 4);
        let first = items.last().unwrap();
        assert_eq!(first["id"], request_id.as_str());
        assert_eq!(first["user"], "Viewer 11");
        assert_eq!(first["tier"], 1);
        assert_eq!(first["amount"], 0);
        assert_eq!(first["status"], "delivered");
        assert_eq!(items[0]["tier"], 3);
        let response = settings(json!({"subSongs": null})).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = send_subscriber_song(&address, &id, &tier1, LINK).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let counters = api.counters.all();
        let count = |name: &str| {
            counters
                .iter()
                .find(|counter| counter.name == name)
                .map_or(0, |counter| counter.total)
        };
        assert_eq!(count("subSongs"), 4);
        assert_eq!(count("subSongsFailed:no-songs-left"), 2);
        assert_eq!(count("subSongsFailed:tier-excluded"), 1);
        assert_eq!(count("subSongsFailed:not-subscribed"), 1);
        assert_eq!(count("subSongsFailed:feature-off"), 2);
        assert_eq!(count("subSongsFailed:offline"), 1);
        assert_eq!(count("songDeliveries"), 4);
    }

    #[tokio::test]
    async fn twitch_subscribers_spin_the_wheel_every_month() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        let viewer = |viewer: &str, tier: Option<u8>| {
            api.auth.viewer_session(
                Platform::Twitch,
                viewer.to_string(),
                format!("Viewer {viewer}"),
                id.clone(),
                tier,
            )
        };
        let spin = async |session: &str| {
            client
                .post(format!("http://{address}/pages/{id}/spin/subscriber"))
                .bearer_auth(session)
                .json(&json!({"message": " Big one "}))
                .send()
                .await
                .unwrap()
        };
        let tier1 = viewer("11", Some(1));
        assert_eq!(spin(&tier1).await.status(), StatusCode::BAD_REQUEST);
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({
                "subSpins": {"tier1": 1, "tier2": 0, "tier3": 2},
                "subSongs": {"tier1": 1, "tier2": 0, "tier3": 2},
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let public: Value = client
            .get(format!("http://{address}/pages/{id}/spin"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            public["subSpins"],
            json!({"tier1": 1, "tier2": 0, "tier3": 2})
        );
        assert_eq!(public["spin"], Value::Null);
        assert_eq!(spin(&tier1).await.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            spin(&viewer("12", None)).await.status(),
            StatusCode::FORBIDDEN
        );
        let mut socket = connect(&address, &key).await;
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
        let response = spin(&tier1).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(response.json::<Value>().await.unwrap(), json!({"left": 0}));
        let request = receive(&mut socket).await;
        let sent = &request["request"]["data"]["spinWheelOfLuck"];
        assert_eq!(sent["name"], "Viewer 11");
        assert_eq!(sent["message"], "Big one");
        assert_eq!(sent["count"], 1);
        answer(&mut socket, &request, Value::Null).await;
        wait_until_added(&api).await;
        let response = spin(&tier1).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.text().await.unwrap(),
            "You have used all your spins this month."
        );
        let upgraded: Value = client
            .get(format!("http://{address}/pages/{id}/spin/subscriber"))
            .bearer_auth(viewer("11", Some(3)))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            upgraded,
            json!({"platform": "twitch", "name": "Viewer 11", "tier": 3, "allowance": 2, "left": 1})
        );
        let status: Value = client
            .get(format!("http://{address}/pages/{id}/spin/subscriber"))
            .bearer_auth(viewer("31", Some(3)))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            status,
            json!({"platform": "twitch", "name": "Viewer 31", "tier": 3, "allowance": 2, "left": 2})
        );
        let recent: Value = client
            .get(format!("http://{address}/account/recent?kind=spin"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(recent["items"][0]["user"], "Viewer 11");
        assert_eq!(recent["items"][0]["tier"], 1);
        assert_eq!(recent["items"][0]["spins"], 1);
        assert_eq!(recent["items"][0]["status"], "delivered");
        let counters = api.counters.all();
        let count = |name: &str| {
            counters
                .iter()
                .find(|counter| counter.name == name)
                .map_or(0, |counter| counter.total)
        };
        assert_eq!(count("subSpins"), 1);
        assert_eq!(count("subSpinsFailed:no-spins-left"), 1);
        assert_eq!(count("subSpinsFailed:offline"), 1);
        assert_eq!(count("spinDeliveries"), 1);
        let songs: Value = client
            .get(format!("http://{address}/pages/{id}/songs/subscriber"))
            .bearer_auth(&tier1)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(songs["left"], 1);
        let racer = viewer("13", Some(1));
        let (first, second) = tokio::join!(spin(&racer), spin(&racer));
        let mut statuses = [first.status(), second.status()];
        statuses.sort();
        assert_eq!(statuses, [StatusCode::ACCEPTED, StatusCode::CONFLICT]);
        let request = receive(&mut socket).await;
        answer(&mut socket, &request, Value::Null).await;
        wait_until_added(&api).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(200), receive(&mut socket))
                .await
                .is_err()
        );
        let tier3 = viewer("31", Some(3));
        assert_eq!(spin(&tier3).await.status(), StatusCode::ACCEPTED);
        let request = receive(&mut socket).await;
        let request_id = request["request"]["data"]["spinWheelOfLuck"]["requestId"].clone();
        drop(socket);
        let mut socket = connect(&address, &key).await;
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"]["spinWheelOfLuck"]["requestId"],
            request_id
        );
        answer(&mut socket, &request, Value::Null).await;
        wait_until_added(&api).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(200), receive(&mut socket))
                .await
                .is_err()
        );
        let recent: Value = client
            .get(format!("http://{address}/account/recent?kind=spin"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(recent["items"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn kick_subscribers_get_the_tier_1_allowance() {
        let (kick, fake) = start_kick().await;
        let (address, api) = start_api_with(None, None, None, Some(kick_app(&kick)), None).await;
        let flow = SignInFlow::new(&address, fake.clone());
        let client = reqwest::Client::new();
        let session = session_of(&flow.sign_in("kick", "kick-a").await);
        let account = account_of(&address, &session).await;
        let id = account["settings"]["id"].as_str().unwrap().to_string();
        let key = key_of(&account);
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({
                "subSongs": {"tier1": 2, "tier2": 5, "tier3": 5},
                "subSpins": {"tier1": 0, "tier2": 1, "tier3": 1},
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let twitch_only = create_account(&api, "2", "Other", "other").page.id;
        for page in ["unknown", twitch_only.as_str()] {
            let response = client
                .get(format!("http://{address}/auth/kick/login?page={page}"))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
        let response = client
            .get(format!("http://{address}/auth/twitch/login?page={id}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let viewer_sign_in = async |code: &str| {
            let (query, cookie) = flow.start(&format!("/auth/kick/login?page={id}")).await;
            assert_eq!(query["scope"], "user:read channel:read");
            flow.finish("kick", code, &query["state"], &cookie).await
        };
        let viewer_of = |page: &str| {
            page.strip_prefix("https://moblin.app/kick/kickerik#viewer=")
                .unwrap_or_else(|| panic!("{page}"))
                .to_string()
        };
        let subscriber = viewer_of(&viewer_sign_in("kick-sub").await);
        let viewer = api.auth.viewer_of(&subscriber).unwrap();
        assert_eq!(
            (
                viewer.platform,
                viewer.id.as_str(),
                viewer.name.as_str(),
                viewer.page.as_str(),
                viewer.tier
            ),
            (Platform::Kick, "777", "Sub", id.as_str(), Some(1))
        );
        assert_eq!(
            api.accounts.account_of_identity(Platform::Kick, "777"),
            None
        );
        assert_eq!(api.auth.session_of(&subscriber), None);
        let allowance = async |session: &str, kind: &str| {
            let response = client
                .get(format!("http://{address}/pages/{id}/{kind}/subscriber"))
                .bearer_auth(session)
                .send()
                .await
                .unwrap();
            (
                response.status(),
                response.json::<Value>().await.unwrap_or_default(),
            )
        };
        assert_eq!(
            allowance(&subscriber, "songs").await,
            (
                StatusCode::OK,
                json!({"platform": "kick", "name": "Sub", "tier": 1, "allowance": 2, "left": 2})
            )
        );
        assert_eq!(
            allowance(&subscriber, "spin").await,
            (
                StatusCode::OK,
                json!({"platform": "kick", "name": "Sub", "tier": 1, "allowance": 0, "left": 0})
            )
        );
        let not_subscribed = viewer_of(&viewer_sign_in("kick-b").await);
        assert_eq!(api.auth.viewer_of(&not_subscribed).unwrap().tier, None);
        let lapsed = viewer_of(&viewer_sign_in("kick-lapsed").await);
        assert_eq!(api.auth.viewer_of(&lapsed).unwrap().tier, None);
        assert_eq!(
            viewer_sign_in("kick-broken").await,
            "https://moblin.app/kick/kickerik#viewerCheckFailed"
        );
        assert_eq!(
            viewer_sign_in("unknown").await,
            "https://moblin.app/kick/kickerik#viewerSignInFailed=kick"
        );
        let (query, _) = flow.start(&format!("/auth/kick/login?page={id}")).await;
        assert_eq!(
            flow.finish(
                "kick",
                "kick-sub",
                &query["state"],
                "moblin_sign_in_state=other"
            )
            .await,
            "https://moblin.app/kick/kickerik#viewerSignInFailed=kick"
        );
        let response = send_subscriber_song(&address, &id, &not_subscribed, LINK).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response.text().await.unwrap(),
            "Only Kick subscribers of this streamer can request songs."
        );
        let response = client
            .post(format!("http://{address}/pages/{id}/spin/subscriber"))
            .bearer_auth(&subscriber)
            .json(&json!({}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response.text().await.unwrap(),
            "Kick subscribers can't spin the wheel here."
        );
        let mut socket = connect(&address, &key).await;
        let response =
            subscriber_song_found(&mut socket, &address, &id, &subscriber, Some(200)).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(response.json::<Value>().await.unwrap(), json!({"left": 1}));
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"]["music"]["command"]["add"]["user"],
            "Sub"
        );
        answer(&mut socket, &request, music(json!([]))).await;
        wait_until_added(&api).await;
        assert_eq!(allowance(&subscriber, "songs").await.1["left"], 1);
        let recent: Value = client
            .get(format!("http://{address}/account/recent?kind=song"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(recent["items"][0]["user"], "Sub");
        assert_eq!(recent["items"][0]["kickSubscriber"], true);
        assert_eq!(recent["items"][0]["amount"], 0);
        assert!(recent["items"][0].get("tier").is_none());
        assert!(recent["items"][0].get("provider").is_none());
        assert_eq!(api.allowances.left(&id, "777", "song", 2).unwrap(), 2);
        assert_eq!(api.allowances.left(&id, "kick:777", "song", 2).unwrap(), 1);
        let twitch_viewer = api.auth.viewer_session(
            Platform::Twitch,
            "777".to_string(),
            "Sub".to_string(),
            id.clone(),
            Some(1),
        );
        assert_eq!(
            allowance(&twitch_viewer, "songs").await.0,
            StatusCode::UNAUTHORIZED
        );
        assert!(
            api.counters
                .all()
                .iter()
                .any(|counter| counter.name == "subSongs" && counter.total == 1)
        );
    }
}
