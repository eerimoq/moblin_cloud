use std::net::IpAddr;
use std::time::Duration;

use log::{info, warn};
use reqwest::{Client, Url};
use serde_json::Value;

pub struct Webhooks {
    client: Client,
    allow_local: bool,
}

impl Webhooks {
    pub fn new(client: Client, allow_local: bool) -> Self {
        Self {
            client,
            allow_local,
        }
    }

    pub fn allows(&self, url: &str) -> bool {
        let Ok(parsed) = Url::parse(url) else {
            return false;
        };
        let Some(host) = parsed.host_str() else {
            return false;
        };
        if url.len() > 500 || !["https", "http"].contains(&parsed.scheme()) {
            return false;
        }
        if self.allow_local {
            return true;
        }
        let host = host.trim_start_matches('[').trim_end_matches(']');
        parsed.scheme() == "https"
            && host.contains('.')
            && host.parse::<IpAddr>().is_err()
            && !host.ends_with(".localhost")
            && !host.ends_with(".local")
            && !host.ends_with(".internal")
    }

    pub async fn deliver(&self, url: &str, body: &Value) {
        for attempt in 1..=6 {
            if attempt > 1 {
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
            match self.client.post(url).json(body).send().await {
                Ok(response) if response.status().is_success() => {
                    info!("posted {} to webhook {url}", body["id"]);
                    return;
                }
                Ok(response) => {
                    info!(
                        "webhook {url} answered {} to {} (attempt {attempt})",
                        response.status(),
                        body["id"]
                    );
                }
                Err(error) => {
                    info!(
                        "posting {} to webhook {url} failed (attempt {attempt}): {error:#}",
                        body["id"]
                    );
                }
            }
        }
        warn!("giving up posting {} to webhook {url}", body["id"]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_public_https_urls_are_allowed() {
        let webhooks = Webhooks::new(Client::new(), false);
        assert!(webhooks.allows("https://example.com/tips?token=x"));
        for url in [
            "http://example.com/tips",
            "https://localhost/tips",
            "https://127.0.0.1/tips",
            "https://[::1]/tips",
            "https://10.0.0.1/tips",
            "https://printer.local/tips",
            "https://metadata.google.internal/tips",
            "ftp://example.com/tips",
            "example.com/tips",
            &format!("https://example.com/{}", "a".repeat(500)),
        ] {
            assert!(!webhooks.allows(url), "{url}");
        }
        let local = Webhooks::new(Client::new(), true);
        assert!(local.allows("http://127.0.0.1:8080/tips"));
        assert!(!local.allows("ftp://127.0.0.1/tips"));
    }
}
