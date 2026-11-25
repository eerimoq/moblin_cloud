use log::warn;
use reqwest::{Client, Url};
use serde::Deserialize;

pub const URL: &str = "https://www.youtube.com";

pub struct YouTube {
    client: Client,
    url: String,
}

#[derive(Debug, PartialEq)]
pub enum LookupError {
    Unavailable,
    Unknown,
}

#[derive(Deserialize)]
struct Embed {
    title: String,
}

impl YouTube {
    pub fn new(client: Client, url: String) -> Self {
        Self { client, url }
    }

    pub async fn title(&self, video: &str) -> Result<String, LookupError> {
        let response = self
            .client
            .get(format!("{}/oembed", self.url))
            .query(&[
                ("url", format!("{URL}/watch?v={video}").as_str()),
                ("format", "json"),
            ])
            .send()
            .await
            .map_err(|error| {
                warn!("looking up YouTube video {video} failed: {error:#}");
                LookupError::Unknown
            })?;
        let status = response.status();
        if status.is_client_error() {
            return Err(LookupError::Unavailable);
        }
        if !status.is_success() {
            warn!("looking up YouTube video {video} failed: {status}");
            return Err(LookupError::Unknown);
        }
        let embed: Embed = response.json().await.map_err(|error| {
            warn!("looking up YouTube video {video} failed: {error:#}");
            LookupError::Unknown
        })?;
        Ok(embed.title.trim().chars().take(100).collect())
    }
}

fn is_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn parse(link: &str) -> Option<Url> {
    let link = link.trim();
    if link.starts_with("http://") || link.starts_with("https://") {
        Url::parse(link).ok()
    } else {
        Url::parse(&format!("https://{link}")).ok()
    }
}

pub fn video_id(link: &str) -> Option<String> {
    let link = link.trim();
    if is_video_id(link) {
        return Some(link.to_string());
    }
    let url = parse(link)?;
    let host = url.host_str()?.to_lowercase();
    let host = ["www.", "m.", "music."]
        .iter()
        .find_map(|prefix| host.strip_prefix(prefix))
        .unwrap_or(&host);
    let mut path = url.path_segments()?;
    let id = match (host, path.next()?) {
        ("youtu.be", id) => id.to_string(),
        ("youtube.com", "watch") => url
            .query_pairs()
            .find(|(name, _)| name == "v")
            .map(|(_, id)| id.into_owned())?,
        ("youtube.com", "shorts" | "live" | "embed") => path.next()?.to_string(),
        _ => return None,
    };
    is_video_id(&id).then_some(id)
}

fn seconds(time: &str) -> Option<u32> {
    if let Ok(seconds) = time.parse() {
        return Some(seconds);
    }
    let mut total: u32 = 0;
    let mut number = String::new();
    for c in time.chars() {
        if c.is_ascii_digit() {
            number.push(c);
            continue;
        }
        let unit = match c {
            'h' => 3600,
            'm' => 60,
            's' => 1,
            _ => return None,
        };
        total = total.checked_add(number.parse::<u32>().ok()?.checked_mul(unit)?)?;
        number.clear();
    }
    number.is_empty().then_some(total)
}

pub fn start(link: &str) -> Option<u32> {
    parse(link)?
        .query_pairs()
        .find(|(name, _)| name == "t" || name == "start")
        .and_then(|(_, time)| seconds(&time))
}

#[cfg(test)]
#[path = "tests/youtube.rs"]
mod tests;
