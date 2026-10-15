use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::Result;
use log::warn;
use serde::{Deserialize, Serialize};

use crate::json_file;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paywall: Option<Paywall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Paywall {
    pub email: String,
    pub price: u32,
    pub currency: String,
    #[serde(
        default,
        rename = "merchantId",
        skip_serializing_if = "Option::is_none"
    )]
    pub merchant_id: Option<String>,
}

pub const CURRENCIES: [&str; 16] = [
    "AUD", "BRL", "CAD", "CHF", "CZK", "DKK", "EUR", "GBP", "HKD", "MXN", "NOK", "NZD", "PLN",
    "SEK", "SGD", "USD",
];

pub const ACCENTS: [&str; 6] = ["green", "purple", "blue", "pink", "orange", "red"];

fn text(text: Option<String>, max: usize) -> Result<Option<String>, SettingsError> {
    let text = text
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());
    match text {
        Some(text) if text.chars().count() > max => Err(SettingsError::InvalidPage),
        text => Ok(text),
    }
}

impl Settings {
    pub fn normalize(self) -> Result<Self, SettingsError> {
        let paywall = match self.paywall {
            Some(paywall) => Some(paywall.normalize().ok_or(SettingsError::InvalidPaywall)?),
            None => None,
        };
        let accent = match self.accent {
            Some(accent) if !ACCENTS.contains(&accent.as_str()) => {
                return Err(SettingsError::InvalidPage);
            }
            accent => accent.filter(|accent| accent != ACCENTS[0]),
        };
        Ok(Self {
            paywall,
            title: text(self.title, 60)?,
            message: text(self.message, 300)?,
            accent,
        })
    }
}

pub fn format_price(price: u32) -> String {
    format!("{}.{:02}", price / 100, price % 100)
}

impl Paywall {
    pub fn normalize(self) -> Option<Self> {
        let email = self.email.trim().to_string();
        let currency = self.currency.trim().to_uppercase();
        let valid = (3..=254).contains(&email.len())
            && email.contains('@')
            && !email.chars().any(char::is_whitespace)
            && (50..=100_000).contains(&self.price)
            && CURRENCIES.contains(&currency.as_str());
        valid.then_some(Self {
            email,
            price: self.price,
            currency,
            merchant_id: self.merchant_id,
        })
    }
}

#[derive(Debug, PartialEq)]
pub enum SettingsError {
    InvalidPaywall,
    InvalidPage,
    Full,
    NotSaved,
}

pub struct SongRequests {
    path: Option<PathBuf>,
    settings: Mutex<HashMap<String, Settings>>,
}

impl SongRequests {
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        let settings = json_file::load(path.as_deref())?;
        Ok(Self {
            path,
            settings: Mutex::new(settings),
        })
    }

    pub fn get(&self, id: &str) -> Settings {
        self.settings
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn set(&self, id: &str, new: Settings) -> Result<Settings, SettingsError> {
        let new = new.normalize()?;
        let mut settings = self.settings.lock().unwrap();
        if !settings.contains_key(id) && settings.len() >= 10000 {
            return Err(SettingsError::Full);
        }
        let old = if new == Settings::default() {
            settings.remove(id)
        } else {
            settings.insert(id.to_string(), new.clone())
        };
        if let Err(error) = json_file::save(self.path.as_deref(), &*settings) {
            warn!("failed to save song request settings: {error:#}");
            match old {
                Some(old) => settings.insert(id.to_string(), old),
                None => settings.remove(id),
            };
            return Err(SettingsError::NotSaved);
        }
        Ok(new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_are_saved() {
        let path = std::env::temp_dir().join(format!(
            "song-requests-{}-{:?}.json",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);
        let song_requests = SongRequests::load(Some(path.clone())).unwrap();
        let paywall = Paywall {
            email: "erik@example.com".to_string(),
            price: 250,
            currency: "SEK".to_string(),
            merchant_id: None,
        };
        let settings = Settings {
            paywall: Some(paywall.clone()),
            title: Some("Hi".to_string()),
            ..Settings::default()
        };
        song_requests.set("a", settings.clone()).unwrap();
        song_requests.set("b", settings).unwrap();
        song_requests.set("b", Settings::default()).unwrap();
        let song_requests = SongRequests::load(Some(path.clone())).unwrap();
        assert_eq!(song_requests.get("a").paywall, Some(paywall));
        assert_eq!(song_requests.get("a").title.as_deref(), Some("Hi"));
        assert_eq!(song_requests.get("b"), Settings::default());
        assert_eq!(song_requests.settings.lock().unwrap().len(), 1);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn paywalls_are_validated() {
        let song_requests = SongRequests::load(None).unwrap();
        let paywall = |email: &str, price, currency: &str| {
            Some(Paywall {
                email: email.to_string(),
                price,
                currency: currency.to_string(),
                merchant_id: None,
            })
        };
        let settings = |paywall| Settings {
            paywall,
            ..Settings::default()
        };
        assert_eq!(
            song_requests
                .set("a", settings(paywall(" erik@example.com ", 250, "sek")))
                .unwrap()
                .paywall,
            paywall("erik@example.com", 250, "SEK")
        );
        assert_eq!(song_requests.get("a").paywall.unwrap().price, 250);
        for (email, price, currency) in [
            ("erik", 250, "SEK"),
            ("er ik@example.com", 250, "SEK"),
            ("erik@example.com", 49, "SEK"),
            ("erik@example.com", 100_001, "SEK"),
            ("erik@example.com", 250, "JPY"),
        ] {
            assert_eq!(
                song_requests.set("a", settings(paywall(email, price, currency))),
                Err(SettingsError::InvalidPaywall)
            );
        }
        assert!(song_requests.set("a", Settings::default()).is_ok());
        assert_eq!(song_requests.get("a"), Settings::default());
    }

    #[test]
    fn pages_are_validated() {
        let song_requests = SongRequests::load(None).unwrap();
        let page = |title: &str, message: &str, accent: &str| Settings {
            title: Some(title.to_string()),
            message: Some(message.to_string()),
            accent: Some(accent.to_string()),
            ..Settings::default()
        };
        assert_eq!(
            song_requests.set("a", page(" Hi! ", "  ", "purple")),
            Ok(Settings {
                title: Some("Hi!".to_string()),
                accent: Some("purple".to_string()),
                ..Settings::default()
            })
        );
        assert_eq!(
            song_requests.set("a", page("", "", "green")),
            Ok(Settings::default())
        );
        assert_eq!(song_requests.get("a"), Settings::default());
        assert!(
            song_requests
                .set("a", page(&"å".repeat(60), &"å".repeat(300), "red"))
                .is_ok()
        );
        for (title, message, accent) in [
            ("a".repeat(61), String::new(), "red"),
            (String::new(), "a".repeat(301), "red"),
            (String::new(), String::new(), "teal"),
        ] {
            assert_eq!(
                song_requests.set("a", page(&title, &message, accent)),
                Err(SettingsError::InvalidPage)
            );
        }
    }

    #[test]
    fn prices_are_formatted() {
        assert_eq!(format_price(50), "0.50");
        assert_eq!(format_price(205), "2.05");
        assert_eq!(format_price(100_000), "1000.00");
    }
}
