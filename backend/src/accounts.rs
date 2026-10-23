use std::collections::HashMap;
use std::sync::Arc;

use log::warn;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use crate::database::Database;
use crate::deliveries::now;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Twitch,
    Kick,
}

impl Platform {
    pub fn name(self) -> &'static str {
        match self {
            Platform::Twitch => "twitch",
            Platform::Kick => "kick",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Platform::Twitch => "Twitch",
            Platform::Kick => "Kick",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "twitch" => Some(Platform::Twitch),
            "kick" => Some(Platform::Kick),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Identity {
    pub platform: Platform,
    pub user_id: String,
    pub login: String,
    pub name: String,
    pub avatar: Option<String>,
}

impl Identity {
    pub fn admin_key(&self) -> String {
        match self.platform {
            Platform::Twitch => self.user_id.clone(),
            Platform::Kick => format!("kick:{}", self.user_id),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Account {
    pub identities: Vec<Identity>,
    pub page: Page,
    pub sessions: u64,
    pub merchant_id: Option<String>,
    pub paypal_tracking_id: Option<String>,
    pub stripe_account_id: Option<String>,
    pub stripe_ready: bool,
}

impl Account {
    pub fn name(&self) -> &str {
        self.identities
            .first()
            .map_or("", |identity| identity.name.as_str())
    }

    pub fn avatar(&self) -> Option<&str> {
        self.identities
            .first()
            .and_then(|identity| identity.avatar.as_deref())
    }

    pub fn identity(&self, platform: Platform) -> Option<&Identity> {
        self.identities
            .iter()
            .find(|identity| identity.platform == platform)
    }

    pub fn twitch(&self) -> Option<&Identity> {
        self.identity(Platform::Twitch)
    }

    pub fn twitch_id(&self) -> Option<&str> {
        self.twitch().map(|twitch| twitch.user_id.as_str())
    }

    pub fn kick(&self) -> Option<&Identity> {
        self.identity(Platform::Kick)
    }

    pub fn kick_id(&self) -> Option<&str> {
        self.kick().map(|kick| kick.user_id.as_str())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    pub id: String,
    pub key: String,
}

#[derive(Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStats {
    pub total: u64,
    pub paypal: u64,
    pub stripe: u64,
    pub premium: u64,
}

#[derive(Debug, PartialEq)]
pub enum AccountError {
    Unknown,
    Full,
    UnknownModerator,
    UnknownDevice,
    TooManyModerators,
    IdentityTaken(Platform),
    UnknownIdentity,
    LastIdentity,
    NotSaved,
}

fn moderators_table(platform: Platform) -> &'static str {
    match platform {
        Platform::Twitch => "twitch_moderators",
        Platform::Kick => "kick_moderators",
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Moderator {
    pub id: String,
    pub login: String,
    pub name: String,
}

#[derive(Debug, PartialEq)]
pub struct KnownDevice {
    pub id: String,
    pub name: String,
    pub last_seen: u64,
}

const ACCOUNT_OF_PAGE: &str = "(SELECT id FROM accounts WHERE page_id = ?1)";

const COLUMNS: &str = "page_id, moblin_key, sessions, merchant_id, paypal_tracking_id, stripe_account_id, stripe_ready";

const IDENTITIES: &str = "(SELECT json_group_array(json_object('platform', platform, 'user_id', user_id, 'login', login, 'name', name, 'avatar', avatar) ORDER BY platform = 'twitch' DESC, platform) FROM identities WHERE account = accounts.id)";

fn account(row: &Row) -> rusqlite::Result<Account> {
    let identities: String = row.get(7)?;
    let identities = serde_json::from_str(&identities).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, error.into())
    })?;
    Ok(Account {
        identities,
        page: Page {
            id: row.get(0)?,
            key: row.get(1)?,
        },
        sessions: row.get(2)?,
        merchant_id: row.get(3)?,
        paypal_tracking_id: row.get(4)?,
        stripe_account_id: row.get(5)?,
        stripe_ready: row.get(6)?,
    })
}

fn read(connection: &Connection, id: &str) -> rusqlite::Result<Option<Account>> {
    connection
        .query_row(
            &format!("SELECT {COLUMNS}, {IDENTITIES} FROM accounts WHERE id = ?1"),
            [id],
            account,
        )
        .optional()
}

fn save_identity(connection: &Connection, id: &str, identity: &Identity) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT OR REPLACE INTO identities (platform, user_id, account, login, name, avatar) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            identity.platform.name(),
            identity.user_id,
            id,
            identity.login,
            identity.name,
            identity.avatar,
        ],
    )?;
    connection.execute(
        "UPDATE identities SET login = '' WHERE platform = ?1 AND user_id != ?2 AND login != '' AND login = lower(trim(?3))",
        params![identity.platform.name(), identity.user_id, identity.login],
    )?;
    Ok(())
}

fn account_of_identity(
    connection: &Connection,
    platform: Platform,
    user_id: &str,
) -> rusqlite::Result<Option<String>> {
    connection
        .query_row(
            "SELECT account FROM identities WHERE platform = ?1 AND user_id = ?2",
            [platform.name(), user_id],
            |row| row.get(0),
        )
        .optional()
}

fn not_saved(error: rusqlite::Error) -> AccountError {
    warn!("failed to save accounts: {error:#}");
    AccountError::NotSaved
}

pub struct Accounts {
    database: Arc<Database>,
}

impl Accounts {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    pub fn get(&self, id: &str) -> Option<Account> {
        self.find("id = ?1", id)
    }

    pub fn signed_in(
        &self,
        identity: &Identity,
        new_account: (String, Page),
    ) -> Result<(String, Account), AccountError> {
        let mut database = self.database.lock();
        let transaction = database.transaction().map_err(not_saved)?;
        let existing = account_of_identity(&transaction, identity.platform, &identity.user_id)
            .map_err(not_saved)?;
        let id = match existing {
            Some(id) => id,
            None => {
                let (id, page) = new_account;
                if count(&transaction).map_err(not_saved)? >= 10000 {
                    return Err(AccountError::Full);
                }
                transaction
                    .execute(
                        &format!("INSERT INTO accounts (id, {COLUMNS}) VALUES (?1, ?2, ?3, 0, NULL, NULL, NULL, 0)"),
                        params![id, page.id, page.key],
                    )
                    .map_err(not_saved)?;
                transaction
                    .execute(
                        "INSERT OR IGNORE INTO pages (id, settings) VALUES (?1, '{}')",
                        [&page.id],
                    )
                    .map_err(not_saved)?;
                id
            }
        };
        save_identity(&transaction, &id, identity).map_err(not_saved)?;
        let account = read(&transaction, &id)
            .map_err(not_saved)?
            .ok_or(AccountError::Unknown)?;
        transaction.commit().map_err(not_saved)?;
        Ok((id, account))
    }

    pub fn link(&self, id: &str, identity: &Identity) -> Result<Account, AccountError> {
        let mut database = self.database.lock();
        let transaction = database.transaction().map_err(not_saved)?;
        read(&transaction, id)
            .map_err(not_saved)?
            .ok_or(AccountError::Unknown)?;
        if let Some(other) = account_of_identity(&transaction, identity.platform, &identity.user_id)
            .map_err(not_saved)?
            && other != id
        {
            return Err(AccountError::IdentityTaken(identity.platform));
        }
        save_identity(&transaction, id, identity).map_err(not_saved)?;
        let account = read(&transaction, id)
            .map_err(not_saved)?
            .ok_or(AccountError::Unknown)?;
        transaction.commit().map_err(not_saved)?;
        Ok(account)
    }

    pub fn unlink(&self, id: &str, platform: Platform) -> Result<Account, AccountError> {
        let mut database = self.database.lock();
        let transaction = database.transaction().map_err(not_saved)?;
        let account = read(&transaction, id)
            .map_err(not_saved)?
            .ok_or(AccountError::Unknown)?;
        if account.identity(platform).is_none() {
            return Err(AccountError::UnknownIdentity);
        }
        if account.identities.len() < 2 {
            return Err(AccountError::LastIdentity);
        }
        transaction
            .execute(
                "DELETE FROM identities WHERE account = ?1 AND platform = ?2",
                [id, platform.name()],
            )
            .map_err(not_saved)?;
        let account = read(&transaction, id)
            .map_err(not_saved)?
            .ok_or(AccountError::Unknown)?;
        transaction.commit().map_err(not_saved)?;
        Ok(account)
    }

    pub fn account_of_identity(&self, platform: Platform, user_id: &str) -> Option<String> {
        account_of_identity(&self.database.lock(), platform, user_id).unwrap_or_else(|error| {
            warn!("failed to read identities: {error:#}");
            None
        })
    }

    pub fn page_of_login(&self, platform: Platform, login: &str) -> Option<String> {
        self.database
            .lock()
            .query_row(
                "SELECT page_id FROM identities JOIN accounts ON accounts.id = identities.account
                 WHERE platform = ?1 AND identities.login != '' AND identities.login = lower(trim(?2))",
                [platform.name(), login],
                |row| row.get(0),
            )
            .optional()
            .unwrap_or_else(|error| {
                warn!("failed to read identities: {error:#}");
                None
            })
    }

    pub fn of_page(&self, page: &str) -> Option<Account> {
        self.find("page_id = ?1", page)
    }

    pub fn page_of_key(&self, key: &str) -> Option<String> {
        self.database
            .lock()
            .query_row(
                "SELECT page_id FROM accounts WHERE moblin_key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()
            .unwrap_or_else(|error| {
                warn!("failed to read accounts: {error:#}");
                None
            })
    }

    pub fn replace_key(&self, id: &str, key: String) -> Result<Account, AccountError> {
        self.change(id, |account| {
            account.page.key = key;
            Ok(())
        })
    }

    pub fn seen_device(&self, page: &str, device: &str) -> Option<String> {
        let mut database = self.database.lock();
        let result = database.transaction().and_then(|transaction| {
            let now = now();
            transaction.execute(
                &format!("DELETE FROM devices WHERE account = {ACCOUNT_OF_PAGE} AND id != ?2 AND last_seen < ?3"),
                params![page, device, now.saturating_sub(30 * 24 * 60 * 60)],
            )?;
            let known = transaction
                .query_row(
                    &format!("UPDATE devices SET last_seen = ?3 WHERE account = {ACCOUNT_OF_PAGE} AND id = ?2 RETURNING name"),
                    params![page, device, now],
                    |row| row.get(0),
                )
                .optional()?;
            let name = match known {
                Some(name) => name,
                None => {
                    transaction.execute(
                        &format!(
                            "DELETE FROM devices WHERE account = {ACCOUNT_OF_PAGE} AND rowid NOT IN
                             (SELECT rowid FROM devices WHERE account = {ACCOUNT_OF_PAGE} ORDER BY last_seen DESC, rowid DESC LIMIT 19)"
                        ),
                        [page],
                    )?;
                    let names = transaction
                        .prepare(&format!("SELECT name FROM devices WHERE account = {ACCOUNT_OF_PAGE}"))?
                        .query_map([page], |row| row.get(0))?
                        .collect::<rusqlite::Result<Vec<String>>>()?;
                    let name = (1..)
                        .map(|number| format!("Moblin {number}"))
                        .find(|name| !names.contains(name))
                        .unwrap();
                    transaction.execute(
                        &format!("INSERT INTO devices (account, id, name, last_seen) VALUES ({ACCOUNT_OF_PAGE}, ?2, ?3, ?4)"),
                        params![page, device, name, now],
                    )?;
                    name
                }
            };
            transaction.commit()?;
            Ok(name)
        });
        result
            .inspect_err(|error| warn!("failed to save device {device} of {page}: {error:#}"))
            .ok()
    }

    pub fn rename_device(&self, page: &str, device: &str, name: &str) -> Result<(), AccountError> {
        let renamed = self
            .database
            .lock()
            .execute(
                &format!(
                    "UPDATE devices SET name = ?3 WHERE account = {ACCOUNT_OF_PAGE} AND id = ?2"
                ),
                params![page, device, name],
            )
            .map_err(not_saved)?;
        if renamed == 0 {
            return Err(AccountError::UnknownDevice);
        }
        Ok(())
    }

    pub fn left_device(&self, page: &str, device: &str) {
        let result = self.database.lock().execute(
            &format!(
                "UPDATE devices SET last_seen = ?3 WHERE account = {ACCOUNT_OF_PAGE} AND id = ?2"
            ),
            params![page, device, now()],
        );
        if let Err(error) = result {
            warn!("failed to save device {device} of {page}: {error:#}");
        }
    }

    pub fn known_devices(&self, page: &str) -> Vec<KnownDevice> {
        let database = self.database.lock();
        let result = database
            .prepare_cached(&format!(
                "SELECT id, name, last_seen FROM devices WHERE account = {ACCOUNT_OF_PAGE}
                 ORDER BY last_seen DESC, rowid DESC"
            ))
            .and_then(|mut statement| {
                statement
                    .query_map([page], |row| {
                        Ok(KnownDevice {
                            id: row.get(0)?,
                            name: row.get(1)?,
                            last_seen: row.get(2)?,
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()
            });
        result.unwrap_or_else(|error| {
            warn!("failed to read the devices of {page}: {error:#}");
            Vec::new()
        })
    }

    pub fn device_names(&self, page: &str) -> HashMap<String, String> {
        let database = self.database.lock();
        let result = database
            .prepare_cached(&format!(
                "SELECT id, name FROM devices WHERE account = {ACCOUNT_OF_PAGE}"
            ))
            .and_then(|mut statement| {
                statement
                    .query_map([page], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<rusqlite::Result<_>>()
            });
        result.unwrap_or_else(|error| {
            warn!("failed to read the devices of {page}: {error:#}");
            HashMap::new()
        })
    }

    pub fn start_paypal(&self, id: &str, tracking_id: String) -> Result<Account, AccountError> {
        self.change(id, |account| {
            account.paypal_tracking_id = Some(tracking_id);
            Ok(())
        })
    }

    pub fn connect_paypal(
        &self,
        id: &str,
        tracking_id: &str,
        merchant_id: String,
    ) -> Result<Account, AccountError> {
        self.change(id, |account| {
            if account.paypal_tracking_id.as_deref() == Some(tracking_id) {
                account.merchant_id = Some(merchant_id);
                account.paypal_tracking_id = None;
            }
            Ok(())
        })
    }

    pub fn of_paypal_tracking_id(&self, tracking_id: &str) -> Option<String> {
        self.database
            .lock()
            .query_row(
                "SELECT id FROM accounts WHERE paypal_tracking_id = ?1",
                [tracking_id],
                |row| row.get(0),
            )
            .optional()
            .unwrap_or_else(|error| {
                warn!("failed to read accounts: {error:#}");
                None
            })
    }

    pub fn disconnect_paypal(&self, merchant_id: &str) -> Vec<String> {
        let database = self.database.lock();
        database
            .prepare_cached(
                "UPDATE accounts SET merchant_id = NULL WHERE merchant_id = ?1 RETURNING id",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([merchant_id], |row| row.get(0))?
                    .collect()
            })
            .unwrap_or_else(|error| {
                warn!("failed to save accounts: {error:#}");
                Vec::new()
            })
    }

    pub fn forget_paypal(&self, id: &str) -> Result<Account, AccountError> {
        self.change(id, |account| {
            account.merchant_id = None;
            account.paypal_tracking_id = None;
            Ok(())
        })
    }

    pub fn start_stripe(&self, id: &str, account_id: String) -> Result<Account, AccountError> {
        self.change(id, |account| {
            if account.stripe_account_id.as_ref() != Some(&account_id) {
                account.stripe_account_id = Some(account_id);
                account.stripe_ready = false;
            }
            Ok(())
        })
    }

    pub fn stripe_ready(
        &self,
        id: &str,
        account_id: &str,
        ready: bool,
    ) -> Result<Account, AccountError> {
        self.change(id, |account| {
            if account.stripe_account_id.as_deref() == Some(account_id) {
                account.stripe_ready = ready;
            }
            Ok(())
        })
    }

    pub fn forget_stripe(&self, id: &str) -> Result<Account, AccountError> {
        self.change(id, |account| {
            account.stripe_account_id = None;
            account.stripe_ready = false;
            Ok(())
        })
    }

    pub fn add_moderator(
        &self,
        platform: Platform,
        broadcaster: &str,
        moderator: &Moderator,
    ) -> Result<(), AccountError> {
        let table = moderators_table(platform);
        let mut database = self.database.lock();
        let transaction = database.transaction().map_err(not_saved)?;
        let listed: u32 = transaction
            .query_row(
                &format!("SELECT count(*) FROM {table} WHERE broadcaster = ?1 AND moderator != ?2"),
                [broadcaster, &moderator.id],
                |row| row.get(0),
            )
            .map_err(not_saved)?;
        if listed >= 50 {
            return Err(AccountError::TooManyModerators);
        }
        transaction
            .execute(
                &format!(
                    "INSERT INTO {table} (broadcaster, moderator, login, name) VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT (broadcaster, moderator) DO UPDATE SET login = excluded.login, name = excluded.name"
                ),
                [broadcaster, &moderator.id, &moderator.login, &moderator.name],
            )
            .map_err(not_saved)?;
        transaction.commit().map_err(not_saved)
    }

    pub fn remove_moderator(
        &self,
        platform: Platform,
        broadcaster: &str,
        moderator: &str,
    ) -> Result<(), AccountError> {
        let table = moderators_table(platform);
        let removed = self
            .database
            .lock()
            .execute(
                &format!("DELETE FROM {table} WHERE broadcaster = ?1 AND moderator = ?2"),
                [broadcaster, moderator],
            )
            .map_err(not_saved)?;
        if removed == 0 {
            return Err(AccountError::UnknownModerator);
        }
        Ok(())
    }

    pub fn moderators(&self, platform: Platform, broadcaster: &str) -> Vec<Moderator> {
        let table = moderators_table(platform);
        let database = self.database.lock();
        let result = database
            .prepare_cached(&format!(
                "SELECT moderator, coalesce(identities.login, {table}.login), coalesce(identities.name, {table}.name)
                 FROM {table}
                 LEFT JOIN identities ON identities.platform = ?1 AND identities.user_id = moderator
                 WHERE broadcaster = ?2
                 ORDER BY 3 COLLATE NOCASE"
            ))
            .and_then(|mut statement| {
                statement
                    .query_map([platform.name(), broadcaster], |row| {
                        Ok(Moderator {
                            id: row.get(0)?,
                            login: row.get(1)?,
                            name: row.get(2)?,
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()
            });
        result.unwrap_or_else(|error| {
            warn!(
                "failed to read the {} moderators of {broadcaster}: {error:#}",
                platform.title()
            );
            Vec::new()
        })
    }

    pub fn moderates(&self, platform: Platform, moderator: &str, broadcaster: &str) -> bool {
        let table = moderators_table(platform);
        self.database
            .lock()
            .query_row(
                &format!("SELECT 1 FROM {table} WHERE moderator = ?1 AND broadcaster = ?2"),
                [moderator, broadcaster],
                |_| Ok(()),
            )
            .optional()
            .unwrap_or_else(|error| {
                warn!("failed to read {} moderators: {error:#}", platform.title());
                None
            })
            .is_some()
    }

    pub fn moderated(&self, platform: Platform, moderator: &str) -> Vec<Account> {
        let table = moderators_table(platform);
        let broadcasters: Vec<String> = {
            let database = self.database.lock();
            let result = database
                .prepare_cached(&format!(
                    "SELECT identities.account FROM {table}
                     JOIN identities ON identities.platform = ?1 AND identities.user_id = broadcaster
                     WHERE moderator = ?2 ORDER BY identities.name COLLATE NOCASE"
                ))
                .and_then(|mut statement| {
                    statement
                        .query_map([platform.name(), moderator], |row| row.get(0))?
                        .collect::<rusqlite::Result<_>>()
                });
            result.unwrap_or_else(|error| {
                warn!("failed to read {} moderators: {error:#}", platform.title());
                Vec::new()
            })
        };
        broadcasters
            .iter()
            .filter_map(|broadcaster| self.get(broadcaster))
            .collect()
    }

    pub fn sign_out(&self, id: &str) -> Result<Account, AccountError> {
        self.change(id, |account| {
            account.sessions += 1;
            Ok(())
        })
    }

    pub fn delete(&self, id: &str) -> Result<Account, AccountError> {
        let mut database = self.database.lock();
        let transaction = database.transaction().map_err(not_saved)?;
        let account = read(&transaction, id)
            .map_err(not_saved)?
            .ok_or(AccountError::Unknown)?;
        let page = account.page.id.as_str();
        let statements = [
            ("DELETE FROM accounts WHERE id = ?1", id),
            ("DELETE FROM identities WHERE account = ?1", id),
            ("DELETE FROM devices WHERE account = ?1", id),
            ("DELETE FROM pages WHERE id = ?1", page),
            ("DELETE FROM page_images WHERE id = ?1", page),
            ("DELETE FROM orders WHERE streamer = ?1", page),
            ("DELETE FROM deliveries WHERE streamer = ?1", page),
            ("DELETE FROM subscriber_allowances WHERE page = ?1", page),
        ];
        for (statement, key) in statements {
            transaction.execute(statement, [key]).map_err(not_saved)?;
        }
        for identity in &account.identities {
            transaction
                .execute(
                    &format!(
                        "DELETE FROM {} WHERE broadcaster = ?1 OR moderator = ?1",
                        moderators_table(identity.platform)
                    ),
                    [&identity.user_id],
                )
                .map_err(not_saved)?;
        }
        transaction.commit().map_err(not_saved)?;
        Ok(account)
    }

    pub fn stats(&self, premium: &[&str]) -> AccountStats {
        self.database
            .lock()
            .query_row(
                "SELECT count(*), count(merchant_id), coalesce(sum(stripe_ready), 0),
                        count(*) FILTER (WHERE EXISTS (
                            SELECT 1 FROM identities WHERE identities.account = accounts.id
                            AND iif(platform = 'twitch', user_id, platform || ':' || user_id)
                                IN (SELECT value FROM json_each(?1))
                        ))
                 FROM accounts",
                [serde_json::to_string(premium).unwrap()],
                |row| {
                    Ok(AccountStats {
                        total: row.get(0)?,
                        paypal: row.get(1)?,
                        stripe: row.get(2)?,
                        premium: row.get(3)?,
                    })
                },
            )
            .unwrap_or_else(|error| {
                warn!("failed to count accounts: {error:#}");
                AccountStats::default()
            })
    }

    fn find(&self, condition: &str, value: &str) -> Option<Account> {
        self.database
            .lock()
            .prepare_cached(&format!(
                "SELECT {COLUMNS}, {IDENTITIES} FROM accounts WHERE {condition} LIMIT 1"
            ))
            .and_then(|mut statement| statement.query_row([value], account).optional())
            .unwrap_or_else(|error| {
                warn!("failed to read accounts: {error:#}");
                None
            })
    }

    fn change(
        &self,
        id: &str,
        change: impl FnOnce(&mut Account) -> Result<(), AccountError>,
    ) -> Result<Account, AccountError> {
        let mut database = self.database.lock();
        let transaction = database.transaction().map_err(not_saved)?;
        let old = read(&transaction, id)
            .map_err(not_saved)?
            .ok_or(AccountError::Unknown)?;
        let mut account = old.clone();
        change(&mut account)?;
        if old == account {
            return Ok(account);
        }
        transaction
            .execute(
                &format!("INSERT OR REPLACE INTO accounts (id, {COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"),
                params![
                    id,
                    account.page.id,
                    account.page.key,
                    account.sessions,
                    account.merchant_id,
                    account.paypal_tracking_id,
                    account.stripe_account_id,
                    account.stripe_ready,
                ],
            )
            .map_err(not_saved)?;
        transaction.commit().map_err(not_saved)?;
        Ok(account)
    }
}

fn count(database: &Connection) -> rusqlite::Result<u64> {
    database.query_row("SELECT count(*) FROM accounts", [], |row| row.get(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database;

    fn page(id: &str, key: &str) -> Page {
        Page {
            id: id.to_string(),
            key: key.to_string(),
        }
    }

    fn twitch(id: &str, name: &str, login: &str) -> Identity {
        Identity {
            platform: Platform::Twitch,
            user_id: id.to_string(),
            login: login.to_string(),
            name: name.to_string(),
            avatar: None,
        }
    }

    fn kick(id: &str, name: &str, login: &str) -> Identity {
        Identity {
            platform: Platform::Kick,
            ..twitch(id, name, login)
        }
    }

    fn sign_in(
        accounts: &Accounts,
        id: &str,
        name: &str,
        login: &str,
        page: Page,
    ) -> Result<Account, AccountError> {
        accounts
            .signed_in(&twitch(id, name, login), (id.to_string(), page))
            .map(|(_, account)| account)
    }

    #[test]
    fn pages_are_kept() {
        let accounts = Accounts::new(Database::memory());
        assert_eq!(accounts.get("1"), None);
        assert_eq!(
            sign_in(&accounts, "1", "Erik", "erik", page("p", "k")),
            Ok(Account {
                identities: vec![twitch("1", "Erik", "erik")],
                page: page("p", "k"),
                ..Account::default()
            })
        );
        let (id, account) = accounts
            .signed_in(
                &twitch("1", "Eerimoq", "eerimoq"),
                ("other".to_string(), page("q", "o")),
            )
            .unwrap();
        assert_eq!(id, "1");
        assert_eq!(account.page, page("p", "k"));
        assert_eq!(accounts.get("1").unwrap().name(), "Eerimoq");
        assert_eq!(accounts.get("other"), None);
        assert_eq!(accounts.page_of_login(Platform::Twitch, "erik"), None);
        assert_eq!(
            accounts
                .page_of_login(Platform::Twitch, "eerimoq")
                .as_deref(),
            Some("p")
        );
    }

    #[test]
    fn platforms_are_linked_and_unlinked() {
        let accounts = Accounts::new(Database::memory());
        assert_eq!(
            accounts.link("1", &kick("7", "Erik", "erik")),
            Err(AccountError::Unknown)
        );
        sign_in(&accounts, "1", "Erik", "erik", page("p", "k")).unwrap();
        let (other, _) = accounts
            .signed_in(
                &kick("8", "Other", "other"),
                ("random".to_string(), page("q", "l")),
            )
            .unwrap();
        assert_eq!(other, "random");
        assert_eq!(accounts.get("random").unwrap().name(), "Other");
        assert_eq!(
            accounts.link("1", &kick("8", "Other", "other")),
            Err(AccountError::IdentityTaken(Platform::Kick))
        );
        assert_eq!(
            accounts.link("random", &twitch("1", "Erik", "erik")),
            Err(AccountError::IdentityTaken(Platform::Twitch))
        );
        let account = accounts.link("1", &kick("7", "Kick Erik", "erik")).unwrap();
        assert_eq!(
            account.identities,
            [twitch("1", "Erik", "erik"), kick("7", "Kick Erik", "erik")]
        );
        assert_eq!(account.name(), "Erik");
        assert_eq!(
            accounts.page_of_login(Platform::Kick, "erik").as_deref(),
            Some("p")
        );
        assert_eq!(
            accounts.page_of_login(Platform::Twitch, "other").as_deref(),
            None
        );
        let (id, account) = accounts
            .signed_in(
                &kick("7", "Kick Erik", "kickerik"),
                ("unused".to_string(), page("r", "m")),
            )
            .unwrap();
        assert_eq!(id, "1");
        assert_eq!(account.identities[1].login, "kickerik");
        assert_eq!(accounts.get("unused"), None);
        assert_eq!(
            accounts.unlink("1", Platform::Twitch).unwrap().identities,
            [kick("7", "Kick Erik", "kickerik")]
        );
        assert_eq!(accounts.get("1").unwrap().name(), "Kick Erik");
        assert_eq!(
            accounts.unlink("1", Platform::Twitch),
            Err(AccountError::UnknownIdentity)
        );
        assert_eq!(
            accounts.unlink("1", Platform::Kick),
            Err(AccountError::LastIdentity)
        );
        assert_eq!(accounts.page_of_login(Platform::Twitch, "erik"), None);
        let (id, _) = accounts
            .signed_in(
                &twitch("1", "Erik", "erik"),
                ("fresh".to_string(), page("s", "n")),
            )
            .unwrap();
        assert_eq!(id, "fresh");
        assert_eq!(
            accounts.page_of_login(Platform::Twitch, "erik").as_deref(),
            Some("s")
        );
        assert_eq!(
            accounts.link("1", &twitch("1", "Erik", "erik")),
            Err(AccountError::IdentityTaken(Platform::Twitch))
        );
        let account = accounts.link("1", &kick("7", "Again", "kickerik")).unwrap();
        assert_eq!(account.identities.len(), 1);
        assert_eq!(account.name(), "Again");
    }

    #[test]
    fn moderators_are_kept_per_platform() {
        let accounts = Accounts::new(Database::memory());
        let (streamer, _) = accounts
            .signed_in(
                &twitch("42", "Streamer", "streamer"),
                ("s".to_string(), page("p", "k")),
            )
            .unwrap();
        sign_in(&accounts, "1", "Mod", "mod", page("q", "l")).unwrap();
        let moderator = Moderator {
            id: "1".to_string(),
            login: "mod".to_string(),
            name: "Mod".to_string(),
        };
        accounts
            .add_moderator(Platform::Twitch, "42", &moderator)
            .unwrap();
        assert_eq!(accounts.moderated(Platform::Twitch, "1")[0].page.id, "p");
        assert!(accounts.moderates(Platform::Twitch, "1", "42"));
        assert!(!accounts.moderates(Platform::Kick, "1", "42"));
        assert!(accounts.moderated(Platform::Kick, "1").is_empty());
        assert_eq!(accounts.moderators(Platform::Twitch, "42"), [moderator]);
        assert!(accounts.moderators(Platform::Kick, "42").is_empty());
        assert_eq!(
            accounts.remove_moderator(Platform::Kick, "42", "1"),
            Err(AccountError::UnknownModerator)
        );
        accounts
            .link("s", &kick("9", "Streamer", "streamer"))
            .unwrap();
        accounts.unlink("s", Platform::Twitch).unwrap();
        assert!(accounts.moderated(Platform::Twitch, "1").is_empty());
        assert_eq!(accounts.get(&streamer).unwrap().identities.len(), 1);
        accounts
            .link("s", &twitch("42", "Streamer", "streamer"))
            .unwrap();
        assert_eq!(accounts.moderated(Platform::Twitch, "1")[0].page.id, "p");
        accounts
            .remove_moderator(Platform::Twitch, "42", "1")
            .unwrap();
        assert!(!accounts.moderates(Platform::Twitch, "1", "42"));
        assert!(accounts.moderated(Platform::Twitch, "1").is_empty());
    }

    #[test]
    fn moderators_are_an_allow_list() {
        let accounts = Accounts::new(Database::memory());
        let moderator = |id: &str, login: &str| Moderator {
            id: id.to_string(),
            login: login.to_string(),
            name: login.to_string(),
        };
        accounts
            .add_moderator(Platform::Kick, "42", &moderator("1", "erik"))
            .unwrap();
        accounts
            .add_moderator(Platform::Kick, "42", &moderator("2", "bob"))
            .unwrap();
        accounts
            .add_moderator(Platform::Kick, "43", &moderator("1", "erik"))
            .unwrap();
        assert_eq!(
            accounts.moderators(Platform::Kick, "42"),
            [moderator("2", "bob"), moderator("1", "erik")]
        );
        assert!(accounts.moderates(Platform::Kick, "1", "42"));
        assert!(accounts.moderates(Platform::Kick, "1", "43"));
        assert!(!accounts.moderates(Platform::Kick, "2", "43"));
        assert!(!accounts.moderates(Platform::Kick, "42", "1"));
        assert_eq!(
            accounts.remove_moderator(Platform::Kick, "42", "3"),
            Err(AccountError::UnknownModerator)
        );
        accounts
            .remove_moderator(Platform::Kick, "42", "1")
            .unwrap();
        assert!(!accounts.moderates(Platform::Kick, "1", "42"));
        assert!(accounts.moderates(Platform::Kick, "1", "43"));
        assert_eq!(
            accounts.moderators(Platform::Kick, "42"),
            [moderator("2", "bob")]
        );
        for n in 0..49 {
            accounts
                .add_moderator(
                    Platform::Kick,
                    "42",
                    &moderator(&format!("{}", 100 + n), "m"),
                )
                .unwrap();
        }
        assert_eq!(accounts.moderators(Platform::Kick, "42").len(), 50);
        assert_eq!(
            accounts.add_moderator(Platform::Kick, "42", &moderator("200", "late")),
            Err(AccountError::TooManyModerators)
        );
        accounts
            .add_moderator(Platform::Kick, "42", &moderator("2", "robert"))
            .unwrap();
        assert_eq!(accounts.moderators(Platform::Kick, "42").len(), 50);
        assert!(
            accounts
                .moderators(Platform::Kick, "42")
                .contains(&moderator("2", "robert"))
        );
    }

    #[test]
    fn kick_moderators_keep_access_when_renamed_and_follow_the_kick_identity() {
        let accounts = Accounts::new(Database::memory());
        let (streamer, _) = accounts
            .signed_in(
                &kick("42", "Streamer", "streamer"),
                ("s".to_string(), page("p", "k")),
            )
            .unwrap();
        accounts
            .add_moderator(
                Platform::Kick,
                "42",
                &Moderator {
                    id: "1".to_string(),
                    login: "bob".to_string(),
                    name: "bob".to_string(),
                },
            )
            .unwrap();
        assert_eq!(accounts.moderated(Platform::Kick, "1")[0].page.id, "p");
        assert!(accounts.moderates(Platform::Kick, "1", "42"));
        accounts
            .signed_in(
                &kick("1", "Robert", "robert"),
                ("m".to_string(), page("q", "l")),
            )
            .unwrap();
        assert!(accounts.moderates(Platform::Kick, "1", "42"));
        assert_eq!(accounts.moderated(Platform::Kick, "1")[0].page.id, "p");
        assert_eq!(
            accounts.moderators(Platform::Kick, "42"),
            [Moderator {
                id: "1".to_string(),
                login: "robert".to_string(),
                name: "Robert".to_string(),
            }]
        );
        assert!(accounts.moderated(Platform::Kick, "2").is_empty());
        accounts
            .link("s", &twitch("7", "Streamer", "streamer"))
            .unwrap();
        accounts.unlink("s", Platform::Kick).unwrap();
        assert!(accounts.moderated(Platform::Kick, "1").is_empty());
        assert_eq!(accounts.get(&streamer).unwrap().identities.len(), 1);
        accounts
            .link("s", &kick("42", "Streamer", "streamer"))
            .unwrap();
        assert_eq!(accounts.moderated(Platform::Kick, "1")[0].page.id, "p");
        accounts.delete("m").unwrap();
        assert!(accounts.moderators(Platform::Kick, "42").is_empty());
        accounts
            .add_moderator(
                Platform::Kick,
                "42",
                &Moderator {
                    id: "1".to_string(),
                    login: "robert".to_string(),
                    name: "Robert".to_string(),
                },
            )
            .unwrap();
        accounts.delete("s").unwrap();
        assert!(accounts.moderators(Platform::Kick, "42").is_empty());
        assert!(!accounts.moderates(Platform::Kick, "1", "42"));
    }

    #[test]
    fn pages_find_their_account() {
        let accounts = Accounts::new(Database::memory());
        accounts
            .signed_in(
                &Identity {
                    avatar: Some("https://example.com/a.png".to_string()),
                    ..twitch("1", "Erik", "erik")
                },
                ("1".to_string(), page("p", "k")),
            )
            .unwrap();
        assert_eq!(accounts.of_page("q"), None);
        let account = accounts.of_page("p").unwrap();
        assert_eq!(account.avatar(), Some("https://example.com/a.png"));
        assert_eq!(
            account.twitch().unwrap().avatar.as_deref(),
            Some("https://example.com/a.png")
        );
        sign_in(&accounts, "1", "Erik", "erik", page("p", "k")).unwrap();
        assert_eq!(accounts.of_page("p").unwrap().avatar(), None);
    }

    #[test]
    fn keys_are_replaced() {
        let accounts = Accounts::new(Database::memory());
        sign_in(&accounts, "1", "Erik", "erik", page("p", "k")).unwrap();
        assert_eq!(accounts.page_of_key("k").as_deref(), Some("p"));
        assert_eq!(
            accounts.replace_key("2", "n".to_string()),
            Err(AccountError::Unknown)
        );
        assert_eq!(
            accounts.replace_key("1", "n".to_string()).unwrap().page,
            page("p", "n")
        );
        assert_eq!(accounts.page_of_key("k"), None);
        assert_eq!(accounts.page_of_key("n").as_deref(), Some("p"));
    }

    #[test]
    fn at_most_20_devices_are_kept_per_account() {
        let accounts = Accounts::new(Database::memory());
        sign_in(&accounts, "1", "Erik", "erik", page("p", "k")).unwrap();
        for device in 0..25 {
            accounts.seen_device("p", &device.to_string()).unwrap();
        }
        let names = accounts.device_names("p");
        assert_eq!(names.len(), 20);
        assert!((0..5).all(|device| !names.contains_key(&device.to_string())));
        assert_eq!(names["5"], "Moblin 6");
        assert_eq!(names["20"], "Moblin 1");
        assert_eq!(names["24"], "Moblin 5");
    }

    #[test]
    fn known_devices_are_listed_last_seen_first() {
        let accounts = Accounts::new(Database::memory());
        sign_in(&accounts, "1", "Erik", "erik", page("p", "k")).unwrap();
        accounts.seen_device("p", "a").unwrap();
        accounts.seen_device("p", "b").unwrap();
        accounts.left_device("q", "a");
        let ids = |accounts: &Accounts| {
            accounts
                .known_devices("p")
                .into_iter()
                .map(|device| device.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&accounts), ["b", "a"]);
        accounts
            .database
            .lock()
            .execute("UPDATE devices SET last_seen = 0", [])
            .unwrap();
        accounts.left_device("p", "a");
        assert_eq!(ids(&accounts), ["a", "b"]);
        assert_eq!(accounts.known_devices("p")[0].name, "Moblin 1");
        assert_eq!(accounts.known_devices("p")[1].last_seen, 0);
    }

    #[test]
    fn devices_get_a_name_when_first_seen() {
        let accounts = Accounts::new(Database::memory());
        sign_in(&accounts, "1", "Erik", "erik", page("p", "k")).unwrap();
        sign_in(&accounts, "2", "Other", "other", page("q", "l")).unwrap();
        assert_eq!(accounts.seen_device("r", "a"), None);
        let seen = |page: &str, device: &str| accounts.seen_device(page, device).unwrap();
        assert_eq!(seen("p", "a"), "Moblin 1");
        assert_eq!(seen("p", "b"), "Moblin 2");
        assert_eq!(seen("q", "a"), "Moblin 1");
        assert_eq!(seen("p", "a"), "Moblin 1");
        accounts
            .database
            .lock()
            .execute(
                "UPDATE devices SET last_seen = 0 WHERE account = '1' AND id = 'a'",
                [],
            )
            .unwrap();
        assert_eq!(seen("p", "c"), "Moblin 1");
        assert_eq!(seen("p", "a"), "Moblin 3");
        assert_eq!(
            accounts.rename_device("p", "d", "Backup"),
            Err(AccountError::UnknownDevice)
        );
        accounts.rename_device("p", "b", "Backup").unwrap();
        accounts.rename_device("q", "a", "Other").unwrap();
        assert_eq!(seen("p", "d"), "Moblin 2");
        assert_eq!(
            accounts.device_names("p"),
            HashMap::from([
                ("a".to_string(), "Moblin 3".to_string()),
                ("b".to_string(), "Backup".to_string()),
                ("c".to_string(), "Moblin 1".to_string()),
                ("d".to_string(), "Moblin 2".to_string())
            ])
        );
    }

    #[test]
    fn signing_out_ends_sessions() {
        let accounts = Accounts::new(Database::memory());
        assert_eq!(accounts.sign_out("1"), Err(AccountError::Unknown));
        sign_in(&accounts, "1", "Erik", "erik", page("p", "k")).unwrap();
        assert_eq!(accounts.sign_out("1").unwrap().sessions, 1);
        assert_eq!(
            sign_in(&accounts, "1", "Erik", "erik", page("q", "o"))
                .unwrap()
                .sessions,
            1
        );
    }

    #[test]
    fn paypal_connects_only_the_latest_onboarding() {
        let accounts = Accounts::new(Database::memory());
        assert_eq!(
            accounts.start_paypal("1", "a".to_string()),
            Err(AccountError::Unknown)
        );
        sign_in(&accounts, "1", "Erik", "erik", page("p", "k")).unwrap();
        accounts.start_paypal("1", "a".to_string()).unwrap();
        accounts.start_paypal("1", "b".to_string()).unwrap();
        let account = accounts
            .connect_paypal("1", "a", "MERCHANT00001".to_string())
            .unwrap();
        assert_eq!(account.merchant_id, None);
        let account = accounts
            .connect_paypal("1", "b", "MERCHANT00002".to_string())
            .unwrap();
        assert_eq!(account.merchant_id.as_deref(), Some("MERCHANT00002"));
        assert_eq!(account.paypal_tracking_id, None);
        accounts.start_paypal("1", "c".to_string()).unwrap();
        assert_eq!(
            accounts.get("1").unwrap().merchant_id.as_deref(),
            Some("MERCHANT00002")
        );
        assert_eq!(accounts.of_paypal_tracking_id("c").as_deref(), Some("1"));
        assert_eq!(accounts.of_paypal_tracking_id("b"), None);
        assert!(accounts.disconnect_paypal("MERCHANT00001").is_empty());
        assert_eq!(accounts.disconnect_paypal("MERCHANT00002"), ["1"]);
        assert_eq!(accounts.get("1").unwrap().merchant_id, None);
        accounts.start_paypal("1", "d".to_string()).unwrap();
        accounts
            .connect_paypal("1", "d", "MERCHANT00003".to_string())
            .unwrap();
        accounts.start_paypal("1", "e".to_string()).unwrap();
        let account = accounts.forget_paypal("1").unwrap();
        assert_eq!(account.merchant_id, None);
        assert_eq!(account.paypal_tracking_id, None);
        assert_eq!(accounts.forget_paypal("2"), Err(AccountError::Unknown));
    }

    #[test]
    fn stripe_is_ready_only_for_the_current_account() {
        let accounts = Accounts::new(Database::memory());
        assert_eq!(
            accounts.start_stripe("1", "acct_1".to_string()),
            Err(AccountError::Unknown)
        );
        sign_in(&accounts, "1", "Erik", "erik", page("p", "k")).unwrap();
        accounts.start_stripe("1", "acct_1".to_string()).unwrap();
        assert!(
            !accounts
                .stripe_ready("1", "acct_2", true)
                .unwrap()
                .stripe_ready
        );
        assert!(
            accounts
                .stripe_ready("1", "acct_1", true)
                .unwrap()
                .stripe_ready
        );
        assert!(
            accounts
                .start_stripe("1", "acct_1".to_string())
                .unwrap()
                .stripe_ready
        );
        let account = accounts.start_stripe("1", "acct_2".to_string()).unwrap();
        assert_eq!(account.stripe_account_id.as_deref(), Some("acct_2"));
        assert!(!account.stripe_ready);
        accounts.stripe_ready("1", "acct_2", true).unwrap();
        let account = accounts.forget_stripe("1").unwrap();
        assert_eq!(account.stripe_account_id, None);
        assert!(!account.stripe_ready);
    }

    #[test]
    fn logins_find_pages() {
        let accounts = Accounts::new(Database::memory());
        assert_eq!(accounts.page_of_login(Platform::Twitch, "erik"), None);
        sign_in(&accounts, "1", "Erik", "erik", page("a", "k")).unwrap();
        assert_eq!(
            accounts.page_of_login(Platform::Twitch, "ERIK").as_deref(),
            Some("a")
        );
        assert_eq!(accounts.page_of_login(Platform::Kick, "erik"), None);
        assert_eq!(accounts.page_of_login(Platform::Twitch, "other"), None);
        assert_eq!(accounts.page_of_login(Platform::Twitch, ""), None);
        sign_in(&accounts, "2", "Erik", "erik", page("c", "l")).unwrap();
        assert_eq!(
            accounts.page_of_login(Platform::Twitch, "erik").as_deref(),
            Some("c")
        );
        assert_eq!(accounts.get("1").unwrap().twitch().unwrap().login, "");
        accounts
            .signed_in(
                &kick("5", "Erik", "erik"),
                ("3".to_string(), page("d", "m")),
            )
            .unwrap();
        assert_eq!(
            accounts.page_of_login(Platform::Kick, "erik").as_deref(),
            Some("d")
        );
        assert_eq!(
            accounts.page_of_login(Platform::Twitch, "erik").as_deref(),
            Some("c")
        );
    }

    #[test]
    fn premium_accounts_are_counted() {
        let accounts = Accounts::new(Database::memory());
        assert_eq!(accounts.stats(&["1"]).premium, 0);
        sign_in(&accounts, "1", "Erik", "erik", page("p", "k")).unwrap();
        sign_in(&accounts, "2", "Other", "other", page("q", "l")).unwrap();
        accounts
            .signed_in(
                &kick("7", "Kicker", "kicker"),
                ("random".to_string(), page("r", "m")),
            )
            .unwrap();
        let stats = accounts.stats(&["1", "1", "3"]);
        assert_eq!((stats.total, stats.premium), (3, 1));
        assert_eq!(accounts.stats(&[]).premium, 0);
        assert_eq!(accounts.stats(&["7"]).premium, 0);
        assert_eq!(accounts.stats(&["kick:7"]).premium, 1);
        accounts.link("1", &kick("8", "Erik", "erik")).unwrap();
        assert_eq!(accounts.stats(&["1", "kick:8"]).premium, 1);
        assert_eq!(accounts.stats(&["kick:8", "kick:7"]).premium, 2);
    }

    #[test]
    fn accounts_are_saved() {
        let path = database::temporary_path("accounts");
        let accounts = Accounts::new(Database::open(Some(&path)).unwrap());
        let id = "0123456789abcdef0123456789abcdef";
        accounts
            .signed_in(
                &twitch("3", "erik", "erik"),
                (id.to_string(), page("p", "k")),
            )
            .unwrap();
        let accounts = Accounts::new(Database::open(Some(&path)).unwrap());
        assert_eq!(
            accounts.get(id),
            Some(Account {
                identities: vec![twitch("3", "erik", "erik")],
                page: page("p", "k"),
                ..Account::default()
            })
        );
        database::remove(&path);
    }
}
