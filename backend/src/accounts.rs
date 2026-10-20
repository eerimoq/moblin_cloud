use std::sync::Arc;

use log::{info, warn};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use crate::database::Database;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Account {
    pub name: String,
    pub login: String,
    pub avatar: Option<String>,
    pub page: Page,
    pub sessions: u64,
    pub merchant_id: Option<String>,
    pub paypal_tracking_id: Option<String>,
    pub stripe_account_id: Option<String>,
    pub stripe_ready: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    pub id: String,
    pub devices: Vec<Device>,
    pub music_device: Option<String>,
    pub wheel_device: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub key: String,
}

impl Page {
    pub fn new(id: String, device: Device) -> Self {
        Self {
            id,
            devices: vec![device],
            music_device: None,
            wheel_device: None,
        }
    }

    pub fn music_device(&self) -> Option<&Device> {
        self.device_or_first(&self.music_device)
    }

    pub fn wheel_device(&self) -> Option<&Device> {
        self.device_or_first(&self.wheel_device)
    }

    fn device_or_first(&self, id: &Option<String>) -> Option<&Device> {
        self.devices
            .iter()
            .find(|device| Some(&device.id) == id.as_ref())
            .or(self.devices.first())
    }

    fn device_mut(&mut self, id: &str) -> Result<&mut Device, AccountError> {
        self.devices
            .iter_mut()
            .find(|device| device.id == id)
            .ok_or(AccountError::UnknownDevice)
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
    TooManyDevices,
    UnknownDevice,
    NotSaved,
}

const COLUMNS: &str = "name, login, avatar, page_id, music_device, wheel_device, sessions, merchant_id, paypal_tracking_id, stripe_account_id, stripe_ready";

const DEVICES: &str = "(SELECT json_group_array(json_object('id', id, 'name', name, 'key', key) ORDER BY position) FROM devices WHERE account = accounts.id)";

const SAME_LOGIN: &str = "login != '' AND login = lower(trim(?1))";

fn account(row: &Row) -> rusqlite::Result<Account> {
    let devices: String = row.get(11)?;
    let devices = serde_json::from_str(&devices).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(11, rusqlite::types::Type::Text, error.into())
    })?;
    Ok(Account {
        name: row.get(0)?,
        login: row.get(1)?,
        avatar: row.get(2)?,
        page: Page {
            id: row.get(3)?,
            devices,
            music_device: row.get(4)?,
            wheel_device: row.get(5)?,
        },
        sessions: row.get(6)?,
        merchant_id: row.get(7)?,
        paypal_tracking_id: row.get(8)?,
        stripe_account_id: row.get(9)?,
        stripe_ready: row.get(10)?,
    })
}

fn save_devices(connection: &Connection, id: &str, devices: &[Device]) -> rusqlite::Result<()> {
    connection.execute("DELETE FROM devices WHERE account = ?1", [id])?;
    for (position, device) in devices.iter().enumerate() {
        connection.execute(
            "INSERT INTO devices (key, account, id, name, position) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![device.key, id, device.id, device.name, position],
        )?;
    }
    Ok(())
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
        id: &str,
        name: &str,
        login: &str,
        avatar: Option<&str>,
        page: Page,
    ) -> Result<Account, AccountError> {
        let account = self.change(id, Some(page), |account| {
            account.name = name.to_string();
            account.login = login.to_string();
            account.avatar = avatar.map(str::to_string);
            Ok(())
        })?;
        if let Err(error) = self.database.lock().execute(
            &format!("UPDATE accounts SET login = '' WHERE id != ?2 AND {SAME_LOGIN}"),
            params![login, id],
        ) {
            warn!("failed to save accounts: {error:#}");
        }
        Ok(account)
    }

    pub fn page_of_login(&self, login: &str) -> Option<String> {
        Some(self.find(SAME_LOGIN, login)?.page.id)
    }

    pub fn of_page(&self, page: &str) -> Option<Account> {
        self.find("page_id = ?1", page)
    }

    pub fn streamer_of_page(&self, page: &str) -> Option<(String, String)> {
        self.database
            .lock()
            .query_row(
                "SELECT id, login FROM accounts WHERE page_id = ?1",
                [page],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .unwrap_or_else(|error| {
                warn!("failed to read accounts: {error:#}");
                None
            })
    }

    pub fn device_of_key(&self, key: &str) -> Option<String> {
        self.database
            .lock()
            .query_row("SELECT id FROM devices WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
            .unwrap_or_else(|error| {
                warn!("failed to read devices: {error:#}");
                None
            })
    }

    pub fn give_missing_pages(&self, mut new_page: impl FnMut() -> Option<Page>) {
        let mut database = self.database.lock();
        let ids: Vec<String> = match database
            .prepare("SELECT id FROM accounts WHERE page_id IS NULL")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get(0))?
                    .collect::<rusqlite::Result<_>>()
            }) {
            Ok(ids) => ids,
            Err(error) => {
                warn!("failed to read accounts: {error:#}");
                return;
            }
        };
        for id in ids {
            let Some(page) = new_page() else {
                warn!("no page could be created for account {id}");
                return;
            };
            let saved = database.transaction().and_then(|transaction| {
                transaction.execute(
                    "UPDATE accounts SET page_id = ?2 WHERE id = ?1",
                    params![id, page.id],
                )?;
                save_devices(&transaction, &id, &page.devices)?;
                transaction.commit()
            });
            if let Err(error) = saved {
                warn!("failed to save accounts: {error:#}");
            } else {
                info!("{id} got a page");
            }
        }
    }

    pub fn add_device(&self, id: &str, device: Device) -> Result<Account, AccountError> {
        self.change(id, None, |account| {
            let page = &mut account.page;
            if page.devices.len() >= 5 {
                return Err(AccountError::TooManyDevices);
            }
            page.devices.push(device);
            Ok(())
        })
    }

    pub fn remove_device(&self, id: &str, device: &str) -> Result<Account, AccountError> {
        self.change(id, None, |account| {
            let page = &mut account.page;
            page.device_mut(device)?;
            page.devices.retain(|other| other.id != device);
            for chosen in [&mut page.music_device, &mut page.wheel_device] {
                if chosen.as_deref() == Some(device) {
                    *chosen = None;
                }
            }
            Ok(())
        })
    }

    pub fn replace_key(
        &self,
        id: &str,
        device: &str,
        key: String,
    ) -> Result<Account, AccountError> {
        self.change(id, None, |account| {
            let page = &mut account.page;
            page.device_mut(device)?.key = key;
            Ok(())
        })
    }

    pub fn choose_devices(
        &self,
        id: &str,
        music: String,
        wheel: String,
    ) -> Result<Account, AccountError> {
        self.change(id, None, |account| {
            let page = &mut account.page;
            page.device_mut(&music)?;
            page.device_mut(&wheel)?;
            page.music_device = Some(music);
            page.wheel_device = Some(wheel);
            Ok(())
        })
    }

    pub fn start_paypal(&self, id: &str, tracking_id: String) -> Result<Account, AccountError> {
        self.change(id, None, |account| {
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
        self.change(id, None, |account| {
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
            .prepare("UPDATE accounts SET merchant_id = NULL WHERE merchant_id = ?1 RETURNING id")
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
        self.change(id, None, |account| {
            account.merchant_id = None;
            account.paypal_tracking_id = None;
            Ok(())
        })
    }

    pub fn start_stripe(&self, id: &str, account_id: String) -> Result<Account, AccountError> {
        self.change(id, None, |account| {
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
        self.change(id, None, |account| {
            if account.stripe_account_id.as_deref() == Some(account_id) {
                account.stripe_ready = ready;
            }
            Ok(())
        })
    }

    pub fn forget_stripe(&self, id: &str) -> Result<Account, AccountError> {
        self.change(id, None, |account| {
            account.stripe_account_id = None;
            account.stripe_ready = false;
            Ok(())
        })
    }

    pub fn sign_out(&self, id: &str) -> Result<Account, AccountError> {
        self.change(id, None, |account| {
            account.sessions += 1;
            Ok(())
        })
    }

    pub fn stats(&self, premium: &[&str]) -> AccountStats {
        self.database
            .lock()
            .query_row(
                "SELECT count(*), count(merchant_id), coalesce(sum(stripe_ready), 0),
                        count(*) FILTER (WHERE id IN (SELECT value FROM json_each(?1)))
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
            .query_row(
                &format!("SELECT {COLUMNS}, {DEVICES} FROM accounts WHERE {condition} LIMIT 1"),
                [value],
                account,
            )
            .optional()
            .unwrap_or_else(|error| {
                warn!("failed to read accounts: {error:#}");
                None
            })
    }

    fn change(
        &self,
        id: &str,
        create: Option<Page>,
        change: impl FnOnce(&mut Account) -> Result<(), AccountError>,
    ) -> Result<Account, AccountError> {
        let mut database = self.database.lock();
        let transaction = database.transaction().map_err(not_saved)?;
        let old = transaction
            .query_row(
                &format!("SELECT {COLUMNS}, {DEVICES} FROM accounts WHERE id = ?1"),
                [id],
                account,
            )
            .optional()
            .map_err(not_saved)?;
        let mut account = match &old {
            Some(account) => account.clone(),
            None => match create {
                None => return Err(AccountError::Unknown),
                Some(_) if count(&transaction).map_err(not_saved)? >= 10000 => {
                    return Err(AccountError::Full);
                }
                Some(page) => Account {
                    page,
                    ..Account::default()
                },
            },
        };
        change(&mut account)?;
        if old.as_ref() == Some(&account) {
            return Ok(account);
        }
        transaction
            .execute(
                &format!("INSERT OR REPLACE INTO accounts (id, {COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"),
                params![
                    id,
                    account.name,
                    account.login,
                    account.avatar,
                    account.page.id,
                    account.page.music_device,
                    account.page.wheel_device,
                    account.sessions,
                    account.merchant_id,
                    account.paypal_tracking_id,
                    account.stripe_account_id,
                    account.stripe_ready,
                ],
            )
            .map_err(not_saved)?;
        if old.as_ref().map(|old| &old.page.devices) != Some(&account.page.devices) {
            save_devices(&transaction, id, &account.page.devices).map_err(not_saved)?;
        }
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

    fn device(id: &str, key: &str) -> Device {
        Device {
            id: id.to_string(),
            name: "Moblin".to_string(),
            key: key.to_string(),
        }
    }

    fn page(id: &str, key: &str) -> Page {
        Page::new(id.to_string(), device(id, key))
    }

    #[test]
    fn pages_are_kept() {
        let accounts = Accounts::new(Database::memory());
        assert_eq!(accounts.get("1"), None);
        assert_eq!(
            accounts.signed_in("1", "Erik", "erik", None, page("p", "k")),
            Ok(Account {
                name: "Erik".to_string(),
                login: "erik".to_string(),
                page: page("p", "k"),
                ..Account::default()
            })
        );
        assert_eq!(
            accounts
                .signed_in("1", "Eerimoq", "eerimoq", None, page("q", "o"))
                .unwrap()
                .page,
            page("p", "k")
        );
        assert_eq!(accounts.get("1").unwrap().name, "Eerimoq");
    }

    #[test]
    fn accounts_without_a_page_get_one() {
        let accounts = Accounts::new(Database::memory());
        accounts
            .signed_in("1", "Erik", "erik", None, page("p", "k"))
            .unwrap();
        accounts
            .database
            .lock()
            .execute_batch(
                "INSERT INTO accounts (id, name, login, sessions) VALUES ('2', 'Old', 'old', 0), ('3', 'Older', 'older', 0)",
            )
            .unwrap();
        assert_eq!(accounts.get("2"), None);
        let mut pages = vec![page("b", "l"), page("a", "m")];
        accounts.give_missing_pages(|| pages.pop());
        assert_eq!(accounts.get("1").unwrap().page, page("p", "k"));
        assert_eq!(accounts.get("2").unwrap().page, page("a", "m"));
        assert_eq!(accounts.get("3").unwrap().page, page("b", "l"));
        assert_eq!(accounts.device_of_key("l").as_deref(), Some("b"));
        accounts.give_missing_pages(|| panic!("no page is missing"));
    }

    #[test]
    fn pages_find_their_account() {
        let accounts = Accounts::new(Database::memory());
        accounts
            .signed_in(
                "1",
                "Erik",
                "erik",
                Some("https://example.com/a.png"),
                page("p", "k"),
            )
            .unwrap();
        assert_eq!(accounts.of_page("q"), None);
        let account = accounts.of_page("p").unwrap();
        assert_eq!(account.login, "erik");
        assert_eq!(account.avatar.as_deref(), Some("https://example.com/a.png"));
        accounts
            .signed_in("1", "Erik", "erik", None, page("p", "k"))
            .unwrap();
        assert_eq!(accounts.of_page("p").unwrap().avatar, None);
    }

    #[test]
    fn keys_are_replaced() {
        let accounts = Accounts::new(Database::memory());
        accounts
            .signed_in("1", "Erik", "erik", None, page("p", "k"))
            .unwrap();
        assert_eq!(accounts.device_of_key("k").as_deref(), Some("p"));
        assert_eq!(
            accounts.replace_key("1", "d", "n".to_string()),
            Err(AccountError::UnknownDevice)
        );
        assert_eq!(
            accounts
                .replace_key("1", "p", "n".to_string())
                .unwrap()
                .page,
            page("p", "n")
        );
        assert_eq!(accounts.device_of_key("k"), None);
        assert_eq!(accounts.device_of_key("n").as_deref(), Some("p"));
    }

    #[test]
    fn devices_are_added_chosen_and_removed() {
        let accounts = Accounts::new(Database::memory());
        assert_eq!(
            accounts.add_device("1", device("a", "ka")),
            Err(AccountError::Unknown)
        );
        accounts
            .signed_in("1", "Erik", "erik", None, page("p", "k"))
            .unwrap();
        for name in ["a", "b", "c", "d"] {
            accounts
                .add_device("1", device(name, &format!("k{name}")))
                .unwrap();
        }
        assert_eq!(
            accounts.add_device("1", device("e", "ke")),
            Err(AccountError::TooManyDevices)
        );
        assert_eq!(accounts.device_of_key("kc").as_deref(), Some("c"));
        let page = accounts.get("1").unwrap().page;
        assert_eq!(page.music_device().unwrap().id, "p");
        assert_eq!(page.wheel_device().unwrap().id, "p");
        assert_eq!(
            accounts.choose_devices("1", "a".to_string(), "x".to_string()),
            Err(AccountError::UnknownDevice)
        );
        let page = accounts
            .choose_devices("1", "a".to_string(), "b".to_string())
            .unwrap()
            .page;
        assert_eq!(page.music_device().unwrap().id, "a");
        assert_eq!(page.wheel_device().unwrap().id, "b");
        let page = accounts.remove_device("1", "a").unwrap().page;
        assert_eq!(page.music_device, None);
        assert_eq!(page.music_device().unwrap().id, "p");
        assert_eq!(page.wheel_device().unwrap().id, "b");
        assert_eq!(accounts.device_of_key("ka"), None);
        assert_eq!(
            accounts.remove_device("1", "a"),
            Err(AccountError::UnknownDevice)
        );
        for name in ["p", "b", "c", "d"] {
            accounts.remove_device("1", name).unwrap();
        }
        let page = accounts.get("1").unwrap().page;
        assert!(page.devices.is_empty());
        assert_eq!(page.music_device(), None);
        assert_eq!(accounts.page_of_login("erik").as_deref(), Some("p"));
    }

    #[test]
    fn signing_out_ends_sessions() {
        let accounts = Accounts::new(Database::memory());
        assert_eq!(accounts.sign_out("1"), Err(AccountError::Unknown));
        accounts
            .signed_in("1", "Erik", "erik", None, page("p", "k"))
            .unwrap();
        assert_eq!(accounts.sign_out("1").unwrap().sessions, 1);
        assert_eq!(
            accounts
                .signed_in("1", "Erik", "erik", None, page("q", "o"))
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
        accounts
            .signed_in("1", "Erik", "erik", None, page("p", "k"))
            .unwrap();
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
        accounts
            .signed_in("1", "Erik", "erik", None, page("p", "k"))
            .unwrap();
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
    fn old_databases_get_the_stripe_columns_and_devices() {
        let path = database::temporary_path("old-accounts");
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE accounts (
                     id TEXT PRIMARY KEY,
                     name TEXT NOT NULL,
                     login TEXT NOT NULL,
                     avatar TEXT,
                     page_id TEXT,
                     page_key TEXT,
                     sessions INTEGER NOT NULL,
                     merchant_id TEXT,
                     paypal_tracking_id TEXT
                 );
                 INSERT INTO accounts VALUES ('1', 'Erik', 'erik', NULL, 'p', 'k', 0, 'M', NULL);",
            )
            .unwrap();
        let accounts = Accounts::new(Database::open(Some(&path)).unwrap());
        let account = accounts.get("1").unwrap();
        assert_eq!(account.merchant_id.as_deref(), Some("M"));
        assert_eq!(account.stripe_account_id, None);
        assert_eq!(account.page.id, "p");
        assert!(account.page.devices.is_empty());
        assert_eq!(accounts.device_of_key("k"), None);
        accounts.start_stripe("1", "acct_1".to_string()).unwrap();
        accounts.stripe_ready("1", "acct_1", true).unwrap();
        let accounts = Accounts::new(Database::open(Some(&path)).unwrap());
        assert!(accounts.get("1").unwrap().stripe_ready);
        database::remove(&path);
    }

    #[test]
    fn old_databases_get_their_devices_moved() {
        let path = database::temporary_path("old-devices");
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE accounts (
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
                 INSERT INTO accounts (id, name, login, page_id, page_devices, wheel_device, sessions) VALUES
                     ('1', 'Erik', 'erik', 'p', '[{\"id\":\"p\",\"name\":\"Moblin\",\"key\":\"k\"},{\"id\":\"b\",\"name\":\"Tablet\",\"key\":\"l\"}]', 'b', 0),
                     ('2', 'Old', 'old', NULL, NULL, NULL, 0);",
            )
            .unwrap();
        let accounts = Accounts::new(Database::open(Some(&path)).unwrap());
        let page = accounts.get("1").unwrap().page;
        assert_eq!(
            page.devices,
            [
                device("p", "k"),
                Device {
                    id: "b".to_string(),
                    name: "Tablet".to_string(),
                    key: "l".to_string(),
                }
            ]
        );
        assert_eq!(page.wheel_device().unwrap().id, "b");
        assert_eq!(accounts.device_of_key("l").as_deref(), Some("b"));
        assert_eq!(accounts.get("2"), None);
        let accounts = Accounts::new(Database::open(Some(&path)).unwrap());
        assert_eq!(accounts.device_of_key("k").as_deref(), Some("p"));
        database::remove(&path);
    }

    #[test]
    fn logins_find_pages() {
        let accounts = Accounts::new(Database::memory());
        assert_eq!(accounts.page_of_login("erik"), None);
        accounts
            .signed_in("1", "Erik", "erik", None, page("a", "k"))
            .unwrap();
        assert_eq!(accounts.page_of_login("ERIK").as_deref(), Some("a"));
        assert_eq!(accounts.page_of_login("other"), None);
        assert_eq!(accounts.page_of_login(""), None);
        accounts
            .signed_in("2", "Erik", "erik", None, page("c", "l"))
            .unwrap();
        assert_eq!(accounts.page_of_login("erik").as_deref(), Some("c"));
        assert_eq!(accounts.get("1").unwrap().login, "");
    }

    #[test]
    fn premium_accounts_are_counted() {
        let accounts = Accounts::new(Database::memory());
        assert_eq!(accounts.stats(&["1"]).premium, 0);
        accounts
            .signed_in("1", "Erik", "erik", None, page("p", "k"))
            .unwrap();
        accounts
            .signed_in("2", "Other", "other", None, page("q", "l"))
            .unwrap();
        let stats = accounts.stats(&["1", "1", "3"]);
        assert_eq!((stats.total, stats.premium), (2, 1));
        assert_eq!(accounts.stats(&[]).premium, 0);
    }

    #[test]
    fn accounts_are_saved() {
        let path = database::temporary_path("accounts");
        let accounts = Accounts::new(Database::open(Some(&path)).unwrap());
        accounts
            .signed_in("3", "erik", "erik", None, page("p", "k"))
            .unwrap();
        let accounts = Accounts::new(Database::open(Some(&path)).unwrap());
        assert_eq!(
            accounts.get("3"),
            Some(Account {
                name: "erik".to_string(),
                login: "erik".to_string(),
                avatar: None,
                page: page("p", "k"),
                ..Account::default()
            })
        );
        database::remove(&path);
    }
}
