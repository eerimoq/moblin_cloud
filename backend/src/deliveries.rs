use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::future::join_all;
use log::{info, warn};
use rusqlite::{Connection, Params, params};
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use tokio::time::sleep;

use crate::accounts::Accounts;
use crate::counters;
use crate::database::Database;
use crate::pages::format_price;
use crate::remote_control::{MusicCommand, RequestError, Streamers};

#[derive(Clone, Debug, PartialEq)]
pub struct Delivery {
    pub streamer: String,
    pub request_id: String,
    pub requested_at: u64,
    pub item: Item,
    pub delivered: Vec<String>,
    pub failure: Option<String>,
    pub finished: bool,
    pub test: bool,
    pub resends: u32,
}

impl Delivery {
    fn moblin_request_id(&self) -> String {
        if self.resends == 0 {
            self.request_id.clone()
        } else {
            format!("{}-{}", self.request_id, self.resends)
        }
    }

    fn counter(&self, event: &str) -> String {
        let test = if self.test { "Test" } else { "" };
        format!("{}{test}{event}", self.item.kind())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Item {
    Song {
        link: String,
        user: String,
        #[serde(default)]
        message: String,
    },
    Spin {
        user: String,
        message: String,
        spins: u32,
    },
    Tip {
        user: String,
        message: String,
        amount: u32,
        currency: String,
    },
}

impl Item {
    pub fn kind(&self) -> &'static str {
        match self {
            Item::Song { .. } => "song",
            Item::Spin { .. } => "spin",
            Item::Tip { .. } => "tip",
        }
    }
}

type Targeted = (Delivery, Vec<String>);

pub struct Deliveries {
    database: Arc<Database>,
    added: Notify,
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl Deliveries {
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            database,
            added: Notify::new(),
        }
    }

    pub fn add_test(&self, streamer: String, request_id: String, item: Item) {
        let database = self.database.lock();
        let result = database
            .execute(
                "INSERT OR IGNORE INTO deliveries (request_id, streamer, requested_at, item, test) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![request_id, streamer, now(), serde_json::to_string(&item).unwrap(), item.kind()],
            )
            .and_then(|_| {
                database.execute(
                    "DELETE FROM deliveries WHERE streamer = ?1 AND test = ?2 AND rowid NOT IN (SELECT rowid FROM deliveries WHERE streamer = ?1 AND test = ?2 ORDER BY rowid DESC LIMIT 5)",
                    [&streamer, item.kind()],
                )
            });
        if let Err(error) = result {
            warn!("failed to save test delivery {request_id}: {error:#}");
        }
        drop(database);
        self.notify();
    }

    pub fn resend(&self, streamer: &str, request_id: &str, item: &Item) -> rusqlite::Result<()> {
        self.database.lock().execute(
            "INSERT INTO deliveries (request_id, streamer, requested_at, item, resends) VALUES (?1, ?2, ?3, ?4, 1)
             ON CONFLICT (request_id) DO UPDATE SET requested_at = excluded.requested_at, delivered = '[]', failure = NULL, finished = 0, resends = resends + 1",
            params![request_id, streamer, now(), serde_json::to_string(item).unwrap()],
        )?;
        self.notify();
        Ok(())
    }

    pub fn notify(&self) {
        self.added.notify_one();
    }

    pub fn count(&self) -> usize {
        self.database
            .lock()
            .query_row(
                "SELECT count(*) FROM deliveries WHERE NOT finished AND failure IS NOT 'not-found' AND delivered = '[]'",
                [],
                |row| row.get(0),
            )
            .unwrap_or_else(|error| {
                warn!("failed to count deliveries: {error:#}");
                0
            })
    }

    pub fn expire(&self) {
        let database = self.database.lock();
        let expired = query(
            &database,
            "DELETE FROM deliveries WHERE requested_at < ?1 AND NOT finished AND failure IS NOT 'not-found' RETURNING *",
            [now().saturating_sub(7 * 24 * 60 * 60)],
        );
        for delivery in expired {
            warn!(
                "giving up delivering {:?} in request {} to remote control streamer {} after 7 days and {} devices got it",
                delivery.item,
                delivery.request_id,
                delivery.streamer,
                delivery.delivered.len()
            );
            if delivery.delivered.is_empty() {
                counters::bump(&database, &delivery.counter("Expiries"));
            }
        }
        if let Err(error) = database.execute(
            "DELETE FROM deliveries WHERE requested_at < ?1 AND (finished OR failure = 'not-found')",
            [now().saturating_sub(30 * 24 * 60 * 60)],
        ) {
            warn!("failed to remove finished deliveries: {error:#}");
        }
    }

    pub fn pending(&self) -> Vec<Delivery> {
        query(
            &self.database.lock(),
            "SELECT * FROM deliveries WHERE NOT finished AND failure IS NOT 'not-found' ORDER BY rowid",
            [],
        )
    }

    pub fn get(&self, request_id: &str) -> Option<Delivery> {
        query(
            &self.database.lock(),
            "SELECT * FROM deliveries WHERE request_id = ?1",
            [request_id],
        )
        .pop()
    }

    fn delivered(&self, request_id: &str, counter: Option<&str>, device: &str, devices: &[String]) {
        let database = self.database.lock();
        let delivered = database
            .query_row(
                "UPDATE deliveries SET delivered = json_insert(delivered, '$[#]', ?2) WHERE request_id = ?1 RETURNING delivered",
                [request_id, device],
                |row| row.get::<_, String>(0),
            )
            .map(|delivered| serde_json::from_str::<Vec<String>>(&delivered).unwrap_or_default());
        match delivered {
            Ok(delivered) if devices.iter().all(|device| delivered.contains(device)) => {
                finish(&database, request_id);
                if let Some(counter) = counter {
                    counters::bump(&database, counter);
                }
            }
            Ok(_) | Err(rusqlite::Error::QueryReturnedNoRows) => {}
            Err(error) => warn!("failed to save delivery {request_id}: {error:#}"),
        }
    }

    fn failed(&self, delivery: &Delivery, reason: &str) -> bool {
        let database = self.database.lock();
        match database.execute(
            "UPDATE deliveries SET failure = ?2 WHERE request_id = ?1 AND failure IS NOT ?2",
            [&delivery.request_id, reason],
        ) {
            Ok(0) => false,
            Ok(_) => {
                counters::bump(
                    &database,
                    &delivery.counter(&format!("DeliveriesFailed:{reason}")),
                );
                true
            }
            Err(error) => {
                warn!(
                    "failed to save the failure of delivery {}: {error:#}",
                    delivery.request_id
                );
                false
            }
        }
    }

    pub async fn run(&self, streamers: &Streamers, accounts: &Accounts) {
        loop {
            self.expire();
            let mut by_device: Vec<(String, Vec<Targeted>)> = Vec::new();
            for delivery in self.pending() {
                let page = accounts
                    .of_page(&delivery.streamer)
                    .map(|account| account.page);
                let devices: Vec<String> = match (&delivery.item, &page) {
                    (_, None) => Vec::new(),
                    (Item::Song { .. }, Some(page)) => page
                        .music_device()
                        .map(|device| device.id.clone())
                        .into_iter()
                        .collect(),
                    (Item::Spin { .. }, Some(page)) => page
                        .wheel_device()
                        .map(|device| device.id.clone())
                        .into_iter()
                        .collect(),
                    (Item::Tip { .. }, Some(page)) => page
                        .enabled_devices()
                        .map(|device| device.id.clone())
                        .filter(|device| !delivery.delivered.contains(device))
                        .collect(),
                };
                if devices.is_empty() && !delivery.delivered.is_empty() {
                    finish(&self.database.lock(), &delivery.request_id);
                    continue;
                }
                for device in &devices {
                    if !streamers.is_connected(device) {
                        continue;
                    }
                    let delivery = (delivery.clone(), devices.clone());
                    match by_device.iter_mut().find(|(other, _)| other == device) {
                        Some((_, deliveries)) => deliveries.push(delivery),
                        None => by_device.push((device.clone(), vec![delivery])),
                    }
                }
            }
            join_all(
                by_device
                    .into_iter()
                    .map(|(device, deliveries)| self.deliver_all(streamers, device, deliveries)),
            )
            .await;
            tokio::select! {
                _ = self.added.notified() => {}
                _ = streamers.connected.notified() => {}
                _ = sleep(Duration::from_secs(10)) => {}
            }
        }
    }

    async fn deliver_all(&self, streamers: &Streamers, device: String, deliveries: Vec<Targeted>) {
        for (delivery, devices) in deliveries {
            let counter = match deliver(streamers, &device, &delivery).await {
                Ok(Outcome::Delivered) => {
                    info!(
                        "delivered {:?} in request {} to remote control streamer {device}",
                        delivery.item, delivery.request_id
                    );
                    Some(delivery.counter("Deliveries"))
                }
                Ok(Outcome::NotFound) => {
                    if self.failed(&delivery, "not-found") {
                        warn!(
                            "{:?} in request {} was not found by remote control streamer {device}",
                            delivery.item, delivery.request_id
                        );
                    }
                    continue;
                }
                Err(RequestError::Offline | RequestError::Timeout) => return,
                Err(error) => {
                    if self.failed(&delivery, "moblin-error") {
                        info!(
                            "delivering request {} to remote control streamer {device} failed: {error:?}, retrying",
                            delivery.request_id
                        );
                    }
                    continue;
                }
            };
            self.delivered(&delivery.request_id, counter.as_deref(), &device, &devices);
        }
    }
}

fn finish(database: &Connection, request_id: &str) {
    if let Err(error) = database.execute(
        "UPDATE deliveries SET finished = 1 WHERE request_id = ?1",
        [request_id],
    ) {
        warn!("failed to finish delivery {request_id}: {error:#}");
    }
}

pub fn insert(
    connection: &Connection,
    streamer: &str,
    request_id: &str,
    item: &Item,
) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT OR IGNORE INTO deliveries (request_id, streamer, requested_at, item) VALUES (?1, ?2, ?3, ?4)",
        params![request_id, streamer, now(), serde_json::to_string(item).unwrap()],
    )?;
    Ok(())
}

fn query(database: &Connection, sql: &str, params: impl Params) -> Vec<Delivery> {
    let deliveries = database.prepare(sql).and_then(|mut statement| {
        statement
            .query_map(params, |row| {
                Ok((
                    row.get("request_id")?,
                    row.get("streamer")?,
                    row.get("requested_at")?,
                    row.get::<_, String>("item")?,
                    row.get::<_, String>("delivered")?,
                    row.get("failure")?,
                    row.get("finished")?,
                    row.get::<_, Option<String>>("test")?.is_some(),
                    row.get("resends")?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
    });
    match deliveries {
        Ok(deliveries) => deliveries
            .into_iter()
            .filter_map(
                |(
                    request_id,
                    streamer,
                    requested_at,
                    item,
                    delivered,
                    failure,
                    finished,
                    test,
                    resends,
                )| {
                    match serde_json::from_str(&item) {
                        Ok(item) => Some(Delivery {
                            streamer,
                            request_id,
                            requested_at,
                            item,
                            delivered: serde_json::from_str(&delivered).unwrap_or_default(),
                            failure,
                            finished,
                            test,
                            resends,
                        }),
                        Err(error) => {
                            warn!("failed to parse delivery {request_id}: {error}");
                            None
                        }
                    }
                },
            )
            .collect(),
        Err(error) => {
            warn!("failed to read deliveries: {error:#}");
            Vec::new()
        }
    }
}

enum Outcome {
    Delivered,
    NotFound,
}

async fn deliver(
    streamers: &Streamers,
    id: &str,
    delivery: &Delivery,
) -> Result<Outcome, RequestError> {
    let request_id = delivery.moblin_request_id();
    match &delivery.item {
        Item::Song {
            link,
            user,
            message,
        } => {
            let add = MusicCommand::Add {
                title: link.clone(),
                user: user.clone(),
                message: (!message.is_empty()).then(|| message.clone()),
                request_id: Some(request_id),
            };
            let status = streamers.music_uncached(id, add).await?;
            if status.song_not_found == Some(true) {
                Ok(Outcome::NotFound)
            } else {
                Ok(Outcome::Delivered)
            }
        }
        Item::Spin {
            user,
            message,
            spins,
        } => {
            streamers
                .spin_wheel(id, user.clone(), message.clone(), *spins, request_id)
                .await?;
            Ok(Outcome::Delivered)
        }
        Item::Tip {
            user,
            message,
            amount,
            currency,
        } => {
            streamers
                .tip(
                    id,
                    user.clone(),
                    message.clone(),
                    format_price(*amount),
                    currency.clone(),
                    request_id,
                )
                .await?;
            Ok(Outcome::Delivered)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database;

    fn song() -> Item {
        Item::Song {
            link: "https://music.apple.com/song/1?i=1".to_string(),
            user: "Viewer".to_string(),
            message: "Play it loud!".to_string(),
        }
    }

    fn tip() -> Item {
        Item::Tip {
            user: "Viewer".to_string(),
            message: "Hi".to_string(),
            amount: 500,
            currency: "USD".to_string(),
        }
    }

    fn spin() -> Item {
        Item::Spin {
            user: "Viewer".to_string(),
            message: String::new(),
            spins: 2,
        }
    }

    fn request_ids(deliveries: &Deliveries) -> Vec<String> {
        deliveries
            .pending()
            .into_iter()
            .map(|delivery| delivery.request_id)
            .collect()
    }

    #[test]
    fn deliveries_are_saved() {
        let path = database::temporary_path("deliveries");
        let deliveries = Deliveries::new(Database::open(Some(&path)).unwrap());
        deliveries.add_test("S".to_string(), "A".to_string(), song());
        deliveries.add_test("S".to_string(), "B".to_string(), tip());
        deliveries.add_test("S".to_string(), "C".to_string(), spin());
        deliveries.add_test("S".to_string(), "C".to_string(), tip());
        finish(&deliveries.database.lock(), "A");
        assert!(deliveries.get("A").unwrap().finished);
        let saved = deliveries.pending();
        let deliveries = Deliveries::new(Database::open(Some(&path)).unwrap());
        assert_eq!(deliveries.pending(), saved);
        assert_eq!(
            saved
                .iter()
                .map(|delivery| &delivery.item)
                .collect::<Vec<_>>(),
            vec![&tip(), &spin()]
        );
        database::remove(&path);
    }

    #[test]
    fn only_the_5_newest_tests_per_kind_are_kept() {
        let deliveries = Deliveries::new(Database::memory());
        for n in 0..7 {
            deliveries.add_test("S".to_string(), format!("T{n}"), tip());
        }
        deliveries.add_test("S".to_string(), "SPIN".to_string(), spin());
        deliveries.add_test("O".to_string(), "OTHER".to_string(), tip());
        insert(&deliveries.database.lock(), "S", "PAID", &tip()).unwrap();
        let ids = |streamer: &str| {
            query(
                &deliveries.database.lock(),
                "SELECT * FROM deliveries WHERE streamer = ?1 AND test IS NOT NULL ORDER BY rowid DESC",
                [streamer],
            )
            .into_iter()
            .map(|delivery| delivery.request_id)
            .collect::<Vec<_>>()
        };
        assert_eq!(ids("S"), vec!["SPIN", "T6", "T5", "T4", "T3", "T2"]);
        assert_eq!(ids("O"), vec!["OTHER"]);
        assert!(deliveries.get("PAID").is_some());
    }

    #[test]
    fn resends_start_over_with_a_new_moblin_request_id() {
        let deliveries = Deliveries::new(Database::memory());
        insert(&deliveries.database.lock(), "S", "PAID", &tip()).unwrap();
        deliveries.delivered("PAID", None, "phone", &["phone".to_string()]);
        let delivery = deliveries.get("PAID").unwrap();
        assert!(delivery.finished);
        assert_eq!(delivery.moblin_request_id(), "PAID");
        deliveries.resend("S", "PAID", &tip()).unwrap();
        let delivery = deliveries.get("PAID").unwrap();
        assert!(!delivery.finished);
        assert!(delivery.delivered.is_empty());
        assert_eq!(delivery.resends, 1);
        assert_eq!(delivery.moblin_request_id(), "PAID-1");
        assert!(deliveries.failed(&delivery, "not-found"));
        deliveries.resend("S", "PAID", &tip()).unwrap();
        let delivery = deliveries.get("PAID").unwrap();
        assert_eq!(delivery.failure, None);
        assert_eq!(delivery.moblin_request_id(), "PAID-2");
        assert_eq!(request_ids(&deliveries), vec!["PAID"]);
        deliveries.resend("S", "GONE", &song()).unwrap();
        assert_eq!(
            deliveries.get("GONE").unwrap().moblin_request_id(),
            "GONE-1"
        );
    }

    #[test]
    fn deliveries_are_given_up_after_7_days() {
        let deliveries = Deliveries::new(Database::memory());
        for (request_id, item, age) in [
            ("A", song(), 8),
            ("B", song(), 6),
            ("C", tip(), 8),
            ("D", tip(), 6),
            ("E", spin(), 8),
            ("F", spin(), 6),
            ("G", tip(), 8),
        ] {
            deliveries.add_test("S".to_string(), request_id.to_string(), item);
            deliveries
                .database
                .lock()
                .execute(
                    "UPDATE deliveries SET requested_at = ?1 WHERE request_id = ?2",
                    params![now() - age * 24 * 60 * 60, request_id],
                )
                .unwrap();
        }
        deliveries.delivered(
            "G",
            None,
            "phone",
            &["phone".to_string(), "tablet".to_string()],
        );
        deliveries.expire();
        assert_eq!(request_ids(&deliveries), vec!["B", "D", "F"]);
        let expiries: Vec<(String, u64)> = deliveries
            .database
            .lock()
            .prepare("SELECT name, value FROM counters ORDER BY name")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            expiries,
            [
                ("songTestExpiries".to_string(), 1),
                ("spinTestExpiries".to_string(), 1),
                ("tipTestExpiries".to_string(), 1),
            ]
        );
    }

    #[test]
    fn only_deliveries_no_device_got_are_counted() {
        let deliveries = Deliveries::new(Database::memory());
        deliveries.add_test("S".to_string(), "A".to_string(), tip());
        deliveries.add_test("S".to_string(), "B".to_string(), tip());
        assert_eq!(deliveries.count(), 2);
        deliveries.delivered(
            "A",
            None,
            "phone",
            &["phone".to_string(), "tablet".to_string()],
        );
        assert!(!deliveries.get("A").unwrap().finished);
        assert_eq!(deliveries.count(), 1);
    }

    #[test]
    fn songs_not_found_stay_songs_and_failures_are_counted_once() {
        let deliveries = Deliveries::new(Database::memory());
        deliveries.add_test("S".to_string(), "A".to_string(), song());
        deliveries.add_test("S".to_string(), "B".to_string(), tip());
        let pending = deliveries.pending();
        assert!(deliveries.failed(&pending[0], "moblin-error"));
        assert!(!deliveries.failed(&pending[0], "moblin-error"));
        assert!(deliveries.failed(&pending[0], "not-found"));
        assert!(!deliveries.failed(&pending[0], "not-found"));
        assert!(deliveries.failed(&pending[1], "moblin-error"));
        assert!(!deliveries.failed(&pending[1], "moblin-error"));
        assert_eq!(request_ids(&deliveries), vec!["B"]);
        assert_eq!(deliveries.count(), 1);
        let song_not_found = deliveries.get("A").unwrap();
        assert_eq!(song_not_found.item, song());
        assert_eq!(song_not_found.failure.as_deref(), Some("not-found"));
        assert_eq!(
            deliveries.get("B").unwrap().failure.as_deref(),
            Some("moblin-error")
        );
        let age = |days: u64| {
            deliveries
                .database
                .lock()
                .execute(
                    "UPDATE deliveries SET requested_at = ?1",
                    [now() - days * 24 * 60 * 60],
                )
                .unwrap();
        };
        age(8);
        deliveries.expire();
        assert!(deliveries.get("A").is_some());
        assert!(deliveries.get("B").is_none());
        age(31);
        deliveries.expire();
        assert!(deliveries.get("A").is_none());
        let counters: Vec<(String, u64)> = deliveries
            .database
            .lock()
            .prepare("SELECT name, value FROM counters ORDER BY name")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            counters,
            [
                ("songTestDeliveriesFailed:moblin-error".to_string(), 1),
                ("songTestDeliveriesFailed:not-found".to_string(), 1),
                ("tipTestDeliveriesFailed:moblin-error".to_string(), 1),
                ("tipTestExpiries".to_string(), 1),
            ]
        );
    }
}
