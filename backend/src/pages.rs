use std::sync::Arc;

use log::warn;
use ring::digest::{SHA256, digest};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::database::Database;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Paywall {
    pub price: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tips {
    pub amounts: Vec<u32>,
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

pub const FONTS: [&str; 6] = ["rounded", "modern", "serif", "mono", "handwritten", "pixel"];

pub fn is_color(accent: &str) -> bool {
    accent.len() == 7
        && accent.starts_with('#')
        && accent[1..].chars().all(|c| c.is_ascii_hexdigit())
}

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
        let accent = match self.accent.map(|accent| accent.to_lowercase()) {
            Some(accent) if !ACCENTS.contains(&accent.as_str()) && !is_color(&accent) => {
                return Err(SettingsError::InvalidPage);
            }
            accent => accent.filter(|accent| accent != ACCENTS[0]),
        };
        let font = match self.font {
            Some(font) if !FONTS.contains(&font.as_str()) => {
                return Err(SettingsError::InvalidPage);
            }
            font => font.filter(|font| font != FONTS[0]),
        };
        Ok(Self {
            currency,
            paywall,
            tips,
            spin,
            title: text(self.title, 60)?,
            message: text(self.message, 300)?,
            accent,
            font,
        })
    }
}

pub fn format_price(price: u32) -> String {
    format!("{}.{:02}", price / 100, price % 100)
}

fn valid_price(price: u32) -> bool {
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
            (1..=5).contains(&amounts.len()) && amounts.iter().all(|amount| valid_price(*amount));
        valid.then_some(Self { amounts })
    }
}

impl Spin {
    pub fn normalize(self) -> Option<Self> {
        let mut options = self.options;
        options.sort_unstable_by_key(|option| option.spins);
        let valid = (1..=5).contains(&options.len())
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

fn not_saved(error: rusqlite::Error) -> SettingsError {
    warn!("failed to save page settings: {error:#}");
    SettingsError::NotSaved
}

pub struct Pages {
    database: Arc<Database>,
}

impl Pages {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    pub fn get(&self, id: &str) -> Settings {
        let settings: rusqlite::Result<Option<String>> = self
            .database
            .lock()
            .query_row("SELECT settings FROM pages WHERE id = ?1", [id], |row| {
                row.get(0)
            })
            .optional();
        match settings {
            Ok(Some(settings)) => serde_json::from_str(&settings).unwrap_or_else(|error| {
                warn!("failed to parse the settings of page {id}: {error}");
                Settings::default()
            }),
            Ok(None) => Settings::default(),
            Err(error) => {
                warn!("failed to read page settings: {error:#}");
                Settings::default()
            }
        }
    }

    pub fn set(&self, id: &str, new: Settings) -> Result<Settings, SettingsError> {
        let new = new.normalize()?;
        let database = self.database.lock();
        if new == Settings::default() {
            database
                .execute("DELETE FROM pages WHERE id = ?1", [id])
                .map_err(not_saved)?;
        } else {
            let full = database
                .query_row(
                    "SELECT NOT EXISTS (SELECT 1 FROM pages WHERE id = ?1) AND count(*) >= 10000 FROM pages",
                    [id],
                    |row| row.get(0),
                )
                .map_err(not_saved)?;
            if full {
                return Err(SettingsError::Full);
            }
            database
                .execute(
                    "INSERT OR REPLACE INTO pages (id, settings) VALUES (?1, ?2)",
                    params![id, serde_json::to_string(&new).unwrap()],
                )
                .map_err(not_saved)?;
        }
        Ok(new)
    }

    pub fn image_version(&self, id: &str) -> Option<String> {
        self.database
            .lock()
            .query_row(
                "SELECT version FROM page_images WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .unwrap_or_else(|error| {
                warn!("failed to read the image version of page {id}: {error:#}");
                None
            })
    }

    pub fn image(&self, id: &str) -> Option<(String, Vec<u8>)> {
        self.database
            .lock()
            .query_row(
                "SELECT version, data FROM page_images WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .unwrap_or_else(|error| {
                warn!("failed to read the image of page {id}: {error:#}");
                None
            })
    }

    pub fn set_image(&self, id: &str, data: &[u8]) -> Result<String, SettingsError> {
        let version = digest(&SHA256, data).as_ref()[..8]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        self.database
            .lock()
            .execute(
                "INSERT OR REPLACE INTO page_images (id, version, data) VALUES (?1, ?2, ?3)",
                params![id, version, data],
            )
            .map_err(not_saved)?;
        Ok(version)
    }

    pub fn remove_image(&self, id: &str) -> Result<(), SettingsError> {
        self.database
            .lock()
            .execute("DELETE FROM page_images WHERE id = ?1", [id])
            .map_err(not_saved)?;
        Ok(())
    }

    pub fn image_count(&self) -> u64 {
        self.database
            .lock()
            .query_row("SELECT count(*) FROM page_images", [], |row| row.get(0))
            .unwrap_or_else(|error| {
                warn!("failed to count page images: {error:#}");
                0
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database;

    #[test]
    fn settings_are_saved() {
        let path = database::temporary_path("pages");
        let pages = Pages::new(Database::open(Some(&path)).unwrap());
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
        let pages = Pages::new(Database::open(Some(&path)).unwrap());
        assert_eq!(pages.get("a").paywall, Some(paywall));
        assert_eq!(pages.get("a").currency(), "SEK");
        assert_eq!(pages.get("a").title.as_deref(), Some("Hi"));
        assert_eq!(pages.get("b"), Settings::default());
        let count: u32 = pages
            .database
            .lock()
            .query_row("SELECT count(*) FROM pages", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
        database::remove(&path);
    }

    #[test]
    fn images_are_saved() {
        let path = database::temporary_path("page-images");
        let pages = Pages::new(Database::open(Some(&path)).unwrap());
        assert_eq!(pages.image_version("a"), None);
        let first = pages.set_image("a", b"first").unwrap();
        assert_eq!(first.len(), 16);
        let second = pages.set_image("a", b"second").unwrap();
        assert_ne!(first, second);
        pages.set_image("b", b"first").unwrap();
        pages.remove_image("b").unwrap();
        pages.set("a", Settings::default()).unwrap();
        let pages = Pages::new(Database::open(Some(&path)).unwrap());
        assert_eq!(pages.image_version("a"), Some(second.clone()));
        assert_eq!(pages.image("a"), Some((second, b"second".to_vec())));
        assert_eq!(pages.image("b"), None);
        assert_eq!(pages.image_count(), 1);
        database::remove(&path);
    }

    #[test]
    fn paywalls_are_validated() {
        let pages = Pages::new(Database::memory());
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
        let pages = Pages::new(Database::memory());
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
        let pages = Pages::new(Database::memory());
        let tips = |amounts: &[u32]| Settings {
            tips: Some(Tips {
                amounts: amounts.to_vec(),
            }),
            ..Settings::default()
        };
        assert_eq!(
            pages.set("a", tips(&[1000, 200, 500, 200])).unwrap(),
            tips(&[200, 500, 1000])
        );
        assert_eq!(pages.get("a").tips.unwrap().amounts, [200, 500, 1000]);
        for amounts in [
            &[][..],
            &[100, 200, 300, 400, 500, 600],
            &[49, 200],
            &[100_001],
        ] {
            assert_eq!(
                pages.set("a", tips(amounts)),
                Err(SettingsError::InvalidTips)
            );
        }
    }

    #[test]
    fn spins_are_validated() {
        let pages = Pages::new(Database::memory());
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
            &[(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)],
        ] {
            assert_eq!(
                pages.set("a", spin(options)),
                Err(SettingsError::InvalidSpin)
            );
        }
    }

    #[test]
    fn pages_are_validated() {
        let pages = Pages::new(Database::memory());
        let page = |title: &str, message: &str, accent: &str, font: &str| Settings {
            title: Some(title.to_string()),
            message: Some(message.to_string()),
            accent: Some(accent.to_string()),
            font: Some(font.to_string()),
            ..Settings::default()
        };
        assert_eq!(
            pages.set("a", page(" Hi! ", "  ", "purple", "serif")),
            Ok(Settings {
                title: Some("Hi!".to_string()),
                accent: Some("purple".to_string()),
                font: Some("serif".to_string()),
                ..Settings::default()
            })
        );
        assert_eq!(
            pages.set("a", page("", "", "#A1B2c3", "rounded")),
            Ok(Settings {
                accent: Some("#a1b2c3".to_string()),
                ..Settings::default()
            })
        );
        assert_eq!(
            pages.set("a", page("", "", "green", "rounded")),
            Ok(Settings::default())
        );
        assert_eq!(pages.get("a"), Settings::default());
        assert!(
            pages
                .set("a", page(&"å".repeat(60), &"å".repeat(300), "red", "pixel"))
                .is_ok()
        );
        for (title, message, accent, font) in [
            ("a".repeat(61), String::new(), "red", "mono"),
            (String::new(), "a".repeat(301), "red", "mono"),
            (String::new(), String::new(), "teal", "mono"),
            (String::new(), String::new(), "#12345", "mono"),
            (String::new(), String::new(), "#12345g", "mono"),
            (String::new(), String::new(), "123456", "mono"),
            (String::new(), String::new(), "red", "comic"),
        ] {
            assert_eq!(
                pages.set("a", page(&title, &message, accent, font)),
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
