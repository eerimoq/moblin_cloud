use std::sync::Arc;

use anyhow::{Context, Result};
use log::warn;
use rusqlite::{Connection, OptionalExtension, params};

use crate::database::Database;

pub struct Allowances {
    database: Arc<Database>,
}

fn used(connection: &Connection, page_id: &str, viewer: &str, kind: &str) -> rusqlite::Result<u32> {
    Ok(connection
        .query_row(
            "SELECT count FROM subscriber_allowances WHERE page = ?1 AND viewer = ?2 AND kind = ?3 AND month = strftime('%Y-%m', 'now')",
            [page_id, viewer, kind],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(0))
}

impl Allowances {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    pub fn left(&self, page_id: &str, viewer: &str, kind: &str, allowance: u32) -> Result<u32> {
        let database = self.database.lock();
        let used = used(&database, page_id, viewer, kind).with_context(|| {
            format!("failed to read the {kind} allowance of {viewer} on page {page_id}")
        })?;
        Ok(allowance.saturating_sub(used))
    }

    pub fn take(
        &self,
        page_id: &str,
        viewer: &str,
        kind: &str,
        allowance: u32,
        then: impl FnOnce(&Connection) -> rusqlite::Result<()>,
    ) -> Result<Option<u32>> {
        let mut database = self.database.lock();
        let transaction = database.transaction()?;
        let count: u32 = transaction.query_row(
            "INSERT INTO subscriber_allowances (page, viewer, kind, month, count) VALUES (?1, ?2, ?3, strftime('%Y-%m', 'now'), 1)
             ON CONFLICT (page, viewer, kind, month) DO UPDATE SET count = count + 1 RETURNING count",
            params![page_id, viewer, kind],
            |row| row.get(0),
        )?;
        if count > allowance {
            return Ok(None);
        }
        then(&transaction)?;
        transaction
            .commit()
            .with_context(|| format!("failed to save the {kind} of {viewer} on page {page_id}"))?;
        Ok(Some(allowance - count))
    }

    pub fn prune(&self) {
        if let Err(error) = self.database.lock().execute(
            "DELETE FROM subscriber_allowances WHERE month < strftime('%Y-%m', 'now')",
            [],
        ) {
            warn!("failed to prune subscriber allowances: {error:#}");
        }
    }
}

#[cfg(test)]
#[path = "tests/subscribers.rs"]
mod tests;
