use std::collections::HashMap;
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
    pub tips: Option<Feature<Tips>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spin: Option<Feature<Spin>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects: Option<Feature<Effects>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_spins: Option<Feature<Tiers>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_effects: Option<Feature<Tiers>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Feature<T> {
    #[serde(default = "enabled")]
    pub enabled: bool,
    #[serde(flatten)]
    pub value: T,
}

fn enabled() -> bool {
    true
}

impl<T> Feature<T> {
    pub fn normalize(self, normalize: impl FnOnce(T) -> Option<T>) -> Option<Self> {
        Some(Self {
            enabled: self.enabled,
            value: normalize(self.value)?,
        })
    }
}

fn active<T>(feature: &Option<Feature<T>>) -> Option<&T> {
    feature
        .as_ref()
        .filter(|feature| feature.enabled)
        .map(|feature| &feature.value)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tips {
    pub amounts: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Effects {
    pub price: u32,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tiers {
    pub tier1: u32,
    pub tier2: u32,
    pub tier3: u32,
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

    pub fn tips(&self) -> Option<&Tips> {
        active(&self.tips)
    }

    pub fn spin(&self) -> Option<&Spin> {
        active(&self.spin)
    }

    pub fn effects(&self) -> Option<&Effects> {
        active(&self.effects)
    }

    pub fn sub_spins(&self) -> Option<&Tiers> {
        active(&self.sub_spins)
    }

    pub fn sub_effects(&self) -> Option<&Tiers> {
        active(&self.sub_effects)
    }

    pub fn normalize(self) -> Result<Self, SettingsError> {
        let currency = match self.currency.map(|currency| currency.trim().to_uppercase()) {
            Some(currency) if !CURRENCIES.contains(&currency.as_str()) => {
                return Err(SettingsError::InvalidCurrency);
            }
            currency => currency.filter(|currency| currency != DEFAULT_CURRENCY),
        };
        let tips = match self.tips {
            Some(tips) => Some(
                tips.normalize(Tips::normalize)
                    .ok_or(SettingsError::InvalidTips)?,
            ),
            None => None,
        };
        let spin = match self.spin {
            Some(spin) => Some(
                spin.normalize(Spin::normalize)
                    .ok_or(SettingsError::InvalidSpin)?,
            ),
            None => None,
        };
        let effects = match self.effects {
            Some(effects) => Some(
                effects
                    .normalize(Effects::normalize)
                    .ok_or(SettingsError::InvalidEffects)?,
            ),
            None => None,
        };
        let tiers = |tiers: Option<Feature<Tiers>>| match tiers {
            Some(tiers) => tiers
                .normalize(Tiers::normalize)
                .map(Some)
                .ok_or(SettingsError::InvalidTiers),
            None => Ok(None),
        };
        let sub_spins = tiers(self.sub_spins)?;
        let sub_effects = tiers(self.sub_effects)?;
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
            tips,
            spin,
            effects,
            sub_spins,
            sub_effects,
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
    (50..=10_000_000).contains(&price)
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
        options.sort_unstable_by_key(|option| (option.spins, option.price));
        let valid = (1..=5).contains(&options.len())
            && options
                .iter()
                .all(|option| (1..=10).contains(&option.spins) && valid_price(option.price));
        valid.then_some(Self { options })
    }
}

impl Effects {
    pub fn normalize(self) -> Option<Self> {
        valid_price(self.price).then_some(self)
    }
}

impl Tiers {
    pub fn normalize(self) -> Option<Self> {
        let tiers = [self.tier1, self.tier2, self.tier3];
        let valid = tiers.iter().all(|spins| *spins <= 100) && tiers.iter().any(|spins| *spins > 0);
        valid.then_some(self)
    }

    pub fn allowance(&self, tier: Option<u8>) -> u32 {
        match tier {
            Some(1) => self.tier1,
            Some(2) => self.tier2,
            Some(3) => self.tier3,
            _ => 0,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum SettingsError {
    InvalidCurrency,
    InvalidTips,
    InvalidSpin,
    InvalidEffects,
    InvalidTiers,
    InvalidPage,
    Full,
    NotSaved,
}

fn not_saved(error: rusqlite::Error) -> SettingsError {
    warn!("failed to save page settings: {error:#}");
    SettingsError::NotSaved
}

#[derive(Debug, Default, PartialEq)]
pub struct Look {
    pub settings: Settings,
    pub image: Option<String>,
    pub icon: Option<String>,
}

impl Look {
    pub fn new(
        id: &str,
        settings: Option<String>,
        image: Option<String>,
        icon: Option<String>,
    ) -> Self {
        let settings = settings
            .map(|settings| {
                serde_json::from_str(&settings).unwrap_or_else(|error| {
                    warn!("failed to parse the settings of page {id}: {error}");
                    Settings::default()
                })
            })
            .unwrap_or_default();
        Self {
            settings,
            image,
            icon,
        }
    }
}

pub struct Pages {
    database: Arc<Database>,
}

impl Pages {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    pub fn get(&self, id: &str) -> Settings {
        self.look(id).settings
    }

    pub fn look(&self, id: &str) -> Look {
        let row = self
            .database
            .lock()
            .prepare_cached(
                "SELECT (SELECT settings FROM pages WHERE id = ?1),
                        (SELECT version FROM page_images WHERE id = ?1),
                        (SELECT icon FROM page_icons WHERE id = ?1)",
            )
            .and_then(|mut statement| {
                statement.query_row([id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            });
        match row {
            Ok((settings, image, icon)) => Look::new(id, settings, image, icon),
            Err(error) => {
                warn!("failed to read page {id}: {error:#}");
                Look::default()
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

    pub fn icons(&self, ids: &[String]) -> HashMap<String, String> {
        let ids = serde_json::to_string(ids).unwrap_or_default();
        let database = self.database.lock();
        let result = database
            .prepare_cached(
                "SELECT id, icon FROM page_icons WHERE id IN (SELECT value FROM json_each(?1))",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([ids], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<rusqlite::Result<_>>()
            });
        result.unwrap_or_else(|error| {
            warn!("failed to read the page icons: {error:#}");
            HashMap::new()
        })
    }

    pub fn set_icon(&self, id: &str, icon: &str) {
        if let Err(error) = self.database.lock().execute(
            "INSERT OR REPLACE INTO page_icons (id, icon) VALUES (?1, ?2)",
            params![id, icon],
        ) {
            warn!("failed to save the icon of page {id}: {error:#}");
        }
    }

    pub fn remove_image(&self, id: &str) -> Result<(), SettingsError> {
        self.database
            .lock()
            .execute("DELETE FROM page_images WHERE id = ?1", [id])
            .map_err(not_saved)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/pages.rs"]
mod tests;
