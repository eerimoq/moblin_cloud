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
    #[serde(default, rename = "merchantId")]
    pub merchant_id: String,
    pub price: u32,
    pub currency: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tips {
    #[serde(rename = "merchantId")]
    pub merchant_id: String,
    pub currency: String,
    pub amounts: Vec<u32>,
    #[serde(default, rename = "webhookUrl")]
    pub webhook_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spin {
    #[serde(rename = "merchantId")]
    pub merchant_id: String,
    pub options: Vec<SpinOption>,
    pub currency: String,
    #[serde(rename = "webhookUrl")]
    pub webhook_url: String,
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

fn valid_merchant_id(merchant_id: &str) -> bool {
    merchant_id.len() == 13 && merchant_id.chars().all(|c| c.is_ascii_alphanumeric())
}

impl Paywall {
    pub fn normalize(self) -> Option<Self> {
        let merchant_id = self.merchant_id.trim().to_uppercase();
        let currency = self.currency.trim().to_uppercase();
        let valid = valid_merchant_id(&merchant_id)
            && valid_price(self.price)
            && CURRENCIES.contains(&currency.as_str());
        valid.then_some(Self {
            merchant_id,
            price: self.price,
            currency,
        })
    }
}

impl Tips {
    pub fn normalize(self) -> Option<Self> {
        let merchant_id = self.merchant_id.trim().to_uppercase();
        let currency = self.currency.trim().to_uppercase();
        let mut amounts = self.amounts;
        amounts.sort_unstable();
        amounts.dedup();
        let valid = valid_merchant_id(&merchant_id)
            && (1..=4).contains(&amounts.len())
            && amounts.iter().all(|amount| valid_price(*amount))
            && CURRENCIES.contains(&currency.as_str());
        valid.then_some(Self {
            merchant_id,
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
        let merchant_id = self.merchant_id.trim().to_uppercase();
        let currency = self.currency.trim().to_uppercase();
        let webhook_url = self.webhook_url.trim().to_string();
        let mut options = self.options;
        options.sort_unstable_by_key(|option| option.spins);
        let valid = valid_merchant_id(&merchant_id)
            && (1..=4).contains(&options.len())
            && options
                .iter()
                .all(|option| (1..=100).contains(&option.spins) && valid_price(option.price))
            && options
                .windows(2)
                .all(|pair| pair[0].spins != pair[1].spins)
            && CURRENCIES.contains(&currency.as_str())
            && !webhook_url.is_empty();
        valid.then_some(Self {
            merchant_id,
            options,
            currency,
            webhook_url,
        })
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
            merchant_id: "ABCDEFGHJKLMN".to_string(),
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
        let paywall = |merchant_id: &str, price, currency: &str| {
            Some(Paywall {
                merchant_id: merchant_id.to_string(),
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
                .set("a", settings(paywall(" abcdefghjklmn ", 250, "sek")))
                .unwrap()
                .paywall,
            paywall("ABCDEFGHJKLMN", 250, "SEK")
        );
        assert_eq!(pages.get("a").paywall.unwrap().price, 250);
        for (merchant_id, price, currency) in [
            ("ABCDEFGHJKLM", 250, "SEK"),
            ("erik@example.com", 250, "SEK"),
            ("ABCDEF HJKLMN", 250, "SEK"),
            ("ABCDEFGHJKLMN", 49, "SEK"),
            ("ABCDEFGHJKLMN", 100_001, "SEK"),
            ("ABCDEFGHJKLMN", 250, "JPY"),
        ] {
            assert_eq!(
                pages.set("a", settings(paywall(merchant_id, price, currency))),
                Err(SettingsError::InvalidPaywall)
            );
        }
        assert!(pages.set("a", Settings::default()).is_ok());
        assert_eq!(pages.get("a"), Settings::default());
    }

    #[test]
    fn tips_are_validated() {
        let pages = Pages::load(None).unwrap();
        let tips = |merchant_id: &str, currency: &str, amounts: &[u32]| Settings {
            tips: Some(Tips {
                merchant_id: merchant_id.to_string(),
                currency: currency.to_string(),
                amounts: amounts.to_vec(),
                webhook_url: None,
            }),
            ..Settings::default()
        };
        assert_eq!(
            pages
                .set("a", tips(" abcdefghjklmn ", "usd", &[1000, 200, 500, 200]))
                .unwrap(),
            tips("ABCDEFGHJKLMN", "USD", &[200, 500, 1000])
        );
        assert_eq!(pages.get("a").tips.unwrap().amounts, [200, 500, 1000]);
        for (merchant_id, currency, amounts) in [
            ("ABCDEFGHJKLM", "USD", &[200][..]),
            ("ABCDEFGHJKLMN", "JPY", &[200]),
            ("ABCDEFGHJKLMN", "USD", &[]),
            ("ABCDEFGHJKLMN", "USD", &[100, 200, 300, 400, 500]),
            ("ABCDEFGHJKLMN", "USD", &[49, 200]),
            ("ABCDEFGHJKLMN", "USD", &[100_001]),
        ] {
            assert_eq!(
                pages.set("a", tips(merchant_id, currency, amounts)),
                Err(SettingsError::InvalidTips)
            );
        }
    }

    #[test]
    fn spins_are_validated() {
        let pages = Pages::load(None).unwrap();
        let spin =
            |merchant_id: &str, options: &[(u32, u32)], currency: &str, webhook_url: &str| {
                Settings {
                    spin: Some(Spin {
                        merchant_id: merchant_id.to_string(),
                        options: options
                            .iter()
                            .map(|(spins, price)| SpinOption {
                                spins: *spins,
                                price: *price,
                            })
                            .collect(),
                        currency: currency.to_string(),
                        webhook_url: webhook_url.to_string(),
                    }),
                    ..Settings::default()
                }
            };
        let url = "https://example.com/spin";
        assert_eq!(
            pages.set(
                "a",
                spin(" abcdefghjklmn ", &[(2, 800), (1, 500)], "eur", url)
            ),
            Ok(spin("ABCDEFGHJKLMN", &[(1, 500), (2, 800)], "EUR", url))
        );
        for (merchant_id, options, currency, webhook_url) in [
            ("ABCDEFGHJKLM", &[(1, 500)][..], "EUR", url),
            ("ABCDEFGHJKLMN", &[(1, 49)], "EUR", url),
            ("ABCDEFGHJKLMN", &[(0, 500)], "EUR", url),
            ("ABCDEFGHJKLMN", &[(101, 500)], "EUR", url),
            ("ABCDEFGHJKLMN", &[(1, 500), (1, 800)], "EUR", url),
            ("ABCDEFGHJKLMN", &[], "EUR", url),
            (
                "ABCDEFGHJKLMN",
                &[(1, 1), (2, 2), (3, 3), (4, 4), (5, 5)],
                "EUR",
                url,
            ),
            ("ABCDEFGHJKLMN", &[(1, 500)], "JPY", url),
            ("ABCDEFGHJKLMN", &[(1, 500)], "EUR", " "),
        ] {
            assert_eq!(
                pages.set("a", spin(merchant_id, options, currency, webhook_url)),
                Err(SettingsError::InvalidSpin)
            );
        }
    }

    #[test]
    fn email_paywalls_load_without_merchant_id() {
        let settings: Settings = serde_json::from_str(
            r#"{"paywall": {"email": "erik@example.com", "price": 250, "currency": "SEK"}}"#,
        )
        .unwrap();
        assert_eq!(settings.paywall.unwrap().merchant_id, "");
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
