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
use serde_json::Value;
use tower_http::cors::{Any, CorsLayer};

use crate::app_attest::AppAttest;
use crate::challenges::Challenges;
use crate::live::Live;
use crate::paypal::{CaptureError, Order, PayPal, PayeeError};
use crate::remote_control::{self, MusicCommand, MusicStatus, RequestError, Streamers};
use crate::song_queue::{self, QueuedSong, SongQueue};
use crate::song_requests::{Paywall, Settings, SettingsError, SongRequests, format_price};
use crate::store::{Channel, Store, Streamer};

const ASSERTION_HEADER: &str = "moblin-assertion";

pub struct Api {
    pub store: Arc<Store>,
    pub challenges: Challenges,
    pub app_attest: Option<AppAttest>,
    pub live: Option<Live>,
    pub remote_control: Streamers,
    pub song_requests: SongRequests,
    pub paypal: Option<PayPal>,
    pub song_queue: SongQueue,
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
            "/remote-control/streamer/{key}/settings",
            get(handle_settings).post(handle_settings_update),
        )
        .route("/remote-control/names/{name}", get(handle_name))
        .route("/remote-control/{id}/paywall", get(handle_paywall))
        .route(
            "/remote-control/{id}/music",
            get(handle_music_status).post(handle_music_add),
        )
        .route("/remote-control/{id}/music/orders", post(handle_order))
        .route(
            "/remote-control/{id}/music/orders/{order_id}/capture",
            post(handle_capture),
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

#[derive(Serialize)]
struct SettingsResponse {
    id: String,
    connected: bool,
    name: Option<String>,
    paywall: Option<Paywall>,
    paypal: bool,
}

fn settings_response(api: &Api, id: String, settings: Settings) -> Json<SettingsResponse> {
    Json(SettingsResponse {
        connected: api.remote_control.is_connected(&id),
        id,
        name: settings.name,
        paywall: settings.paywall,
        paypal: api.paypal.is_some(),
    })
}

async fn handle_settings(
    State(api): State<Arc<Api>>,
    Path(key): Path<String>,
) -> Result<Json<SettingsResponse>, StatusCode> {
    let id = remote_control::id_of(&key).ok_or(StatusCode::NOT_FOUND)?;
    let settings = api.song_requests.get(&id);
    Ok(settings_response(&api, id, settings))
}

#[derive(Deserialize)]
struct SettingsRequest {
    name: Option<String>,
    paywall: Option<Paywall>,
}

async fn handle_settings_update(
    State(api): State<Arc<Api>>,
    Path(key): Path<String>,
    Json(request): Json<SettingsRequest>,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let id = remote_control::id_of(&key)
        .ok_or((StatusCode::NOT_FOUND, "unknown streamer".to_string()))?;
    let paywall = match request.paywall {
        Some(paywall) => Some(paywall_with_merchant_id(&api, &id, paywall).await?),
        None => None,
    };
    let settings = api
        .song_requests
        .set(&id, request.name.as_deref().unwrap_or(""), paywall)
        .map_err(|error| match error {
            SettingsError::InvalidName => (
                StatusCode::BAD_REQUEST,
                "the name must be 3 to 25 letters, digits and dashes".to_string(),
            ),
            SettingsError::InvalidPaywall => invalid_paywall(),
            SettingsError::NameTaken => (StatusCode::CONFLICT, "the name is taken".to_string()),
            SettingsError::Full => (
                StatusCode::SERVICE_UNAVAILABLE,
                "too many streamers".to_string(),
            ),
            SettingsError::NotSaved => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "the settings could not be saved".to_string(),
            ),
        })?;
    info!(
        "remote control streamer {id} set name {:?} and paywall {:?}",
        settings.name, settings.paywall
    );
    Ok(settings_response(&api, id, settings))
}

fn invalid_paywall() -> (StatusCode, String) {
    (
        StatusCode::BAD_REQUEST,
        "the paywall needs a PayPal email, a price of 0.50 to 1000 and a supported currency"
            .to_string(),
    )
}

async fn paywall_with_merchant_id(
    api: &Api,
    id: &str,
    paywall: Paywall,
) -> Result<Paywall, (StatusCode, String)> {
    let mut paywall = paywall.normalize().ok_or_else(invalid_paywall)?;
    let paypal = api.paypal.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "payments are unavailable".to_string(),
    ))?;
    paywall.merchant_id = match api
        .song_requests
        .get(id)
        .paywall
        .filter(|current| current.email == paywall.email)
        .and_then(|current| current.merchant_id)
    {
        Some(merchant_id) => Some(merchant_id),
        None => paypal
            .merchant_id(&paywall.email, paywall.price, &paywall.currency)
            .await
            .map_err(|error| match error {
                PayeeError::Invalid => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "PayPal can't send payments to that email".to_string(),
                ),
                PayeeError::Failed => (
                    StatusCode::BAD_GATEWAY,
                    "PayPal could not be reached".to_string(),
                ),
            })?,
    };
    Ok(paywall)
}

#[derive(Serialize)]
struct NameResponse {
    id: String,
}

async fn handle_name(
    State(api): State<Arc<Api>>,
    Path(name): Path<String>,
) -> Result<Json<NameResponse>, StatusCode> {
    let id = api
        .song_requests
        .id_of_name(&name)
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(NameResponse { id }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicPaywall {
    price: u32,
    currency: String,
    client_id: Option<String>,
    merchant_id: String,
}

#[derive(Serialize)]
struct PaywallResponse {
    paywall: Option<PublicPaywall>,
}

async fn handle_paywall(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
) -> Json<PaywallResponse> {
    Json(PaywallResponse {
        paywall: api
            .song_requests
            .get(&id)
            .paywall
            .map(|paywall| PublicPaywall {
                price: paywall.price,
                currency: paywall.currency,
                client_id: api
                    .paypal
                    .as_ref()
                    .map(|paypal| paypal.client_id().to_string()),
                merchant_id: paywall.merchant_id.unwrap_or(paywall.email),
            }),
    })
}

type Failure = (StatusCode, String);

fn failure(status: StatusCode, text: &str) -> Failure {
    (status, text.to_string())
}

fn music_error(error: RequestError) -> Failure {
    match error {
        RequestError::Offline => failure(
            StatusCode::NOT_FOUND,
            "The streamer is offline right now. Try again when they are live.",
        ),
        RequestError::TooFast => failure(
            StatusCode::TOO_MANY_REQUESTS,
            "A song was just requested. Try again in a few seconds.",
        ),
        RequestError::Timeout => failure(
            StatusCode::GATEWAY_TIMEOUT,
            "The streamer's Moblin did not answer. It may need an update.",
        ),
        RequestError::Failed => failure(
            StatusCode::BAD_GATEWAY,
            "The streamer's Moblin could not do it. Try again in a moment.",
        ),
    }
}

async fn handle_music_status(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
) -> Result<Json<MusicStatus>, Failure> {
    let status = api
        .remote_control
        .music(&id, MusicCommand::Status {})
        .await
        .map_err(music_error)?;
    Ok(Json(status))
}

#[derive(Deserialize)]
struct SongRequest {
    song: String,
    user: Option<String>,
}

struct RequestedSong {
    link: String,
    user: String,
    name: String,
}

fn song_and_user(request: SongRequest) -> Result<(String, String), Failure> {
    let link = remote_control::song_link(&request.song).ok_or(failure(
        StatusCode::BAD_REQUEST,
        "Paste the Apple Music share link of a song.",
    ))?;
    let user = request
        .user
        .map(|user| user.trim().chars().take(25).collect::<String>())
        .filter(|user| !user.is_empty())
        .unwrap_or_else(|| "Moblin website".to_string());
    Ok((link, user))
}

async fn requested_song(
    api: &Api,
    id: &str,
    (link, user): (String, String),
) -> Result<RequestedSong, Failure> {
    let song = api
        .remote_control
        .find(id, link.clone())
        .await
        .map_err(music_error)?
        .ok_or(failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            "That song is not available on the streamer's Apple Music.",
        ))?;
    Ok(RequestedSong {
        link,
        user,
        name: song.name(),
    })
}

fn queue(
    api: &Api,
    id: String,
    request_id: String,
    song: RequestedSong,
) -> (StatusCode, Json<Value>) {
    info!(
        "queued {:?} by {:?} in request {request_id} for remote control streamer {id}",
        song.link, song.user
    );
    api.song_queue.add(QueuedSong {
        streamer: id,
        request_id,
        link: song.link,
        user: song.user,
        requested_at: song_queue::now(),
    });
    (
        StatusCode::ACCEPTED,
        Json(Value::Object(Default::default())),
    )
}

async fn handle_music_add(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    Json(request): Json<SongRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    if api.song_requests.get(&id).paywall.is_some() {
        return Err(failure(
            StatusCode::PAYMENT_REQUIRED,
            "The streamer now asks for a payment. Try again.",
        ));
    }
    let song_and_user = song_and_user(request)?;
    api.remote_control
        .limit_requests(&id)
        .map_err(music_error)?;
    let song = requested_song(&api, &id, song_and_user).await?;
    let request_id = api.remote_control.new_request_id().ok_or(failure(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Something went wrong. Try again.",
    ))?;
    Ok(queue(&api, id, request_id, song))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OrderResponse {
    order_id: String,
}

async fn handle_order(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    Json(request): Json<SongRequest>,
) -> Result<Json<OrderResponse>, Failure> {
    let paywall = api.song_requests.get(&id).paywall.ok_or(failure(
        StatusCode::BAD_REQUEST,
        "The streamer no longer asks for a payment. Reload the page.",
    ))?;
    let paypal = api.paypal.as_ref().ok_or(failure(
        StatusCode::SERVICE_UNAVAILABLE,
        "Payments are unavailable right now. Try again later.",
    ))?;
    let song = requested_song(&api, &id, song_and_user(request)?).await?;
    let order = Order {
        streamer: id.clone(),
        link: song.link,
        song: song.name,
        user: song.user,
        email: paywall.email,
        merchant_id: paywall.merchant_id,
        price: paywall.price,
        currency: paywall.currency,
    };
    let order_id = paypal.create_order(order).await.map_err(|error| {
        warn!("creating a PayPal order for remote control streamer {id} failed: {error:#}");
        failure(
            StatusCode::BAD_GATEWAY,
            "PayPal could not start the payment. Try again.",
        )
    })?;
    Ok(Json(OrderResponse { order_id }))
}

async fn handle_capture(
    State(api): State<Arc<Api>>,
    Path((id, order_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    tokio::spawn(capture(api, id, order_id))
        .await
        .unwrap_or_else(|_| {
            Err(failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Something went wrong with the payment.",
            ))
        })
}

async fn capture(
    api: Arc<Api>,
    id: String,
    order_id: String,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let paypal = api.paypal.as_ref().ok_or(failure(
        StatusCode::SERVICE_UNAVAILABLE,
        "Payments are unavailable right now. Try again later.",
    ))?;
    let order = paypal
        .order(&order_id)
        .filter(|order| order.streamer == id)
        .ok_or(failure(
            StatusCode::GONE,
            "This payment is unknown or expired, so no money was taken.",
        ))?;
    if !api.remote_control.is_connected(&id) {
        return Err(failure(
            StatusCode::NOT_FOUND,
            "The streamer went offline, so your payment was not taken.",
        ));
    }
    paypal
        .capture(&order_id, &order)
        .await
        .map_err(|error| match error {
            CaptureError::NotApproved => failure(
                StatusCode::PAYMENT_REQUIRED,
                "The payment was not completed. No song was added.",
            ),
            CaptureError::AlreadyCaptured => {
                failure(StatusCode::CONFLICT, "This payment was already used.")
            }
            CaptureError::Failed => failure(
                StatusCode::BAD_GATEWAY,
                "PayPal could not complete the payment.",
            ),
        })?;
    info!(
        "{:?} paid {} {} in PayPal order {order_id}",
        order.user,
        format_price(order.price),
        order.currency
    );
    let song = RequestedSong {
        link: order.link,
        user: order.user,
        name: order.song,
    };
    Ok(queue(&api, id, order_id, song))
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
        start_with(None).await
    }

    async fn start_with(paypal: Option<PayPal>) -> String {
        start_api(paypal).await.0
    }

    async fn start_api(paypal: Option<PayPal>) -> (String, Arc<Api>) {
        let api = Arc::new(Api {
            store: Arc::new(Store::new(6)),
            challenges: Challenges::new(),
            app_attest: None,
            live: None,
            remote_control: Streamers::new(),
            song_requests: SongRequests::load(None).unwrap(),
            paypal,
            song_queue: SongQueue::load(None).unwrap(),
        });
        tokio::spawn({
            let api = api.clone();
            async move { api.song_queue.run(&api.remote_control).await }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn({
            let api = api.clone();
            async move { axum::serve(listener, router(api)).await }
        });
        (format!("127.0.0.1:{}", address.port()), api)
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

    async fn identify<S>(socket: &mut S)
    where
        S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
            + SinkExt<Message>
            + Unpin,
        <S as futures_util::Sink<Message>>::Error: std::fmt::Debug,
    {
        let hello = receive(socket).await;
        assert_eq!(hello["hello"]["apiVersion"], "0.1");
        socket
            .send(Message::text(
                json!({"identify": {"streamerId": "A", "authentication": "x"}}).to_string(),
            ))
            .await
            .unwrap();
        assert_eq!(
            receive(socket).await,
            json!({"identified": {"result": {"ok": {}}}})
        );
    }

    async fn start_paypal() -> (String, Arc<std::sync::Mutex<Option<Value>>>) {
        let order = Arc::new(std::sync::Mutex::new(None::<Value>));
        let router = Router::new()
            .route(
                "/v1/oauth2/token",
                post(|| async {
                    Json(json!({"access_token": "t", "token_type": "Bearer", "expires_in": 3600}))
                }),
            )
            .route(
                "/v2/checkout/orders",
                post({
                    let order = order.clone();
                    move |Json(request): Json<Value>| async move {
                        if request["purchase_units"][0]["payee"]["email_address"]
                            == "nobody@example.com"
                        {
                            return (
                                StatusCode::UNPROCESSABLE_ENTITY,
                                Json(json!({"details": [{"issue": "PAYEE_ACCOUNT_INVALID"}]})),
                            );
                        }
                        *order.lock().unwrap() = Some(request);
                        (
                            StatusCode::CREATED,
                            Json(json!({
                                "id": "ORDER1",
                                "status": "CREATED",
                                "purchase_units": [{"payee": {"merchant_id": "MERCHANT1"}}],
                            })),
                        )
                    }
                }),
            )
            .route(
                "/v2/checkout/orders/{id}/capture",
                post({
                    let order = order.clone();
                    move |Path(id): Path<String>| async move {
                        let amount =
                            order.lock().unwrap().as_ref().unwrap()["purchase_units"][0]["amount"]
                                .clone();
                        if id != "ORDER1" {
                            return (
                                StatusCode::UNPROCESSABLE_ENTITY,
                                Json(json!({"details": [{"issue": "ORDER_NOT_APPROVED"}]})),
                            );
                        }
                        (
                            StatusCode::CREATED,
                            Json(json!({
                                "id": id,
                                "status": "COMPLETED",
                                "purchase_units": [{"payments": {"captures": [
                                    {"status": "COMPLETED", "amount": amount},
                                ]}}],
                            })),
                        )
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await });
        (format!("http://{address}"), order)
    }

    const LINK: &str = "https://music.apple.com/song/1440818839?i=1440818839";

    async fn answer<S>(socket: &mut S, request: &Value, data: Value)
    where
        S: SinkExt<Message> + Unpin,
        <S as futures_util::Sink<Message>>::Error: std::fmt::Debug,
    {
        socket
            .send(Message::text(
                json!({"response": {
                    "id": request["request"]["id"],
                    "result": {"ok": {}},
                    "data": data,
                }})
                .to_string(),
            ))
            .await
            .unwrap();
    }

    fn music(songs: Value) -> Value {
        json!({"music": {"status": {
            "playing": false,
            "currentSongIndex": null,
            "songs": songs,
            "songNotFound": false,
        }}})
    }

    async fn connect(
        address: &str,
        key: &str,
    ) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>
    {
        let (mut socket, _) = tokio_tungstenite::connect_async(format!(
            "ws://{address}/remote-control/streamer/{key}"
        ))
        .await
        .unwrap();
        identify(&mut socket).await;
        socket
    }

    async fn start_paid() -> (String, Arc<Api>, Arc<std::sync::Mutex<Option<Value>>>) {
        let (paypal_url, paypal_order) = start_paypal().await;
        let (address, api) = start_api(Some(PayPal::new(
            reqwest::Client::new(),
            paypal_url,
            "id".to_string(),
            "secret".to_string(),
        )))
        .await;
        (address, api, paypal_order)
    }

    async fn paid_streamer(address: &str) -> (String, String) {
        let client = reqwest::Client::new();
        let streamer: Value = client
            .post(format!("http://{address}/remote-control/streamers"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let key = streamer["key"].as_str().unwrap().to_string();
        let response = client
            .post(format!(
                "http://{address}/remote-control/streamer/{key}/settings"
            ))
            .json(&json!({"name": "erik", "paywall": {
                "email": "erik@example.com", "price": 250, "currency": "SEK", "merchantId": "FAKE",
            }}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        (key, streamer["id"].as_str().unwrap().to_string())
    }

    async fn request_song<S>(socket: &mut S, url: &str) -> Value
    where
        S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
            + SinkExt<Message>
            + Unpin,
        <S as futures_util::Sink<Message>>::Error: std::fmt::Debug,
    {
        let created = tokio::spawn({
            let url = url.to_string();
            async move {
                reqwest::Client::new()
                    .post(url)
                    .json(&json!({"song": "https://music.apple.com/us/song/x/1440818839", "user": "Viewer"}))
                    .send()
                    .await
                    .unwrap()
                    .json::<Value>()
                    .await
                    .unwrap()
            }
        });
        let request = receive(socket).await;
        assert_eq!(
            request["request"]["data"],
            json!({"music": {"command": {"find": {"title": LINK}}}})
        );
        answer(
            socket,
            &request,
            json!({"musicFind": {"song": {"id": "1440818839", "title": "Song", "artist": "Band"}}}),
        )
        .await;
        created.await.unwrap()
    }

    async fn wait_until_added(api: &Api) {
        for _ in 0..100 {
            if api.song_queue.pending().is_empty() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("the song is still queued");
    }

    #[tokio::test]
    async fn paid_songs_are_added_after_capture() {
        let (address, api, paypal_order) = start_paid().await;
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
        let id = streamer["id"].as_str().unwrap();
        let settings_url = format!("http://{address}/remote-control/streamer/{key}/settings");
        let response = client
            .post(&settings_url)
            .json(&json!({"name": "erik", "paywall": {
                "email": "nobody@example.com", "price": 250, "currency": "SEK",
            }}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let settings: Value = client
            .post(&settings_url)
            .json(&json!({"name": "erik", "paywall": {
                "email": "erik@example.com", "price": 250, "currency": "SEK", "merchantId": "FAKE",
            }}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            settings["paywall"],
            json!({
                "email": "erik@example.com",
                "price": 250,
                "currency": "SEK",
                "merchantId": "MERCHANT1",
            })
        );
        assert_eq!(settings["paypal"], true);
        let public: Value = client
            .get(format!("http://{address}/remote-control/{id}/paywall"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            public,
            json!({"paywall": {
                "price": 250,
                "currency": "SEK",
                "clientId": "id",
                "merchantId": "MERCHANT1",
            }})
        );
        let music_url = format!("http://{address}/remote-control/{id}/music");
        let response = client
            .post(&music_url)
            .json(&json!({"song": LINK}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
        let orders_url = format!("{music_url}/orders");
        let response = client
            .post(&orders_url)
            .json(&json!({"song": "Song", "user": "Viewer"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
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
            json!({"merchant_id": "MERCHANT1"})
        );
        assert_eq!(
            sent["purchase_units"][0]["description"],
            "Song request: Band - Song"
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
                "requestId": "ORDER1",
            }}}})
        );
        assert!(!api.song_queue.pending().is_empty());
        answer(
            &mut socket,
            &request,
            music(json!([{"id": "q1", "title": "Song"}])),
        )
        .await;
        wait_until_added(&api).await;
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::GONE);
    }

    #[tokio::test]
    async fn paid_songs_are_retried_until_added_once() {
        let (address, api, _) = start_paid().await;
        let (key, id) = paid_streamer(&address).await;
        let orders_url = format!("http://{address}/remote-control/{id}/music/orders");
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
        assert!(!api.song_queue.pending().is_empty());
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
    async fn songs_are_added_through_the_streamer() {
        let (address, api) = start_api(None).await;
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
        let response = client
            .post(&music_url)
            .json(&json!({"song": LINK}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let mut socket = connect(&address, key).await;
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        let response = client
            .post(&music_url)
            .json(&json!({"song": "another"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.text().await.unwrap(),
            "Paste the Apple Music share link of a song."
        );
        assert_eq!(request_song(&mut socket, &music_url).await, json!({}));
        let response = client
            .post(&music_url)
            .json(&json!({"song": LINK}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        let request = receive(&mut socket).await;
        let add = &request["request"]["data"]["music"]["command"]["add"];
        assert_eq!(add["title"], LINK);
        assert_eq!(add["user"], "Viewer");
        let request_id = add["requestId"].as_str().unwrap().to_string();
        assert_eq!(request_id.len(), 22);
        answer(
            &mut socket,
            &request,
            music(json!([{"id": "q1", "title": "Song", "user": "Viewer"}])),
        )
        .await;
        wait_until_added(&api).await;
        let cached: Value = client
            .get(&music_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            cached["songs"],
            json!([{"id": "q1", "title": "Song", "artist": null, "user": "Viewer"}])
        );
        socket.close(None).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let response = client.get(&music_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn songs_not_found_are_dropped() {
        let (address, api) = start_api(None).await;
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
        let id = streamer["id"].as_str().unwrap();
        let music_url = format!("http://{address}/remote-control/{id}/music");
        let mut socket = connect(&address, key).await;
        assert_eq!(request_song(&mut socket, &music_url).await, json!({}));
        let request = receive(&mut socket).await;
        let mut status = music(json!([]));
        status["music"]["status"]["songNotFound"] = json!(true);
        answer(&mut socket, &request, status).await;
        wait_until_added(&api).await;
    }

    #[tokio::test]
    async fn settings_are_changed_with_the_key() {
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
        let id = streamer["id"].as_str().unwrap();
        let settings_url = format!("http://{address}/remote-control/streamer/{key}/settings");
        let settings: Value = client
            .get(&settings_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            settings,
            json!({"id": id, "connected": false, "name": null, "paywall": null, "paypal": false})
        );
        let response = client
            .post(&settings_url)
            .json(&json!({"name": null}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let settings: Value = client
            .post(&settings_url)
            .json(&json!({"name": "Erik"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            settings,
            json!({"id": id, "connected": false, "name": "erik", "paywall": null, "paypal": false})
        );
        let name: Value = client
            .get(format!("http://{address}/remote-control/names/erik"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(name, json!({"id": id}));
        let response = client
            .get(format!("http://{address}/remote-control/names/other"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let other: Value = client
            .post(format!("http://{address}/remote-control/streamers"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let response = client
            .post(format!(
                "http://{address}/remote-control/streamer/{}/settings",
                other["key"].as_str().unwrap()
            ))
            .json(&json!({"name": "erik"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let response = client
            .get(format!(
                "http://{address}/remote-control/streamer/{id}/settings"
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
