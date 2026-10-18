use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use log::warn;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::database::Database;
use crate::deliveries::now;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Item {
    Song { link: String, name: String },
    Tip { message: Option<String> },
    Spin { spins: u32, message: Option<String> },
}

impl Item {
    pub fn kind(&self) -> &'static str {
        match self {
            Item::Song { .. } => "song",
            Item::Tip { .. } => "tip",
            Item::Spin { .. } => "spin",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Payee {
    PayPal(String),
    Stripe(String),
}

impl Payee {
    pub fn provider(&self) -> &'static str {
        match self {
            Payee::PayPal(_) => "PayPal",
            Payee::Stripe(_) => "Stripe",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Order {
    pub streamer: String,
    pub item: Item,
    pub user: String,
    pub payee: Payee,
    pub price: u32,
    pub currency: String,
}

impl Order {
    pub fn description(&self) -> String {
        let (description, message) = match &self.item {
            Item::Song { name, .. } => (format!("Song request: {name}"), None),
            Item::Tip { message } => (format!("Tip from {}", self.user), message.as_ref()),
            Item::Spin { spins: 1, message } => {
                (format!("Wheel spin by {}", self.user), message.as_ref())
            }
            Item::Spin { spins, message } => (
                format!("{spins} wheel spins by {}", self.user),
                message.as_ref(),
            ),
        };
        let description = match message {
            Some(message) => format!("{description}: {message}"),
            None => description,
        };
        description.chars().take(127).collect()
    }
}

#[derive(Debug, PartialEq)]
pub enum CaptureError {
    NotApproved,
    AlreadyCaptured,
    Pending,
    Refused,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PaymentStatus {
    Captured,
    Pending,
    NotCaptured,
    Gone,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    Created,
    Capturing,
    Captured,
}

impl Stage {
    fn name(self) -> &'static str {
        match self {
            Stage::Created => "created",
            Stage::Capturing => "capturing",
            Stage::Captured => "captured",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        [Stage::Created, Stage::Capturing, Stage::Captured]
            .into_iter()
            .find(|state| state.name() == name)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Unsettled {
    pub id: String,
    pub order: Order,
}

pub struct Orders {
    database: Arc<Database>,
}

impl Orders {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    pub fn make_room(&self) -> Result<()> {
        let unfinished: u64 = self
            .database
            .lock()
            .query_row(
                "SELECT count(*) FROM orders WHERE state != 'captured'",
                [],
                |row| row.get(0),
            )
            .context("failed to count orders")?;
        if unfinished >= 10000 {
            bail!("too many unfinished payments");
        }
        Ok(())
    }

    pub fn expire(&self) -> Vec<(String, Order)> {
        let database = self.database.lock();
        let rows = database
            .prepare(
                "DELETE FROM orders WHERE state = 'created' AND created_at < ?1 RETURNING id, details",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([now().saturating_sub(3 * 60 * 60)], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()
            });
        match rows {
            Ok(rows) => rows
                .into_iter()
                .filter_map(|(id, details)| {
                    let order = serde_json::from_str(&details)
                        .map_err(|error| warn!("failed to parse order {id}: {error}"))
                        .ok()?;
                    Some((id, order))
                })
                .collect(),
            Err(error) => {
                warn!("failed to remove expired orders: {error:#}");
                Vec::new()
            }
        }
    }

    pub fn insert(&self, id: &str, order: &Order) -> Result<()> {
        self.database
            .lock()
            .execute(
                "INSERT OR REPLACE INTO orders (id, streamer, state, created_at, updated_at, details, kind) VALUES (?1, ?2, 'created', ?3, ?3, ?4, ?5)",
                params![id, order.streamer, now(), serde_json::to_string(order)?, order.item.kind()],
            )
            .with_context(|| format!("failed to save order {id}"))?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<(Stage, Order)> {
        let row = self
            .database
            .lock()
            .query_row(
                "SELECT state, details FROM orders WHERE id = ?1",
                [id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional();
        match row {
            Ok(Some((state, details))) => Some((
                Stage::parse(&state)?,
                serde_json::from_str(&details)
                    .map_err(|error| warn!("failed to parse order {id}: {error}"))
                    .ok()?,
            )),
            Ok(None) => None,
            Err(error) => {
                warn!("failed to read order {id}: {error:#}");
                None
            }
        }
    }

    pub fn transition(&self, id: &str, from: Stage, to: Stage) -> Result<bool> {
        let changed = self
            .database
            .lock()
            .execute(
                "UPDATE orders SET state = ?3, updated_at = ?4 WHERE id = ?1 AND state = ?2",
                params![id, from.name(), to.name(), now()],
            )
            .with_context(|| format!("failed to update order {id}"))?;
        Ok(changed == 1)
    }

    pub fn captured(
        &self,
        id: &str,
        then: impl FnOnce(&Connection) -> rusqlite::Result<()>,
    ) -> Result<bool> {
        let mut database = self.database.lock();
        let transaction = database.transaction()?;
        let state: Option<String> = transaction
            .query_row("SELECT state FROM orders WHERE id = ?1", [id], |row| {
                row.get(0)
            })
            .optional()?;
        if state.as_deref() == Some("captured") {
            return Ok(false);
        }
        transaction.execute(
            "UPDATE orders SET state = 'captured', updated_at = ?2 WHERE id = ?1",
            params![id, now()],
        )?;
        then(&transaction)?;
        transaction
            .commit()
            .with_context(|| format!("failed to save the capture of order {id}"))?;
        Ok(true)
    }

    pub fn remove(&self, id: &str) {
        if let Err(error) = self
            .database
            .lock()
            .execute("DELETE FROM orders WHERE id = ?1", [id])
        {
            warn!("failed to remove order {id}: {error:#}");
        }
    }

    pub fn unfinished(&self) -> usize {
        self.database
            .lock()
            .query_row(
                "SELECT count(*) FROM orders WHERE state != 'captured'",
                [],
                |row| row.get(0),
            )
            .unwrap_or_else(|error| {
                warn!("failed to count orders: {error:#}");
                0
            })
    }

    pub fn unsettled(&self, min_age: Duration) -> Vec<Unsettled> {
        let database = self.database.lock();
        let rows = database
            .prepare(
                "SELECT id, details FROM orders WHERE state = 'capturing' AND updated_at <= ?1 ORDER BY updated_at",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([now().saturating_sub(min_age.as_secs())], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()
            });
        match rows {
            Ok(rows) => rows
                .into_iter()
                .filter_map(|(id, details)| {
                    let order = serde_json::from_str(&details)
                        .map_err(|error| warn!("failed to parse order {id}: {error}"))
                        .ok()?;
                    Some(Unsettled { id, order })
                })
                .collect(),
            Err(error) => {
                warn!("failed to read unsettled orders: {error:#}");
                Vec::new()
            }
        }
    }

    pub fn recent(
        &self,
        streamer: &str,
        kind: &str,
        before: Option<&str>,
        limit: usize,
    ) -> Vec<(String, u64, Order)> {
        let rows = self
            .database
            .lock()
            .prepare(
                "SELECT id, updated_at, details FROM orders
                 WHERE streamer = ?1 AND kind = ?2 AND state = 'captured'
                   AND (?3 IS NULL OR (updated_at, rowid) < (SELECT updated_at, rowid FROM orders WHERE id = ?3 AND streamer = ?1))
                 ORDER BY updated_at DESC, rowid DESC LIMIT ?4",
            )
            .and_then(|mut statement| {
                statement
                    .query_map(params![streamer, kind, before, limit], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, u64>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()
            });
        match rows {
            Ok(rows) => rows
                .into_iter()
                .filter_map(|(id, at, details)| {
                    let order = serde_json::from_str(&details)
                        .map_err(|error| warn!("failed to parse order {id}: {error}"))
                        .ok()?;
                    Some((id, at, order))
                })
                .collect(),
            Err(error) => {
                warn!("failed to read recent orders: {error:#}");
                Vec::new()
            }
        }
    }

    #[cfg(test)]
    pub fn age(&self, id: &str, seconds: u64) {
        self.database
            .lock()
            .execute(
                "UPDATE orders SET created_at = created_at - ?2, updated_at = updated_at - ?2 WHERE id = ?1",
                params![id, seconds],
            )
            .unwrap();
    }

    #[cfg(test)]
    pub fn read_only(&self, on: bool) {
        self.database
            .lock()
            .pragma_update(None, "query_only", on)
            .unwrap();
    }

    pub fn prune(&self) {
        let database = self.database.lock();
        let cutoff = now().saturating_sub(30 * 24 * 60 * 60);
        let given_up = database
            .prepare("DELETE FROM orders WHERE state != 'captured' AND created_at < ?1 RETURNING id, state")
            .and_then(|mut statement| {
                statement
                    .query_map([cutoff], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()
            });
        match given_up {
            Ok(given_up) => {
                for (id, state) in given_up {
                    warn!("giving up on {state} order {id} after 30 days");
                }
            }
            Err(error) => warn!("failed to prune orders: {error:#}"),
        }
        if let Err(error) = database.execute(
            "DELETE FROM orders WHERE state = 'captured' AND updated_at < ?1",
            [cutoff],
        ) {
            warn!("failed to prune captured orders: {error:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_of_older_orders_are_filled_in() {
        let path = crate::database::temporary_path("orders-kind");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE orders (id TEXT PRIMARY KEY, streamer TEXT NOT NULL, state TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, details TEXT NOT NULL)",
            )
            .unwrap();
        let items = [
            (
                "SONG",
                Item::Song {
                    link: "l".to_string(),
                    name: "n".to_string(),
                },
            ),
            ("TIP", Item::Tip { message: None }),
            (
                "SPIN",
                Item::Spin {
                    spins: 1,
                    message: None,
                },
            ),
        ];
        for (id, item) in &items {
            connection
                .execute(
                    "INSERT INTO orders VALUES (?1, 'p', 'captured', 1, 1, ?2)",
                    params![
                        id,
                        serde_json::to_string(&Order {
                            item: item.clone(),
                            ..order()
                        })
                        .unwrap()
                    ],
                )
                .unwrap();
        }
        drop(connection);
        let orders = Orders::new(Database::open(Some(&path)).unwrap());
        for (id, item) in &items {
            let listed: Vec<String> = orders
                .recent("p", item.kind(), None, 10)
                .into_iter()
                .map(|(id, _, _)| id)
                .collect();
            assert_eq!(listed, vec![id.to_string()]);
        }
        crate::database::remove(&path);
    }

    fn order() -> Order {
        Order {
            streamer: "p".to_string(),
            item: Item::Tip {
                message: Some("Hi".to_string()),
            },
            user: "Viewer".to_string(),
            payee: Payee::PayPal("MERCHANT00001".to_string()),
            price: 700,
            currency: "USD".to_string(),
        }
    }

    #[test]
    fn orders_survive_restarts() {
        let database = Database::memory();
        let orders = Orders::new(database.clone());
        orders.make_room().unwrap();
        orders.insert("ORDER1", &order()).unwrap();
        assert!(
            orders
                .transition("ORDER1", Stage::Created, Stage::Capturing)
                .unwrap()
        );
        let orders = Orders::new(database);
        assert_eq!(orders.get("ORDER1"), Some((Stage::Capturing, order())));
        assert_eq!(orders.get("ORDER2"), None);
        assert_eq!(orders.unfinished(), 1);
        assert_eq!(
            orders.unsettled(Duration::ZERO),
            vec![Unsettled {
                id: "ORDER1".to_string(),
                order: order(),
            }]
        );
        assert!(orders.unsettled(Duration::from_secs(60)).is_empty());
    }

    #[test]
    fn transitions_only_from_the_expected_state() {
        let orders = Orders::new(Database::memory());
        orders.insert("ORDER1", &order()).unwrap();
        assert!(
            !orders
                .transition("ORDER1", Stage::Capturing, Stage::Created)
                .unwrap()
        );
        assert!(
            orders
                .transition("ORDER1", Stage::Created, Stage::Capturing)
                .unwrap()
        );
        assert!(
            !orders
                .transition("ORDER1", Stage::Created, Stage::Capturing)
                .unwrap()
        );
        assert!(
            !orders
                .transition("ORDER2", Stage::Created, Stage::Capturing)
                .unwrap()
        );
        orders
            .captured("ORDER1", |connection| {
                connection.execute(
                    "INSERT INTO counters (name, day, value) VALUES ('x', 'd', 1)",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        assert!(
            !orders
                .captured("ORDER1", |_| panic!("captured twice"))
                .unwrap()
        );
        assert_eq!(orders.get("ORDER1").unwrap().0, Stage::Captured);
        assert_eq!(orders.unfinished(), 0);
        assert!(orders.unsettled(Duration::ZERO).is_empty());
        orders.remove("ORDER1");
        assert_eq!(orders.get("ORDER1"), None);
    }

    #[test]
    fn a_failed_capture_record_keeps_the_order_capturing() {
        let orders = Orders::new(Database::memory());
        orders.insert("ORDER1", &order()).unwrap();
        orders
            .transition("ORDER1", Stage::Created, Stage::Capturing)
            .unwrap();
        assert!(
            orders
                .captured("ORDER1", |connection| {
                    connection.execute("INSERT INTO nowhere VALUES (1)", [])?;
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(orders.get("ORDER1").unwrap().0, Stage::Capturing);
    }

    #[test]
    fn prune_gives_old_orders_up_and_forgets_old_captures() {
        let database = Database::memory();
        let orders = Orders::new(database.clone());
        for id in [
            "OLD_CAPTURING",
            "OLD_CAPTURED",
            "NEW_CAPTURING",
            "NEW_CAPTURED",
        ] {
            orders.insert(id, &order()).unwrap();
        }
        for id in ["OLD_CAPTURING", "NEW_CAPTURING"] {
            assert!(
                orders
                    .transition(id, Stage::Created, Stage::Capturing)
                    .unwrap()
            );
        }
        for id in ["OLD_CAPTURED", "NEW_CAPTURED"] {
            orders.captured(id, |_| Ok(())).unwrap();
        }
        database
            .lock()
            .execute(
                "UPDATE orders SET created_at = created_at - 31 * 24 * 60 * 60, updated_at = updated_at - 31 * 24 * 60 * 60 WHERE id LIKE 'OLD%'",
                [],
            )
            .unwrap();
        orders.prune();
        for id in ["OLD_CAPTURING", "OLD_CAPTURED"] {
            assert_eq!(orders.get(id), None, "{id}");
        }
        assert_eq!(orders.get("NEW_CAPTURING").unwrap().0, Stage::Capturing);
        assert_eq!(orders.get("NEW_CAPTURED").unwrap().0, Stage::Captured);
        assert_eq!(orders.unsettled(Duration::ZERO).len(), 1);
    }

    #[test]
    fn created_orders_expire_after_three_hours() {
        let database = Database::memory();
        let orders = Orders::new(database.clone());
        orders.insert("OLD", &order()).unwrap();
        orders.insert("NEW", &order()).unwrap();
        database
            .lock()
            .execute(
                "UPDATE orders SET created_at = created_at - 4 * 60 * 60 WHERE id = 'OLD'",
                [],
            )
            .unwrap();
        assert_eq!(orders.expire(), vec![("OLD".to_string(), order())]);
        assert_eq!(orders.get("OLD"), None);
        assert!(orders.get("NEW").is_some());
    }
}
