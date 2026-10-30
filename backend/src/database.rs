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
        let columns = |table: &str, column: &str| -> Result<bool> {
            let count: u32 = connection
                .query_row(
                    "SELECT count(*) FROM pragma_table_info(?1) WHERE name = ?2",
                    [table, column],
                    |row| row.get(0),
                )
                .with_context(|| format!("failed to read the {table} columns"))?;
            Ok(count > 0)
        };
        for platform in ["twitch", "kick"] {
            if columns(&format!("{platform}_moderators"), "moderator")? {
                connection
                    .execute_batch(&format!(
                        "ALTER TABLE {platform}_moderators RENAME TO {platform}_assistants;
                         ALTER TABLE {platform}_assistants RENAME COLUMN moderator TO assistant;
                         DROP INDEX IF EXISTS {platform}_moderators_moderator;"
                    ))
                    .with_context(|| format!("failed to rename the {platform} moderators"))?;
            }
        }
        for table in ["twitch_assistants", "kick_assistants"] {
            for column in ["preview", "go_live", "edit_tips", "remote_control"] {
                if columns(table, "broadcaster")? && !columns(table, column)? {
                    connection
                        .execute_batch(&format!(
                            "ALTER TABLE {table} ADD COLUMN {column} INTEGER NOT NULL DEFAULT 0"
                        ))
                        .with_context(|| format!("failed to add the {table} {column} column"))?;
                }
            }
        }
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
    fn old_moderators_become_assistants_without_permissions_on_start() {
        let path = temporary_path("moderators-renamed");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE twitch_moderators (
                     broadcaster TEXT NOT NULL,
                     moderator TEXT NOT NULL,
                     login TEXT NOT NULL,
                     name TEXT NOT NULL,
                     PRIMARY KEY (broadcaster, moderator)
                 );
                 CREATE INDEX twitch_moderators_moderator ON twitch_moderators (moderator);
                 INSERT INTO twitch_moderators VALUES ('42', '1', 'mod', 'Mod');",
            )
            .unwrap();
        drop(connection);
        for _ in 0..2 {
            let database = Database::open(Some(&path)).unwrap();
            let connection = database.lock();
            for table in ["twitch_assistants", "kick_assistants"] {
                let columns: u32 = connection
                    .query_row(
                        "SELECT count(*) FROM pragma_table_info(?1) WHERE name IN ('preview', 'go_live', 'edit_tips', 'remote_control')",
                        [table],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(columns, 4);
            }
            let moderators: u32 = connection
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name LIKE '%moderator%'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(moderators, 0);
            let (preview, go_live, edit_tips, remote_control): (bool, bool, bool, bool) = connection
                .query_row(
                    "SELECT preview, go_live, edit_tips, remote_control FROM twitch_assistants WHERE assistant = '1'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .unwrap();
            assert!(!preview);
            assert!(!go_live);
            assert!(!edit_tips);
            assert!(!remote_control);
        }
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
