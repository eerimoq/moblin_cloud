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
                 PRAGMA synchronous = FULL;
                 DROP TABLE IF EXISTS moderators;
                 DROP TABLE IF EXISTS moderated_channels;
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
                 CREATE INDEX IF NOT EXISTS accounts_login ON accounts (login);
                 CREATE TABLE IF NOT EXISTS devices (
                     key TEXT PRIMARY KEY,
                     account TEXT NOT NULL,
                     id TEXT NOT NULL,
                     name TEXT NOT NULL,
                     position INTEGER NOT NULL,
                     enabled INTEGER NOT NULL DEFAULT 1
                 );
                 CREATE INDEX IF NOT EXISTS devices_account ON devices (account);
                 CREATE TABLE IF NOT EXISTS channel_moderators (
                     broadcaster TEXT NOT NULL,
                     moderator TEXT NOT NULL,
                     login TEXT NOT NULL,
                     name TEXT NOT NULL,
                     PRIMARY KEY (broadcaster, moderator)
                 );
                 CREATE TABLE IF NOT EXISTS enabled_moderators (
                     broadcaster TEXT NOT NULL,
                     moderator TEXT NOT NULL,
                     PRIMARY KEY (broadcaster, moderator)
                 );
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
                 );",
            )
            .context("failed to create the database tables")?;
        for (table, column, definition) in [
            ("accounts", "stripe_account_id", "TEXT"),
            ("accounts", "stripe_ready", "INTEGER NOT NULL DEFAULT 0"),
            ("accounts", "music_device", "TEXT"),
            ("accounts", "wheel_device", "TEXT"),
            ("devices", "enabled", "INTEGER NOT NULL DEFAULT 1"),
            ("deliveries", "delivered", "TEXT NOT NULL DEFAULT '[]'"),
            ("deliveries", "failure", "TEXT"),
            ("deliveries", "finished", "INTEGER NOT NULL DEFAULT 0"),
            ("deliveries", "test", "TEXT"),
            ("deliveries", "resends", "INTEGER NOT NULL DEFAULT 0"),
            ("orders", "kind", "TEXT"),
        ] {
            if !has_column(&connection, table, column)? {
                connection
                    .execute_batch(&format!(
                        "ALTER TABLE {table} ADD COLUMN {column} {definition}"
                    ))
                    .with_context(|| format!("failed to add {column} to the {table} table"))?;
            }
        }
        connection
            .execute_batch(
                "UPDATE orders SET kind = CASE
                     WHEN json_extract(details, '$.item.Song') IS NOT NULL THEN 'song'
                     WHEN json_extract(details, '$.item.Tip') IS NOT NULL THEN 'tip'
                     ELSE 'spin'
                 END WHERE kind IS NULL;
                 CREATE INDEX IF NOT EXISTS orders_recent ON orders (streamer, kind, state, updated_at);
                 CREATE INDEX IF NOT EXISTS deliveries_streamer ON deliveries (streamer, test);
                 CREATE INDEX IF NOT EXISTS deliveries_open ON deliveries (requested_at) WHERE NOT finished AND failure IS NOT 'not-found';",
            )
            .context("failed to index the orders and deliveries")?;
        if has_column(&connection, "accounts", "page_devices")? {
            connection
                .execute_batch(
                    "INSERT OR IGNORE INTO devices (key, account, id, name, position)
                     SELECT value ->> 'key', accounts.id, value ->> 'id', value ->> 'name', json_each.key
                     FROM accounts, json_each(accounts.page_devices);
                     ALTER TABLE accounts DROP COLUMN page_devices;",
                )
                .context("failed to move the Moblin devices to their own table")?;
        }
        if has_column(&connection, "accounts", "page_key")? {
            connection
                .execute_batch(
                    "DROP INDEX IF EXISTS accounts_page_key;
                     ALTER TABLE accounts DROP COLUMN page_key;",
                )
                .context("failed to remove the old Moblin Cloud keys")?;
        }
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

fn has_column(connection: &Connection, table: &str, column: &str) -> Result<bool> {
    connection
        .query_row(
            "SELECT count(*) > 0 FROM pragma_table_info(?1) WHERE name = ?2",
            [table, column],
            |row| row.get(0),
        )
        .with_context(|| format!("failed to read the {table} table"))
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

    #[test]
    fn the_moderated_channels_tables_are_dropped() {
        let path = temporary_path("moderators");
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE moderators (moderator TEXT NOT NULL, broadcaster TEXT NOT NULL);
                 CREATE TABLE moderated_channels (moderator TEXT NOT NULL, broadcaster TEXT NOT NULL);",
            )
            .unwrap();
        let database = Database::open(Some(&path)).unwrap();
        assert!(!has_column(&database.lock(), "moderators", "broadcaster").unwrap());
        assert!(!has_column(&database.lock(), "moderated_channels", "broadcaster").unwrap());
        drop(database);
        remove(&path);
    }
}
