use std::fmt;

use reqwest::{Client, RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::account::{Account, Connect, Kind, Recent};

#[derive(serde::Deserialize)]
struct Session {
    session: String,
}

#[derive(Clone)]
pub struct Backend {
    client: Client,
    pub url: String,
    pub session: Option<String>,
}

#[derive(Clone, Debug)]
pub struct BackendError {
    pub status: Option<StatusCode>,
    pub text: String,
}

impl BackendError {
    pub fn signed_out(&self) -> bool {
        self.status == Some(StatusCode::UNAUTHORIZED)
    }
}

impl fmt::Display for BackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match (self.status, self.text.is_empty()) {
            (Some(status), true) => write!(formatter, "The backend answered {status}."),
            (None, _) => write!(formatter, "Couldn't reach Moblin Cloud: {}", self.text),
            _ => formatter.write_str(&self.text),
        }
    }
}

impl From<reqwest::Error> for BackendError {
    fn from(error: reqwest::Error) -> Self {
        Self {
            status: error.status(),
            text: error.to_string(),
        }
    }
}

impl Backend {
    pub fn new(url: &str) -> Self {
        Self {
            client: Client::new(),
            url: url.trim_end_matches('/').to_string(),
            session: None,
        }
    }

    pub fn request(&self, request: RequestBuilder) -> RequestBuilder {
        match &self.session {
            Some(session) => request.bearer_auth(session),
            None => request,
        }
    }

    pub async fn send(&self, request: RequestBuilder) -> Result<Response, BackendError> {
        let response = self.request(request).send().await?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        Err(BackendError {
            status: Some(status),
            text: response.text().await.unwrap_or_default(),
        })
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, BackendError> {
        let request = self.client.get(format!("{}{path}", self.url));
        Ok(self.send(request).await?.json().await?)
    }

    async fn post(&self, path: &str, body: Value) -> Result<(), BackendError> {
        let request = self.client.post(format!("{}{path}", self.url)).json(&body);
        self.send(request).await?;
        Ok(())
    }

    pub fn get_url(&self, path: &str) -> RequestBuilder {
        self.client.get(format!("{}{path}", self.url))
    }

    pub async fn session(&self, code: &str, verifier: &str) -> Result<String, BackendError> {
        let request = self
            .client
            .post(format!("{}/auth/session", self.url))
            .json(&serde_json::json!({ "code": code, "verifier": verifier }));
        let session: Session = self.send(request).await?.json().await?;
        Ok(session.session)
    }

    pub async fn connect(&self) -> Result<Connect, BackendError> {
        let request = self
            .client
            .post(format!("{}/auth/connect", self.url))
            .json(&serde_json::json!({}));
        Ok(self.send(request).await?.json().await?)
    }

    pub async fn connect_session(&self, device_code: &str) -> Result<Option<String>, BackendError> {
        let request = self
            .client
            .post(format!("{}/auth/connect/session", self.url))
            .json(&serde_json::json!({ "deviceCode": device_code }));
        let response = self.send(request).await?;
        if response.status() == StatusCode::ACCEPTED {
            return Ok(None);
        }
        Ok(Some(response.json::<Session>().await?.session))
    }

    pub async fn account(&self, base: &str) -> Result<Account, BackendError> {
        self.get(base).await
    }

    pub async fn recent(
        &self,
        base: &str,
        kind: Kind,
        limit: usize,
        before: Option<&str>,
    ) -> Result<Recent, BackendError> {
        let mut path = format!("{base}/recent?kind={}&limit={limit}", kind.name());
        if let Some(before) = before {
            path.push_str(&format!("&before={before}"));
        }
        self.get(&path).await
    }

    pub async fn resend(&self, base: &str, order: &str) -> Result<(), BackendError> {
        self.post(
            &format!("{base}/recent/{order}/resend"),
            serde_json::json!({}),
        )
        .await
    }

    pub async fn fart(&self, base: &str) -> Result<(), BackendError> {
        self.post(
            &format!("{base}/test/effect"),
            serde_json::json!({ "effect": "fart" }),
        )
        .await
    }

    pub async fn control(&self, page: &str, device: &str, body: Value) -> Result<(), BackendError> {
        self.post(
            &format!(
                "/account/pages/{page}/devices/{}/control",
                url::form_urlencoded::byte_serialize(device.as_bytes()).collect::<String>()
            ),
            body,
        )
        .await
    }

    pub async fn layout(&self, page: &str) -> Result<Value, BackendError> {
        let answer: Value = self.get(&format!("/account/pages/{page}/monitor")).await?;
        Ok(answer["layout"].clone())
    }

    pub async fn save_layout(&self, page: &str, layout: Value) -> Result<(), BackendError> {
        self.post(
            &format!("/account/pages/{page}/monitor"),
            serde_json::json!({ "layout": layout }),
        )
        .await
    }
}
