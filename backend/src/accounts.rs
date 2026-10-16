use std::sync::Arc;

use log::warn;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::database::Database;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Account {
    pub name: String,
    pub login: String,
    pub avatar: Option<String>,
    pub page: Option<Page>,
    pub sessions: u64,
    pub merchant_id: Option<String>,
    pub paypal_tracking_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub id: String,
    pub key: String,
}

#[derive(Debug, PartialEq)]
pub enum AccountError {
    Unknown,
    NoPage,
    Full,
    NotSaved,
}

const COLUMNS: &str =
    "name, login, avatar, page_id, page_key, sessions, merchant_id, paypal_tracking_id";

const SAME_LOGIN: &str = "login != '' AND lower(login) = lower(trim(?1))";

fn account(row: &Row) -> rusqlite::Result<Account> {
    let page_id: Option<String> = row.get(3)?;
    let page_key: Option<String> = row.get(4)?;
    Ok(Account {
        name: row.get(0)?,
        login: row.get(1)?,
        avatar: row.get(2)?,
        page: page_id.zip(page_key).map(|(id, key)| Page { id, key }),
        sessions: row.get(5)?,
        merchant_id: row.get(6)?,
        paypal_tracking_id: row.get(7)?,
    })
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
    ) -> Result<Account, AccountError> {
        let account = self.change(id, true, |account| {
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
        self.find(SAME_LOGIN, login)?.page.map(|page| page.id)
    }

    pub fn of_page(&self, page: &str) -> Option<Account> {
        self.find("page_id = ?1", page)
    }

    pub fn page_of_key(&self, key: &str) -> Option<String> {
        self.find("page_key = ?1", key)?.page.map(|page| page.id)
    }

    pub fn give_page(&self, id: &str, page: Page) -> Result<Account, AccountError> {
        self.change(id, false, |account| {
            account.page.get_or_insert(page);
            Ok(())
        })
    }

    pub fn replace_key(&self, id: &str, key: String) -> Result<Account, AccountError> {
        self.change(id, false, |account| {
            account.page.as_mut().ok_or(AccountError::NoPage)?.key = key;
            Ok(())
        })
    }

    pub fn start_paypal(&self, id: &str, tracking_id: String) -> Result<Account, AccountError> {
        self.change(id, false, |account| {
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
        self.change(id, false, |account| {
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
        self.change(id, false, |account| {
            account.merchant_id = None;
            account.paypal_tracking_id = None;
            Ok(())
        })
    }

    pub fn sign_out(&self, id: &str) -> Result<Account, AccountError> {
        self.change(id, false, |account| {
            account.sessions += 1;
            Ok(())
        })
    }

    fn find(&self, condition: &str, value: &str) -> Option<Account> {
        self.database
            .lock()
            .query_row(
                &format!("SELECT {COLUMNS} FROM accounts WHERE {condition} LIMIT 1"),
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
        create: bool,
        change: impl FnOnce(&mut Account) -> Result<(), AccountError>,
    ) -> Result<Account, AccountError> {
        let database = self.database.lock();
        let old = database
            .query_row(
                &format!("SELECT {COLUMNS} FROM accounts WHERE id = ?1"),
                [id],
                account,
            )
            .optional()
            .map_err(not_saved)?;
        let mut account = match &old {
            Some(account) => account.clone(),
            None if !create => return Err(AccountError::Unknown),
            None if count(&database).map_err(not_saved)? >= 10000 => {
                return Err(AccountError::Full);
            }
            None => Account::default(),
        };
        change(&mut account)?;
        if old.as_ref() == Some(&account) {
            return Ok(account);
        }
        database
            .execute(
                &format!("INSERT OR REPLACE INTO accounts (id, {COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"),
                params![
                    id,
                    account.name,
                    account.login,
                    account.avatar,
                    account.page.as_ref().map(|page| &page.id),
                    account.page.as_ref().map(|page| &page.key),
                    account.sessions,
                    account.merchant_id,
                    account.paypal_tracking_id,
                ],
            )
            .map_err(not_saved)?;
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

    #[test]
    fn pages_are_kept() {
        let accounts = Accounts::new(Database::memory());
        assert_eq!(accounts.get("1"), None);
        assert_eq!(
            accounts.give_page("1", page("p", "k")),
            Err(AccountError::Unknown)
        );
        assert_eq!(
            accounts.signed_in("1", "Erik", "erik", None),
            Ok(Account {
                name: "Erik".to_string(),
                login: "erik".to_string(),
                ..Account::default()
            })
        );
        assert_eq!(
            accounts.replace_key("1", "k".to_string()),
            Err(AccountError::NoPage)
        );
        accounts.give_page("1", page("p", "k")).unwrap();
        assert_eq!(
            accounts.give_page("1", page("q", "o")).unwrap().page,
            Some(page("p", "k"))
        );
        assert_eq!(
            accounts
                .signed_in("1", "Eerimoq", "eerimoq", None)
                .unwrap()
                .page,
            Some(page("p", "k"))
        );
        assert_eq!(accounts.get("1").unwrap().name, "Eerimoq");
    }

    #[test]
    fn pages_find_their_account() {
        let accounts = Accounts::new(Database::memory());
        accounts
            .signed_in("1", "Erik", "erik", Some("https://example.com/a.png"))
            .unwrap();
        assert_eq!(accounts.of_page("p"), None);
        accounts.give_page("1", page("p", "k")).unwrap();
        let account = accounts.of_page("p").unwrap();
        assert_eq!(account.login, "erik");
        assert_eq!(account.avatar.as_deref(), Some("https://example.com/a.png"));
        accounts.signed_in("1", "Erik", "erik", None).unwrap();
        assert_eq!(accounts.of_page("p").unwrap().avatar, None);
    }

    #[test]
    fn keys_are_replaced() {
        let accounts = Accounts::new(Database::memory());
        accounts.signed_in("1", "Erik", "erik", None).unwrap();
        accounts.give_page("1", page("p", "k")).unwrap();
        assert_eq!(accounts.page_of_key("k").as_deref(), Some("p"));
        assert_eq!(
            accounts.replace_key("1", "n".to_string()).unwrap().page,
            Some(page("p", "n"))
        );
        assert_eq!(accounts.page_of_key("k"), None);
        assert_eq!(accounts.page_of_key("n").as_deref(), Some("p"));
    }

    #[test]
    fn signing_out_ends_sessions() {
        let accounts = Accounts::new(Database::memory());
        assert_eq!(accounts.sign_out("1"), Err(AccountError::Unknown));
        accounts.signed_in("1", "Erik", "erik", None).unwrap();
        assert_eq!(accounts.sign_out("1").unwrap().sessions, 1);
        assert_eq!(
            accounts
                .signed_in("1", "Erik", "erik", None)
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
        accounts.signed_in("1", "Erik", "erik", None).unwrap();
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
    fn logins_find_pages() {
        let accounts = Accounts::new(Database::memory());
        accounts.signed_in("1", "Erik", "erik", None).unwrap();
        assert_eq!(accounts.page_of_login("erik"), None);
        accounts.give_page("1", page("a", "k")).unwrap();
        assert_eq!(accounts.page_of_login("ERIK").as_deref(), Some("a"));
        assert_eq!(accounts.page_of_login("other"), None);
        assert_eq!(accounts.page_of_login(""), None);
        accounts.signed_in("2", "Erik", "erik", None).unwrap();
        accounts.give_page("2", page("c", "l")).unwrap();
        assert_eq!(accounts.page_of_login("erik").as_deref(), Some("c"));
        assert_eq!(accounts.get("1").unwrap().login, "");
    }

    #[test]
    fn accounts_are_saved() {
        let path = database::temporary_path("accounts");
        let accounts = Accounts::new(Database::open(Some(&path)).unwrap());
        accounts.signed_in("3", "erik", "erik", None).unwrap();
        accounts.give_page("3", page("p", "k")).unwrap();
        let accounts = Accounts::new(Database::open(Some(&path)).unwrap());
        assert_eq!(
            accounts.get("3"),
            Some(Account {
                name: "erik".to_string(),
                login: "erik".to_string(),
                avatar: None,
                page: Some(page("p", "k")),
                ..Account::default()
            })
        );
        database::remove(&path);
    }
}
