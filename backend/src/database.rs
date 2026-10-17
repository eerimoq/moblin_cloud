use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::{Context, Result};
use rusqlite::Connection;

pub struct Database(Mutex<Connection>);

impl Database {
    pub fn open(path: Option<&Path>) -> Result<Arc<Self>> {
        let connection = match path {
            Some(path) => Connection::open(path)
                .with_context(|| format!("failed to open {}", path.display()))?,
            None => Connection::open_in_memory()?,
        };
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = FULL;
                 CREATE TABLE IF NOT EXISTS accounts (
                     id TEXT PRIMARY KEY,
                     name TEXT NOT NULL,
                     login TEXT NOT NULL,
                     avatar TEXT,
                     page_id TEXT,
                     page_key TEXT,
                     sessions INTEGER NOT NULL,
                     merchant_id TEXT,
                     paypal_tracking_id TEXT,
                     stripe_account_id TEXT,
                     stripe_ready INTEGER NOT NULL DEFAULT 0
                 );
                 CREATE INDEX IF NOT EXISTS accounts_page_id ON accounts (page_id);
                 CREATE INDEX IF NOT EXISTS accounts_page_key ON accounts (page_key);
                 CREATE TABLE IF NOT EXISTS pages (
                     id TEXT PRIMARY KEY,
                     settings TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS deliveries (
                     request_id TEXT PRIMARY KEY,
                     streamer TEXT NOT NULL,
                     requested_at INTEGER NOT NULL,
                     item TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS streamers (
                     position INTEGER PRIMARY KEY,
                     channels TEXT NOT NULL
                 );",
            )
            .context("failed to create the database tables")?;
        for (column, definition) in [
            ("stripe_account_id", "TEXT"),
            ("stripe_ready", "INTEGER NOT NULL DEFAULT 0"),
        ] {
            let exists: bool = connection
                .query_row(
                    "SELECT count(*) > 0 FROM pragma_table_info('accounts') WHERE name = ?1",
                    [column],
                    |row| row.get(0),
                )
                .context("failed to read the accounts table")?;
            if !exists {
                connection
                    .execute_batch(&format!(
                        "ALTER TABLE accounts ADD COLUMN {column} {definition}"
                    ))
                    .with_context(|| format!("failed to add {column} to the accounts table"))?;
            }
        }
        Ok(Arc::new(Self(Mutex::new(connection))))
    }

    #[cfg(test)]
    pub fn memory() -> Arc<Self> {
        Self::open(None).unwrap()
    }

    pub fn lock(&self) -> MutexGuard<'_, Connection> {
        self.0.lock().unwrap()
    }
}

#[cfg(test)]
pub fn temporary_path(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "moblin-website-{name}-{}.sqlite",
        std::process::id()
    ));
    remove(&path);
    path
}

#[cfg(test)]
pub fn remove(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}
