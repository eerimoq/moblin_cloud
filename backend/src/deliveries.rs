use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use futures_util::future::join_all;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use tokio::time::sleep;

use crate::json_file;
use crate::pages::format_price;
use crate::remote_control::{MusicCommand, RequestError, Streamers};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Delivery {
    pub streamer: String,
    pub request_id: String,
    pub requested_at: u64,
    #[serde(flatten)]
    pub item: Item,
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

pub struct Deliveries {
    path: Option<PathBuf>,
    deliveries: Mutex<Vec<Delivery>>,
    added: Notify,
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl Deliveries {
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        let deliveries = json_file::load(path.as_deref())?;
        Ok(Self {
            path,
            deliveries: Mutex::new(deliveries),
            added: Notify::new(),
        })
    }

    pub fn add(&self, streamer: String, request_id: String, item: Item) {
        let mut deliveries = self.deliveries.lock().unwrap();
        if deliveries
            .iter()
            .any(|delivery| delivery.request_id == request_id)
        {
            return;
        }
        deliveries.push(Delivery {
            streamer,
            request_id,
            requested_at: now(),
            item,
        });
        self.save(&deliveries);
        self.added.notify_one();
    }

    pub fn pending(&self) -> Vec<Delivery> {
        let mut deliveries = self.deliveries.lock().unwrap();
        let count = deliveries.len();
        deliveries.retain(|delivery| {
            let expired = now().saturating_sub(delivery.requested_at) > 7 * 24 * 60 * 60;
            if expired {
                warn!(
                    "giving up delivering {:?} in request {} to remote control streamer {}",
                    delivery.item, delivery.request_id, delivery.streamer
                );
            }
            !expired
        });
        if deliveries.len() != count {
            self.save(&deliveries);
        }
        deliveries.clone()
    }

    fn remove(&self, request_id: &str) {
        let mut deliveries = self.deliveries.lock().unwrap();
        deliveries.retain(|delivery| delivery.request_id != request_id);
        self.save(&deliveries);
    }

    fn save(&self, deliveries: &Vec<Delivery>) {
        if let Err(error) = json_file::save(self.path.as_deref(), deliveries) {
            warn!("failed to save the deliveries: {error:#}");
        }
    }

    pub async fn run(&self, streamers: &Streamers) {
        loop {
            let mut by_streamer: Vec<(String, Vec<Delivery>)> = Vec::new();
            for delivery in self.pending() {
                if !streamers.is_connected(&delivery.streamer) {
                    continue;
                }
                match by_streamer
                    .iter_mut()
                    .find(|(streamer, _)| *streamer == delivery.streamer)
                {
                    Some((_, deliveries)) => deliveries.push(delivery),
                    None => by_streamer.push((delivery.streamer.clone(), vec![delivery])),
                }
            }
            join_all(
                by_streamer
                    .into_iter()
                    .map(|(_, deliveries)| self.deliver_all(streamers, deliveries)),
            )
            .await;
            tokio::select! {
                _ = self.added.notified() => {}
                _ = streamers.connected.notified() => {}
                _ = sleep(Duration::from_secs(10)) => {}
            }
        }
    }

    async fn deliver_all(&self, streamers: &Streamers, deliveries: Vec<Delivery>) {
        for delivery in deliveries {
            match deliver(streamers, &delivery).await {
                Ok(Outcome::Delivered) => info!(
                    "delivered {:?} in request {} to remote control streamer {}",
                    delivery.item, delivery.request_id, delivery.streamer
                ),
                Ok(Outcome::NotFound) => warn!(
                    "{:?} in request {} was not found by remote control streamer {}",
                    delivery.item, delivery.request_id, delivery.streamer
                ),
                Err(RequestError::Offline) => return,
                Err(error) => {
                    info!(
                        "delivering request {} to remote control streamer {} failed: {error:?}",
                        delivery.request_id, delivery.streamer
                    );
                    continue;
                }
            }
            self.remove(&delivery.request_id);
        }
    }
}

enum Outcome {
    Delivered,
    NotFound,
}

async fn deliver(streamers: &Streamers, delivery: &Delivery) -> Result<Outcome, RequestError> {
    let id = &delivery.streamer;
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
        let path = std::env::temp_dir().join(format!(
            "moblin-website-deliveries-{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let deliveries = Deliveries::load(Some(path.clone())).unwrap();
        deliveries.add("S".to_string(), "A".to_string(), song());
        deliveries.add("S".to_string(), "B".to_string(), tip());
        deliveries.add("S".to_string(), "C".to_string(), spin());
        deliveries.add("S".to_string(), "C".to_string(), tip());
        deliveries.remove("A");
        let saved = deliveries.pending();
        let deliveries = Deliveries::load(Some(path.clone())).unwrap();
        assert_eq!(deliveries.pending(), saved);
        assert_eq!(
            saved
                .iter()
                .map(|delivery| &delivery.item)
                .collect::<Vec<_>>(),
            vec![&tip(), &spin()]
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn queued_songs_of_before_are_loaded() {
        let delivery: Delivery = serde_json::from_str(
            r#"{"streamer":"S","requestId":"A","link":"https://music.apple.com/song/1?i=1","user":"Viewer","requestedAt":5}"#,
        )
        .unwrap();
        assert_eq!(delivery.item, song());
        assert_eq!(delivery.requested_at, 5);
    }

    #[test]
    fn old_deliveries_are_dropped() {
        let deliveries = Deliveries::load(None).unwrap();
        for (request_id, item, age) in [
            ("A", song(), 8),
            ("B", song(), 6),
            ("C", tip(), 8),
            ("D", tip(), 6),
            ("E", spin(), 8),
            ("F", spin(), 6),
        ] {
            deliveries.deliveries.lock().unwrap().push(Delivery {
                streamer: "S".to_string(),
                request_id: request_id.to_string(),
                requested_at: now() - age * 24 * 60 * 60,
                item,
            });
        }
        assert_eq!(request_ids(&deliveries), vec!["B", "D", "F"]);
    }
}
