use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::Result;
use log::warn;
use serde::{Deserialize, Serialize};

use crate::json_file;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub name: Option<String>,
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
    InvalidName,
    InvalidPaywall,
    NameTaken,
    Full,
    NotSaved,
}

pub struct SongRequests {
    path: Option<PathBuf>,
    settings: Mutex<HashMap<String, Settings>>,
}

pub fn normalize_name(name: &str) -> Option<String> {
    let name = name.trim().to_lowercase();
    let valid = (3..=25).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-');
    valid.then_some(name)
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

    pub fn id_of_name(&self, name: &str) -> Option<String> {
        let name = normalize_name(name)?;
        self.settings
            .lock()
            .unwrap()
            .iter()
            .find(|(_, settings)| settings.name.as_ref() == Some(&name))
            .map(|(id, _)| id.clone())
    }

    pub fn set(
        &self,
        id: &str,
        name: Option<&str>,
        paywall: Option<Paywall>,
    ) -> Result<Settings, SettingsError> {
        let name = match name.map(str::trim).filter(|name| !name.is_empty()) {
            Some(name) => Some(normalize_name(name).ok_or(SettingsError::InvalidName)?),
            None => None,
        };
        let paywall = match paywall {
            Some(paywall) => Some(paywall.normalize().ok_or(SettingsError::InvalidPaywall)?),
            None => None,
        };
        let new = Settings { name, paywall };
        let mut settings = self.settings.lock().unwrap();
        if new.name.is_some()
            && settings
                .iter()
                .any(|(other_id, other)| other_id != id && other.name == new.name)
        {
            return Err(SettingsError::NameTaken);
        }
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
    fn names_are_normalized() {
        assert_eq!(normalize_name(" Erik-123 ").as_deref(), Some("erik-123"));
        assert_eq!(normalize_name("ab"), None);
        assert_eq!(normalize_name(&"a".repeat(26)), None);
        assert_eq!(normalize_name("-erik"), None);
        assert_eq!(normalize_name("erik-"), None);
        assert_eq!(normalize_name("er ik"), None);
        assert_eq!(normalize_name("erik_"), None);
        assert_eq!(normalize_name("éric"), None);
    }

    #[test]
    fn names_are_unique() {
        let song_requests = SongRequests::load(None).unwrap();
        assert_eq!(song_requests.get("a"), Settings::default());
        assert_eq!(
            song_requests.set("a", Some("Erik"), None),
            Ok(Settings {
                name: Some("erik".to_string()),
                paywall: None,
            })
        );
        assert_eq!(song_requests.id_of_name("ERIK").as_deref(), Some("a"));
        assert_eq!(
            song_requests.set("b", Some("erik"), None),
            Err(SettingsError::NameTaken)
        );
        assert_eq!(
            song_requests.set("b", Some("e r"), None),
            Err(SettingsError::InvalidName)
        );
        assert!(song_requests.set("a", Some("erik"), None).is_ok());
        assert!(song_requests.set("a", Some(" "), None).is_ok());
        assert_eq!(song_requests.id_of_name("erik"), None);
        assert!(song_requests.set("b", Some("erik"), None).is_ok());
        assert_eq!(song_requests.id_of_name("erik").as_deref(), Some("b"));
    }

    #[test]
    fn settings_are_saved() {
        let path = std::env::temp_dir().join(format!(
            "song-requests-{}-{:?}.json",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);
        let song_requests = SongRequests::load(Some(path.clone())).unwrap();
        song_requests.set("a", Some("erik"), None).unwrap();
        song_requests.set("b", Some("other"), None).unwrap();
        song_requests.set("b", None, None).unwrap();
        let song_requests = SongRequests::load(Some(path.clone())).unwrap();
        assert_eq!(song_requests.id_of_name("erik").as_deref(), Some("a"));
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
                .set("a", None, paywall(" erik@example.com ", 250, "sek"))
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
                song_requests.set("a", None, paywall(email, price, currency)),
                Err(SettingsError::InvalidPaywall)
            );
        }
        assert!(song_requests.set("a", None, None).is_ok());
        assert_eq!(song_requests.get("a"), Settings::default());
    }

    #[test]
    fn prices_are_formatted() {
        assert_eq!(format_price(50), "0.50");
        assert_eq!(format_price(205), "2.05");
        assert_eq!(format_price(100_000), "1000.00");
    }
}
