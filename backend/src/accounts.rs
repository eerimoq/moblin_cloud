use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use log::warn;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use crate::database::Database;
use crate::deliveries::now;
use crate::pages::Look;

#[derive(Clone, Copy, Debug, PartialEq, Hash, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Twitch,
    Kick,
}

impl Platform {
    pub fn user_key(self, user_id: &str) -> String {
        match self {
            Platform::Twitch => user_id.to_string(),
            Platform::Kick => format!("kick:{user_id}"),
        }
    }

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
        format!("{}:{}", self.platform.name(), self.user_id)
    }
}

pub fn admin_key_of(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(String::new());
    }
    let key = match value.split_once(':') {
        None => format!("twitch:{value}"),
        Some(("twitch" | "kick", _)) => value.to_string(),
        Some(_) => return Err("expected a Twitch user ID, twitch:<ID> or kick:<ID>".to_string()),
    };
    match key.split_once(':') {
        Some((_, "")) => Err("expected a user ID".to_string()),
        _ => Ok(key),
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Account {
    pub id: String,
    pub identities: Vec<Identity>,
    pub page_id: String,
    pub moblin_cloud_key: String,
    pub sessions: u64,
    pub merchant_id: Option<String>,
    pub paypal_tracking_id: Option<String>,
    pub stripe_account_id: Option<String>,
    pub stripe_ready: bool,
    pub premium: bool,
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
    UnknownAssistant,
    UnknownDevice,
    TooManyAssistants,
    IdentityTaken(Platform),
    UnknownIdentity,
    LastIdentity,
    NotSaved,
}

fn assistants_table(platform: Platform) -> &'static str {
    match platform {
        Platform::Twitch => "twitch_assistants",
        Platform::Kick => "kick_assistants",
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Assistant {
    #[serde(rename = "id")]
    pub user_id: String,
    pub login: String,
    pub name: String,
    pub avatar: Option<String>,
    pub preview: bool,
    pub go_live: bool,
    pub edit_tips: bool,
    pub remote_control: Access,
    pub last_active: Option<u64>,
    pub last_remote_control: Option<u64>,
}

impl Assistant {
    pub fn new(user_id: String, login: String, name: String) -> Self {
        Self {
            user_id,
            login,
            name,
            avatar: None,
            preview: false,
            go_live: false,
            edit_tips: false,
            remote_control: Access::None,
            last_active: None,
            last_remote_control: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Permission {
    Preview,
    GoLive,
    EditTips,
}

impl Permission {
    fn column(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::GoLive => "go_live",
            Self::EditTips => "edit_tips",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    #[default]
    None,
    View,
    Control,
}

impl Access {
    fn of(level: u8) -> Self {
        match level {
            0 => Self::None,
            1 => Self::View,
            _ => Self::Control,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rights {
    pub preview: bool,
    pub go_live: bool,
    pub edit_tips: bool,
    pub remote_control: Access,
}

impl Rights {
    pub fn all() -> Self {
        Self {
            preview: true,
            go_live: true,
            edit_tips: true,
            remote_control: Access::Control,
        }
    }

    pub fn permits(&self, permission: Permission) -> bool {
        match permission {
            Permission::Preview => self.preview,
            Permission::GoLive => self.go_live,
            Permission::EditTips => self.edit_tips,
        }
    }

    pub fn or(self, other: Self) -> Self {
        Self {
            preview: self.preview || other.preview,
            go_live: self.go_live || other.go_live,
            edit_tips: self.edit_tips || other.edit_tips,
            remote_control: self.remote_control.max(other.remote_control),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct KnownDevice {
    pub id: String,
    pub name: String,
    pub last_seen: u64,
}

const ACCOUNT_OF_PAGE: &str = "(SELECT id FROM accounts WHERE page_id = ?1)";

pub fn device_ids(connection: &Connection, page_id: &str) -> Vec<String> {
    let result = connection
        .prepare_cached(&format!(
            "SELECT id FROM devices WHERE account = {ACCOUNT_OF_PAGE} ORDER BY last_seen DESC, rowid DESC"
        ))
        .and_then(|mut statement| {
            statement
                .query_map([page_id], |row| row.get(0))?
                .collect::<rusqlite::Result<_>>()
        });
    result.unwrap_or_else(|error| {
        warn!("failed to read the devices of {page_id}: {error:#}");
        Vec::new()
    })
}

const COLUMNS: &str = "id, page_id, moblin_key, sessions, merchant_id, paypal_tracking_id, stripe_account_id, stripe_ready, premium";

const IDENTITIES: &str = "(SELECT json_group_array(json_object('platform', platform, 'user_id', user_id, 'login', login, 'name', name, 'avatar', avatar) ORDER BY platform = 'twitch' DESC, platform) FROM identities WHERE account = accounts.id)";

fn account(row: &Row) -> rusqlite::Result<Account> {
    let identities: String = row.get(9)?;
    let identities = serde_json::from_str(&identities).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, error.into())
    })?;
    Ok(Account {
        id: row.get(0)?,
        identities,
        page_id: row.get(1)?,
        moblin_cloud_key: row.get(2)?,
        sessions: row.get(3)?,
        merchant_id: row.get(4)?,
        paypal_tracking_id: row.get(5)?,
        stripe_account_id: row.get(6)?,
        stripe_ready: row.get(7)?,
        premium: row.get(8)?,
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
    active: Mutex<HashMap<(String, String, bool), Instant>>,
}

impl Accounts {
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            database,
            active: Mutex::default(),
        }
    }

    pub fn active(&self, account: &str, page_id: &str, remote_control: bool) -> bool {
        {
            let mut active = self.active.lock().unwrap();
            let key = |remote_control| (account.to_string(), page_id.to_string(), remote_control);
            if active
                .get(&key(remote_control))
                .is_some_and(|at| at.elapsed() < Duration::from_secs(30))
            {
                return false;
            }
            active.retain(|_, at| at.elapsed() < Duration::from_secs(30));
            active.insert(key(remote_control), Instant::now());
            active.insert(key(false), Instant::now());
        }
        let result = self.database.lock().execute(
            if remote_control {
                "INSERT INTO assistant_activity (account, page, at, remote_control_at) VALUES (?1, ?2, ?3, ?3)
                 ON CONFLICT (account, page) DO UPDATE SET at = excluded.at, remote_control_at = excluded.at"
            } else {
                "INSERT INTO assistant_activity (account, page, at) VALUES (?1, ?2, ?3)
                 ON CONFLICT (account, page) DO UPDATE SET at = excluded.at"
            },
            params![account, page_id, now()],
        );
        if let Err(error) = result {
            warn!("failed to save the activity of {account} on {page_id}: {error:#}");
            return false;
        }
        true
    }

    pub fn get(&self, id: &str) -> Option<Account> {
        self.find("id = ?1", id)
    }

    pub fn signed_in(
        &self,
        identity: &Identity,
        new_id: String,
        page_id: String,
        moblin_cloud_key: String,
    ) -> Result<Account, AccountError> {
        let mut database = self.database.lock();
        let transaction = database.transaction().map_err(not_saved)?;
        let existing = account_of_identity(&transaction, identity.platform, &identity.user_id)
            .map_err(not_saved)?;
        let id = match existing {
            Some(id) => id,
            None => {
                let id = new_id;
                if count(&transaction).map_err(not_saved)? >= 10000 {
                    return Err(AccountError::Full);
                }
                transaction
                    .execute(
                        &format!("INSERT INTO accounts ({COLUMNS}) VALUES (?1, ?2, ?3, 0, NULL, NULL, NULL, 0, 0)"),
                        params![id, page_id, moblin_cloud_key],
                    )
                    .map_err(not_saved)?;
                transaction
                    .execute(
                        "INSERT OR IGNORE INTO pages (id, settings) VALUES (?1, '{}')",
                        [&page_id],
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
        Ok(account)
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

    pub fn of_page(&self, page_id: &str) -> Option<Account> {
        self.find("page_id = ?1", page_id)
    }

    pub fn page(&self, page_id: &str) -> Option<(Account, Look)> {
        self.database
            .lock()
            .prepare_cached(&format!(
                "SELECT {COLUMNS}, {IDENTITIES},
                        (SELECT settings FROM pages WHERE id = accounts.page_id),
                        (SELECT version FROM page_images WHERE id = accounts.page_id),
                        (SELECT icon FROM page_icons WHERE id = accounts.page_id)
                 FROM accounts WHERE page_id = ?1 LIMIT 1"
            ))
            .and_then(|mut statement| {
                statement
                    .query_row([page_id], |row| {
                        Ok((
                            account(row)?,
                            Look::new(page_id, row.get(10)?, row.get(11)?, row.get(12)?),
                        ))
                    })
                    .optional()
            })
            .unwrap_or_else(|error| {
                warn!("failed to read accounts: {error:#}");
                None
            })
    }

    pub fn get_many(&self, ids: &[&str]) -> HashMap<String, Account> {
        let ids = serde_json::to_string(ids).unwrap_or_default();
        let database = self.database.lock();
        let result = database
            .prepare_cached(&format!(
                "SELECT {COLUMNS}, {IDENTITIES} FROM accounts
                 WHERE id IN (SELECT value FROM json_each(?1))"
            ))
            .and_then(|mut statement| {
                statement
                    .query_map([ids], |row| {
                        let account = account(row)?;
                        Ok((account.id.clone(), account))
                    })?
                    .collect::<rusqlite::Result<_>>()
            });
        result.unwrap_or_else(|error| {
            warn!("failed to read accounts: {error:#}");
            HashMap::new()
        })
    }

    pub fn page_of_moblin_cloud_key(&self, moblin_cloud_key: &str) -> Option<String> {
        self.database
            .lock()
            .query_row(
                "SELECT page_id FROM accounts WHERE moblin_key = ?1",
                [moblin_cloud_key],
                |row| row.get(0),
            )
            .optional()
            .unwrap_or_else(|error| {
                warn!("failed to read accounts: {error:#}");
                None
            })
    }

    pub fn replace_moblin_cloud_key(
        &self,
        id: &str,
        moblin_cloud_key: String,
    ) -> Result<Account, AccountError> {
        self.change(id, |account| {
            account.moblin_cloud_key = moblin_cloud_key;
            Ok(())
        })
    }

    pub fn seen_device(&self, page_id: &str, device: &str) -> Option<String> {
        let mut database = self.database.lock();
        let result = database.transaction().and_then(|transaction| {
            let now = now();
            transaction.execute(
                &format!("DELETE FROM devices WHERE account = {ACCOUNT_OF_PAGE} AND id != ?2 AND last_seen < ?3"),
                params![page_id, device, now.saturating_sub(30 * 24 * 60 * 60)],
            )?;
            let known = transaction
                .query_row(
                    &format!("UPDATE devices SET last_seen = ?3 WHERE account = {ACCOUNT_OF_PAGE} AND id = ?2 RETURNING name"),
                    params![page_id, device, now],
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
                        [page_id],
                    )?;
                    let names = transaction
                        .prepare(&format!("SELECT name FROM devices WHERE account = {ACCOUNT_OF_PAGE}"))?
                        .query_map([page_id], |row| row.get(0))?
                        .collect::<rusqlite::Result<Vec<String>>>()?;
                    let name = (1..)
                        .map(|number| format!("Moblin {number}"))
                        .find(|name| !names.contains(name))
                        .unwrap();
                    transaction.execute(
                        &format!("INSERT INTO devices (account, id, name, last_seen) VALUES ({ACCOUNT_OF_PAGE}, ?2, ?3, ?4)"),
                        params![page_id, device, name, now],
                    )?;
                    name
                }
            };
            transaction.commit()?;
            Ok(name)
        });
        result
            .inspect_err(|error| warn!("failed to save device {device} of {page_id}: {error:#}"))
            .ok()
    }

    pub fn rename_device(
        &self,
        page_id: &str,
        device: &str,
        name: &str,
    ) -> Result<(), AccountError> {
        let renamed = self
            .database
            .lock()
            .execute(
                &format!(
                    "UPDATE devices SET name = ?3 WHERE account = {ACCOUNT_OF_PAGE} AND id = ?2"
                ),
                params![page_id, device, name],
            )
            .map_err(not_saved)?;
        if renamed == 0 {
            return Err(AccountError::UnknownDevice);
        }
        Ok(())
    }

    pub fn remove_device(&self, page_id: &str, device: &str) -> Result<(), AccountError> {
        let removed = self
            .database
            .lock()
            .execute(
                &format!("DELETE FROM devices WHERE account = {ACCOUNT_OF_PAGE} AND id = ?2"),
                params![page_id, device],
            )
            .map_err(not_saved)?;
        if removed == 0 {
            return Err(AccountError::UnknownDevice);
        }
        Ok(())
    }

    pub fn monitor_layout(&self, id: &str, page_id: &str) -> Option<String> {
        self.database
            .lock()
            .query_row(
                "SELECT layout FROM monitor_layouts WHERE account = ?1
                 ORDER BY page = ?2 DESC, updated_at DESC, rowid DESC LIMIT 1",
                [id, page_id],
                |row| row.get(0),
            )
            .optional()
            .unwrap_or_else(|error| {
                warn!("failed to read the monitor layout of {id}: {error:#}");
                None
            })
    }

    pub fn set_monitor_layout(
        &self,
        id: &str,
        page_id: &str,
        layout: &str,
    ) -> Result<(), AccountError> {
        self.database
            .lock()
            .execute(
                "INSERT INTO monitor_layouts (account, page, layout, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (account, page) DO UPDATE
                 SET layout = excluded.layout, updated_at = excluded.updated_at",
                params![id, page_id, layout, now()],
            )
            .map_err(not_saved)?;
        Ok(())
    }

    pub fn left_device(&self, page_id: &str, device: &str) {
        let result = self.database.lock().execute(
            &format!(
                "UPDATE devices SET last_seen = ?3 WHERE account = {ACCOUNT_OF_PAGE} AND id = ?2"
            ),
            params![page_id, device, now()],
        );
        if let Err(error) = result {
            warn!("failed to save device {device} of {page_id}: {error:#}");
        }
    }

    pub fn known_devices(&self, page_id: &str) -> Vec<KnownDevice> {
        let database = self.database.lock();
        let result = database
            .prepare_cached(&format!(
                "SELECT id, name, last_seen FROM devices WHERE account = {ACCOUNT_OF_PAGE}
                 ORDER BY last_seen DESC, rowid DESC"
            ))
            .and_then(|mut statement| {
                statement
                    .query_map([page_id], |row| {
                        Ok(KnownDevice {
                            id: row.get(0)?,
                            name: row.get(1)?,
                            last_seen: row.get(2)?,
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()
            });
        result.unwrap_or_else(|error| {
            warn!("failed to read the devices of {page_id}: {error:#}");
            Vec::new()
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

    pub fn set_premium(&self, id: &str, premium: bool) -> Result<Account, AccountError> {
        self.change(id, |account| {
            account.premium = premium;
            Ok(())
        })
    }

    pub fn premium(&self) -> Vec<Account> {
        self.list(
            "premium ORDER BY (SELECT min(login) FROM identities WHERE account = accounts.id)",
            [],
        )
    }

    pub fn search(&self, text: &str) -> Vec<Account> {
        let text = text.trim().trim_start_matches('@');
        if text.is_empty() {
            return Vec::new();
        }
        let escaped = text
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        self.list(
            "id IN (SELECT account FROM identities
                     WHERE login LIKE ?1 ESCAPE '\\' OR name LIKE ?1 ESCAPE '\\' OR user_id = ?3)
             ORDER BY NOT EXISTS (SELECT 1 FROM identities
                                  WHERE account = accounts.id AND login LIKE ?2 ESCAPE '\\'),
                      (SELECT min(login) FROM identities WHERE account = accounts.id)
             LIMIT 20",
            [
                format!("%{escaped}%"),
                format!("{escaped}%"),
                text.to_string(),
            ],
        )
    }

    fn list(&self, condition: &str, values: impl rusqlite::Params) -> Vec<Account> {
        let database = self.database.lock();
        let result = database
            .prepare_cached(&format!(
                "SELECT {COLUMNS}, {IDENTITIES} FROM accounts WHERE {condition}"
            ))
            .and_then(|mut statement| {
                statement
                    .query_map(values, account)?
                    .collect::<rusqlite::Result<_>>()
            });
        result.unwrap_or_else(|error| {
            warn!("failed to read accounts: {error:#}");
            Vec::new()
        })
    }

    pub fn forget_stripe(&self, id: &str) -> Result<Account, AccountError> {
        self.change(id, |account| {
            account.stripe_account_id = None;
            account.stripe_ready = false;
            Ok(())
        })
    }

    pub fn add_assistant(
        &self,
        platform: Platform,
        broadcaster: &str,
        assistant: &Assistant,
    ) -> Result<(), AccountError> {
        let table = assistants_table(platform);
        let mut database = self.database.lock();
        let transaction = database.transaction().map_err(not_saved)?;
        let listed: u32 = transaction
            .query_row(
                &format!("SELECT count(*) FROM {table} WHERE broadcaster = ?1 AND assistant != ?2"),
                [broadcaster, &assistant.user_id],
                |row| row.get(0),
            )
            .map_err(not_saved)?;
        if listed >= 50 {
            return Err(AccountError::TooManyAssistants);
        }
        transaction
            .execute(
                &format!(
                    "INSERT INTO {table} (broadcaster, assistant, login, name) VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT (broadcaster, assistant) DO UPDATE SET login = excluded.login, name = excluded.name"
                ),
                [broadcaster, &assistant.user_id, &assistant.login, &assistant.name],
            )
            .map_err(not_saved)?;
        transaction.commit().map_err(not_saved)
    }

    pub fn remove_assistant(
        &self,
        platform: Platform,
        broadcaster: &str,
        assistant: &str,
    ) -> Result<(), AccountError> {
        let table = assistants_table(platform);
        let removed = self
            .database
            .lock()
            .execute(
                &format!("DELETE FROM {table} WHERE broadcaster = ?1 AND assistant = ?2"),
                [broadcaster, assistant],
            )
            .map_err(not_saved)?;
        if removed == 0 {
            return Err(AccountError::UnknownAssistant);
        }
        Ok(())
    }

    pub fn set_assistant_permission(
        &self,
        platform: Platform,
        broadcaster: &str,
        assistant: &str,
        permission: Permission,
        on: bool,
    ) -> Result<(), AccountError> {
        self.set_assistant_column(
            platform,
            broadcaster,
            assistant,
            permission.column(),
            on as u8,
        )
    }

    pub fn set_assistant_access(
        &self,
        platform: Platform,
        broadcaster: &str,
        assistant: &str,
        access: Access,
    ) -> Result<(), AccountError> {
        self.set_assistant_column(
            platform,
            broadcaster,
            assistant,
            "remote_control",
            access as u8,
        )
    }

    fn set_assistant_column(
        &self,
        platform: Platform,
        broadcaster: &str,
        assistant: &str,
        column: &str,
        value: u8,
    ) -> Result<(), AccountError> {
        let table = assistants_table(platform);
        let changed = self
            .database
            .lock()
            .execute(
                &format!(
                    "UPDATE {table} SET {column} = ?3 WHERE broadcaster = ?1 AND assistant = ?2"
                ),
                (broadcaster, assistant, value),
            )
            .map_err(not_saved)?;
        if changed == 0 {
            return Err(AccountError::UnknownAssistant);
        }
        Ok(())
    }

    pub fn assistants_of(
        &self,
        broadcasters: [Option<&str>; 2],
    ) -> (Vec<Assistant>, Vec<Assistant>) {
        let arm = |platform: Platform, broadcaster: &str| {
            let table = assistants_table(platform);
            let name = platform.name();
            format!(
                "SELECT '{name}' AS platform, assistant, coalesce(identities.login, {table}.login), coalesce(identities.name, {table}.name) AS name, identities.avatar, preview, go_live, edit_tips, remote_control, assistant_activity.at, assistant_activity.remote_control_at
                 FROM {table}
                 LEFT JOIN identities ON identities.platform = '{name}' AND identities.user_id = assistant
                 LEFT JOIN assistant_activity ON assistant_activity.account = identities.account
                     AND assistant_activity.page = (
                         SELECT accounts.page_id FROM identities AS streamer
                         JOIN accounts ON accounts.id = streamer.account
                         WHERE streamer.platform = '{name}' AND streamer.user_id = {broadcaster}
                     )
                 WHERE broadcaster = {broadcaster}"
            )
        };
        let database = self.database.lock();
        let result = database
            .prepare_cached(&format!(
                "{} UNION ALL {} ORDER BY platform DESC, name COLLATE NOCASE",
                arm(Platform::Twitch, "?1"),
                arm(Platform::Kick, "?2")
            ))
            .and_then(|mut statement| {
                statement
                    .query_map(broadcasters, |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            Assistant {
                                user_id: row.get(1)?,
                                login: row.get(2)?,
                                name: row.get(3)?,
                                avatar: row.get(4)?,
                                preview: row.get(5)?,
                                go_live: row.get(6)?,
                                edit_tips: row.get(7)?,
                                remote_control: Access::of(row.get(8)?),
                                last_active: row.get(9)?,
                                last_remote_control: row.get(10)?,
                            },
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()
            });
        let mut twitch = Vec::new();
        let mut kick = Vec::new();
        for (platform, assistant) in result.unwrap_or_else(|error| {
            warn!("failed to read assistants: {error:#}");
            Vec::new()
        }) {
            match Platform::parse(&platform) {
                Some(Platform::Twitch) => twitch.push(assistant),
                Some(Platform::Kick) => kick.push(assistant),
                None => {}
            }
        }
        (twitch, kick)
    }

    pub fn rights(&self, platform: Platform, assistant: &str, broadcaster: &str) -> Option<Rights> {
        let table = assistants_table(platform);
        self.database
            .lock()
            .query_row(
                &format!(
                    "SELECT preview, go_live, edit_tips, remote_control FROM {table} WHERE assistant = ?1 AND broadcaster = ?2"
                ),
                [assistant, broadcaster],
                |row| {
                    Ok(Rights {
                        preview: row.get(0)?,
                        go_live: row.get(1)?,
                        edit_tips: row.get(2)?,
                        remote_control: Access::of(row.get(3)?),
                    })
                },
            )
            .optional()
            .unwrap_or_else(|error| {
                warn!("failed to read {} assistants: {error:#}", platform.title());
                None
            })
    }

    pub fn assisted_by(&self, assistants: [Option<&str>; 2]) -> Vec<(Platform, Account)> {
        let arm = |platform: Platform, assistant: &str| {
            let table = assistants_table(platform);
            let name = platform.name();
            format!(
                "SELECT {COLUMNS}, {IDENTITIES}, '{name}' AS platform,
                        (SELECT name FROM identities WHERE account = accounts.id AND platform = '{name}') AS channel
                 FROM accounts
                 WHERE id IN (SELECT identities.account FROM {table}
                              JOIN identities ON identities.platform = '{name}' AND identities.user_id = broadcaster
                              WHERE assistant = {assistant})"
            )
        };
        let database = self.database.lock();
        let result = database
            .prepare_cached(&format!(
                "{} UNION ALL {} ORDER BY platform DESC, channel COLLATE NOCASE",
                arm(Platform::Twitch, "?1"),
                arm(Platform::Kick, "?2")
            ))
            .and_then(|mut statement| {
                statement
                    .query_map(assistants, |row| {
                        Ok((row.get::<_, String>(10)?, account(row)?))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()
            });
        result
            .unwrap_or_else(|error| {
                warn!("failed to read assistants: {error:#}");
                Vec::new()
            })
            .into_iter()
            .filter_map(|(platform, account)| Some((Platform::parse(&platform)?, account)))
            .collect()
    }

    pub fn page_of_identity(&self, platform: Platform, user_id: &str) -> Option<String> {
        self.database
            .lock()
            .query_row(
                "SELECT page_id FROM identities JOIN accounts ON accounts.id = identities.account
                 WHERE platform = ?1 AND user_id = ?2",
                [platform.name(), user_id],
                |row| row.get(0),
            )
            .optional()
            .unwrap_or_else(|error| {
                warn!("failed to read identities: {error:#}");
                None
            })
    }

    pub fn pages_of_logins(
        &self,
        logins: &[(Platform, String)],
    ) -> Vec<(Platform, String, String)> {
        let logins = logins
            .iter()
            .map(|(platform, login)| [platform.name(), login.as_str()])
            .collect::<Vec<_>>();
        let logins = serde_json::to_string(&logins).unwrap_or_default();
        let database = self.database.lock();
        let result = database
            .prepare_cached(
                "SELECT platform, identities.login, page_id FROM identities
                 JOIN accounts ON accounts.id = identities.account
                 WHERE identities.login != '' AND (platform, identities.login) IN
                     (SELECT json_extract(value, '$[0]'), json_extract(value, '$[1]') FROM json_each(?1))",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([logins], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()
            });
        result
            .unwrap_or_else(|error| {
                warn!("failed to read identities: {error:#}");
                Vec::new()
            })
            .into_iter()
            .filter_map(|(platform, login, page_id)| {
                Some((Platform::parse(&platform)?, login, page_id))
            })
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
        let page_id = account.page_id.as_str();
        let statements = [
            ("DELETE FROM accounts WHERE id = ?1", id),
            ("DELETE FROM identities WHERE account = ?1", id),
            ("DELETE FROM devices WHERE account = ?1", id),
            ("DELETE FROM monitor_layouts WHERE account = ?1", id),
            ("DELETE FROM monitor_layouts WHERE page = ?1", page_id),
            ("DELETE FROM assistant_activity WHERE account = ?1", id),
            ("DELETE FROM assistant_activity WHERE page = ?1", page_id),
            ("DELETE FROM pages WHERE id = ?1", page_id),
            ("DELETE FROM page_images WHERE id = ?1", page_id),
            ("DELETE FROM page_icons WHERE id = ?1", page_id),
            ("DELETE FROM orders WHERE streamer = ?1", page_id),
            ("DELETE FROM deliveries WHERE streamer = ?1", page_id),
            ("DELETE FROM subscriber_allowances WHERE page = ?1", page_id),
        ];
        for (statement, key) in statements {
            transaction.execute(statement, [key]).map_err(not_saved)?;
        }
        for identity in &account.identities {
            transaction
                .execute(
                    &format!(
                        "DELETE FROM {} WHERE broadcaster = ?1 OR assistant = ?1",
                        assistants_table(identity.platform)
                    ),
                    [&identity.user_id],
                )
                .map_err(not_saved)?;
        }
        transaction.commit().map_err(not_saved)?;
        Ok(account)
    }

    pub fn stats(&self, admins: &[&str]) -> AccountStats {
        self.database
            .lock()
            .query_row(
                "SELECT count(*), count(merchant_id), coalesce(sum(stripe_ready), 0),
                        count(*) FILTER (WHERE premium OR EXISTS (
                            SELECT 1 FROM identities WHERE identities.account = accounts.id
                            AND platform || ':' || user_id
                                IN (SELECT value FROM json_each(?1))
                        ))
                 FROM accounts",
                [serde_json::to_string(admins).unwrap()],
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
                &format!("INSERT OR REPLACE INTO accounts ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"),
                params![
                    id,
                    account.page_id,
                    account.moblin_cloud_key,
                    account.sessions,
                    account.merchant_id,
                    account.paypal_tracking_id,
                    account.stripe_account_id,
                    account.stripe_ready,
                    account.premium,
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
#[path = "tests/accounts.rs"]
mod tests;
