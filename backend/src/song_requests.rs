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

    pub fn set(&self, id: &str, paywall: Option<Paywall>) -> Result<Settings, SettingsError> {
        let paywall = match paywall {
            Some(paywall) => Some(paywall.normalize().ok_or(SettingsError::InvalidPaywall)?),
            None => None,
        };
        let new = Settings { paywall };
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
        song_requests.set("a", Some(paywall.clone())).unwrap();
        song_requests.set("b", Some(paywall.clone())).unwrap();
        song_requests.set("b", None).unwrap();
        let song_requests = SongRequests::load(Some(path.clone())).unwrap();
        assert_eq!(song_requests.get("a").paywall, Some(paywall));
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
        assert_eq!(
            song_requests
                .set("a", paywall(" erik@example.com ", 250, "sek"))
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
                song_requests.set("a", paywall(email, price, currency)),
                Err(SettingsError::InvalidPaywall)
            );
        }
        assert!(song_requests.set("a", None).is_ok());
        assert_eq!(song_requests.get("a"), Settings::default());
    }

    #[test]
    fn prices_are_formatted() {
        assert_eq!(format_price(50), "0.50");
        assert_eq!(format_price(205), "2.05");
        assert_eq!(format_price(100_000), "1000.00");
    }
}
