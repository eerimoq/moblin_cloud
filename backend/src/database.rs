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
                     page_devices TEXT,
                     music_device TEXT,
                     wheel_device TEXT,
                     sessions INTEGER NOT NULL,
                     merchant_id TEXT,
                     paypal_tracking_id TEXT,
                     stripe_account_id TEXT,
                     stripe_ready INTEGER NOT NULL DEFAULT 0
                 );
                 CREATE INDEX IF NOT EXISTS accounts_page_id ON accounts (page_id);
                 CREATE TABLE IF NOT EXISTS pages (
                     id TEXT PRIMARY KEY,
                     settings TEXT NOT NULL
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
                 CREATE TABLE IF NOT EXISTS deliveries (
                     request_id TEXT PRIMARY KEY,
                     streamer TEXT NOT NULL,
                     requested_at INTEGER NOT NULL,
                     item TEXT NOT NULL,
                     delivered TEXT NOT NULL DEFAULT '[]',
                     failure TEXT,
                     finished INTEGER NOT NULL DEFAULT 0,
                     test TEXT
                 );
                 CREATE TABLE IF NOT EXISTS streamers (
                     position INTEGER PRIMARY KEY,
                     channels TEXT NOT NULL
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
            ("accounts", "page_devices", "TEXT"),
            ("accounts", "music_device", "TEXT"),
            ("accounts", "wheel_device", "TEXT"),
            ("deliveries", "delivered", "TEXT NOT NULL DEFAULT '[]'"),
            ("deliveries", "failure", "TEXT"),
            ("deliveries", "finished", "INTEGER NOT NULL DEFAULT 0"),
            ("deliveries", "test", "TEXT"),
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
                 CREATE INDEX IF NOT EXISTS orders_recent ON orders (streamer, kind, state, updated_at);",
            )
            .context("failed to index the orders")?;
        if has_column(&connection, "accounts", "page_key")? {
            connection
                .execute_batch(
                    "DROP INDEX IF EXISTS accounts_page_key;
                     ALTER TABLE accounts DROP COLUMN page_key;",
                )
                .context("failed to remove the old Moblin Cloud keys")?;
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
