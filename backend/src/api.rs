use std::sync::Arc;

use anyhow::{Context, Result};
use axum::body::Bytes;
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};

use crate::app_attest::AppAttest;
use crate::challenges::Challenges;
use crate::live::Live;
use crate::remote_control::{self, MusicCommand, MusicStatus, RequestError, Streamers};
use crate::store::{Channel, Store, Streamer};

const ASSERTION_HEADER: &str = "moblin-assertion";

pub struct Api {
    pub store: Arc<Store>,
    pub challenges: Challenges,
    pub app_attest: Option<AppAttest>,
    pub live: Option<Live>,
    pub remote_control: Streamers,
}

pub fn router(api: Arc<Api>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(HeaderValue::from_static("https://moblin.app"))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);
    Router::new()
        .route("/streamers", get(handle_streamers))
        .route("/streamers/live/challenge", post(handle_challenge))
        .route("/streamers/live", post(handle_streamers_live))
        .route("/twitch/live", get(handle_twitch_live))
        .route(
            "/remote-control/streamers",
            post(handle_remote_control_streamers),
        )
        .route(
            "/remote-control/streamer/{key}",
            get(handle_remote_control_streamer),
        )
        .route(
            "/remote-control/{id}/music",
            get(handle_music_status).post(handle_music_add),
        )
        .layer(cors)
        .with_state(api)
}

#[derive(Serialize)]
struct StreamersResponse {
    streamers: Vec<Streamer>,
}

async fn handle_streamers(State(api): State<Arc<Api>>) -> Json<StreamersResponse> {
    Json(StreamersResponse {
        streamers: api.store.streamers(),
    })
}

#[derive(Serialize)]
struct TwitchLiveResponse {
    channel: String,
    live: bool,
}

async fn handle_twitch_live(
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
struct ChallengeResponse {
    challenge: String,
}

async fn handle_challenge(
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
struct LiveRequest {
    channels: Vec<Channel>,
    challenge: Option<String>,
    key_id: Option<String>,
    attestation: Option<String>,
    attestation_challenge: Option<String>,
}

async fn handle_streamers_live(
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

#[derive(Serialize)]
struct RemoteControlStreamerResponse {
    key: String,
    id: String,
}

async fn handle_remote_control_streamers(
    State(api): State<Arc<Api>>,
) -> Result<Json<RemoteControlStreamerResponse>, StatusCode> {
    let (key, id) = api
        .remote_control
        .new_key()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(RemoteControlStreamerResponse { key, id }))
}

async fn handle_remote_control_streamer(
    State(api): State<Arc<Api>>,
    Path(key): Path<String>,
    upgrade: WebSocketUpgrade,
) -> Result<Response, StatusCode> {
    let id = remote_control::id_of(&key).ok_or(StatusCode::NOT_FOUND)?;
    Ok(upgrade.on_upgrade(move |socket| async move { api.remote_control.serve(id, socket).await }))
}

fn music_error(error: RequestError) -> (StatusCode, String) {
    match error {
        RequestError::Offline => (StatusCode::NOT_FOUND, "the streamer is offline".to_string()),
        RequestError::TooFast => (
            StatusCode::TOO_MANY_REQUESTS,
            "a song was just added, try again in a few seconds".to_string(),
        ),
        RequestError::Timeout => (
            StatusCode::GATEWAY_TIMEOUT,
            "the streamer did not answer".to_string(),
        ),
        RequestError::Failed => (
            StatusCode::BAD_GATEWAY,
            "the streamer could not do it".to_string(),
        ),
    }
}

async fn handle_music_status(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
) -> Result<Json<MusicStatus>, (StatusCode, String)> {
    let status = api
        .remote_control
        .music(&id, MusicCommand::Status {})
        .await
        .map_err(music_error)?;
    Ok(Json(status))
}

#[derive(Deserialize)]
struct AddSongRequest {
    song: String,
    user: Option<String>,
}

async fn handle_music_add(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    Json(request): Json<AddSongRequest>,
) -> Result<Json<MusicStatus>, (StatusCode, String)> {
    let title = remote_control::normalize_song(&request.song).ok_or((
        StatusCode::BAD_REQUEST,
        "the song must be 1 to 200 characters".to_string(),
    ))?;
    let user = request
        .user
        .map(|user| user.trim().chars().take(25).collect::<String>())
        .filter(|user| !user.is_empty())
        .unwrap_or_else(|| "Moblin website".to_string());
    let status = api
        .remote_control
        .music(
            &id,
            MusicCommand::Add {
                title: title.clone(),
                user: user.clone(),
            },
        )
        .await
        .map_err(music_error)?;
    info!("added {title:?} by {user:?} to the queue of remote control streamer {id}");
    Ok(Json(status))
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

fn base64(text: &str) -> Result<Vec<u8>> {
    BASE64_STANDARD.decode(text).context("not base64")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures_util::{SinkExt, StreamExt};
    use serde_json::{Value, json};
    use tokio_tungstenite::tungstenite::Message;

    use super::*;

    async fn start() -> String {
        let api = Arc::new(Api {
            store: Arc::new(Store::new(6)),
            challenges: Challenges::new(),
            app_attest: None,
            live: None,
            remote_control: Streamers::new(),
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router(api)).await });
        format!("127.0.0.1:{}", address.port())
    }

    async fn receive<S>(socket: &mut S) -> Value
    where
        S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    {
        loop {
            if let Message::Text(text) = socket.next().await.unwrap().unwrap() {
                return serde_json::from_str(&text).unwrap();
            }
        }
    }

    #[tokio::test]
    async fn songs_are_added_through_the_streamer() {
        let address = start().await;
        let client = reqwest::Client::new();
        let streamer: Value = client
            .post(format!("http://{address}/remote-control/streamers"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let key = streamer["key"].as_str().unwrap();
        let id = streamer["id"].as_str().unwrap().to_string();
        let music_url = format!("http://{address}/remote-control/{id}/music");
        let response = client.get(&music_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let (mut socket, _) = tokio_tungstenite::connect_async(format!(
            "ws://{address}/remote-control/streamer/{key}"
        ))
        .await
        .unwrap();
        let hello = receive(&mut socket).await;
        assert_eq!(hello["hello"]["apiVersion"], "0.1");
        socket
            .send(Message::text(
                json!({"identify": {"streamerId": "A", "authentication": "x"}}).to_string(),
            ))
            .await
            .unwrap();
        assert_eq!(
            receive(&mut socket).await,
            json!({"identified": {"result": {"ok": {}}}})
        );
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        let add = tokio::spawn({
            let client = client.clone();
            let music_url = music_url.clone();
            async move {
                client
                    .post(music_url)
                    .json(&json!({"song": "1440818839", "user": " Viewer "}))
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
            json!({"music": {"command": {"add": {
                "title": "https://music.apple.com/song/1440818839?i=1440818839",
                "user": "Viewer",
            }}}})
        );
        let status = json!({
            "playing": true,
            "currentSongIndex": 0,
            "songs": [{"id": "q1", "title": "Song", "artist": "Band", "user": "Viewer"}],
            "songNotFound": false,
        });
        socket
            .send(Message::text(
                json!({"response": {
                    "id": request["request"]["id"],
                    "result": {"ok": {}},
                    "data": {"music": {"status": status}},
                }})
                .to_string(),
            ))
            .await
            .unwrap();
        assert_eq!(add.await.unwrap(), status);
        let response = client
            .post(&music_url)
            .json(&json!({"song": "another"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        let cached: Value = client
            .get(&music_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(cached["songs"], status["songs"]);
        socket.close(None).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let response = client.get(&music_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
