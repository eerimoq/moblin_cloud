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

use crate::accounts::{Identity, Moderator, Platform};

const STATE_LIFETIME: Duration = Duration::from_secs(600);
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
}

#[derive(Debug)]
pub struct StateError {
    pub page: Option<String>,
    pub error: anyhow::Error,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Viewer {
    #[serde(default = "twitch")]
    pub platform: Platform,
    pub id: String,
    pub name: String,
    pub page: String,
    pub tier: Option<u8>,
    pub issued: u64,
}

fn twitch() -> Platform {
    Platform::Twitch
}

impl Viewer {
    pub fn key(&self) -> String {
        match self.platform {
            Platform::Twitch => self.id.clone(),
            Platform::Kick => format!("kick:{}", self.id),
        }
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

    pub fn twitch_login_url(&self, page: Option<&str>, link: bool) -> Option<(String, String)> {
        let app = self.twitch.as_ref()?;
        let state = self.new_state(SignInState {
            page: page.map(str::to_string),
            link,
            verifier: None,
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

    pub fn kick_login_url(&self, page: Option<&str>, link: bool) -> Option<(String, String)> {
        let app = self.kick.as_ref()?;
        let verifier = self.random_token()?;
        let challenge = BASE64_URL_SAFE_NO_PAD.encode(digest(&SHA256, verifier.as_bytes()));
        let state = self.new_state(SignInState {
            page: page.map(str::to_string),
            link,
            verifier: Some(verifier),
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

    pub async fn kick_user(&self, username: &str) -> Result<Option<Moderator>> {
        let app = self.kick.as_ref().context("Kick not configured")?;
        let token = self
            .access_token(
                &app.token_url,
                &[
                    ("client_id", app.client_id.as_str()),
                    ("client_secret", app.client_secret.as_str()),
                    ("grant_type", "client_credentials"),
                ],
            )
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
            .map(|channel| Moderator {
                id: channel.broadcaster_user_id,
                name: channel.slug.clone(),
                login: channel.slug.to_lowercase(),
                avatar: None,
                preview: false,
                go_live: false,
                edit_tips: false,
            }))
    }

    pub async fn twitch_user(&self, username: &str) -> Result<Option<Moderator>> {
        let app = self.twitch.as_ref().context("Twitch not configured")?;
        let token = self
            .access_token(
                &app.token_url,
                &[
                    ("client_id", app.client_id.as_str()),
                    ("client_secret", app.client_secret.as_str()),
                    ("grant_type", "client_credentials"),
                ],
            )
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
            .map(|user| Moderator {
                id: user.id,
                name: user
                    .display_name
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| user.login.clone()),
                login: user.login.to_lowercase(),
                avatar: None,
                preview: false,
                go_live: false,
                edit_tips: false,
            }))
    }

    pub async fn user(&self, platform: Platform, username: &str) -> Result<Option<Moderator>> {
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
    use serde_json::json;

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
    fn states_are_single_use() {
        let auth = new_auth("secret");
        let (url, state) = auth.twitch_login_url(None, false).unwrap();
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
        let (url, state) = auth.twitch_login_url(Some("page"), false).unwrap();
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
        let (_, state) = auth.twitch_login_url(Some("page"), false).unwrap();
        assert_eq!(
            auth.consume_state(&state, Some(&state))
                .unwrap()
                .page
                .as_deref(),
            Some("page")
        );
        let (_, state) = auth.twitch_login_url(None, true).unwrap();
        assert!(auth.consume_state(&state, Some(&state)).unwrap().link);
    }

    #[test]
    fn kick_signs_in_with_pkce() {
        let auth = new_auth("secret");
        let (url, state) = auth.kick_login_url(None, true).unwrap();
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
        let (other, _) = auth.kick_login_url(None, false).unwrap();
        assert_ne!(query_of(&other)["code_challenge"], query["code_challenge"]);
        let (_, state) = auth.kick_login_url(Some("page"), false).unwrap();
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
        assert!(without.kick_login_url(None, false).is_none());
        assert!(without.twitch_login_url(None, false).is_none());
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
        let old = auth.sign(&format!(
            "{VIEWER_PREFIX}{}",
            json!({"id": "7", "name": "Viewer", "page": "page", "tier": null, "issued": now()})
        ));
        assert_eq!(auth.viewer_of(&old).unwrap().platform, Platform::Twitch);
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
