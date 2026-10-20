use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use reqwest::{Client, Url};
use ring::hmac;
use ring::rand::{SecureRandom, SystemRandom};
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
        Self {
            client_id,
            client_secret,
            authorize_url: "https://id.twitch.tv/oauth2/authorize".to_string(),
            token_url: "https://id.twitch.tv/oauth2/token".to_string(),
            users_url: "https://api.twitch.tv/helix/users".to_string(),
            subscriptions_url: "https://api.twitch.tv/helix/subscriptions/user".to_string(),
            moderators_url: "https://api.twitch.tv/helix/moderation/moderators".to_string(),
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
    fake_twitch_login: Option<String>,
    key: hmac::Key,
    random: SystemRandom,
    pending: Mutex<HashMap<String, (Instant, Option<String>)>>,
}

impl Auth {
    pub fn new(
        client: Client,
        public_url: String,
        frontend_url: String,
        twitch: Option<OAuthApp>,
        fake_twitch_login: Option<String>,
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
            fake_twitch_login,
            key,
            random,
            pending: Mutex::new(HashMap::new()),
        })
    }

    fn redirect_uri(&self) -> String {
        format!("{}/auth/twitch/callback", self.public_url)
    }

    pub fn login_url(
        &self,
        fake_login: Option<&str>,
        page: Option<&str>,
    ) -> Option<(String, String)> {
        if let Some(default_login) = &self.fake_twitch_login {
            let login = fake_login.unwrap_or(default_login).to_lowercase();
            let valid = (1..=25).contains(&login.len())
                && login.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            if !valid {
                return None;
            }
            let state = self.new_state(page)?;
            return Url::parse_with_params(
                &self.redirect_uri(),
                [("code", login.as_str()), ("state", state.as_str())],
            )
            .ok()
            .map(|url| (url.into(), state));
        }
        let app = self.twitch.as_ref()?;
        let state = self.new_state(page)?;
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

    fn new_state(&self, page: Option<&str>) -> Option<String> {
        let mut bytes = [0; 32];
        self.random.fill(&mut bytes).ok()?;
        let state = BASE64_URL_SAFE_NO_PAD.encode(bytes);
        {
            let mut pending = self.pending.lock().unwrap();
            if pending.len() >= MAX_PENDING {
                pending.retain(|_, (created, _)| created.elapsed() < STATE_LIFETIME);
                if pending.len() >= MAX_PENDING {
                    return None;
                }
            }
            pending.insert(state.clone(), (Instant::now(), page.map(str::to_string)));
        }
        Some(state)
    }

    pub fn consume_state(
        &self,
        state: &str,
        browser_state: Option<&str>,
    ) -> Result<Option<String>, StateError> {
        let Some((created, page)) = self.pending.lock().unwrap().remove(state) else {
            return Err(StateError {
                page: None,
                error: anyhow!("unknown state"),
            });
        };
        if created.elapsed() >= STATE_LIFETIME {
            return Err(StateError {
                page,
                error: anyhow!("expired state"),
            });
        }
        if browser_state != Some(state) {
            return Err(StateError {
                page,
                error: anyhow!("the state is from another browser"),
            });
        }
        Ok(page)
    }

    pub async fn callback(&self, code: &str) -> Result<SignIn> {
        if self.fake_twitch_login.is_some() {
            return Ok(SignIn {
                user: User {
                    id: format!("fake-{code}"),
                    name: code.to_string(),
                    login: code.to_string(),
                    avatar: None,
                },
                token: None,
            });
        }
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
        if self.fake_twitch_login.is_some() {
            return Ok(sign_in
                .user
                .login
                .chars()
                .last()
                .and_then(|last| last.to_digit(10))
                .filter(|tier| (1..=3).contains(tier))
                .map(|tier| tier as u8));
        }
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

    pub fn fake_moderated_channels(&self, sign_in: &SignIn) -> Option<Vec<String>> {
        self.fake_twitch_login.as_ref()?;
        Some(
            sign_in
                .user
                .login
                .strip_prefix("mod_")
                .map(|broadcasters| {
                    broadcasters
                        .split('_')
                        .map(|broadcaster| format!("fake-{broadcaster}"))
                        .collect()
                })
                .unwrap_or_default(),
        )
    }

    pub async fn channel_moderators(&self, sign_in: &SignIn) -> Result<Vec<ChannelModerator>> {
        if self.fake_twitch_login.is_some() {
            return Ok(Vec::new());
        }
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
            None,
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
        let (url, state) = auth.login_url(Some("other"), None).unwrap();
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
            (Instant::now() - STATE_LIFETIME * 2, None),
        );
        assert!(auth.consume_state("old", Some("old")).is_err());
        let (url, state) = auth.login_url(None, Some("page")).unwrap();
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
        let (_, state) = auth.login_url(None, Some("page")).unwrap();
        assert_eq!(
            auth.consume_state(&state, Some(&state)).unwrap().as_deref(),
            Some("page")
        );
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

    #[tokio::test]
    async fn fake_sign_in_skips_twitch() {
        let auth = Auth::new(
            Client::new(),
            "http://localhost:5173/api".to_string(),
            "http://localhost:5173".to_string(),
            None,
            Some("streamer".to_string()),
            None,
        )
        .unwrap();
        let url = Url::parse(&auth.login_url(Some("Erik"), None).unwrap().0).unwrap();
        assert_eq!(url.path(), "/api/auth/twitch/callback");
        let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
        let state = query["state"].as_str();
        let (_, other) = auth.login_url(None, None).unwrap();
        assert!(auth.consume_state(&other, Some(state)).is_err());
        assert_eq!(auth.consume_state(state, Some(state)).unwrap(), None);
        let sign_in = auth.callback(&query["code"]).await.unwrap();
        assert_eq!(
            sign_in.user,
            User {
                id: "fake-erik".to_string(),
                name: "erik".to_string(),
                login: "erik".to_string(),
                avatar: None,
            }
        );
        assert_eq!(auth.subscription_tier(&sign_in, "1").await.unwrap(), None);
        for (login, tier) in [("sub1", Some(1)), ("sub3", Some(3)), ("sub4", None)] {
            let sign_in = auth.callback(login).await.unwrap();
            assert_eq!(auth.subscription_tier(&sign_in, "1").await.unwrap(), tier);
        }
        for (login, broadcasters) in [
            ("mod_erik", vec!["fake-erik"]),
            ("mod_erik_bob", vec!["fake-erik", "fake-bob"]),
            ("erik", vec![]),
        ] {
            let sign_in = auth.callback(login).await.unwrap();
            assert_eq!(
                auth.fake_moderated_channels(&sign_in).unwrap(),
                broadcasters
            );
        }
        assert_eq!(auth.login_url(Some("bad login"), None), None);
        assert!(
            auth.login_url(None, None)
                .unwrap()
                .0
                .contains("code=streamer")
        );
        assert_eq!(
            auth.state_cookie("s"),
            "moblin_sign_in_state=s; Max-Age=600; HttpOnly; SameSite=Lax"
        );
    }
}
