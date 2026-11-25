use reqwest::Url;

use crate::youtube;

#[derive(Debug, PartialEq)]
pub struct Clip {
    pub title: String,
    pub seconds: u32,
    pub link: String,
    pub thumbnail: Option<String>,
}

pub fn title(title: Option<String>) -> String {
    title
        .map(|title| title.trim().chars().take(100).collect::<String>())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "Clip".to_string())
}

fn is_slug(slug: &str) -> bool {
    (1..=100).contains(&slug.len())
        && slug
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn host_of(url: &Url) -> Option<String> {
    let host = url.host_str()?.to_lowercase();
    Some(
        ["www.", "m."]
            .iter()
            .find_map(|prefix| host.strip_prefix(prefix))
            .unwrap_or(&host)
            .to_string(),
    )
}

pub fn twitch_slug(link: &str) -> Option<String> {
    let url = youtube::parse(link)?;
    let mut path = url.path_segments()?;
    let slug = match (host_of(&url)?.as_str(), path.next()?) {
        ("clips.twitch.tv", "embed") => url
            .query_pairs()
            .find(|(name, _)| name == "clip")
            .map(|(_, slug)| slug.into_owned())?,
        ("clips.twitch.tv", slug) => slug.to_string(),
        ("twitch.tv", _) => match (path.next()?, path.next()?) {
            ("clip", slug) => slug.to_string(),
            _ => return None,
        },
        _ => return None,
    };
    is_slug(&slug).then_some(slug)
}

pub fn is_https(link: &str) -> bool {
    Url::parse(link).is_ok_and(|url| url.scheme() == "https")
}

#[cfg(test)]
#[path = "tests/clips.rs"]
mod tests;
