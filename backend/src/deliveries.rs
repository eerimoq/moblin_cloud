use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::future::join_all;
use log::{info, warn};
use rusqlite::{Connection, Params, params};
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use tokio::time::sleep;

use crate::accounts::Accounts;
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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Item {
    Song {
        link: String,
        user: String,
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

    pub fn add(&self, streamer: String, request_id: String, item: Item) {
        if let Err(error) = self.database.lock().execute(
            "INSERT OR IGNORE INTO deliveries (request_id, streamer, requested_at, item) VALUES (?1, ?2, ?3, ?4)",
            params![request_id, streamer, now(), serde_json::to_string(&item).unwrap()],
        ) {
            warn!("failed to save delivery {request_id}: {error:#}");
        }
        self.added.notify_one();
    }

    pub fn pending(&self) -> Vec<Delivery> {
        let database = self.database.lock();
        let expired = query(
            &database,
            "DELETE FROM deliveries WHERE requested_at < ?1 RETURNING *",
            [now().saturating_sub(7 * 24 * 60 * 60)],
        );
        for delivery in expired {
            warn!(
                "giving up delivering {:?} in request {} to remote control streamer {}",
                delivery.item, delivery.request_id, delivery.streamer
            );
        }
        query(&database, "SELECT * FROM deliveries ORDER BY rowid", [])
    }

    fn delivered(&self, request_id: &str, device: &str, devices: &[String]) {
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
                if let Err(error) =
                    database.execute("DELETE FROM deliveries WHERE request_id = ?1", [request_id])
                {
                    warn!("failed to remove delivery {request_id}: {error:#}");
                }
            }
            Ok(_) | Err(rusqlite::Error::QueryReturnedNoRows) => {}
            Err(error) => warn!("failed to save delivery {request_id}: {error:#}"),
        }
    }

    fn remove(&self, request_id: &str) {
        if let Err(error) = self
            .database
            .lock()
            .execute("DELETE FROM deliveries WHERE request_id = ?1", [request_id])
        {
            warn!("failed to remove delivery {request_id}: {error:#}");
        }
    }

    pub async fn run(&self, streamers: &Streamers, accounts: &Accounts) {
        loop {
            let mut by_device: Vec<(String, Vec<Targeted>)> = Vec::new();
            for delivery in self.pending() {
                let page = accounts
                    .of_page(&delivery.streamer)
                    .and_then(|account| account.page);
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
                        .devices
                        .iter()
                        .map(|device| device.id.clone())
                        .filter(|device| !delivery.delivered.contains(device))
                        .collect(),
                };
                if devices.is_empty() && !delivery.delivered.is_empty() {
                    self.remove(&delivery.request_id);
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
            match deliver(streamers, &device, &delivery).await {
                Ok(Outcome::Delivered) => info!(
                    "delivered {:?} in request {} to remote control streamer {device}",
                    delivery.item, delivery.request_id
                ),
                Ok(Outcome::NotFound) => warn!(
                    "{:?} in request {} was not found by remote control streamer {device}",
                    delivery.item, delivery.request_id
                ),
                Err(RequestError::Offline | RequestError::Timeout) => return,
                Err(error) => {
                    info!(
                        "delivering request {} to remote control streamer {device} failed: {error:?}",
                        delivery.request_id
                    );
                    continue;
                }
            }
            self.delivered(&delivery.request_id, &device, &devices);
        }
    }
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
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
    });
    match deliveries {
        Ok(deliveries) => deliveries
            .into_iter()
            .filter_map(|(request_id, streamer, requested_at, item, delivered)| {
                match serde_json::from_str(&item) {
                    Ok(item) => Some(Delivery {
                        streamer,
                        request_id,
                        requested_at,
                        item,
                        delivered: serde_json::from_str(&delivered).unwrap_or_default(),
                    }),
                    Err(error) => {
                        warn!("failed to parse delivery {request_id}: {error}");
                        None
                    }
                }
            })
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
    let request_id = delivery.request_id.clone();
    match &delivery.item {
        Item::Song { link, user } => {
            let add = MusicCommand::Add {
                title: link.clone(),
                user: user.clone(),
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
        deliveries.add("S".to_string(), "A".to_string(), song());
        deliveries.add("S".to_string(), "B".to_string(), tip());
        deliveries.add("S".to_string(), "C".to_string(), spin());
        deliveries.add("S".to_string(), "C".to_string(), tip());
        deliveries.remove("A");
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
    fn old_deliveries_are_dropped() {
        let deliveries = Deliveries::new(Database::memory());
        for (request_id, item, age) in [
            ("A", song(), 8),
            ("B", song(), 6),
            ("C", tip(), 8),
            ("D", tip(), 6),
            ("E", spin(), 8),
            ("F", spin(), 6),
        ] {
            deliveries.add("S".to_string(), request_id.to_string(), item);
            deliveries
                .database
                .lock()
                .execute(
                    "UPDATE deliveries SET requested_at = ?1 WHERE request_id = ?2",
                    params![now() - age * 24 * 60 * 60, request_id],
                )
                .unwrap();
        }
        assert_eq!(request_ids(&deliveries), vec!["B", "D", "F"]);
    }
}
