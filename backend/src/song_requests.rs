use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{Context, Result};
use log::warn;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub enabled: bool,
    pub name: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            name: None,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum SettingsError {
    InvalidName,
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
        let settings = match &path {
            Some(path) => match fs::read(path) {
                Ok(data) => serde_json::from_slice(&data)
                    .with_context(|| format!("failed to parse {}", path.display()))?,
                Err(error) if error.kind() == ErrorKind::NotFound => HashMap::new(),
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to read {}", path.display()));
                }
            },
            None => HashMap::new(),
        };
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
        enabled: bool,
        name: Option<&str>,
    ) -> Result<Settings, SettingsError> {
        let name = match name.map(str::trim).filter(|name| !name.is_empty()) {
            Some(name) => Some(normalize_name(name).ok_or(SettingsError::InvalidName)?),
            None => None,
        };
        let new = Settings { enabled, name };
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
        if let Err(error) = self.save(&settings) {
            warn!("failed to save song request settings: {error:#}");
            match old {
                Some(old) => settings.insert(id.to_string(), old),
                None => settings.remove(id),
            };
            return Err(SettingsError::NotSaved);
        }
        Ok(new)
    }

    fn save(&self, settings: &HashMap<String, Settings>) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, serde_json::to_vec(settings)?)
            .with_context(|| format!("failed to write {}", temporary.display()))?;
        fs::rename(&temporary, path)
            .with_context(|| format!("failed to rename to {}", path.display()))?;
        Ok(())
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
            song_requests.set("a", false, Some("Erik")),
            Ok(Settings {
                enabled: false,
                name: Some("erik".to_string())
            })
        );
        assert_eq!(song_requests.id_of_name("ERIK").as_deref(), Some("a"));
        assert_eq!(
            song_requests.set("b", true, Some("erik")),
            Err(SettingsError::NameTaken)
        );
        assert_eq!(
            song_requests.set("b", true, Some("e r")),
            Err(SettingsError::InvalidName)
        );
        assert!(song_requests.set("a", true, Some("erik")).is_ok());
        assert!(song_requests.set("a", true, Some(" ")).is_ok());
        assert_eq!(song_requests.id_of_name("erik"), None);
        assert!(song_requests.set("b", true, Some("erik")).is_ok());
        assert_eq!(song_requests.id_of_name("erik").as_deref(), Some("b"));
    }

    #[test]
    fn settings_are_saved() {
        let path = std::env::temp_dir().join(format!(
            "song-requests-{}-{:?}.json",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_file(&path);
        let song_requests = SongRequests::load(Some(path.clone())).unwrap();
        song_requests.set("a", false, Some("erik")).unwrap();
        song_requests.set("b", false, None).unwrap();
        song_requests.set("b", true, None).unwrap();
        let song_requests = SongRequests::load(Some(path.clone())).unwrap();
        assert_eq!(song_requests.id_of_name("erik").as_deref(), Some("a"));
        assert!(!song_requests.get("a").enabled);
        assert_eq!(song_requests.get("b"), Settings::default());
        assert_eq!(song_requests.settings.lock().unwrap().len(), 1);
        fs::remove_file(&path).unwrap();
    }
}
