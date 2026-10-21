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
        let old_moderators: u32 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table'
                 AND name IN ('enabled_twitch_moderators', 'enabled_moderators', 'channel_moderators')",
                [],
                |row| row.get(0),
            )
            .context("failed to read the database tables")?;
        if old_moderators > 0 {
            connection
                .execute_batch(
                    "DROP TABLE IF EXISTS twitch_moderators;
                     DROP TABLE IF EXISTS enabled_twitch_moderators;
                     DROP TABLE IF EXISTS channel_moderators;
                     DROP TABLE IF EXISTS enabled_moderators;",
                )
                .context("failed to drop the old moderator tables")?;
        }
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;
                 CREATE TABLE IF NOT EXISTS accounts (
                     id TEXT PRIMARY KEY,
                     name TEXT NOT NULL,
                     login TEXT NOT NULL,
                     avatar TEXT,
                     page_id TEXT,
                     music_device TEXT,
                     wheel_device TEXT,
                     sessions INTEGER NOT NULL,
                     merchant_id TEXT,
                     paypal_tracking_id TEXT,
                     stripe_account_id TEXT,
                     stripe_ready INTEGER NOT NULL DEFAULT 0
                 );
                 CREATE INDEX IF NOT EXISTS accounts_page_id ON accounts (page_id);
                 DROP INDEX IF EXISTS accounts_login;
                 CREATE TABLE IF NOT EXISTS devices (
                     key TEXT PRIMARY KEY,
                     account TEXT NOT NULL,
                     id TEXT NOT NULL,
                     name TEXT NOT NULL,
                     position INTEGER NOT NULL,
                     enabled INTEGER NOT NULL DEFAULT 1
                 );
                 CREATE INDEX IF NOT EXISTS devices_account ON devices (account);
                 CREATE TABLE IF NOT EXISTS twitch_moderators (
                     broadcaster TEXT NOT NULL,
                     moderator TEXT NOT NULL,
                     login TEXT NOT NULL,
                     name TEXT NOT NULL,
                     PRIMARY KEY (broadcaster, moderator)
                 );
                 CREATE INDEX IF NOT EXISTS twitch_moderators_moderator ON twitch_moderators (moderator);
                 CREATE TABLE IF NOT EXISTS pages (
                     id TEXT PRIMARY KEY,
                     settings TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS page_images (
                     id TEXT PRIMARY KEY,
                     version TEXT NOT NULL,
                     data BLOB NOT NULL
                 );
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
                 CREATE TABLE IF NOT EXISTS streamers (
                     position INTEGER PRIMARY KEY,
                     channels TEXT NOT NULL
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
                 CREATE INDEX IF NOT EXISTS deliveries_open ON deliveries (requested_at) WHERE NOT finished AND failure IS NOT 'not-found';
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
                 CREATE TABLE IF NOT EXISTS kick_moderators (
                     broadcaster TEXT NOT NULL,
                     moderator TEXT NOT NULL,
                     login TEXT NOT NULL,
                     name TEXT NOT NULL,
                     PRIMARY KEY (broadcaster, moderator)
                 );
                 CREATE INDEX IF NOT EXISTS kick_moderators_moderator ON kick_moderators (moderator);
                 INSERT OR IGNORE INTO identities (platform, user_id, account, login, name, avatar)
                     SELECT 'twitch', id, id, login, name, avatar FROM accounts
                     WHERE id NOT IN (SELECT account FROM identities);
                 BEGIN;
                 CREATE TEMP TABLE renamed_accounts AS
                     SELECT id AS old, lower(hex(randomblob(16))) AS new FROM accounts
                     WHERE length(id) != 32 OR id IS NOT lower(hex(unhex(id)));
                 UPDATE accounts SET id = (SELECT new FROM renamed_accounts WHERE old = accounts.id)
                     WHERE id IN (SELECT old FROM renamed_accounts);
                 UPDATE devices SET account = (SELECT new FROM renamed_accounts WHERE old = devices.account)
                     WHERE account IN (SELECT old FROM renamed_accounts);
                 UPDATE identities SET account = (SELECT new FROM renamed_accounts WHERE old = identities.account)
                     WHERE account IN (SELECT old FROM renamed_accounts);
                 DROP TABLE renamed_accounts;
                 COMMIT;",
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

    pub fn size(&self) -> u64 {
        let Some(path) = &self.path else {
            return 0;
        };
        ["", "-wal", "-shm"]
            .into_iter()
            .filter_map(|suffix| std::fs::metadata(format!("{}{suffix}", path.display())).ok())
            .map(|metadata| metadata.len())
            .sum()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_twitch_moderator_tables_are_dropped_on_start() {
        let path = temporary_path("moderator-tables");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE channel_moderators (broadcaster TEXT NOT NULL, moderator TEXT NOT NULL, login TEXT NOT NULL, name TEXT NOT NULL, PRIMARY KEY (broadcaster, moderator));
                 CREATE INDEX channel_moderators_moderator ON channel_moderators (moderator);
                 CREATE TABLE enabled_moderators (broadcaster TEXT NOT NULL, moderator TEXT NOT NULL, PRIMARY KEY (broadcaster, moderator));
                 CREATE TABLE twitch_moderators (broadcaster TEXT NOT NULL, moderator TEXT NOT NULL, login TEXT NOT NULL, name TEXT NOT NULL, PRIMARY KEY (broadcaster, moderator));
                 CREATE INDEX twitch_moderators_moderator ON twitch_moderators (moderator);
                 CREATE TABLE enabled_twitch_moderators (broadcaster TEXT NOT NULL, moderator TEXT NOT NULL, PRIMARY KEY (broadcaster, moderator));
                 CREATE TABLE kick_moderators (broadcaster TEXT NOT NULL, moderator TEXT NOT NULL, login TEXT NOT NULL, name TEXT NOT NULL, PRIMARY KEY (broadcaster, moderator));
                 INSERT INTO channel_moderators VALUES ('1', '2', 'mod', 'Mod');
                 INSERT INTO enabled_moderators VALUES ('1', '2');
                 INSERT INTO twitch_moderators VALUES ('1', '3', 'other', 'Other');
                 INSERT INTO enabled_twitch_moderators VALUES ('1', '3');
                 INSERT INTO kick_moderators VALUES ('4', '5', 'kickmod', 'KickMod');",
            )
            .unwrap();
        drop(connection);
        let count = |connection: &Connection, sql: &str| -> u32 {
            connection.query_row(sql, [], |row| row.get(0)).unwrap()
        };
        {
            let database = Database::open(Some(&path)).unwrap();
            let connection = database.lock();
            assert_eq!(
                count(&connection, "SELECT count(*) FROM twitch_moderators"),
                0
            );
            assert_eq!(
                count(&connection, "SELECT count(*) FROM kick_moderators"),
                1
            );
            assert_eq!(
                count(
                    &connection,
                    "SELECT count(*) FROM sqlite_master WHERE name IN ('channel_moderators', 'enabled_moderators', 'channel_moderators_moderator', 'enabled_twitch_moderators')"
                ),
                0
            );
            connection
                .execute_batch("INSERT INTO twitch_moderators VALUES ('1', '6', 'new', 'New')")
                .unwrap();
        }
        let database = Database::open(Some(&path)).unwrap();
        assert_eq!(
            count(&database.lock(), "SELECT count(*) FROM twitch_moderators"),
            1
        );
        drop(database);
        remove(&path);
    }

    #[test]
    fn size_is_the_files_on_disk() {
        assert_eq!(Database::memory().size(), 0);
        let path = temporary_path("size");
        let database = Database::open(Some(&path)).unwrap();
        database
            .lock()
            .execute("INSERT INTO pages (id, settings) VALUES ('a', '{}')", [])
            .unwrap();
        let expected: u64 = ["", "-wal", "-shm"]
            .into_iter()
            .filter_map(|suffix| std::fs::metadata(format!("{}{suffix}", path.display())).ok())
            .map(|metadata| metadata.len())
            .sum();
        assert!(expected > std::fs::metadata(&path).unwrap().len());
        assert_eq!(database.size(), expected);
        drop(database);
        remove(&path);
    }
}
