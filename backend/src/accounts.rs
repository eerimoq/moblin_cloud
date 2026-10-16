use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::Result;
use log::warn;
use serde::{Deserialize, Serialize};

use crate::json_file;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Account {
    pub name: String,
    #[serde(default)]
    pub login: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<Page>,
    #[serde(default)]
    pub sessions: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merchant_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paypal_tracking_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

pub struct Accounts {
    path: Option<PathBuf>,
    accounts: Mutex<HashMap<String, Account>>,
}

impl Accounts {
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        let accounts = json_file::load(path.as_deref())?;
        Ok(Self {
            path,
            accounts: Mutex::new(accounts),
        })
    }

    pub fn get(&self, id: &str) -> Option<Account> {
        self.accounts.lock().unwrap().get(id).cloned()
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
        let mut accounts = self.accounts.lock().unwrap();
        let mut changed = false;
        for (other_id, other) in accounts.iter_mut() {
            if other_id != id && same_login(&other.login, login) {
                other.login.clear();
                changed = true;
            }
        }
        if changed && let Err(error) = json_file::save(self.path.as_deref(), &*accounts) {
            warn!("failed to save accounts: {error:#}");
        }
        Ok(account)
    }

    pub fn page_of_login(&self, login: &str) -> Option<String> {
        self.accounts
            .lock()
            .unwrap()
            .values()
            .find(|account| same_login(&account.login, login))
            .and_then(|account| account.page.as_ref())
            .map(|page| page.id.clone())
    }

    pub fn of_page(&self, page: &str) -> Option<Account> {
        self.accounts
            .lock()
            .unwrap()
            .values()
            .find(|account| account.page.as_ref().is_some_and(|own| own.id == page))
            .cloned()
    }

    pub fn page_of_key(&self, key: &str) -> Option<String> {
        self.accounts
            .lock()
            .unwrap()
            .values()
            .filter_map(|account| account.page.as_ref())
            .find(|page| page.key == key)
            .map(|page| page.id.clone())
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
        self.accounts
            .lock()
            .unwrap()
            .iter()
            .find(|(_, account)| account.paypal_tracking_id.as_deref() == Some(tracking_id))
            .map(|(id, _)| id.clone())
    }

    pub fn disconnect_paypal(&self, merchant_id: &str) -> Vec<String> {
        let ids: Vec<String> = self
            .accounts
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, account)| account.merchant_id.as_deref() == Some(merchant_id))
            .map(|(id, _)| id.clone())
            .collect();
        ids.into_iter()
            .filter(|id| {
                self.change(id, false, |account| {
                    account.merchant_id = None;
                    Ok(())
                })
                .is_ok()
            })
            .collect()
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

    fn change(
        &self,
        id: &str,
        create: bool,
        change: impl FnOnce(&mut Account) -> Result<(), AccountError>,
    ) -> Result<Account, AccountError> {
        let mut accounts = self.accounts.lock().unwrap();
        let old = accounts.get(id).cloned();
        let mut account = match &old {
            Some(account) => account.clone(),
            None if !create => return Err(AccountError::Unknown),
            None if accounts.len() >= 10000 => return Err(AccountError::Full),
            None => Account::default(),
        };
        change(&mut account)?;
        if old.as_ref() == Some(&account) {
            return Ok(account);
        }
        accounts.insert(id.to_string(), account.clone());
        if let Err(error) = json_file::save(self.path.as_deref(), &*accounts) {
            warn!("failed to save accounts: {error:#}");
            match old {
                Some(old) => accounts.insert(id.to_string(), old),
                None => accounts.remove(id),
            };
            return Err(AccountError::NotSaved);
        }
        Ok(account)
    }
}

fn same_login(a: &str, b: &str) -> bool {
    !a.is_empty() && a.eq_ignore_ascii_case(b.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(id: &str, key: &str) -> Page {
        Page {
            id: id.to_string(),
            key: key.to_string(),
        }
    }

    #[test]
    fn pages_are_kept() {
        let accounts = Accounts::load(None).unwrap();
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
        let accounts = Accounts::load(None).unwrap();
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
        let accounts = Accounts::load(None).unwrap();
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
        let accounts = Accounts::load(None).unwrap();
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
        let accounts = Accounts::load(None).unwrap();
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
        let accounts = Accounts::load(None).unwrap();
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
        let path = std::env::temp_dir().join(format!(
            "accounts-{}-{:?}.json",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);
        let accounts = Accounts::load(Some(path.clone())).unwrap();
        accounts.signed_in("3", "erik", "erik", None).unwrap();
        accounts.give_page("3", page("p", "k")).unwrap();
        let accounts = Accounts::load(Some(path.clone())).unwrap();
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
        std::fs::remove_file(&path).unwrap();
    }
}
