use std::sync::Arc;

use anyhow::{Context, Result};
use log::warn;
use rusqlite::{Connection, OptionalExtension, params};

use crate::database::Database;

pub struct Allowances {
    database: Arc<Database>,
}

fn month(connection: &Connection) -> rusqlite::Result<String> {
    connection.query_row("SELECT strftime('%Y-%m', 'now')", [], |row| row.get(0))
}

fn used(
    connection: &Connection,
    page: &str,
    viewer: &str,
    kind: &str,
    month: &str,
) -> rusqlite::Result<u32> {
    Ok(connection
        .query_row(
            "SELECT count FROM subscriber_allowances WHERE page = ?1 AND viewer = ?2 AND kind = ?3 AND month = ?4",
            [page, viewer, kind, month],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(0))
}

impl Allowances {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    pub fn left(&self, page: &str, viewer: &str, kind: &str, allowance: u32) -> Result<u32> {
        let database = self.database.lock();
        let used = month(&database)
            .and_then(|month| used(&database, page, viewer, kind, &month))
            .with_context(|| {
                format!("failed to read the {kind} allowance of {viewer} on page {page}")
            })?;
        Ok(allowance.saturating_sub(used))
    }

    pub fn take(
        &self,
        page: &str,
        viewer: &str,
        kind: &str,
        allowance: u32,
        then: impl FnOnce(&Connection) -> rusqlite::Result<()>,
    ) -> Result<Option<u32>> {
        let mut database = self.database.lock();
        let transaction = database.transaction()?;
        let month = month(&transaction)?;
        let left = allowance.saturating_sub(used(&transaction, page, viewer, kind, &month)?);
        if left == 0 {
            return Ok(None);
        }
        transaction.execute(
            "INSERT INTO subscriber_allowances (page, viewer, kind, month, count) VALUES (?1, ?2, ?3, ?4, 1)
             ON CONFLICT (page, viewer, kind, month) DO UPDATE SET count = count + 1",
            params![page, viewer, kind, month],
        )?;
        then(&transaction)?;
        transaction
            .commit()
            .with_context(|| format!("failed to save the {kind} of {viewer} on page {page}"))?;
        Ok(Some(left - 1))
    }

    pub fn prune(&self) {
        if let Err(error) = self.database.lock().execute(
            "DELETE FROM subscriber_allowances WHERE month < strftime('%Y-%m', 'now')",
            [],
        ) {
            warn!("failed to prune subscriber allowances: {error:#}");
        }
    }

    #[cfg(test)]
    pub fn last_month(&self) {
        self.database
            .lock()
            .execute(
                "UPDATE subscriber_allowances SET month = strftime('%Y-%m', 'now', 'start of month', '-1 month')",
                [],
            )
            .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_are_counted_apart_and_old_months_pruned() {
        let allowances = Allowances::new(Database::memory());
        let take = |kind, allowance| {
            allowances
                .take("p", "v", kind, allowance, |_| Ok(()))
                .unwrap()
        };
        assert_eq!(take("song", 2), Some(1));
        assert_eq!(take("song", 2), Some(0));
        assert_eq!(take("song", 2), None);
        assert_eq!(take("spin", 1), Some(0));
        assert_eq!(take("spin", 1), None);
        assert_eq!(allowances.left("p", "v", "song", 3).unwrap(), 1);
        assert_eq!(allowances.left("p", "other", "song", 3).unwrap(), 3);
        assert_eq!(allowances.left("other", "v", "song", 3).unwrap(), 3);
        allowances.last_month();
        assert_eq!(take("song", 3), Some(2));
        allowances.prune();
        let rows: u32 = allowances
            .database
            .lock()
            .query_row("SELECT count(*) FROM subscriber_allowances", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(rows, 1);
        assert_eq!(allowances.left("p", "v", "song", 3).unwrap(), 2);
        assert_eq!(allowances.left("p", "v", "spin", 1).unwrap(), 1);
    }

    #[test]
    fn nothing_is_counted_when_saving_the_order_fails() {
        let allowances = Allowances::new(Database::memory());
        assert!(
            allowances
                .take("p", "v", "song", 2, |_| Err(rusqlite::Error::InvalidQuery))
                .is_err()
        );
        assert_eq!(allowances.left("p", "v", "song", 2).unwrap(), 2);
    }
}
