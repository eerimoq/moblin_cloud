use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use reqwest::{Client, Url};
use ring::digest::{SHA256, digest};
use ring::hmac;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Deserializer, Serialize};

use crate::accounts::{Assistant, Identity, Platform};

const STATE_LIFETIME: Duration = Duration::from_secs(600);
const CODE_LIFETIME: Duration = Duration::from_secs(60);
const CONNECT_LIFETIME: Duration = Duration::from_secs(600);
const MAX_PENDING: usize = 10_000;
const SESSION_LIFETIME: u64 = 30 * 24 * 60 * 60;
const VIEWER_LIFETIME: u64 = 24 * 60 * 60;
const LINK_LIFETIME: u64 = 600;
const VIEWER_PREFIX: &str = "viewer:";
const LINK_PREFIX: &str = "link:";
pub const STATE_COOKIE: &str = "moblin_sign_in_state";

pub struct OAuthApp {
    pub client_id: String,
    pub client_secret: String,
    pub authorize_url: String,
    pub token_url: String,
    pub users_url: String,
    pub subscriptions_url: String,
}

impl OAuthApp {
    pub fn twitch(client_id: String, client_secret: String) -> Self {
        Self::at(
            "https://id.twitch.tv",
            "https://api.twitch.tv",
            client_id,
            client_secret,
        )
    }

    pub fn at(id_url: &str, api_url: &str, client_id: String, client_secret: String) -> Self {
        let id_url = id_url.trim_end_matches('/');
        let api_url = api_url.trim_end_matches('/');
        Self {
            client_id,
            client_secret,
            authorize_url: format!("{id_url}/oauth2/authorize"),
            token_url: format!("{id_url}/oauth2/token"),
            users_url: format!("{api_url}/helix/users"),
            subscriptions_url: format!("{api_url}/helix/subscriptions/user"),
        }
    }
}

pub struct KickApp {
    pub client_id: String,
    pub client_secret: String,
    pub authorize_url: String,
    pub token_url: String,
    pub users_url: String,
    pub channels_url: String,
    pub channel_users_url: String,
}

impl KickApp {
    pub fn kick(client_id: String, client_secret: String) -> Self {
        let mut app = Self::at(
            "https://id.kick.com",
            "https://api.kick.com",
            client_id,
            client_secret,
        );
        app.channel_users_url = "https://kick.com/api/v2/channels".to_string();
        app
    }

    pub fn at(id_url: &str, api_url: &str, client_id: String, client_secret: String) -> Self {
        let id_url = id_url.trim_end_matches('/');
        let api_url = api_url.trim_end_matches('/');
        Self {
            client_id,
            client_secret,
            authorize_url: format!("{id_url}/oauth/authorize"),
            token_url: format!("{id_url}/oauth/token"),
            users_url: format!("{api_url}/public/v1/users"),
            channels_url: format!("{api_url}/public/v1/channels"),
            channel_users_url: format!("{api_url}/api/v2/channels"),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Session {
    pub account: String,
    pub generation: u64,
    pub issued: u64,
}

impl Session {
    pub fn is_fresh(&self) -> bool {
        now() < self.issued + 600
    }
}

pub struct SignIn {
    pub identity: Identity,
    token: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SignInState {
    pub page: Option<String>,
    pub link: bool,
    pub verifier: Option<String>,
    pub terminal: Option<Terminal>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Terminal {
    pub port: u16,
    pub challenge: String,
}

impl Terminal {
    pub fn new(port: u16, challenge: String) -> Option<Self> {
        (challenge.len() == 43
            && challenge
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'))
        .then_some(Self { port, challenge })
    }
}

struct TerminalCode {
    created: Instant,
    challenge: String,
    session: String,
}

enum Decision {
    Pending,
    Approved(String),
    Denied,
}

struct ConnectSignIn {
    created: Instant,
    user_code: String,
    decision: Decision,
}

#[derive(Default)]
struct ConnectSignIns {
    by_device: HashMap<String, ConnectSignIn>,
    by_user: HashMap<String, String>,
}

impl ConnectSignIns {
    fn remove(&mut self, device_code: &str) -> Option<ConnectSignIn> {
        let sign_in = self.by_device.remove(device_code)?;
        self.by_user.remove(&sign_in.user_code);
        Some(sign_in)
    }
}

#[derive(Debug, PartialEq)]
pub enum ConnectPoll {
    Pending,
    Session(String),
    Denied,
    Gone,
}

pub fn user_code_of(code: &str) -> String {
    code.chars()
        .filter(char::is_ascii_alphabetic)
        .map(|letter| letter.to_ascii_uppercase())
        .collect()
}

pub fn challenge_of(verifier: &str) -> String {
    BASE64_URL_SAFE_NO_PAD.encode(digest(&SHA256, verifier.as_bytes()))
}

#[derive(Debug)]
pub struct StateError {
    pub page: Option<String>,
    pub error: anyhow::Error,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Viewer {
    pub platform: Platform,
    pub id: String,
    pub name: String,
    pub page: String,
    pub tier: Option<u8>,
    pub issued: u64,
}

impl Viewer {
    pub fn key(&self) -> String {
        self.platform.user_key(&self.id)
    }
}

#[derive(Serialize, Deserialize)]
struct Link {
    #[serde(flatten)]
    identity: Identity,
    issued: u64,
}

pub struct Auth {
    client: Client,
    pub backend_url: String,
    pub frontend_url: String,
    twitch: Option<OAuthApp>,
    kick: Option<KickApp>,
    key: hmac::Key,
    random: SystemRandom,
    pending: Mutex<HashMap<String, (Instant, SignInState)>>,
    codes: Mutex<HashMap<String, TerminalCode>>,
    connects: Mutex<ConnectSignIns>,
}

impl Auth {
    pub fn new(
        client: Client,
        backend_url: String,
        frontend_url: String,
        twitch: Option<OAuthApp>,
        kick: Option<KickApp>,
        secret: Option<&str>,
    ) -> Result<Self> {
        let random = SystemRandom::new();
        let key = match secret {
            Some(secret) => hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes()),
            None => hmac::Key::generate(hmac::HMAC_SHA256, &random)
                .map_err(|_| anyhow!("failed to generate a session secret"))?,
        };
        Ok(Self {
            client,
            backend_url: backend_url.trim_end_matches('/').to_string(),
            frontend_url: frontend_url.trim_end_matches('/').to_string(),
            twitch,
            kick,
            key,
            random,
            pending: Mutex::new(HashMap::new()),
            codes: Mutex::new(HashMap::new()),
            connects: Mutex::default(),
        })
    }

    pub fn has_twitch(&self) -> bool {
        self.twitch.is_some()
    }

    pub fn has_kick(&self) -> bool {
        self.kick.is_some()
    }

    fn redirect_uri(&self, platform: Platform) -> String {
        format!("{}/auth/{}/callback", self.backend_url, platform.name())
    }

    pub fn login_url(
        &self,
        platform: Platform,
        page: Option<&str>,
        link: bool,
        terminal: Option<Terminal>,
    ) -> Option<(String, String)> {
        match platform {
            Platform::Twitch => self.twitch_login_url(page, link, terminal),
            Platform::Kick => self.kick_login_url(page, link, terminal),
        }
    }

    fn twitch_login_url(
        &self,
        page: Option<&str>,
        link: bool,
        terminal: Option<Terminal>,
    ) -> Option<(String, String)> {
        let app = self.twitch.as_ref()?;
        let state = self.new_state(SignInState {
            page: page.map(str::to_string),
            link,
            verifier: None,
            terminal,
        })?;
        let scope = if page.is_some() {
            "user:read:subscriptions"
        } else {
            "openid"
        };
        Url::parse_with_params(
            &app.authorize_url,
            [
                ("client_id", app.client_id.as_str()),
                ("redirect_uri", self.redirect_uri(Platform::Twitch).as_str()),
                ("response_type", "code"),
                ("scope", scope),
                ("state", state.as_str()),
            ],
        )
        .ok()
        .map(|url| (url.into(), state))
    }

    fn kick_login_url(
        &self,
        page: Option<&str>,
        link: bool,
        terminal: Option<Terminal>,
    ) -> Option<(String, String)> {
        let app = self.kick.as_ref()?;
        let verifier = self.random_token()?;
        let challenge = challenge_of(&verifier);
        let state = self.new_state(SignInState {
            page: page.map(str::to_string),
            link,
            verifier: Some(verifier),
            terminal,
        })?;
        Url::parse_with_params(
            &app.authorize_url,
            [
                ("client_id", app.client_id.as_str()),
                ("redirect_uri", self.redirect_uri(Platform::Kick).as_str()),
                ("response_type", "code"),
                ("scope", "user:read channel:read"),
                ("code_challenge", challenge.as_str()),
                ("code_challenge_method", "S256"),
                ("state", state.as_str()),
            ],
        )
        .ok()
        .map(|url| (url.into(), state))
    }

    pub fn state_cookie(&self, state: &str) -> String {
        let secure = if self.backend_url.starts_with("https://") {
            "; Secure"
        } else {
            ""
        };
        format!("{STATE_COOKIE}={state}; Max-Age=600; HttpOnly; SameSite=Lax{secure}")
    }

    pub fn cleared_state_cookie(&self) -> String {
        self.state_cookie("").replace("Max-Age=600", "Max-Age=0")
    }

    fn random_token(&self) -> Option<String> {
        let mut bytes = [0; 32];
        self.random.fill(&mut bytes).ok()?;
        Some(BASE64_URL_SAFE_NO_PAD.encode(bytes))
    }

    fn new_state(&self, state: SignInState) -> Option<String> {
        let token = self.random_token()?;
        {
            let mut pending = self.pending.lock().unwrap();
            if pending.len() >= MAX_PENDING {
                pending.retain(|_, (created, _)| created.elapsed() < STATE_LIFETIME);
                if pending.len() >= MAX_PENDING {
                    return None;
                }
            }
            pending.insert(token.clone(), (Instant::now(), state));
        }
        Some(token)
    }

    pub fn terminal_code(&self, challenge: String, session: String) -> Option<String> {
        let code = self.random_token()?;
        let mut codes = self.codes.lock().unwrap();
        if codes.len() >= MAX_PENDING {
            codes.retain(|_, code| code.created.elapsed() < CODE_LIFETIME);
            if codes.len() >= MAX_PENDING {
                return None;
            }
        }
        codes.insert(
            code.clone(),
            TerminalCode {
                created: Instant::now(),
                challenge,
                session,
            },
        );
        Some(code)
    }

    pub fn session_of_code(&self, code: &str, verifier: &str) -> Option<String> {
        let code = self.codes.lock().unwrap().remove(code)?;
        (code.created.elapsed() < CODE_LIFETIME && challenge_of(verifier) == code.challenge)
            .then_some(code.session)
    }

    fn user_code(&self) -> Option<String> {
        let mut code = String::new();
        while code.len() < 8 {
            let mut bytes = [0; 16];
            self.random.fill(&mut bytes).ok()?;
            for byte in bytes.into_iter().filter(|byte| *byte < 240) {
                if code.len() < 8 {
                    code.push(b"BCDFGHJKLMNPQRSTVWXZ"[usize::from(byte % 20)].into());
                }
            }
        }
        Some(code)
    }

    pub fn start_connect(&self) -> Option<(String, String)> {
        let device_code = self.random_token()?;
        let mut connects = self.connects.lock().unwrap();
        if connects.by_device.len() >= MAX_PENDING {
            let expired: Vec<String> = connects
                .by_device
                .iter()
                .filter(|(_, sign_in)| sign_in.created.elapsed() >= CONNECT_LIFETIME)
                .map(|(device_code, _)| device_code.clone())
                .collect();
            for device_code in expired {
                connects.remove(&device_code);
            }
            if connects.by_device.len() >= MAX_PENDING {
                return None;
            }
        }
        let user_code = loop {
            let code = self.user_code()?;
            if !connects.by_user.contains_key(&code) {
                break code;
            }
        };
        connects
            .by_user
            .insert(user_code.clone(), device_code.clone());
        connects.by_device.insert(
            device_code.clone(),
            ConnectSignIn {
                created: Instant::now(),
                user_code: user_code.clone(),
                decision: Decision::Pending,
            },
        );
        Some((device_code, user_code))
    }

    pub fn decide_connect(&self, user_code: &str, session: Option<String>) -> bool {
        let mut connects = self.connects.lock().unwrap();
        let Some(device_code) = connects.by_user.get(&user_code_of(user_code)).cloned() else {
            return false;
        };
        let Some(sign_in) = connects.by_device.get_mut(&device_code) else {
            return false;
        };
        if sign_in.created.elapsed() >= CONNECT_LIFETIME
            || !matches!(sign_in.decision, Decision::Pending)
        {
            return false;
        }
        sign_in.decision = match session {
            Some(session) => Decision::Approved(session),
            None => Decision::Denied,
        };
        true
    }

    pub fn connect_session(&self, device_code: &str) -> ConnectPoll {
        let mut connects = self.connects.lock().unwrap();
        let Some(sign_in) = connects.by_device.get(device_code) else {
            return ConnectPoll::Gone;
        };
        if sign_in.created.elapsed() < CONNECT_LIFETIME
            && matches!(sign_in.decision, Decision::Pending)
        {
            return ConnectPoll::Pending;
        }
        let sign_in = connects.remove(device_code).unwrap();
        if sign_in.created.elapsed() >= CONNECT_LIFETIME {
            return ConnectPoll::Gone;
        }
        match sign_in.decision {
            Decision::Approved(session) => ConnectPoll::Session(session),
            _ => ConnectPoll::Denied,
        }
    }

    pub fn consume_state(
        &self,
        state: &str,
        browser_state: Option<&str>,
    ) -> Result<SignInState, StateError> {
        let Some((created, pending)) = self.pending.lock().unwrap().remove(state) else {
            return Err(StateError {
                page: None,
                error: anyhow!("unknown state"),
            });
        };
        if created.elapsed() >= STATE_LIFETIME {
            return Err(StateError {
                page: pending.page,
                error: anyhow!("expired state"),
            });
        }
        if browser_state != Some(state) {
            return Err(StateError {
                page: pending.page,
                error: anyhow!("the state is from another browser"),
            });
        }
        Ok(pending)
    }

    async fn access_token(&self, url: &str, form: &[(&str, &str)]) -> Result<String> {
        #[derive(Deserialize)]
        struct Token {
            access_token: String,
        }
        let token: Token = self
            .client
            .post(url)
            .form(form)
            .send()
            .await
            .context("failed to exchange the code")?
            .error_for_status()
            .context("the code was refused")?
            .json()
            .await
            .context("unexpected answer to the code exchange")?;
        Ok(token.access_token)
    }

    pub async fn twitch_callback(&self, code: &str) -> Result<SignIn> {
        let app = self.twitch.as_ref().context("Twitch not configured")?;
        let token = self
            .access_token(
                &app.token_url,
                &[
                    ("client_id", app.client_id.as_str()),
                    ("client_secret", app.client_secret.as_str()),
                    ("code", code),
                    ("grant_type", "authorization_code"),
                    ("redirect_uri", self.redirect_uri(Platform::Twitch).as_str()),
                ],
            )
            .await?;
        #[derive(Deserialize)]
        struct Users {
            data: Vec<TwitchUser>,
        }
        #[derive(Deserialize)]
        struct TwitchUser {
            id: String,
            login: String,
            display_name: Option<String>,
            profile_image_url: Option<String>,
        }
        let users: Users = self
            .client
            .get(&app.users_url)
            .bearer_auth(&token)
            .header("Client-Id", &app.client_id)
            .send()
            .await
            .context("failed to get the user")?
            .error_for_status()
            .context("the user was refused")?
            .json()
            .await
            .context("unexpected answer to the user request")?;
        let user = users.data.into_iter().next().context("no user")?;
        if user.id.is_empty() || user.login.is_empty() {
            bail!("no user id or login");
        }
        Ok(SignIn {
            identity: Identity {
                platform: Platform::Twitch,
                name: user
                    .display_name
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| user.login.clone()),
                login: user.login.to_lowercase(),
                avatar: user
                    .profile_image_url
                    .filter(|url| url.starts_with("https://")),
                user_id: user.id,
            },
            token: Some(token),
        })
    }

    pub async fn kick_callback(&self, code: &str, verifier: &str) -> Result<SignIn> {
        let app = self.kick.as_ref().context("Kick not configured")?;
        let token = self
            .access_token(
                &app.token_url,
                &[
                    ("client_id", app.client_id.as_str()),
                    ("client_secret", app.client_secret.as_str()),
                    ("code", code),
                    ("code_verifier", verifier),
                    ("grant_type", "authorization_code"),
                    ("redirect_uri", self.redirect_uri(Platform::Kick).as_str()),
                ],
            )
            .await?;
        #[derive(Deserialize)]
        struct Users {
            data: Vec<KickUser>,
        }
        #[derive(Deserialize)]
        struct KickUser {
            #[serde(deserialize_with = "id_text")]
            user_id: String,
            name: Option<String>,
            profile_picture: Option<String>,
        }
        #[derive(Deserialize)]
        struct Channels {
            data: Vec<KickChannel>,
        }
        #[derive(Deserialize)]
        struct KickChannel {
            #[serde(deserialize_with = "id_text")]
            broadcaster_user_id: String,
            slug: String,
        }
        let users: Users = self
            .client
            .get(&app.users_url)
            .bearer_auth(&token)
            .send()
            .await
            .context("failed to get the user")?
            .error_for_status()
            .context("the user was refused")?
            .json()
            .await
            .context("unexpected answer to the user request")?;
        let user = users.data.into_iter().next().context("no user")?;
        if user.user_id.is_empty() {
            bail!("no user id");
        }
        let channels: Channels = self
            .client
            .get(&app.channels_url)
            .bearer_auth(&token)
            .send()
            .await
            .context("failed to get the channel")?
            .error_for_status()
            .context("the channel was refused")?
            .json()
            .await
            .context("unexpected answer to the channel request")?;
        let channel = channels
            .data
            .into_iter()
            .find(|channel| channel.broadcaster_user_id == user.user_id)
            .context("no channel")?;
        if channel.slug.is_empty() {
            bail!("no channel slug");
        }
        Ok(SignIn {
            identity: Identity {
                platform: Platform::Kick,
                name: user
                    .name
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| channel.slug.clone()),
                login: channel.slug.to_lowercase(),
                avatar: user
                    .profile_picture
                    .filter(|url| url.starts_with("https://")),
                user_id: user.user_id,
            },
            token: None,
        })
    }

    pub async fn twitch_subscription_tier(
        &self,
        sign_in: &SignIn,
        broadcaster: &str,
    ) -> Result<Option<u8>> {
        let app = self.twitch.as_ref().context("Twitch not configured")?;
        let token = sign_in.token.as_deref().context("no access token")?;
        let response = self
            .client
            .get(&app.subscriptions_url)
            .query(&[
                ("broadcaster_id", broadcaster),
                ("user_id", &sign_in.identity.user_id),
            ])
            .bearer_auth(token)
            .header("Client-Id", &app.client_id)
            .send()
            .await
            .context("failed to get the subscription")?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        #[derive(Deserialize)]
        struct Subscriptions {
            data: Vec<Subscription>,
        }
        #[derive(Deserialize)]
        struct Subscription {
            tier: String,
        }
        let subscriptions: Subscriptions = response
            .error_for_status()
            .context("the subscription was refused")?
            .json()
            .await
            .context("unexpected answer to the subscription request")?;
        Ok(subscriptions
            .data
            .first()
            .and_then(|subscription| match subscription.tier.as_str() {
                "1000" => Some(1),
                "2000" => Some(2),
                "3000" => Some(3),
                _ => None,
            }))
    }

    pub async fn kick_subscriber(&self, streamer: &str, viewer: &str) -> Result<bool> {
        let app = self.kick.as_ref().context("Kick not configured")?;
        #[derive(Deserialize)]
        struct ChannelUser {
            #[serde(default)]
            badges: Vec<Badge>,
        }
        #[derive(Deserialize)]
        struct Badge {
            #[serde(rename = "type")]
            kind: String,
            #[serde(default = "yes")]
            active: bool,
        }
        fn yes() -> bool {
            true
        }
        let user: ChannelUser = self
            .client
            .get(format!(
                "{}/{streamer}/users/{viewer}",
                app.channel_users_url
            ))
            .send()
            .await
            .context("failed to get the channel user")?
            .error_for_status()
            .context("the channel user was refused")?
            .json()
            .await
            .context("unexpected answer to the channel user request")?;
        Ok(user
            .badges
            .iter()
            .any(|badge| badge.kind == "subscriber" && badge.active))
    }

    async fn app_token(
        &self,
        token_url: &str,
        client_id: &str,
        client_secret: &str,
    ) -> Result<String> {
        self.access_token(
            token_url,
            &[
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("grant_type", "client_credentials"),
            ],
        )
        .await
    }

    async fn kick_user(&self, username: &str) -> Result<Option<Assistant>> {
        let app = self.kick.as_ref().context("Kick not configured")?;
        let token = self
            .app_token(&app.token_url, &app.client_id, &app.client_secret)
            .await?;
        #[derive(Deserialize)]
        struct Channels {
            data: Vec<Channel>,
        }
        #[derive(Deserialize)]
        struct Channel {
            #[serde(deserialize_with = "id_text")]
            broadcaster_user_id: String,
            slug: String,
        }
        let channels: Channels = self
            .client
            .get(&app.channels_url)
            .query(&[("slug", username)])
            .bearer_auth(&token)
            .send()
            .await
            .context("failed to get the channel")?
            .error_for_status()
            .context("the channel was refused")?
            .json()
            .await
            .context("unexpected answer to the channel request")?;
        Ok(channels
            .data
            .into_iter()
            .find(|channel| !channel.broadcaster_user_id.is_empty() && !channel.slug.is_empty())
            .map(|channel| {
                Assistant::new(
                    channel.broadcaster_user_id,
                    channel.slug.to_lowercase(),
                    channel.slug,
                )
            }))
    }

    async fn twitch_user(&self, username: &str) -> Result<Option<Assistant>> {
        let app = self.twitch.as_ref().context("Twitch not configured")?;
        let token = self
            .app_token(&app.token_url, &app.client_id, &app.client_secret)
            .await?;
        #[derive(Deserialize)]
        struct Users {
            data: Vec<User>,
        }
        #[derive(Deserialize)]
        struct User {
            id: String,
            login: String,
            display_name: Option<String>,
        }
        let users: Users = self
            .client
            .get(&app.users_url)
            .query(&[("login", username)])
            .bearer_auth(&token)
            .header("Client-Id", &app.client_id)
            .send()
            .await
            .context("failed to get the user")?
            .error_for_status()
            .context("the user was refused")?
            .json()
            .await
            .context("unexpected answer to the user request")?;
        Ok(users
            .data
            .into_iter()
            .find(|user| !user.id.is_empty() && !user.login.is_empty())
            .map(|user| {
                Assistant::new(
                    user.id,
                    user.login.to_lowercase(),
                    user.display_name
                        .filter(|name| !name.is_empty())
                        .unwrap_or(user.login),
                )
            }))
    }

    pub async fn user(&self, platform: Platform, username: &str) -> Result<Option<Assistant>> {
        match platform {
            Platform::Twitch => self.twitch_user(username).await,
            Platform::Kick => self.kick_user(username).await,
        }
    }

    pub fn session(&self, account: &str, generation: u64) -> String {
        self.session_at(account, generation, now())
    }

    pub fn session_at(&self, account: &str, generation: u64, issued: u64) -> String {
        self.sign(&format!("{account}:{generation}:{issued}"))
    }

    pub fn session_of(&self, session: &str) -> Option<Session> {
        let payload = self.verified(session)?;
        if payload.starts_with(VIEWER_PREFIX) || payload.starts_with(LINK_PREFIX) {
            return None;
        }
        let (rest, issued) = payload.rsplit_once(':')?;
        let (account, generation) = rest.rsplit_once(':')?;
        let issued = issued.parse::<u64>().ok()?;
        let generation = generation.parse().ok()?;
        (issued + SESSION_LIFETIME > now()).then(|| Session {
            account: account.to_string(),
            generation,
            issued,
        })
    }

    pub fn viewer_session(
        &self,
        platform: Platform,
        id: String,
        name: String,
        page: String,
        tier: Option<u8>,
    ) -> String {
        let viewer = Viewer {
            platform,
            id,
            name,
            page,
            tier,
            issued: now(),
        };
        self.sign(&format!(
            "{VIEWER_PREFIX}{}",
            serde_json::to_string(&viewer).unwrap()
        ))
    }

    pub fn viewer_of(&self, session: &str) -> Option<Viewer> {
        let payload = self.verified(session)?;
        let viewer: Viewer = serde_json::from_str(payload.strip_prefix(VIEWER_PREFIX)?).ok()?;
        (viewer.issued + VIEWER_LIFETIME > now()).then_some(viewer)
    }

    pub fn link_token(&self, identity: Identity) -> String {
        let link = Link {
            identity,
            issued: now(),
        };
        self.sign(&format!(
            "{LINK_PREFIX}{}",
            serde_json::to_string(&link).unwrap()
        ))
    }

    pub fn link_of(&self, token: &str) -> Option<Identity> {
        let payload = self.verified(token)?;
        let link: Link = serde_json::from_str(payload.strip_prefix(LINK_PREFIX)?).ok()?;
        (link.issued + LINK_LIFETIME > now()).then_some(link.identity)
    }

    fn sign(&self, payload: &str) -> String {
        let tag = hmac::sign(&self.key, payload.as_bytes());
        format!(
            "{}.{}",
            BASE64_URL_SAFE_NO_PAD.encode(payload),
            BASE64_URL_SAFE_NO_PAD.encode(tag.as_ref())
        )
    }

    fn verified(&self, token: &str) -> Option<String> {
        let (payload, tag) = token.split_once('.')?;
        let payload = BASE64_URL_SAFE_NO_PAD.decode(payload).ok()?;
        let tag = BASE64_URL_SAFE_NO_PAD.decode(tag).ok()?;
        hmac::verify(&self.key, &payload, &tag).ok()?;
        String::from_utf8(payload).ok()
    }
}

fn id_text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::String(text) => text,
        serde_json::Value::Number(number) => number.to_string(),
        _ => String::new(),
    })
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {

    use super::*;

    fn new_auth(secret: &str) -> Auth {
        Auth::new(
            Client::new(),
            "https://api.example.com/".to_string(),
            "https://example.com".to_string(),
            Some(OAuthApp::twitch("id".to_string(), "secret".to_string())),
            Some(KickApp::kick("kid".to_string(), "ksecret".to_string())),
            Some(secret),
        )
        .unwrap()
    }

    fn query_of(url: &str) -> HashMap<String, String> {
        Url::parse(url)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect()
    }

    #[test]
    fn sessions_are_verified() {
        let auth = new_auth("secret");
        let session = auth.session_of(&auth.session("1", 3)).unwrap();
        assert_eq!(session.account, "1");
        assert_eq!(session.generation, 3);
        assert!(session.is_fresh());
        let session = auth.session_at("1", 0, now() - 601);
        assert!(!auth.session_of(&session).unwrap().is_fresh());
        assert_eq!(new_auth("other").session_of(&session), None);
        let expired = auth.session_at("1", 0, now() - SESSION_LIFETIME);
        assert_eq!(auth.session_of(&expired), None);
        let (_, tag) = session.split_once('.').unwrap();
        let forged = format!(
            "{}.{tag}",
            BASE64_URL_SAFE_NO_PAD.encode(format!("2:0:{}", now()))
        );
        assert_eq!(auth.session_of(&forged), None);
        assert_eq!(auth.session_of("garbage"), None);
    }

    #[test]
    fn connect_codes_are_decided_once_and_expire() {
        let auth = new_auth("secret");
        let (device_code, user_code) = auth.start_connect().unwrap();
        assert_eq!(user_code.len(), 8);
        assert!(
            user_code
                .bytes()
                .all(|byte| b"BCDFGHJKLMNPQRSTVWXZ".contains(&byte))
        );
        assert_eq!(auth.connect_session(&device_code), ConnectPoll::Pending);
        let typed = format!(" {}-{} ", user_code[..4].to_lowercase(), &user_code[4..]);
        assert_eq!(user_code_of(&typed), user_code);
        assert!(auth.decide_connect(&typed, Some("session".to_string())));
        assert!(!auth.decide_connect(&user_code, None));
        assert_eq!(
            auth.connect_session(&device_code),
            ConnectPoll::Session("session".to_string())
        );
        assert_eq!(auth.connect_session(&device_code), ConnectPoll::Gone);
        assert!(!auth.decide_connect(&user_code, Some("session".to_string())));
        let (device_code, user_code) = auth.start_connect().unwrap();
        assert!(auth.decide_connect(&user_code, None));
        assert_eq!(auth.connect_session(&device_code), ConnectPoll::Denied);
        assert_eq!(auth.connect_session(&device_code), ConnectPoll::Gone);
        assert!(!auth.decide_connect("BBBBBBBB", None));
        assert_eq!(auth.connect_session("unknown"), ConnectPoll::Gone);
        let (device_code, user_code) = auth.start_connect().unwrap();
        auth.connects
            .lock()
            .unwrap()
            .by_device
            .get_mut(&device_code)
            .unwrap()
            .created = Instant::now() - CONNECT_LIFETIME * 2;
        assert!(!auth.decide_connect(&user_code, Some("session".to_string())));
        assert_eq!(auth.connect_session(&device_code), ConnectPoll::Gone);
        let codes: std::collections::HashSet<_> =
            (0..100).map(|_| auth.start_connect().unwrap().1).collect();
        assert_eq!(codes.len(), 100);
    }

    #[test]
    fn terminal_codes_are_single_use_and_need_the_verifier() {
        let auth = new_auth("secret");
        let challenge = challenge_of("verifier");
        assert!(Terminal::new(1234, challenge.clone()).is_some());
        assert!(Terminal::new(1234, "short".to_string()).is_none());
        assert!(Terminal::new(1234, format!("{}+", &challenge[1..])).is_none());
        let code = auth
            .terminal_code(challenge.clone(), "session".to_string())
            .unwrap();
        assert_eq!(auth.session_of_code(&code, "other"), None);
        assert_eq!(auth.session_of_code(&code, "verifier"), None);
        let code = auth
            .terminal_code(challenge.clone(), "session".to_string())
            .unwrap();
        assert_eq!(
            auth.session_of_code(&code, "verifier"),
            Some("session".to_string())
        );
        assert_eq!(auth.session_of_code(&code, "verifier"), None);
        auth.codes.lock().unwrap().insert(
            "old".to_string(),
            TerminalCode {
                created: Instant::now() - CODE_LIFETIME * 2,
                challenge,
                session: "session".to_string(),
            },
        );
        assert_eq!(auth.session_of_code("old", "verifier"), None);
    }

    #[test]
    fn states_are_single_use() {
        let auth = new_auth("secret");
        let (url, state) = auth.twitch_login_url(None, false, None).unwrap();
        let url = Url::parse(&url).unwrap();
        let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(url.host_str(), Some("id.twitch.tv"));
        assert_eq!(
            query["redirect_uri"],
            "https://api.example.com/auth/twitch/callback"
        );
        assert_eq!(query["scope"], "openid");
        assert_eq!(query["state"], state);
        let state = Some(state.as_str());
        assert_eq!(
            auth.consume_state(&query["state"], state).unwrap(),
            SignInState::default()
        );
        assert!(auth.consume_state(&query["state"], state).is_err());
        auth.pending.lock().unwrap().insert(
            "old".to_string(),
            (Instant::now() - STATE_LIFETIME * 2, SignInState::default()),
        );
        assert!(auth.consume_state("old", Some("old")).is_err());
        let (url, state) = auth.twitch_login_url(Some("page"), false, None).unwrap();
        let url = Url::parse(&url).unwrap();
        let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(query["scope"], "user:read:subscriptions");
        assert_eq!(
            auth.consume_state(&state, Some("other"))
                .unwrap_err()
                .page
                .as_deref(),
            Some("page")
        );
        let (_, state) = auth.twitch_login_url(Some("page"), false, None).unwrap();
        assert_eq!(
            auth.consume_state(&state, Some(&state))
                .unwrap()
                .page
                .as_deref(),
            Some("page")
        );
        let (_, state) = auth.twitch_login_url(None, true, None).unwrap();
        assert!(auth.consume_state(&state, Some(&state)).unwrap().link);
    }

    #[test]
    fn kick_signs_in_with_pkce() {
        let auth = new_auth("secret");
        let (url, state) = auth.kick_login_url(None, true, None).unwrap();
        let parsed = Url::parse(&url).unwrap();
        assert_eq!(parsed.host_str(), Some("id.kick.com"));
        assert_eq!(parsed.path(), "/oauth/authorize");
        let query = query_of(&url);
        assert_eq!(query["client_id"], "kid");
        assert_eq!(
            query["redirect_uri"],
            "https://api.example.com/auth/kick/callback"
        );
        assert_eq!(query["response_type"], "code");
        assert_eq!(query["scope"], "user:read channel:read");
        assert_eq!(query["code_challenge_method"], "S256");
        assert_eq!(query["state"], state);
        let pending = auth.consume_state(&state, Some(&state)).unwrap();
        assert!(pending.link);
        assert_eq!(pending.page, None);
        let verifier = pending.verifier.unwrap();
        assert!(verifier.len() >= 43);
        assert_eq!(
            query["code_challenge"],
            BASE64_URL_SAFE_NO_PAD.encode(digest(&SHA256, verifier.as_bytes()))
        );
        let (other, _) = auth.kick_login_url(None, false, None).unwrap();
        assert_ne!(query_of(&other)["code_challenge"], query["code_challenge"]);
        let (_, state) = auth.kick_login_url(Some("page"), false, None).unwrap();
        let pending = auth.consume_state(&state, Some(&state)).unwrap();
        assert_eq!(pending.page.as_deref(), Some("page"));
        assert!(!pending.link);
        let without = Auth::new(
            Client::new(),
            "https://api.example.com".to_string(),
            "https://example.com".to_string(),
            None,
            None,
            None,
        )
        .unwrap();
        assert!(without.kick_login_url(None, false, None).is_none());
        assert!(without.twitch_login_url(None, false, None).is_none());
    }

    #[test]
    fn viewer_sessions_are_not_streamer_sessions() {
        let auth = new_auth("secret");
        let session = auth.viewer_session(
            Platform::Twitch,
            "7".to_string(),
            "Viewer".to_string(),
            "page".to_string(),
            Some(2),
        );
        let viewer = auth.viewer_of(&session).unwrap();
        assert_eq!(
            (
                viewer.platform,
                viewer.id.as_str(),
                viewer.name.as_str(),
                viewer.page.as_str(),
                viewer.tier
            ),
            (Platform::Twitch, "7", "Viewer", "page", Some(2))
        );
        assert_eq!(viewer.key(), "7");
        let kick = auth.viewer_session(
            Platform::Kick,
            "7".to_string(),
            "Viewer".to_string(),
            "page".to_string(),
            Some(1),
        );
        let kick = auth.viewer_of(&kick).unwrap();
        assert_eq!(kick.platform, Platform::Kick);
        assert_eq!(kick.key(), "kick:7");
        assert_eq!(auth.session_of(&session), None);
        assert_eq!(new_auth("other").viewer_of(&session), None);
        assert_eq!(auth.viewer_of(&auth.session("1", 0)), None);
        let expired = auth.sign(&format!(
            "{VIEWER_PREFIX}{}",
            serde_json::to_string(&Viewer {
                issued: now() - VIEWER_LIFETIME,
                ..viewer
            })
            .unwrap()
        ));
        assert_eq!(auth.viewer_of(&expired), None);
    }

    #[test]
    fn link_tokens_carry_the_identity_for_ten_minutes() {
        let auth = new_auth("secret");
        let identity = Identity {
            platform: Platform::Kick,
            user_id: "123".to_string(),
            login: "erik".to_string(),
            name: "Erik".to_string(),
            avatar: Some("https://files.kick.com/erik.png".to_string()),
        };
        let token = auth.link_token(identity.clone());
        assert_eq!(auth.link_of(&token), Some(identity.clone()));
        assert_eq!(auth.session_of(&token), None);
        assert_eq!(auth.viewer_of(&token), None);
        assert_eq!(new_auth("other").link_of(&token), None);
        assert_eq!(auth.link_of(&auth.session("1", 0)), None);
        let expired = auth.sign(&format!(
            "{LINK_PREFIX}{}",
            serde_json::to_string(&Link {
                identity,
                issued: now() - LINK_LIFETIME,
            })
            .unwrap()
        ));
        assert_eq!(auth.link_of(&expired), None);
    }

    #[test]
    fn state_cookie_is_secure_over_https() {
        assert_eq!(
            new_auth("secret").state_cookie("s"),
            "moblin_sign_in_state=s; Max-Age=600; HttpOnly; SameSite=Lax; Secure"
        );
        let local = Auth::new(
            Client::new(),
            "http://localhost:5173/api".to_string(),
            "http://localhost:5173".to_string(),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            local.state_cookie("s"),
            "moblin_sign_in_state=s; Max-Age=600; HttpOnly; SameSite=Lax"
        );
    }

    #[test]
    fn oauth_app_at_another_twitch() {
        let app = OAuthApp::at(
            "http://localhost:8081/",
            "http://localhost:8081",
            "id".to_string(),
            "secret".to_string(),
        );
        assert_eq!(app.authorize_url, "http://localhost:8081/oauth2/authorize");
        assert_eq!(app.token_url, "http://localhost:8081/oauth2/token");
        assert_eq!(app.users_url, "http://localhost:8081/helix/users");
        assert_eq!(
            app.subscriptions_url,
            "http://localhost:8081/helix/subscriptions/user"
        );
        assert_eq!(
            OAuthApp::twitch("id".to_string(), "secret".to_string()).authorize_url,
            "https://id.twitch.tv/oauth2/authorize"
        );
    }

    #[test]
    fn kick_app_at_another_kick() {
        let app = KickApp::at(
            "http://localhost:8082/",
            "http://localhost:8082",
            "id".to_string(),
            "secret".to_string(),
        );
        assert_eq!(app.authorize_url, "http://localhost:8082/oauth/authorize");
        assert_eq!(app.token_url, "http://localhost:8082/oauth/token");
        assert_eq!(app.users_url, "http://localhost:8082/public/v1/users");
        assert_eq!(app.channels_url, "http://localhost:8082/public/v1/channels");
        assert_eq!(
            app.channel_users_url,
            "http://localhost:8082/api/v2/channels"
        );
        let app = KickApp::kick("id".to_string(), "secret".to_string());
        assert_eq!(app.authorize_url, "https://id.kick.com/oauth/authorize");
        assert_eq!(app.token_url, "https://id.kick.com/oauth/token");
        assert_eq!(app.users_url, "https://api.kick.com/public/v1/users");
        assert_eq!(app.channels_url, "https://api.kick.com/public/v1/channels");
        assert_eq!(app.channel_users_url, "https://kick.com/api/v2/channels");
    }
}
