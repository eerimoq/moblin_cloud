use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use tokio::time::sleep;

use crate::json_file;
use crate::remote_control::{MusicCommand, Streamers};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueuedSong {
    pub streamer: String,
    pub request_id: String,
    pub link: String,
    pub user: String,
    pub requested_at: u64,
}

pub struct SongQueue {
    path: Option<PathBuf>,
    songs: Mutex<Vec<QueuedSong>>,
    added: Notify,
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl SongQueue {
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        let songs = json_file::load(path.as_deref())?;
        Ok(Self {
            path,
            songs: Mutex::new(songs),
            added: Notify::new(),
        })
    }

    pub fn add(&self, song: QueuedSong) {
        let mut songs = self.songs.lock().unwrap();
        songs.push(song);
        self.save(&songs);
        self.added.notify_one();
    }

    pub fn pending(&self) -> Vec<QueuedSong> {
        let mut songs = self.songs.lock().unwrap();
        let count = songs.len();
        songs.retain(|song| {
            let expired = now().saturating_sub(song.requested_at) > 7 * 24 * 60 * 60;
            if expired {
                warn!(
                    "giving up adding {:?} in request {} to the queue of remote control streamer {}",
                    song.link, song.request_id, song.streamer
                );
            }
            !expired
        });
        if songs.len() != count {
            self.save(&songs);
        }
        songs.clone()
    }

    fn remove(&self, request_id: &str) {
        let mut songs = self.songs.lock().unwrap();
        songs.retain(|song| song.request_id != request_id);
        self.save(&songs);
    }

    fn save(&self, songs: &Vec<QueuedSong>) {
        if let Err(error) = json_file::save(self.path.as_deref(), songs) {
            warn!("failed to save the song queue: {error:#}");
        }
    }

    pub async fn run(&self, streamers: &Streamers) {
        loop {
            for song in self.pending() {
                if !streamers.is_connected(&song.streamer) {
                    continue;
                }
                match deliver(streamers, &song).await {
                    Delivery::Added => info!(
                        "added {:?} by {:?} in request {} to the queue of remote control streamer {}",
                        song.link, song.user, song.request_id, song.streamer
                    ),
                    Delivery::NotFound => warn!(
                        "{:?} in request {} was not found by remote control streamer {}",
                        song.link, song.request_id, song.streamer
                    ),
                    Delivery::Failed => continue,
                }
                self.remove(&song.request_id);
            }
            tokio::select! {
                _ = self.added.notified() => {}
                _ = streamers.connected.notified() => {}
                _ = sleep(Duration::from_secs(10)) => {}
            }
        }
    }
}

enum Delivery {
    Added,
    NotFound,
    Failed,
}

async fn deliver(streamers: &Streamers, song: &QueuedSong) -> Delivery {
    let add = MusicCommand::Add {
        title: song.link.clone(),
        user: song.user.clone(),
        request_id: Some(song.request_id.clone()),
    };
    match streamers.music_uncached(&song.streamer, add).await {
        Ok(status) if status.song_not_found == Some(true) => Delivery::NotFound,
        Ok(_) => Delivery::Added,
        Err(_) => Delivery::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(request_id: &str, requested_at: u64) -> QueuedSong {
        QueuedSong {
            streamer: "S".to_string(),
            request_id: request_id.to_string(),
            link: "https://music.apple.com/song/1?i=1".to_string(),
            user: "Viewer".to_string(),
            requested_at,
        }
    }

    #[test]
    fn songs_are_saved() {
        let path = std::env::temp_dir().join(format!(
            "moblin-website-song-queue-{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let requested_at = now();
        let song_queue = SongQueue::load(Some(path.clone())).unwrap();
        song_queue.add(song("A", requested_at));
        song_queue.add(song("B", requested_at));
        song_queue.remove("A");
        let song_queue = SongQueue::load(Some(path.clone())).unwrap();
        assert_eq!(song_queue.pending(), vec![song("B", requested_at)]);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn old_songs_are_dropped() {
        let song_queue = SongQueue::load(None).unwrap();
        song_queue.add(song("A", now() - 8 * 24 * 60 * 60));
        song_queue.add(song("B", now() - 6 * 24 * 60 * 60));
        let pending: Vec<String> = song_queue
            .pending()
            .into_iter()
            .map(|song| song.request_id)
            .collect();
        assert_eq!(pending, vec!["B"]);
    }
}
