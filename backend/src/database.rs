use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::{Context, Result};
use rusqlite::Connection;

pub struct Database {
    connection: Mutex<Connection>,
    path: Option<PathBuf>,
}

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
                 PRAGMA synchronous = NORMAL;
                 CREATE TABLE IF NOT EXISTS accounts (
                     id TEXT PRIMARY KEY,
                     page_id TEXT,
                     moblin_key TEXT,
                     sessions INTEGER NOT NULL,
                     merchant_id TEXT,
                     paypal_tracking_id TEXT,
                     stripe_account_id TEXT,
                     stripe_ready INTEGER NOT NULL DEFAULT 0
                 );
                 CREATE INDEX IF NOT EXISTS accounts_page_id ON accounts (page_id);
                 CREATE UNIQUE INDEX IF NOT EXISTS accounts_moblin_key ON accounts (moblin_key);
                 CREATE TABLE IF NOT EXISTS devices (
                     account TEXT NOT NULL,
                     id TEXT NOT NULL,
                     name TEXT NOT NULL,
                     last_seen INTEGER NOT NULL,
                     PRIMARY KEY (account, id)
                 );
                 CREATE TABLE IF NOT EXISTS twitch_assistants (
                     broadcaster TEXT NOT NULL,
                     assistant TEXT NOT NULL,
                     login TEXT NOT NULL,
                     name TEXT NOT NULL,
                     preview INTEGER NOT NULL DEFAULT 0,
                     go_live INTEGER NOT NULL DEFAULT 0,
                     edit_tips INTEGER NOT NULL DEFAULT 0,
                     remote_control INTEGER NOT NULL DEFAULT 0,
                     PRIMARY KEY (broadcaster, assistant)
                 );
                 CREATE INDEX IF NOT EXISTS twitch_assistants_assistant ON twitch_assistants (assistant);
                 CREATE TABLE IF NOT EXISTS pages (
                     id TEXT PRIMARY KEY,
                     settings TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS page_images (
                     id TEXT PRIMARY KEY,
                     version TEXT NOT NULL,
                     data BLOB NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS page_icons (
                     id TEXT PRIMARY KEY,
                     icon TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS monitor_layouts (
                     account TEXT NOT NULL,
                     page TEXT NOT NULL,
                     layout TEXT NOT NULL,
                     updated_at INTEGER NOT NULL,
                     PRIMARY KEY (account, page)
                 );
                 CREATE INDEX IF NOT EXISTS monitor_layouts_page ON monitor_layouts (page);
                 CREATE TABLE IF NOT EXISTS orders (
                     id TEXT PRIMARY KEY,
                     streamer TEXT NOT NULL,
                     state TEXT NOT NULL,
                     created_at INTEGER NOT NULL,
                     updated_at INTEGER NOT NULL,
                     details TEXT NOT NULL,
                     kind TEXT
                 );
                 CREATE INDEX IF NOT EXISTS orders_created ON orders (created_at) WHERE state = 'created';
                 CREATE INDEX IF NOT EXISTS orders_capturing ON orders (updated_at) WHERE state = 'capturing';
                 CREATE TABLE IF NOT EXISTS deliveries (
                     request_id TEXT PRIMARY KEY,
                     streamer TEXT NOT NULL,
                     requested_at INTEGER NOT NULL,
                     item TEXT NOT NULL,
                     delivered TEXT NOT NULL DEFAULT '[]',
                     failure TEXT,
                     finished INTEGER NOT NULL DEFAULT 0,
                     test TEXT,
                     resends INTEGER NOT NULL DEFAULT 0
                 );
                 CREATE TABLE IF NOT EXISTS subscriber_allowances (
                     page TEXT NOT NULL,
                     viewer TEXT NOT NULL,
                     kind TEXT NOT NULL,
                     month TEXT NOT NULL,
                     count INTEGER NOT NULL,
                     PRIMARY KEY (page, viewer, kind, month)
                 );
                 CREATE TABLE IF NOT EXISTS counters (
                     name TEXT NOT NULL,
                     day TEXT NOT NULL,
                     value INTEGER NOT NULL,
                     PRIMARY KEY (name, day)
                 );
                 CREATE INDEX IF NOT EXISTS orders_recent ON orders (streamer, kind, state, updated_at);
                 CREATE INDEX IF NOT EXISTS deliveries_streamer ON deliveries (streamer, test);
                 CREATE INDEX IF NOT EXISTS deliveries_unfinished ON deliveries (requested_at) WHERE NOT finished;
                 CREATE TABLE IF NOT EXISTS identities (
                     platform TEXT NOT NULL,
                     user_id TEXT NOT NULL,
                     account TEXT NOT NULL,
                     login TEXT NOT NULL,
                     name TEXT NOT NULL,
                     avatar TEXT,
                     PRIMARY KEY (platform, user_id)
                 );
                 CREATE INDEX IF NOT EXISTS identities_account ON identities (account);
                 CREATE INDEX IF NOT EXISTS identities_login ON identities (platform, login);
                 CREATE TABLE IF NOT EXISTS kick_assistants (
                     broadcaster TEXT NOT NULL,
                     assistant TEXT NOT NULL,
                     login TEXT NOT NULL,
                     name TEXT NOT NULL,
                     preview INTEGER NOT NULL DEFAULT 0,
                     go_live INTEGER NOT NULL DEFAULT 0,
                     edit_tips INTEGER NOT NULL DEFAULT 0,
                     remote_control INTEGER NOT NULL DEFAULT 0,
                     PRIMARY KEY (broadcaster, assistant)
                 );
                 CREATE INDEX IF NOT EXISTS kick_assistants_assistant ON kick_assistants (assistant);",
            )
            .context("failed to create the database tables")?;
        connection.set_prepared_statement_cache_capacity(64);
        Ok(Arc::new(Self {
            connection: Mutex::new(connection),
            path: path.map(Path::to_path_buf),
        }))
    }

    #[cfg(test)]
    pub fn memory() -> Arc<Self> {
        Self::open(None).unwrap()
    }

    pub fn lock(&self) -> MutexGuard<'_, Connection> {
        self.connection.lock().unwrap()
    }

    pub fn files(&self) -> Vec<(String, u64)> {
        let Some(path) = &self.path else {
            return vec![];
        };
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        ["", "-wal", "-shm"]
            .into_iter()
            .filter_map(|suffix| {
                let metadata = std::fs::metadata(format!("{}{suffix}", path.display())).ok()?;
                Some((format!("{name}{suffix}"), metadata.len()))
            })
            .collect()
    }
}

#[cfg(test)]
pub fn temporary_path(name: &str) -> std::path::PathBuf {
    let path =
        std::env::temp_dir().join(format!("moblin-cloud-{name}-{}.sqlite", std::process::id()));
    remove(&path);
    path
}

#[cfg(test)]
pub fn remove(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_the_files_on_disk() {
        assert!(Database::memory().files().is_empty());
        let path = temporary_path("size");
        let database = Database::open(Some(&path)).unwrap();
        database
            .lock()
            .execute("INSERT INTO pages (id, settings) VALUES ('a', '{}')", [])
            .unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let expected: Vec<(String, u64)> = ["", "-wal", "-shm"]
            .into_iter()
            .filter_map(|suffix| {
                let metadata = std::fs::metadata(format!("{}{suffix}", path.display())).ok()?;
                Some((format!("{name}{suffix}"), metadata.len()))
            })
            .collect();
        assert!(expected.len() > 1);
        assert_eq!(database.files(), expected);
        drop(database);
        remove(&path);
    }
}
