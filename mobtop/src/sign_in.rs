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
mod tests {
    use super::*;

    #[test]
    fn the_code_is_taken_from_the_redirect() {
        assert_eq!(
            result_of("GET /?code=abc-def HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n"),
            Some(Ok("abc-def".to_string()))
        );
        assert_eq!(
            result_of("GET /?signInFailed HTTP/1.1\r\n\r\n"),
            Some(Err("Signing in failed. Try again.".to_string()))
        );
        assert_eq!(result_of("GET /favicon.ico HTTP/1.1\r\n\r\n"), None);
        assert_eq!(result_of("GET / HTTP/1.1\r\n\r\n"), None);
        assert_eq!(result_of("GET /?session=a HTTP/1.1\r\n\r\n"), None);
        assert_eq!(result_of("POST /?code=a HTTP/1.1\r\n\r\n"), None);
        assert_eq!(result_of(""), None);
    }

    #[tokio::test]
    async fn the_browser_is_answered_and_the_code_returned() {
        let listening = listen("http://backend", Platform::Kick).await.unwrap();
        let url = Url::parse(&listening.url).unwrap();
        assert_eq!(url.path(), "/auth/kick/login");
        let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(query["challenge"], challenge_of(&listening.verifier));
        assert_eq!(query["challenge"].len(), 43);
        assert_ne!(
            listen("http://backend", Platform::Kick)
                .await
                .unwrap()
                .verifier,
            listening.verifier
        );
        let port = &query["port"];
        let waiting = tokio::spawn(wait(listening.listener, |code| {
            Box::pin(async move { Ok(format!("session of {code}")) })
        }));
        let mut socket = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .unwrap();
        socket
            .write_all(b"GET /?code=c HTTP/1.1\r\n\r\n")
            .await
            .unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).await.unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
        assert!(response.contains("Go back to mobtop."));
        assert_eq!(waiting.await.unwrap(), Ok("session of c".to_string()));
    }
}
