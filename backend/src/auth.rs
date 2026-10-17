use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use reqwest::{Client, Url};
use ring::hmac;
use ring::rand::{SecureRandom, SystemRandom};
use serde::Deserialize;

const STATE_LIFETIME: Duration = Duration::from_secs(600);
const MAX_PENDING: usize = 10_000;
const SESSION_LIFETIME: u64 = 30 * 24 * 60 * 60;
pub const STATE_COOKIE: &str = "moblin_sign_in_state";

pub struct OAuthApp {
    pub client_id: String,
    pub client_secret: String,
    pub authorize_url: String,
    pub token_url: String,
    pub users_url: String,
}

impl OAuthApp {
    pub fn twitch(client_id: String, client_secret: String) -> Self {
        Self {
            client_id,
            client_secret,
            authorize_url: "https://id.twitch.tv/oauth2/authorize".to_string(),
            token_url: "https://id.twitch.tv/oauth2/token".to_string(),
            users_url: "https://api.twitch.tv/helix/users".to_string(),
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

pub struct Auth {
    client: Client,
    public_url: String,
    pub frontend_url: String,
    twitch: Option<OAuthApp>,
    fake: bool,
    key: hmac::Key,
    random: SystemRandom,
    pending: Mutex<HashMap<String, Instant>>,
}

impl Auth {
    pub fn new(
        client: Client,
        public_url: String,
        frontend_url: String,
        twitch: Option<OAuthApp>,
        fake: bool,
        secret: Option<&str>,
    ) -> Result<Self> {
        let random = SystemRandom::new();
        let key = match secret {
            Some(secret) => hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes()),
            None => hmac::Key::generate(hmac::HMAC_SHA256, &random)
                .map_err(|_| anyhow::anyhow!("failed to generate a session secret"))?,
        };
        Ok(Self {
            client,
            public_url: public_url.trim_end_matches('/').to_string(),
            frontend_url: frontend_url.trim_end_matches('/').to_string(),
            twitch,
            fake,
            key,
            random,
            pending: Mutex::new(HashMap::new()),
        })
    }

    fn redirect_uri(&self) -> String {
        format!("{}/auth/twitch/callback", self.public_url)
    }

    pub fn login_url(&self, fake_login: Option<&str>) -> Option<(String, String)> {
        if self.fake {
            let login = fake_login.unwrap_or("streamer").to_lowercase();
            let valid = (1..=25).contains(&login.len())
                && login.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            if !valid {
                return None;
            }
            let state = self.new_state()?;
            return Url::parse_with_params(
                &self.redirect_uri(),
                [("code", login.as_str()), ("state", state.as_str())],
            )
            .ok()
            .map(|url| (url.into(), state));
        }
        let app = self.twitch.as_ref()?;
        let state = self.new_state()?;
        Url::parse_with_params(
            &app.authorize_url,
            [
                ("client_id", app.client_id.as_str()),
                ("redirect_uri", self.redirect_uri().as_str()),
                ("response_type", "code"),
                ("scope", "openid"),
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

    fn new_state(&self) -> Option<String> {
        let mut bytes = [0; 32];
        self.random.fill(&mut bytes).ok()?;
        let state = BASE64_URL_SAFE_NO_PAD.encode(bytes);
        {
            let mut pending = self.pending.lock().unwrap();
            if pending.len() >= MAX_PENDING {
                pending.retain(|_, created| created.elapsed() < STATE_LIFETIME);
                if pending.len() >= MAX_PENDING {
                    return None;
                }
            }
            pending.insert(state.clone(), Instant::now());
        }
        Some(state)
    }

    fn consume_state(&self, state: &str) -> bool {
        let created = self.pending.lock().unwrap().remove(state);
        created.is_some_and(|created| created.elapsed() < STATE_LIFETIME)
    }

    pub async fn callback(
        &self,
        code: &str,
        state: &str,
        browser_state: Option<&str>,
    ) -> Result<User> {
        if !self.consume_state(state) {
            bail!("unknown or expired state");
        }
        if browser_state != Some(state) {
            bail!("the state is from another browser");
        }
        if self.fake {
            return Ok(User {
                id: format!("fake-{code}"),
                name: code.to_string(),
                login: code.to_string(),
                avatar: None,
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
        Ok(User {
            name: user
                .display_name
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| user.login.clone()),
            login: user.login.to_lowercase(),
            avatar: user
                .profile_image_url
                .filter(|url| url.starts_with("https://")),
            id: user.id,
        })
    }

    pub fn session(&self, account: &str, generation: u64) -> String {
        self.session_at(account, generation, now())
    }

    pub fn session_at(&self, account: &str, generation: u64, issued: u64) -> String {
        let payload = format!("{account}:{generation}:{issued}");
        let tag = hmac::sign(&self.key, payload.as_bytes());
        format!(
            "{}.{}",
            BASE64_URL_SAFE_NO_PAD.encode(&payload),
            BASE64_URL_SAFE_NO_PAD.encode(tag.as_ref())
        )
    }

    pub fn session_of(&self, session: &str) -> Option<Session> {
        let (payload, tag) = session.split_once('.')?;
        let payload = BASE64_URL_SAFE_NO_PAD.decode(payload).ok()?;
        let tag = BASE64_URL_SAFE_NO_PAD.decode(tag).ok()?;
        hmac::verify(&self.key, &payload, &tag).ok()?;
        let (rest, issued) = std::str::from_utf8(&payload).ok()?.rsplit_once(':')?;
        let (account, generation) = rest.rsplit_once(':')?;
        let issued = issued.parse::<u64>().ok()?;
        let generation = generation.parse().ok()?;
        (issued + SESSION_LIFETIME > now()).then(|| Session {
            account: account.to_string(),
            generation,
            issued,
        })
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
            false,
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
        let (url, state) = auth.login_url(Some("other")).unwrap();
        let url = Url::parse(&url).unwrap();
        let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(url.host_str(), Some("id.twitch.tv"));
        assert_eq!(
            query["redirect_uri"],
            "https://api.example.com/auth/twitch/callback"
        );
        assert_eq!(query["state"], state);
        assert!(auth.consume_state(&query["state"]));
        assert!(!auth.consume_state(&query["state"]));
        auth.pending
            .lock()
            .unwrap()
            .insert("old".to_string(), Instant::now() - STATE_LIFETIME * 2);
        assert!(!auth.consume_state("old"));
    }

    #[tokio::test]
    async fn fake_sign_in_skips_twitch() {
        let auth = Auth::new(
            Client::new(),
            "http://localhost:5173/api".to_string(),
            "http://localhost:5173".to_string(),
            None,
            true,
            None,
        )
        .unwrap();
        let url = Url::parse(&auth.login_url(Some("Erik")).unwrap().0).unwrap();
        assert_eq!(url.path(), "/api/auth/twitch/callback");
        let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
        let state = query["state"].as_str();
        let (_, other) = auth.login_url(None).unwrap();
        assert!(
            auth.callback(&query["code"], &other, Some(state))
                .await
                .is_err()
        );
        assert_eq!(
            auth.callback(&query["code"], state, Some(state))
                .await
                .unwrap(),
            User {
                id: "fake-erik".to_string(),
                name: "erik".to_string(),
                login: "erik".to_string(),
                avatar: None,
            }
        );
        assert!(auth.callback("erik", state, Some(state)).await.is_err());
        assert_eq!(auth.login_url(Some("bad login")), None);
        assert!(auth.login_url(None).unwrap().0.contains("code=streamer"));
        assert_eq!(
            auth.state_cookie("s"),
            "moblin_sign_in_state=s; Max-Age=600; HttpOnly; SameSite=Lax"
        );
    }
}
