use std::pin::Pin;
use std::time::Duration;

use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use ring::digest::{SHA256, digest};
use ring::rand::{SecureRandom, SystemRandom};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Platform {
    Twitch,
    Kick,
}

impl Platform {
    pub fn name(self) -> &'static str {
        match self {
            Platform::Twitch => "twitch",
            Platform::Kick => "kick",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Platform::Twitch => "Twitch",
            Platform::Kick => "Kick",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Platform::Twitch => "t",
            Platform::Kick => "k",
        }
    }
}

pub struct Listening {
    pub listener: TcpListener,
    pub url: String,
    pub verifier: String,
}

pub async fn listen(backend_url: &str, platform: Platform) -> std::io::Result<Listening> {
    let mut bytes = [0; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| std::io::Error::other("no random numbers"))?;
    let verifier = BASE64_URL_SAFE_NO_PAD.encode(bytes);
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let url = format!(
        "{backend_url}/auth/{}/login?port={port}&challenge={}",
        platform.name(),
        challenge_of(&verifier)
    );
    Ok(Listening {
        listener,
        url,
        verifier,
    })
}

fn challenge_of(verifier: &str) -> String {
    BASE64_URL_SAFE_NO_PAD.encode(digest(&SHA256, verifier.as_bytes()))
}

pub type Exchange = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;

pub async fn wait(
    listener: TcpListener,
    exchange: impl Fn(String) -> Exchange,
) -> Result<String, String> {
    tokio::time::timeout(Duration::from_secs(600), accept(listener, exchange))
        .await
        .unwrap_or_else(|_| Err("Signing in took too long. Try again.".to_string()))
}

async fn accept(
    listener: TcpListener,
    exchange: impl Fn(String) -> Exchange,
) -> Result<String, String> {
    loop {
        let Ok((mut socket, _)) = listener.accept().await else {
            continue;
        };
        let mut request = Vec::new();
        let mut buffer = [0; 1024];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") && request.len() < 8192 {
            match socket.read(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(read) => request.extend_from_slice(&buffer[..read]),
            }
        }
        let result = match result_of(&String::from_utf8_lossy(&request)) {
            Some(Ok(code)) => Some(exchange(code).await),
            result => result,
        };
        let (status, page) = match &result {
            Some(Ok(_)) => ("200 OK", "Signed in to Moblin Cloud. Go back to mobtop."),
            Some(Err(_)) => (
                "200 OK",
                "Signing in to Moblin Cloud failed. Go back to mobtop.",
            ),
            None => ("404 Not Found", "Not found."),
        };
        let body = format!(
            "<!doctype html><meta charset=\"utf-8\"><title>mobtop</title><p style=\"font-family: sans-serif\">{page}</p>"
        );
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
        if let Some(result) = result {
            return result;
        }
    }
}

fn result_of(request: &str) -> Option<Result<String, String>> {
    let target = request
        .lines()
        .next()?
        .strip_prefix("GET ")?
        .split(' ')
        .next()?;
    let url = Url::parse(&format!("http://127.0.0.1{target}")).ok()?;
    if url.path() != "/" {
        return None;
    }
    for (name, value) in url.query_pairs() {
        match name.as_ref() {
            "code" if !value.is_empty() => return Some(Ok(value.into_owned())),
            "signInFailed" => return Some(Err("Signing in failed. Try again.".to_string())),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
#[path = "tests/sign_in.rs"]
mod tests;
