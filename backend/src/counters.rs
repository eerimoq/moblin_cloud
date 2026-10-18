use std::sync::Arc;

use log::warn;
use rusqlite::Connection;
use serde::Serialize;

use crate::database::Database;

#[derive(Debug, PartialEq, Serialize)]
pub struct Counter {
    pub name: String,
    pub today: u64,
    pub week: u64,
    pub total: u64,
}

pub struct Counters {
    database: Arc<Database>,
}

pub fn bump(connection: &Connection, name: &str) {
    if let Err(error) = connection.execute(
        "INSERT INTO counters (name, day, value) VALUES (?1, date('now'), 1)
         ON CONFLICT (name, day) DO UPDATE SET value = value + 1",
        [name],
    ) {
        warn!("failed to count {name}: {error:#}");
    }
}

impl Counters {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    pub fn bump(&self, name: &str) {
        bump(&self.database.lock(), name);
    }

    pub fn all(&self) -> Vec<Counter> {
        let database = self.database.lock();
        let counters = database
            .prepare(
                "SELECT name,
                        coalesce(sum(value) FILTER (WHERE day = date('now')), 0),
                        coalesce(sum(value) FILTER (WHERE day >= date('now', '-6 days')), 0),
                        sum(value)
                 FROM counters GROUP BY name ORDER BY name",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| {
                        Ok(Counter {
                            name: row.get(0)?,
                            today: row.get(1)?,
                            week: row.get(2)?,
                            total: row.get(3)?,
                        })
                    })?
                    .collect()
            });
        counters.unwrap_or_else(|error| {
            warn!("failed to read counters: {error:#}");
            Vec::new()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_and_read() {
        let database = Database::memory();
        let counters = Counters::new(database.clone());
        assert!(counters.all().is_empty());
        counters.bump("signIns");
        counters.bump("signIns");
        bump(&database.lock(), "livePosts");
        database
            .lock()
            .execute(
                "INSERT INTO counters (name, day, value) VALUES ('signIns', date('now', '-3 days'), 5), ('signIns', date('now', '-10 days'), 7)",
                [],
            )
            .unwrap();
        assert_eq!(
            counters.all(),
            [
                Counter {
                    name: "livePosts".to_string(),
                    today: 1,
                    week: 1,
                    total: 1,
                },
                Counter {
                    name: "signIns".to_string(),
                    today: 2,
                    week: 7,
                    total: 14,
                },
            ]
        );
    }
}
