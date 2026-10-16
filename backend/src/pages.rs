use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::Result;
use log::warn;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::json_file;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(
        default,
        rename = "songsDisabled",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub songs_disabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tips {
    pub amounts: Vec<u32>,
    #[serde(default, rename = "webhookUrl")]
    pub webhook_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spin {
    pub options: Vec<SpinOption>,
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

const DEFAULT_CURRENCY: &str = "USD";

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
    pub fn currency(&self) -> &str {
        self.currency.as_deref().unwrap_or(DEFAULT_CURRENCY)
    }

    pub fn normalize(self) -> Result<Self, SettingsError> {
        let currency = match self.currency.map(|currency| currency.trim().to_uppercase()) {
            Some(currency) if !CURRENCIES.contains(&currency.as_str()) => {
                return Err(SettingsError::InvalidCurrency);
            }
            currency => currency.filter(|currency| currency != DEFAULT_CURRENCY),
        };
        let paywall = match self.paywall {
            Some(_) if self.songs_disabled => None,
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
            songs_disabled: self.songs_disabled,
            currency,
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
        valid_price(self.price).then_some(self)
    }
}

impl Tips {
    pub fn normalize(self) -> Option<Self> {
        let mut amounts = self.amounts;
        amounts.sort_unstable();
        amounts.dedup();
        let valid =
            (1..=4).contains(&amounts.len()) && amounts.iter().all(|amount| valid_price(*amount));
        valid.then_some(Self {
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
        let mut options = self.options;
        options.sort_unstable_by_key(|option| option.spins);
        let valid = (1..=4).contains(&options.len())
            && options
                .iter()
                .all(|option| (1..=10).contains(&option.spins) && valid_price(option.price))
            && options
                .windows(2)
                .all(|pair| pair[0].spins != pair[1].spins);
        valid.then_some(Self { options })
    }
}

#[derive(Debug, PartialEq)]
pub enum SettingsError {
    InvalidCurrency,
    InvalidPaywall,
    InvalidTips,
    InvalidSpin,
    InvalidPage,
    Full,
    NotSaved,
}

fn with_currency(mut settings: Value) -> Value {
    if settings.get("currency").is_none() {
        let currency = ["paywall", "tips", "spin"]
            .iter()
            .find_map(|key| settings[key].get("currency").cloned());
        if let (Some(currency), Some(settings)) = (currency, settings.as_object_mut()) {
            settings.insert("currency".to_string(), currency);
        }
    }
    settings
}

pub struct Pages {
    path: Option<PathBuf>,
    settings: Mutex<HashMap<String, Settings>>,
}

impl Pages {
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        let settings: HashMap<String, Value> = json_file::load(path.as_deref())?;
        let settings = settings
            .into_iter()
            .map(|(id, settings)| Ok((id, serde_json::from_value(with_currency(settings))?)))
            .collect::<Result<_>>()?;
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
        let paywall = Paywall { price: 250 };
        let settings = Settings {
            currency: Some("SEK".to_string()),
            paywall: Some(paywall.clone()),
            title: Some("Hi".to_string()),
            ..Settings::default()
        };
        pages.set("a", settings.clone()).unwrap();
        pages.set("b", settings).unwrap();
        pages.set("b", Settings::default()).unwrap();
        let pages = Pages::load(Some(path.clone())).unwrap();
        assert_eq!(pages.get("a").paywall, Some(paywall));
        assert_eq!(pages.get("a").currency(), "SEK");
        assert_eq!(pages.get("a").title.as_deref(), Some("Hi"));
        assert_eq!(pages.get("b"), Settings::default());
        assert_eq!(pages.settings.lock().unwrap().len(), 1);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn paywalls_are_validated() {
        let pages = Pages::load(None).unwrap();
        let settings = |price| Settings {
            paywall: Some(Paywall { price }),
            ..Settings::default()
        };
        assert_eq!(pages.set("a", settings(250)), Ok(settings(250)));
        for price in [49, 100_001] {
            assert_eq!(
                pages.set("a", settings(price)),
                Err(SettingsError::InvalidPaywall)
            );
        }
        assert!(pages.set("a", Settings::default()).is_ok());
        assert_eq!(pages.get("a"), Settings::default());
    }

    #[test]
    fn currencies_are_validated() {
        let pages = Pages::load(None).unwrap();
        let settings = |currency: &str| Settings {
            currency: Some(currency.to_string()),
            ..Settings::default()
        };
        assert_eq!(pages.set("a", settings(" sek ")), Ok(settings("SEK")));
        assert_eq!(pages.get("a").currency(), "SEK");
        assert_eq!(pages.set("a", settings("usd")), Ok(Settings::default()));
        assert_eq!(pages.get("a").currency(), "USD");
        assert_eq!(
            pages.set("a", settings("JPY")),
            Err(SettingsError::InvalidCurrency)
        );
    }

    #[test]
    fn tips_are_validated() {
        let pages = Pages::load(None).unwrap();
        let tips = |amounts: &[u32]| Settings {
            tips: Some(Tips {
                amounts: amounts.to_vec(),
                webhook_url: None,
            }),
            ..Settings::default()
        };
        assert_eq!(
            pages.set("a", tips(&[1000, 200, 500, 200])).unwrap(),
            tips(&[200, 500, 1000])
        );
        assert_eq!(pages.get("a").tips.unwrap().amounts, [200, 500, 1000]);
        for amounts in [&[][..], &[100, 200, 300, 400, 500], &[49, 200], &[100_001]] {
            assert_eq!(
                pages.set("a", tips(amounts)),
                Err(SettingsError::InvalidTips)
            );
        }
    }

    #[test]
    fn spins_are_validated() {
        let pages = Pages::load(None).unwrap();
        let spin = |options: &[(u32, u32)]| Settings {
            spin: Some(Spin {
                options: options
                    .iter()
                    .map(|(spins, price)| SpinOption {
                        spins: *spins,
                        price: *price,
                    })
                    .collect(),
            }),
            ..Settings::default()
        };
        assert_eq!(
            pages.set("a", spin(&[(2, 800), (1, 500)])),
            Ok(spin(&[(1, 500), (2, 800)]))
        );
        for options in [
            &[(1, 49)][..],
            &[(0, 500)],
            &[(11, 500)],
            &[(1, 500), (1, 800)],
            &[],
            &[(1, 1), (2, 2), (3, 3), (4, 4), (5, 5)],
        ] {
            assert_eq!(
                pages.set("a", spin(options)),
                Err(SettingsError::InvalidSpin)
            );
        }
    }

    #[test]
    fn old_currencies_move_to_the_page() {
        let path = std::env::temp_dir().join(format!(
            "pages-old-{}-{:?}.json",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::write(
            &path,
            r#"{
                "a": {"tips": {"currency": "SEK", "amounts": [500]}},
                "b": {"spin": {"currency": "EUR", "options": [{"spins": 1, "price": 500}]}},
                "c": {"title": "Hi"}
            }"#,
        )
        .unwrap();
        let pages = Pages::load(Some(path.clone())).unwrap();
        assert_eq!(pages.get("a").currency(), "SEK");
        assert_eq!(pages.get("b").currency(), "EUR");
        assert_eq!(pages.get("c").currency(), "USD");
        std::fs::remove_file(&path).unwrap();
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
    fn disabled_songs_have_no_paywall() {
        let pages = Pages::load(None).unwrap();
        let settings = Settings {
            songs_disabled: true,
            paywall: Some(Paywall { price: 250 }),
            ..Settings::default()
        };
        assert_eq!(
            pages.set("a", settings),
            Ok(Settings {
                songs_disabled: true,
                ..Settings::default()
            })
        );
        assert!(pages.get("a").songs_disabled);
    }

    #[test]
    fn prices_are_formatted() {
        assert_eq!(format_price(50), "0.50");
        assert_eq!(format_price(205), "2.05");
        assert_eq!(format_price(100_000), "1000.00");
    }
}
