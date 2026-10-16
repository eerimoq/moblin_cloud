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
    pub tips: Option<Tips>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spin: Option<Spin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Paywall {
    pub price: u32,
    pub currency: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tips {
    pub currency: String,
    pub amounts: Vec<u32>,
    #[serde(default, rename = "webhookUrl")]
    pub webhook_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spin {
    pub options: Vec<SpinOption>,
    pub currency: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpinOption {
    pub spins: u32,
    pub price: u32,
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
        let tips = match self.tips {
            Some(tips) => Some(tips.normalize().ok_or(SettingsError::InvalidTips)?),
            None => None,
        };
        let spin = match self.spin {
            Some(spin) => Some(spin.normalize().ok_or(SettingsError::InvalidSpin)?),
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
            tips,
            spin,
            title: text(self.title, 60)?,
            message: text(self.message, 300)?,
            accent,
        })
    }
}

pub fn format_price(price: u32) -> String {
    format!("{}.{:02}", price / 100, price % 100)
}

pub fn valid_price(price: u32) -> bool {
    (50..=100_000).contains(&price)
}

impl Paywall {
    pub fn normalize(self) -> Option<Self> {
        let currency = self.currency.trim().to_uppercase();
        let valid = valid_price(self.price) && CURRENCIES.contains(&currency.as_str());
        valid.then_some(Self {
            price: self.price,
            currency,
        })
    }
}

impl Tips {
    pub fn normalize(self) -> Option<Self> {
        let currency = self.currency.trim().to_uppercase();
        let mut amounts = self.amounts;
        amounts.sort_unstable();
        amounts.dedup();
        let valid = (1..=4).contains(&amounts.len())
            && amounts.iter().all(|amount| valid_price(*amount))
            && CURRENCIES.contains(&currency.as_str());
        valid.then_some(Self {
            currency,
            amounts,
            webhook_url: self
                .webhook_url
                .map(|url| url.trim().to_string())
                .filter(|url| !url.is_empty()),
        })
    }
}

impl Spin {
    pub fn normalize(self) -> Option<Self> {
        let currency = self.currency.trim().to_uppercase();
        let mut options = self.options;
        options.sort_unstable_by_key(|option| option.spins);
        let valid = (1..=4).contains(&options.len())
            && options
                .iter()
                .all(|option| (1..=10).contains(&option.spins) && valid_price(option.price))
            && options
                .windows(2)
                .all(|pair| pair[0].spins != pair[1].spins)
            && CURRENCIES.contains(&currency.as_str());
        valid.then_some(Self { options, currency })
    }
}

#[derive(Debug, PartialEq)]
pub enum SettingsError {
    InvalidPaywall,
    InvalidTips,
    InvalidSpin,
    InvalidPage,
    Full,
    NotSaved,
}

pub struct Pages {
    path: Option<PathBuf>,
    settings: Mutex<HashMap<String, Settings>>,
}

impl Pages {
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
            warn!("failed to save page settings: {error:#}");
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
            "pages-{}-{:?}.json",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);
        let pages = Pages::load(Some(path.clone())).unwrap();
        let paywall = Paywall {
            price: 250,
            currency: "SEK".to_string(),
        };
        let settings = Settings {
            paywall: Some(paywall.clone()),
            title: Some("Hi".to_string()),
            ..Settings::default()
        };
        pages.set("a", settings.clone()).unwrap();
        pages.set("b", settings).unwrap();
        pages.set("b", Settings::default()).unwrap();
        let pages = Pages::load(Some(path.clone())).unwrap();
        assert_eq!(pages.get("a").paywall, Some(paywall));
        assert_eq!(pages.get("a").title.as_deref(), Some("Hi"));
        assert_eq!(pages.get("b"), Settings::default());
        assert_eq!(pages.settings.lock().unwrap().len(), 1);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn paywalls_are_validated() {
        let pages = Pages::load(None).unwrap();
        let paywall = |price, currency: &str| {
            Some(Paywall {
                price,
                currency: currency.to_string(),
            })
        };
        let settings = |paywall| Settings {
            paywall,
            ..Settings::default()
        };
        assert_eq!(
            pages
                .set("a", settings(paywall(250, " sek ")))
                .unwrap()
                .paywall,
            paywall(250, "SEK")
        );
        assert_eq!(pages.get("a").paywall.unwrap().price, 250);
        for (price, currency) in [(49, "SEK"), (100_001, "SEK"), (250, "JPY")] {
            assert_eq!(
                pages.set("a", settings(paywall(price, currency))),
                Err(SettingsError::InvalidPaywall)
            );
        }
        assert!(pages.set("a", Settings::default()).is_ok());
        assert_eq!(pages.get("a"), Settings::default());
    }

    #[test]
    fn tips_are_validated() {
        let pages = Pages::load(None).unwrap();
        let tips = |currency: &str, amounts: &[u32]| Settings {
            tips: Some(Tips {
                currency: currency.to_string(),
                amounts: amounts.to_vec(),
                webhook_url: None,
            }),
            ..Settings::default()
        };
        assert_eq!(
            pages.set("a", tips("usd", &[1000, 200, 500, 200])).unwrap(),
            tips("USD", &[200, 500, 1000])
        );
        assert_eq!(pages.get("a").tips.unwrap().amounts, [200, 500, 1000]);
        for (currency, amounts) in [
            ("JPY", &[200][..]),
            ("USD", &[]),
            ("USD", &[100, 200, 300, 400, 500]),
            ("USD", &[49, 200]),
            ("USD", &[100_001]),
        ] {
            assert_eq!(
                pages.set("a", tips(currency, amounts)),
                Err(SettingsError::InvalidTips)
            );
        }
    }

    #[test]
    fn spins_are_validated() {
        let pages = Pages::load(None).unwrap();
        let spin = |options: &[(u32, u32)], currency: &str| Settings {
            spin: Some(Spin {
                options: options
                    .iter()
                    .map(|(spins, price)| SpinOption {
                        spins: *spins,
                        price: *price,
                    })
                    .collect(),
                currency: currency.to_string(),
            }),
            ..Settings::default()
        };
        assert_eq!(
            pages.set("a", spin(&[(2, 800), (1, 500)], "eur")),
            Ok(spin(&[(1, 500), (2, 800)], "EUR"))
        );
        for (options, currency) in [
            (&[(1, 49)][..], "EUR"),
            (&[(0, 500)], "EUR"),
            (&[(11, 500)], "EUR"),
            (&[(1, 500), (1, 800)], "EUR"),
            (&[], "EUR"),
            (&[(1, 1), (2, 2), (3, 3), (4, 4), (5, 5)], "EUR"),
            (&[(1, 500)], "JPY"),
        ] {
            assert_eq!(
                pages.set("a", spin(options, currency)),
                Err(SettingsError::InvalidSpin)
            );
        }
    }

    #[test]
    fn old_merchant_ids_are_ignored() {
        let settings: Settings = serde_json::from_str(
            r#"{"paywall": {"merchantId": "ABCDEFGHJKLMN", "price": 250, "currency": "SEK"}}"#,
        )
        .unwrap();
        assert_eq!(settings.paywall.unwrap().price, 250);
    }

    #[test]
    fn pages_are_validated() {
        let pages = Pages::load(None).unwrap();
        let page = |title: &str, message: &str, accent: &str| Settings {
            title: Some(title.to_string()),
            message: Some(message.to_string()),
            accent: Some(accent.to_string()),
            ..Settings::default()
        };
        assert_eq!(
            pages.set("a", page(" Hi! ", "  ", "purple")),
            Ok(Settings {
                title: Some("Hi!".to_string()),
                accent: Some("purple".to_string()),
                ..Settings::default()
            })
        );
        assert_eq!(
            pages.set("a", page("", "", "green")),
            Ok(Settings::default())
        );
        assert_eq!(pages.get("a"), Settings::default());
        assert!(
            pages
                .set("a", page(&"å".repeat(60), &"å".repeat(300), "red"))
                .is_ok()
        );
        for (title, message, accent) in [
            ("a".repeat(61), String::new(), "red"),
            (String::new(), "a".repeat(301), "red"),
            (String::new(), String::new(), "teal"),
        ] {
            assert_eq!(
                pages.set("a", page(&title, &message, accent)),
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
