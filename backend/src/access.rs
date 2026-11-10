use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use log::warn;
use rusqlite::types::Type;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::accounts::{Identity, Platform};
use crate::database::Database;
use crate::deliveries::now;

const RESTRICTED: &str = "restrictSignIn";

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub platform: Platform,
    pub user_id: String,
    pub login: String,
    pub name: String,
    pub avatar: Option<String>,
    pub at: u64,
}

#[derive(Debug, Default, PartialEq, Serialize)]
pub struct SignInState {
    pub restricted: bool,
    pub allowed: Vec<Vec<Person>>,
    pub requests: Vec<Person>,
}

pub struct SignInAccess {
    database: Arc<Database>,
    restricted: AtomicBool,
}

fn person(row: &rusqlite::Row) -> rusqlite::Result<Person> {
    let platform = Platform::parse(&row.get::<_, String>(0)?)
        .ok_or_else(|| rusqlite::Error::InvalidColumnType(0, "platform".to_string(), Type::Text))?;
    Ok(Person {
        platform,
        user_id: row.get(1)?,
        login: row.get(2)?,
        name: row.get(3)?,
        avatar: row.get(4)?,
        at: row.get(5)?,
    })
}

fn requests(connection: &Connection) -> rusqlite::Result<Vec<Person>> {
    connection
        .prepare_cached(
            "SELECT platform, user_id, login, name, avatar, at FROM sign_in_requests
             ORDER BY at DESC, login",
        )?
        .query_map([], person)?
        .collect()
}

fn allowed(connection: &Connection) -> rusqlite::Result<Vec<Vec<Person>>> {
    let mut groups: Vec<(Option<String>, Vec<Person>)> = Vec::new();
    for row in connection
        .prepare_cached(
            "SELECT allowed.platform, allowed.user_id, allowed.login, allowed.name,
                    allowed.avatar, allowed.at, identities.account
             FROM allowed_identities AS allowed LEFT JOIN identities
             ON identities.platform = allowed.platform AND identities.user_id = allowed.user_id
             ORDER BY allowed.at DESC, allowed.login",
        )?
        .query_map([], |row| {
            Ok((person(row)?, row.get::<_, Option<String>>(6)?))
        })?
    {
        let (person, account) = row?;
        match groups
            .iter_mut()
            .find(|(group, _)| account.is_some() && *group == account)
        {
            Some((_, people)) => people.push(person),
            None => groups.push((account, vec![person])),
        }
    }
    Ok(groups.into_iter().map(|(_, people)| people).collect())
}

impl SignInAccess {
    pub fn new(database: Arc<Database>) -> Self {
        let restricted = database
            .lock()
            .query_row(
                "SELECT value FROM server_settings WHERE name = ?1",
                [RESTRICTED],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .unwrap_or_else(|error| {
                warn!("failed to read who may sign in, only letting the allowed: {error:#}");
                Some("true".to_string())
            })
            .is_some_and(|value| value == "true");
        Self {
            database,
            restricted: AtomicBool::new(restricted),
        }
    }

    pub fn restricted(&self) -> bool {
        self.restricted.load(Ordering::Relaxed)
    }

    pub fn set_restricted(&self, restricted: bool) -> rusqlite::Result<()> {
        let mut database = self.database.lock();
        let transaction = database.transaction()?;
        let first = transaction
            .query_row(
                "SELECT 1 FROM server_settings WHERE name = ?1",
                [RESTRICTED],
                |_| Ok(()),
            )
            .optional()?
            .is_none();
        if !restricted && first {
            return Ok(());
        }
        if first {
            transaction.execute(
                "INSERT OR IGNORE INTO allowed_identities (platform, user_id, login, name, avatar, at)
                 SELECT platform, user_id, login, name, avatar, ?1 FROM identities",
                [now()],
            )?;
        }
        transaction.execute(
            "INSERT OR REPLACE INTO server_settings (name, value) VALUES (?1, ?2)",
            [RESTRICTED, if restricted { "true" } else { "false" }],
        )?;
        transaction.commit()?;
        self.restricted.store(restricted, Ordering::Relaxed);
        Ok(())
    }

    pub fn allows(&self, identities: &[Identity]) -> bool {
        let database = self.database.lock();
        identities.iter().any(|identity| {
            database
                .query_row(
                    "SELECT 1 FROM allowed_identities WHERE platform = ?1 AND user_id = ?2",
                    [identity.platform.name(), &identity.user_id],
                    |_| Ok(()),
                )
                .optional()
                .unwrap_or_else(|error| {
                    warn!("failed to read the allowed identities: {error:#}");
                    None
                })
                .is_some()
        })
    }

    pub fn request(&self, identity: &Identity) -> rusqlite::Result<()> {
        let mut database = self.database.lock();
        let transaction = database.transaction()?;
        transaction.execute(
            "INSERT OR REPLACE INTO sign_in_requests (platform, user_id, login, name, avatar, at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                identity.platform.name(),
                identity.user_id,
                identity.login,
                identity.name,
                identity.avatar,
                now(),
            ],
        )?;
        transaction.execute(
            "DELETE FROM sign_in_requests WHERE rowid NOT IN
             (SELECT rowid FROM sign_in_requests ORDER BY at DESC, rowid DESC LIMIT 100)",
            [],
        )?;
        transaction.commit()
    }

    pub fn allow(&self, platform: Platform, user_id: &str) -> rusqlite::Result<bool> {
        let mut database = self.database.lock();
        let transaction = database.transaction()?;
        let added = transaction.execute(
            "INSERT OR REPLACE INTO allowed_identities (platform, user_id, login, name, avatar, at)
             SELECT platform, user_id, login, name, avatar, ?3 FROM (
                 SELECT platform, user_id, login, name, avatar, 0 AS rank FROM sign_in_requests
                 WHERE platform = ?1 AND user_id = ?2
                 UNION ALL
                 SELECT platform, user_id, login, name, avatar, 1 FROM identities
                 WHERE platform = ?1 AND user_id = ?2
                 ORDER BY rank LIMIT 1
             )",
            params![platform.name(), user_id, now()],
        )?;
        transaction.execute(
            "DELETE FROM sign_in_requests WHERE platform = ?1 AND user_id = ?2",
            [platform.name(), user_id],
        )?;
        transaction.commit()?;
        Ok(added > 0)
    }

    pub fn remove(&self, platform: Platform, user_id: &str) -> rusqlite::Result<()> {
        self.database.lock().execute(
            "DELETE FROM allowed_identities WHERE platform = ?1 AND user_id = ?2
             OR (platform, user_id) IN (
                 SELECT platform, user_id FROM identities WHERE account =
                     (SELECT account FROM identities WHERE platform = ?1 AND user_id = ?2)
             )",
            [platform.name(), user_id],
        )?;
        Ok(())
    }

    pub fn dismiss(&self, platform: Platform, user_id: &str) -> rusqlite::Result<()> {
        self.database.lock().execute(
            "DELETE FROM sign_in_requests WHERE platform = ?1 AND user_id = ?2",
            [platform.name(), user_id],
        )?;
        Ok(())
    }

    pub fn state(&self) -> SignInState {
        let database = self.database.lock();
        SignInState {
            restricted: self.restricted(),
            allowed: allowed(&database).unwrap_or_else(|error| {
                warn!("failed to read the allowed identities: {error:#}");
                Vec::new()
            }),
            requests: requests(&database).unwrap_or_else(|error| {
                warn!("failed to read the sign in requests: {error:#}");
                Vec::new()
            }),
        }
    }
}

#[cfg(test)]
#[path = "tests/access.rs"]
mod tests;
