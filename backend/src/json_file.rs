use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub fn load<T: DeserializeOwned + Default>(path: Option<&Path>) -> Result<T> {
    let Some(path) = path else {
        return Ok(T::default());
    };
    match fs::read(path) {
        Ok(data) => serde_json::from_slice(&data)
            .with_context(|| format!("failed to parse {}", path.display())),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(T::default()),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

pub fn save(path: Option<&Path>, value: &impl Serialize) -> Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, serde_json::to_vec(value)?)
        .with_context(|| format!("failed to write {}", temporary.display()))?;
    fs::rename(&temporary, path)
        .with_context(|| format!("failed to rename to {}", path.display()))?;
    Ok(())
}
