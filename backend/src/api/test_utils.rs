use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use axum::http::StatusCode;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

use super::*;
use crate::accounts::{Account, Accounts, Identity, Moderator, Page, Platform};
use crate::auth::{Auth, KickApp, OAuthApp};
use crate::challenges::Challenges;
use crate::counters::Counters;
use crate::database::Database;
use crate::deliveries::Deliveries;
use crate::pages::Pages;
use crate::payments::Orders;
use crate::paypal::PayPal;
use crate::remote_control::Streamers;
use crate::store::Store;
use crate::stripe::Stripe;
use crate::subscribers::Allowances;

mod fake_kick;
mod fake_paypal;
mod fake_stripe;
mod fake_twitch;

pub(crate) use fake_kick::*;
pub(crate) use fake_paypal::*;
pub(crate) use fake_stripe::*;
pub(crate) use fake_twitch::*;

pub(crate) async fn start() -> String {
    start_with(None).await
}

pub(crate) async fn start_with(paypal: Option<PayPal>) -> String {
    start_api(paypal).await.0
}

pub(crate) async fn start_api(paypal: Option<PayPal>) -> (String, Arc<Api>) {
    start_api_with(paypal, None, None, None, None).await
}

pub(crate) async fn start_api_with(
    paypal: Option<PayPal>,
    stripe: Option<Stripe>,
    twitch: Option<OAuthApp>,
    kick: Option<KickApp>,
    pages: Option<Pages>,
) -> (String, Arc<Api>) {
    start_api_lingering(paypal, stripe, twitch, kick, pages, Duration::ZERO).await
}

pub(crate) async fn start_api_lingering(
    paypal: Option<PayPal>,
    stripe: Option<Stripe>,
    twitch: Option<OAuthApp>,
    kick: Option<KickApp>,
    pages: Option<Pages>,
    linger: Duration,
) -> (String, Arc<Api>) {
    let database = Database::memory();
    let pages = pages.unwrap_or_else(|| Pages::new(database.clone()));
    let api = Arc::new(Api {
        store: Arc::new(Store::new(6, Database::memory())),
        challenges: Challenges::new(),
        app_attest: None,
        live: None,
        remote_control: Streamers::new(linger),
        pages,
        paypal,
        stripe,
        orders: Orders::new(database.clone()),
        allowances: Allowances::new(database.clone()),
        deliveries: Deliveries::new(database.clone()),
        auth: Auth::new(
            reqwest::Client::new(),
            "http://localhost".to_string(),
            "https://moblin.app".to_string(),
            twitch,
            kick,
            Some("secret"),
        )
        .unwrap(),
        accounts: Accounts::new(database.clone()),
        counters: Counters::new(database.clone()),
        database,
        admins: vec!["admin".to_string()],
        premium_accounts: vec!["premium".to_string()],
        started: 0,
        tests: Mutex::default(),
    });
    tokio::spawn({
        let api = api.clone();
        async move { api.deliveries.run(&api.remote_control).await }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn({
        let api = api.clone();
        async move { axum::serve(listener, router(api)).await }
    });
    (format!("127.0.0.1:{}", address.port()), api)
}

pub(crate) async fn receive<S>(socket: &mut S) -> Value
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        if let Message::Text(text) = socket.next().await.unwrap().unwrap() {
            return serde_json::from_str(&text).unwrap();
        }
    }
}

pub(crate) async fn identify<S>(socket: &mut S, device: &str)
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
            json!({"identify": {"streamerId": device, "authentication": "x"}}).to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(
        receive(socket).await,
        json!({"identified": {"result": {"ok": {}}}})
    );
}

pub(crate) async fn answer<S>(socket: &mut S, request: &Value, data: Value)
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

pub(crate) type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

pub(crate) async fn connect(address: &str, key: &str) -> Socket {
    connect_as(address, key, "A").await
}

pub(crate) async fn connect_as(address: &str, key: &str, device: &str) -> Socket {
    let (mut socket, _) =
        tokio_tungstenite::connect_async(format!("ws://{address}/remote-control/streamer/{key}"))
            .await
            .unwrap();
    identify(&mut socket, device).await;
    socket
}

pub(crate) async fn start_paid() -> (String, Arc<Api>, Arc<std::sync::Mutex<Option<Value>>>) {
    let (address, api, fake) = start_paid_fake().await;
    (address, api, fake.order)
}

pub(crate) async fn start_paid_fake() -> (String, Arc<Api>, FakePayPal) {
    let (paypal_url, fake) = start_paypal().await;
    let (address, api) = start_api(Some(new_paypal(paypal_url))).await;
    (address, api, fake)
}

pub(crate) async fn paypal_tip_order_as(address: &str, api: &Api) -> (String, String) {
    paypal_tip_order_in(address, api, "USD").await
}

pub(crate) async fn paypal_tip_order_in(
    address: &str,
    api: &Api,
    currency: &str,
) -> (String, String) {
    let (session, _, id) = streamer(address, api).await;
    connect_paypal(api);
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{address}/account/settings"))
        .bearer_auth(&session)
        .json(&json!({"currency": currency, "tips": {"amounts": [500]}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let orders_url = format!("http://{address}/pages/{id}/tips/orders");
    let order: Value = client
        .post(&orders_url)
        .json(&json!({"amount": 700, "user": "Viewer"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(order, json!({"orderId": "ORDER1"}));
    (session, format!("{orders_url}/ORDER1/capture"))
}

pub(crate) fn connect_paypal(api: &Api) {
    api.accounts.start_paypal("1", "t".to_string()).unwrap();
    api.accounts
        .connect_paypal("1", "t", "MERCHANT00001".to_string())
        .unwrap();
}

pub(crate) fn key_of(account: &Value) -> String {
    account["key"].as_str().unwrap().to_string()
}

pub(crate) fn create_account(api: &Api, id: &str, name: &str, login: &str) -> Account {
    let key = api.remote_control.new_key();
    let page = api.remote_control.new_page_id().unwrap();
    api.accounts
        .signed_in(
            &twitch_identity(id, name, login),
            (id.to_string(), Page { id: page, key }),
        )
        .unwrap()
        .1
}

pub(crate) fn twitch_identity(id: &str, name: &str, login: &str) -> Identity {
    Identity {
        platform: Platform::Twitch,
        user_id: id.to_string(),
        login: login.to_string(),
        name: name.to_string(),
        avatar: None,
    }
}

pub(crate) fn twitch_moderator(id: &str, login: &str, name: &str) -> Moderator {
    Moderator {
        id: id.to_string(),
        login: login.to_string(),
        name: name.to_string(),
    }
}

pub(crate) async fn streamer(_address: &str, api: &Api) -> (String, String, String) {
    let account = create_account(api, "1", "Erik", "erik");
    (
        api.auth.session("1", 0),
        account.page.key.clone(),
        account.page.id,
    )
}

pub(crate) async fn wait_until_added(api: &Api) {
    for _ in 0..100 {
        if api.deliveries.pending().is_empty() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("still queued");
}

pub(crate) fn location(response: &reqwest::Response) -> String {
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    response.headers()["location"].to_str().unwrap().to_string()
}

pub(crate) struct SignInFlow {
    pub(crate) client: reqwest::Client,
    pub(crate) address: String,
    pub(crate) fake_kick: FakeKick,
}

impl SignInFlow {
    pub(crate) fn new(address: &str, fake_kick: FakeKick) -> Self {
        Self {
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
            address: address.to_string(),
            fake_kick,
        }
    }

    pub(crate) async fn start(&self, path: &str) -> (HashMap<String, String>, String) {
        let response = self
            .client
            .get(format!("http://{}{path}", self.address))
            .send()
            .await
            .unwrap();
        let cookie = response.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        let query: HashMap<_, _> = reqwest::Url::parse(&location(&response))
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect();
        if let Some(challenge) = query.get("code_challenge") {
            *self.fake_kick.challenge.lock().unwrap() = Some(challenge.clone());
        }
        (query, cookie)
    }

    pub(crate) async fn finish(
        &self,
        platform: &str,
        code: &str,
        state: &str,
        cookie: &str,
    ) -> String {
        location(
            &self
                .client
                .get(format!(
                    "http://{}/auth/{platform}/callback?code={code}&state={state}",
                    self.address
                ))
                .header("cookie", cookie)
                .send()
                .await
                .unwrap(),
        )
    }

    pub(crate) async fn sign_in(&self, platform: &str, code: &str) -> String {
        let (query, cookie) = self.start(&format!("/auth/{platform}/login")).await;
        self.finish(platform, code, &query["state"], &cookie).await
    }

    pub(crate) async fn link(&self, platform: &str, code: &str) -> String {
        let (query, cookie) = self.start(&format!("/auth/{platform}/login?link=1")).await;
        self.finish(platform, code, &query["state"], &cookie).await
    }
}

pub(crate) fn session_of(dashboard: &str) -> String {
    dashboard
        .strip_prefix("https://moblin.app/dashboard/#session=")
        .unwrap_or_else(|| panic!("{dashboard}"))
        .to_string()
}

pub(crate) async fn account_of(address: &str, session: &str) -> Value {
    reqwest::Client::new()
        .get(format!("http://{address}/account"))
        .bearer_auth(session)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

pub(crate) async fn get_json(address: &str, path: &str) -> (StatusCode, Value) {
    let response = reqwest::Client::new()
        .get(format!("http://{address}{path}"))
        .send()
        .await
        .unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}
