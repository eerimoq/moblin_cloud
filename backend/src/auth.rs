use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use reqwest::{Client, Url};
use ring::rand::{SecureRandom, SystemRandom};
use ring::{digest, hmac};
use serde::{Deserialize, Serialize};

use crate::accounts::ChannelModerator;

const STATE_LIFETIME: Duration = Duration::from_secs(600);
const MAX_PENDING: usize = 10_000;
const SESSION_LIFETIME: u64 = 30 * 24 * 60 * 60;
const VIEWER_LIFETIME: u64 = 24 * 60 * 60;
const VIEWER_PREFIX: &str = "viewer:";
pub const STATE_COOKIE: &str = "moblin_sign_in_state";

pub struct OAuthApp {
    pub client_id: String,
    pub client_secret: String,
    pub authorize_url: String,
    pub token_url: String,
    pub users_url: String,
    pub subscriptions_url: String,
    pub moderators_url: String,
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
            moderators_url: format!("{api_url}/helix/moderation/moderators"),
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
}

impl KickApp {
    pub fn new(client_id: String, client_secret: String) -> Self {
        Self::at(
            "https://id.kick.com",
            "https://api.kick.com",
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
            authorize_url: format!("{id_url}/oauth/authorize"),
            token_url: format!("{id_url}/oauth/token"),
            users_url: format!("{api_url}/public/v1/users"),
            channels_url: format!("{api_url}/public/v1/channels"),
        }
    }
}

struct Pending {
    created: Instant,
    page: Option<String>,
    verifier: Option<String>,
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

#[derive(Debug, PartialEq)]
pub struct User {
    pub id: String,
    pub name: String,
    pub login: String,
    pub avatar: Option<String>,
}

pub struct SignIn {
    pub user: User,
    token: Option<String>,
}

#[derive(Debug)]
pub struct StateError {
    pub page: Option<String>,
    pub error: anyhow::Error,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Viewer {
    pub id: String,
    pub name: String,
    pub page: String,
    pub tier: Option<u8>,
    pub issued: u64,
}

pub struct Auth {
    client: Client,
    public_url: String,
    pub frontend_url: String,
    twitch: Option<OAuthApp>,
    kick: Option<KickApp>,
    key: hmac::Key,
    random: SystemRandom,
    pending: Mutex<HashMap<String, Pending>>,
}

impl Auth {
    pub fn new(
        client: Client,
        public_url: String,
        frontend_url: String,
        twitch: Option<OAuthApp>,
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
            public_url: public_url.trim_end_matches('/').to_string(),
            frontend_url: frontend_url.trim_end_matches('/').to_string(),
            twitch,
            kick: None,
            key,
            random,
            pending: Mutex::new(HashMap::new()),
        })
    }

    pub fn with_kick(mut self, kick: Option<KickApp>) -> Self {
        self.kick = kick;
        self
    }

    pub fn kick_configured(&self) -> bool {
        self.kick.is_some()
    }

    fn redirect_uri(&self) -> String {
        format!("{}/auth/twitch/callback", self.public_url)
    }

    fn kick_redirect_uri(&self) -> String {
        format!("{}/auth/kick/callback", self.public_url)
    }

    pub fn login_url(&self, page: Option<&str>) -> Option<(String, String)> {
        let app = self.twitch.as_ref()?;
        let state = self.new_state(page, None)?;
        let scope = if page.is_some() {
            "user:read:subscriptions"
        } else {
            "openid moderation:read"
        };
        Url::parse_with_params(
            &app.authorize_url,
            [
                ("client_id", app.client_id.as_str()),
                ("redirect_uri", self.redirect_uri().as_str()),
                ("response_type", "code"),
                ("scope", scope),
                ("state", state.as_str()),
            ],
        )
        .ok()
        .map(|url| (url.into(), state))
    }

    pub fn kick_login_url(&self) -> Option<(String, String)> {
        let app = self.kick.as_ref()?;
        let verifier = self.random_string()?;
        let challenge =
            BASE64_URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, verifier.as_bytes()));
        let state = self.new_state(None, Some(verifier))?;
        Url::parse_with_params(
            &app.authorize_url,
            [
                ("client_id", app.client_id.as_str()),
                ("redirect_uri", self.kick_redirect_uri().as_str()),
                ("response_type", "code"),
                ("scope", "user:read channel:read"),
                ("state", state.as_str()),
                ("code_challenge", challenge.as_str()),
                ("code_challenge_method", "S256"),
            ],
        )
        .ok()
        .map(|url| (url.into(), state))
    }

    pub fn state_cookie(&self, state: &str) -> String {
        let secure = if self.public_url.starts_with("https://") {
            "; Secure"
        } else {
            ""
        };
        format!("{STATE_COOKIE}={state}; Max-Age=600; HttpOnly; SameSite=Lax{secure}")
    }

    pub fn cleared_state_cookie(&self) -> String {
        self.state_cookie("").replace("Max-Age=600", "Max-Age=0")
    }

    fn random_string(&self) -> Option<String> {
        let mut bytes = [0; 32];
        self.random.fill(&mut bytes).ok()?;
        Some(BASE64_URL_SAFE_NO_PAD.encode(bytes))
    }

    fn new_state(&self, page: Option<&str>, verifier: Option<String>) -> Option<String> {
        let state = self.random_string()?;
        let mut pending = self.pending.lock().unwrap();
        if pending.len() >= MAX_PENDING {
            pending.retain(|_, pending| pending.created.elapsed() < STATE_LIFETIME);
            if pending.len() >= MAX_PENDING {
                return None;
            }
        }
        pending.insert(
            state.clone(),
            Pending {
                created: Instant::now(),
                page: page.map(str::to_string),
                verifier,
            },
        );
        Some(state)
    }

    fn consume(&self, state: &str, browser_state: Option<&str>) -> Result<Pending, StateError> {
        let Some(pending) = self.pending.lock().unwrap().remove(state) else {
            return Err(StateError {
                page: None,
                error: anyhow!("unknown state"),
            });
        };
        if pending.created.elapsed() >= STATE_LIFETIME {
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

    pub fn consume_state(
        &self,
        state: &str,
        browser_state: Option<&str>,
    ) -> Result<Option<String>, StateError> {
        let pending = self.consume(state, browser_state)?;
        if pending.verifier.is_some() {
            return Err(StateError {
                page: None,
                error: anyhow!("the state is from a Kick sign in"),
            });
        }
        Ok(pending.page)
    }

    pub fn consume_kick_state(&self, state: &str, browser_state: Option<&str>) -> Result<String> {
        self.consume(state, browser_state)
            .map_err(|error| error.error)?
            .verifier
            .context("the state is from a Twitch sign in")
    }

    pub async fn kick_callback(&self, code: &str, verifier: &str) -> Result<User> {
        let app = self.kick.as_ref().context("Kick not configured")?;
        #[derive(Deserialize)]
        struct Token {
            access_token: String,
        }
        let token: Token = self
            .client
            .post(&app.token_url)
            .form(&[
                ("grant_type", "authorization_code"),
                ("client_id", app.client_id.as_str()),
                ("client_secret", app.client_secret.as_str()),
                ("redirect_uri", self.kick_redirect_uri().as_str()),
                ("code", code),
                ("code_verifier", verifier),
            ])
            .send()
            .await
            .context("failed to exchange the code")?
            .error_for_status()
            .context("the code was refused")?
            .json()
            .await
            .context("unexpected answer to the code exchange")?;
        #[derive(Deserialize)]
        struct Data<T> {
            data: Vec<T>,
        }
        #[derive(Deserialize)]
        struct KickUser {
            user_id: u64,
            name: String,
            profile_picture: Option<String>,
        }
        #[derive(Deserialize)]
        struct KickChannel {
            broadcaster_user_id: u64,
            slug: String,
        }
        let users: Data<KickUser> = self
            .client
            .get(&app.users_url)
            .bearer_auth(&token.access_token)
            .send()
            .await
            .context("failed to get the user")?
            .error_for_status()
            .context("the user was refused")?
            .json()
            .await
            .context("unexpected answer to the user request")?;
        let user = users.data.into_iter().next().context("no user")?;
        let channels: Data<KickChannel> = self
            .client
            .get(&app.channels_url)
            .bearer_auth(&token.access_token)
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
        Ok(User {
            id: format!("kick-{}", user.user_id),
            name: if user.name.is_empty() {
                channel.slug.clone()
            } else {
                user.name
            },
            login: channel.slug.to_lowercase(),
            avatar: user
                .profile_picture
                .filter(|url| url.starts_with("https://")),
        })
    }

    pub async fn callback(&self, code: &str) -> Result<SignIn> {
        let app = self.twitch.as_ref().context("Twitch not configured")?;
        #[derive(Deserialize)]
        struct Token {
            access_token: String,
        }
        let token: Token = self
            .client
            .post(&app.token_url)
            .form(&[
                ("client_id", app.client_id.as_str()),
                ("client_secret", app.client_secret.as_str()),
                ("code", code),
                ("grant_type", "authorization_code"),
                ("redirect_uri", self.redirect_uri().as_str()),
            ])
            .send()
            .await
            .context("failed to exchange the code")?
            .error_for_status()
            .context("the code was refused")?
            .json()
            .await
            .context("unexpected answer to the code exchange")?;
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
            .bearer_auth(&token.access_token)
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
            user: User {
                name: user
                    .display_name
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| user.login.clone()),
                login: user.login.to_lowercase(),
                avatar: user
                    .profile_image_url
                    .filter(|url| url.starts_with("https://")),
                id: user.id,
            },
            token: Some(token.access_token),
        })
    }

    pub async fn subscription_tier(
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
                ("user_id", &sign_in.user.id),
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

    pub async fn channel_moderators(&self, sign_in: &SignIn) -> Result<Vec<ChannelModerator>> {
        let app = self.twitch.as_ref().context("Twitch not configured")?;
        let token = sign_in.token.as_deref().context("no access token")?;
        #[derive(Deserialize)]
        struct Moderators {
            data: Vec<Moderator>,
            #[serde(default)]
            pagination: Pagination,
        }
        #[derive(Deserialize)]
        struct Moderator {
            user_id: String,
            user_login: String,
            user_name: String,
        }
        #[derive(Default, Deserialize)]
        struct Pagination {
            cursor: Option<String>,
        }
        let mut moderators = Vec::new();
        let mut after = None;
        for _ in 0..20 {
            let mut query = vec![
                ("broadcaster_id", sign_in.user.id.clone()),
                ("first", "100".to_string()),
            ];
            if let Some(cursor) = after {
                query.push(("after", cursor));
            }
            let page: Moderators = self
                .client
                .get(&app.moderators_url)
                .query(&query)
                .bearer_auth(token)
                .header("Client-Id", &app.client_id)
                .send()
                .await
                .context("failed to get the moderators")?
                .error_for_status()
                .context("the moderators were refused")?
                .json()
                .await
                .context("unexpected answer to the moderators request")?;
            moderators.extend(page.data.into_iter().map(|moderator| ChannelModerator {
                name: if moderator.user_name.is_empty() {
                    moderator.user_login.clone()
                } else {
                    moderator.user_name
                },
                login: moderator.user_login.to_lowercase(),
                id: moderator.user_id,
                enabled: false,
            }));
            match page.pagination.cursor.filter(|cursor| !cursor.is_empty()) {
                Some(cursor) => after = Some(cursor),
                None => break,
            }
        }
        Ok(moderators)
    }

    pub fn session(&self, account: &str, generation: u64) -> String {
        self.session_at(account, generation, now())
    }

    pub fn session_at(&self, account: &str, generation: u64, issued: u64) -> String {
        self.sign(&format!("{account}:{generation}:{issued}"))
    }

    pub fn session_of(&self, session: &str) -> Option<Session> {
        let payload = self.verified(session)?;
        if payload.starts_with(VIEWER_PREFIX) {
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
        id: String,
        name: String,
        page: String,
        tier: Option<u8>,
    ) -> String {
        let viewer = Viewer {
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
            Some(secret),
        )
        .unwrap()
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
        let (url, state) = auth.login_url(None).unwrap();
        let url = Url::parse(&url).unwrap();
        let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(url.host_str(), Some("id.twitch.tv"));
        assert_eq!(
            query["redirect_uri"],
            "https://api.example.com/auth/twitch/callback"
        );
        assert_eq!(query["scope"], "openid moderation:read");
        assert_eq!(query["state"], state);
        let state = Some(state.as_str());
        assert_eq!(auth.consume_state(&query["state"], state).unwrap(), None);
        assert!(auth.consume_state(&query["state"], state).is_err());
        auth.pending.lock().unwrap().insert(
            "old".to_string(),
            Pending {
                created: Instant::now() - STATE_LIFETIME * 2,
                page: None,
                verifier: None,
            },
        );
        assert!(auth.consume_state("old", Some("old")).is_err());
        let (url, state) = auth.login_url(Some("page")).unwrap();
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
        let (_, state) = auth.login_url(Some("page")).unwrap();
        assert_eq!(
            auth.consume_state(&state, Some(&state)).unwrap().as_deref(),
            Some("page")
        );
    }

    #[test]
    fn kick_sign_in_uses_pkce() {
        assert_eq!(new_auth("secret").kick_login_url(), None);
        let auth = new_auth("secret")
            .with_kick(Some(KickApp::new("kid".to_string(), "ksecret".to_string())));
        let (url, state) = auth.kick_login_url().unwrap();
        let url = Url::parse(&url).unwrap();
        let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(url.host_str(), Some("id.kick.com"));
        assert_eq!(url.path(), "/oauth/authorize");
        assert_eq!(query["client_id"], "kid");
        assert_eq!(
            query["redirect_uri"],
            "https://api.example.com/auth/kick/callback"
        );
        assert_eq!(query["scope"], "user:read channel:read");
        assert_eq!(query["code_challenge_method"], "S256");
        assert_eq!(query["state"], state);
        let verifier = auth
            .pending
            .lock()
            .unwrap()
            .get(&state)
            .unwrap()
            .verifier
            .clone()
            .unwrap();
        assert_eq!(
            query["code_challenge"],
            BASE64_URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, verifier.as_bytes()))
        );
        assert!(auth.consume_kick_state(&state, Some("other")).is_err());
        let (_, state) = auth.kick_login_url().unwrap();
        assert!(auth.consume_state(&state, Some(&state)).is_err());
        let (_, state) = auth.kick_login_url().unwrap();
        let other = auth.consume_kick_state(&state, Some(&state)).unwrap();
        assert_eq!(other.len(), 43);
        assert_ne!(other, verifier);
        assert!(auth.consume_kick_state(&state, Some(&state)).is_err());
    }

    #[test]
    fn twitch_states_are_not_kick_states() {
        let auth = new_auth("secret")
            .with_kick(Some(KickApp::new("kid".to_string(), "ksecret".to_string())));
        let (_, state) = auth.login_url(None).unwrap();
        assert!(auth.consume_kick_state(&state, Some(&state)).is_err());
    }

    #[test]
    fn viewer_sessions_are_not_streamer_sessions() {
        let auth = new_auth("secret");
        let session = auth.viewer_session(
            "7".to_string(),
            "Viewer".to_string(),
            "page".to_string(),
            Some(2),
        );
        let viewer = auth.viewer_of(&session).unwrap();
        assert_eq!(
            (
                viewer.id.as_str(),
                viewer.name.as_str(),
                viewer.page.as_str(),
                viewer.tier
            ),
            ("7", "Viewer", "page", Some(2))
        );
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
            app.moderators_url,
            "http://localhost:8081/helix/moderation/moderators"
        );
        assert_eq!(
            OAuthApp::twitch("id".to_string(), "secret".to_string()).authorize_url,
            "https://id.twitch.tv/oauth2/authorize"
        );
    }
}
