use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Result, bail};
use log::warn;
use rusqlite::params;
use serde::{Deserialize, Serialize, Serializer};
use tokio::sync::Notify;
use tokio::sync::futures::Notified;
use tokio::time::Instant;

use crate::database::Database;

const MAX_CHANNELS: usize = 5;
const MAX_NAME_LENGTH: usize = 40;
const RETRY_DELAY: Duration = Duration::from_secs(30);
const RETRY_MAX_DELAY: Duration = Duration::from_secs(60 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Twitch,
    YouTube,
    Kick,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    pub platform: Platform,
    pub name: String,
}

pub type ChannelKey = (Platform, String);

impl Channel {
    fn validate(&self) -> Result<()> {
        let name = &self.name;
        let plain = name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
        if name.is_empty() || name.len() > MAX_NAME_LENGTH || !plain {
            bail!("{name:?} is not a valid {:?} channel", self.platform);
        }
        Ok(())
    }

    pub fn validate_all(channels: &[Channel]) -> Result<()> {
        if channels.is_empty() {
            bail!("at least one channel");
        }
        if channels.len() > MAX_CHANNELS {
            bail!("at most {MAX_CHANNELS} channels");
        }
        channels.iter().try_for_each(Channel::validate)
    }

    pub fn key(&self) -> ChannelKey {
        (self.platform, self.name.to_ascii_lowercase())
    }

    fn same_as(&self, other: &Channel) -> bool {
        self.key() == other.key()
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}/{}", self.platform, self.name)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub avatar: Option<String>,
    pub display_name: Option<String>,
}

const NO_PROFILE: Profile = Profile {
    avatar: None,
    display_name: None,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lookup {
    Pending { failures: u32, due: Instant },
    Done(Profile),
}

impl Lookup {
    fn pending() -> Self {
        Lookup::Pending {
            failures: 0,
            due: Instant::now(),
        }
    }

    pub fn profile(&self) -> &Profile {
        match self {
            Lookup::Done(profile) => profile,
            Lookup::Pending { .. } => &NO_PROFILE,
        }
    }
}

impl Serialize for Lookup {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.profile().serialize(serializer)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stream {
    pub category: Option<String>,
    pub title: Option<String>,
    pub thumbnail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ListedChannel {
    #[serde(flatten)]
    pub channel: Channel,
    #[serde(flatten)]
    pub lookup: Lookup,
    pub live: bool,
    #[serde(flatten)]
    pub stream: Stream,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Streamer {
    pub channels: Vec<ListedChannel>,
}

#[derive(Serialize, Deserialize)]
struct SavedChannel {
    channel: Channel,
    profile: Option<Profile>,
    live: bool,
    stream: Stream,
}

impl From<&ListedChannel> for SavedChannel {
    fn from(listed: &ListedChannel) -> Self {
        Self {
            channel: listed.channel.clone(),
            profile: match &listed.lookup {
                Lookup::Done(profile) => Some(profile.clone()),
                Lookup::Pending { .. } => None,
            },
            live: listed.live,
            stream: listed.stream.clone(),
        }
    }
}

impl From<SavedChannel> for ListedChannel {
    fn from(saved: SavedChannel) -> Self {
        Self {
            channel: saved.channel,
            lookup: saved.profile.map_or_else(Lookup::pending, Lookup::Done),
            live: saved.live,
            stream: saved.stream,
        }
    }
}

impl Streamer {
    fn same_as(&self, other: &Streamer) -> bool {
        self.channels.iter().any(|mine| {
            other
                .channels
                .iter()
                .any(|theirs| mine.channel.same_as(&theirs.channel))
        })
    }

    fn listed(&self, channel: &Channel) -> Option<&ListedChannel> {
        self.channels
            .iter()
            .find(|listed| listed.channel.same_as(channel))
    }

    fn is_live(&self) -> bool {
        self.channels.iter().any(|listed| listed.live)
    }
}

pub struct Store {
    max_streamers: usize,
    streamers: Mutex<Vec<Streamer>>,
    database: Arc<Database>,
    lookup_pending: Notify,
    went_live: Notify,
}

impl Store {
    pub fn new(max_streamers: usize, database: Arc<Database>) -> Self {
        let store = Self {
            max_streamers,
            streamers: Mutex::new(Vec::new()),
            database,
            lookup_pending: Notify::new(),
            went_live: Notify::new(),
        };
        let mut streamers = store.load();
        if !streamers.is_empty() {
            store.trim(&mut streamers);
            store.save(&streamers);
            store.went_live.notify_one();
        }
        *store.streamers.lock().unwrap() = streamers;
        store
    }

    fn load(&self) -> Vec<Streamer> {
        let rows = self
            .database
            .lock()
            .prepare_cached("SELECT channels FROM streamers ORDER BY position")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()
            });
        let rows = rows.unwrap_or_else(|error| {
            warn!("failed to read the streamers: {error:#}");
            Vec::new()
        });
        rows.into_iter()
            .filter_map(
                |row| match serde_json::from_str::<Vec<SavedChannel>>(&row) {
                    Ok(channels) => Some(Streamer {
                        channels: channels.into_iter().map(ListedChannel::from).collect(),
                    }),
                    Err(error) => {
                        warn!("failed to parse a saved streamer: {error}");
                        None
                    }
                },
            )
            .collect()
    }

    fn save(&self, streamers: &[Streamer]) {
        let mut database = self.database.lock();
        let saved = database.transaction().and_then(|transaction| {
            transaction.execute("DELETE FROM streamers", [])?;
            for (position, streamer) in streamers.iter().enumerate() {
                let channels: Vec<SavedChannel> =
                    streamer.channels.iter().map(SavedChannel::from).collect();
                transaction.execute(
                    "INSERT INTO streamers (position, channels) VALUES (?1, ?2)",
                    params![position, serde_json::to_string(&channels).unwrap()],
                )?;
            }
            transaction.commit()
        });
        if let Err(error) = saved {
            warn!("failed to save the streamers: {error:#}");
        }
    }

    fn trim(&self, streamers: &mut Vec<Streamer>) {
        let recent_streamers_count = self.max_streamers.min(streamers.len());
        let old_streamers = streamers.split_off(recent_streamers_count);
        streamers.extend(
            old_streamers
                .into_iter()
                .filter(Streamer::is_live)
                .take(self.max_streamers),
        );
    }

    pub fn streamers(&self) -> Vec<Streamer> {
        let mut streamers = self.streamers.lock().unwrap().clone();
        streamers.sort_by_key(|streamer| !streamer.is_live());
        streamers.truncate(self.max_streamers);
        streamers
    }

    pub fn streamer_live(&self, channels: Vec<Channel>) {
        let mut streamers = self.streamers.lock().unwrap();
        let mut streamer = Streamer {
            channels: channels
                .into_iter()
                .map(|channel| ListedChannel {
                    channel,
                    lookup: Lookup::pending(),
                    live: false,
                    stream: Stream::default(),
                })
                .collect(),
        };
        let (same, others): (Vec<_>, Vec<_>) = streamers
            .drain(..)
            .partition(|other| other.same_as(&streamer));
        *streamers = others;
        for listed in &mut streamer.channels {
            if let Some(known) = same.iter().find_map(|other| other.listed(&listed.channel)) {
                listed.lookup = known.lookup.clone();
                listed.live = known.live;
                listed.stream = known.stream.clone();
            }
        }
        streamers.insert(0, streamer);
        self.trim(&mut streamers);
        self.save(&streamers);
        self.lookup_pending.notify_one();
        self.went_live.notify_one();
    }

    pub fn went_live(&self) -> Notified<'_> {
        self.went_live.notified()
    }

    pub fn channels(&self, platform: Platform) -> Vec<Channel> {
        self.streamers
            .lock()
            .unwrap()
            .iter()
            .flat_map(|streamer| &streamer.channels)
            .filter(|listed| listed.channel.platform == platform)
            .map(|listed| listed.channel.clone())
            .collect()
    }

    pub fn set_live(&self, channel: &Channel, stream: Option<Stream>) {
        let mut streamers = self.streamers.lock().unwrap();
        if let Some(listed) = streamers
            .iter_mut()
            .flat_map(|streamer| &mut streamer.channels)
            .find(|listed| listed.channel.same_as(channel))
        {
            let live = stream.is_some();
            let stream = stream.unwrap_or_default();
            if listed.live != live || listed.stream != stream {
                listed.live = live;
                listed.stream = stream;
                self.save(&streamers);
            }
        }
    }

    pub fn next_lookup(&self) -> Option<(Channel, Instant)> {
        self.streamers
            .lock()
            .unwrap()
            .iter()
            .flat_map(|streamer| &streamer.channels)
            .filter_map(|listed| match listed.lookup {
                Lookup::Pending { due, .. } => Some((&listed.channel, due)),
                Lookup::Done(_) => None,
            })
            .min_by_key(|(_, due)| *due)
            .map(|(channel, due)| (channel.clone(), due))
    }

    pub fn lookup_pending(&self) -> Notified<'_> {
        self.lookup_pending.notified()
    }

    pub fn looked_up(&self, channel: &Channel, profile: Profile) {
        self.set_lookup(channel, |_| Lookup::Done(profile));
    }

    pub fn lookup_failed(&self, channel: &Channel) -> Option<Duration> {
        let mut delay = None;
        self.set_lookup(channel, |lookup| {
            let failures = match lookup {
                Lookup::Pending { failures, .. } => failures + 1,
                Lookup::Done(_) => 1,
            };
            let retry_in = RETRY_DELAY
                .saturating_mul(2u32.saturating_pow(failures - 1))
                .min(RETRY_MAX_DELAY);
            delay = Some(retry_in);
            Lookup::Pending {
                failures,
                due: Instant::now() + retry_in,
            }
        });
        delay
    }

    fn set_lookup(&self, channel: &Channel, lookup: impl FnOnce(&Lookup) -> Lookup) {
        let mut streamers = self.streamers.lock().unwrap();
        let listed = streamers
            .iter_mut()
            .flat_map(|streamer| &mut streamer.channels)
            .find(|listed| listed.channel.same_as(channel));
        if let Some(listed) = listed {
            listed.lookup = lookup(&listed.lookup);
            self.save(&streamers);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database;

    fn channel(platform: Platform, handle: &str) -> Channel {
        Channel {
            platform,
            name: handle.to_string(),
        }
    }

    fn twitch(handle: &str) -> Vec<Channel> {
        vec![channel(Platform::Twitch, handle)]
    }

    fn handles(store: &Store) -> Vec<String> {
        store
            .streamers()
            .into_iter()
            .map(|streamer| streamer.channels[0].channel.name.clone())
            .collect()
    }

    fn tracked(store: &Store) -> Vec<String> {
        store
            .channels(Platform::Twitch)
            .into_iter()
            .map(|channel| channel.name)
            .collect()
    }

    fn lookups(store: &Store) -> Vec<Lookup> {
        store
            .streamers()
            .into_iter()
            .flat_map(|streamer| streamer.channels)
            .map(|listed| listed.lookup)
            .collect()
    }

    fn lives(store: &Store) -> Vec<bool> {
        store
            .streamers()
            .into_iter()
            .flat_map(|streamer| streamer.channels)
            .map(|listed| listed.live)
            .collect()
    }

    fn stream(category: &str) -> Stream {
        Stream {
            category: Some(category.to_string()),
            title: None,
            thumbnail: None,
        }
    }

    fn categories(store: &Store) -> Vec<Option<String>> {
        store
            .streamers()
            .into_iter()
            .flat_map(|streamer| streamer.channels)
            .map(|listed| listed.stream.category)
            .collect()
    }

    fn pending_in(lookup: &Lookup) -> Duration {
        match lookup {
            Lookup::Pending { due, .. } => due.saturating_duration_since(Instant::now()),
            Lookup::Done(_) => panic!("{lookup:?} is not pending"),
        }
    }

    fn profile(avatar: &str, display_name: &str) -> Profile {
        Profile {
            avatar: Some(avatar.to_string()),
            display_name: Some(display_name.to_string()),
        }
    }

    #[test]
    fn newest_first_and_one_entry_per_streamer() {
        let store = Store::new(24, Database::memory());
        store.streamer_live(twitch("anna"));
        store.streamer_live(twitch("bob"));
        store.streamer_live(twitch("Anna"));
        assert_eq!(handles(&store), ["Anna", "bob"]);
        store.streamer_live(twitch("carl"));
        store.streamer_live(twitch("bob"));
        assert_eq!(handles(&store), ["bob", "carl", "Anna"]);
    }

    #[test]
    fn sharing_a_channel_means_same_streamer() {
        let store = Store::new(24, Database::memory());
        store.streamer_live(twitch("anna"));
        let mut channels = twitch("anna");
        channels.push(channel(Platform::Kick, "anna_irl"));
        store.streamer_live(channels);
        let streamers = store.streamers();
        assert_eq!(handles(&store), ["anna"]);
        assert_eq!(streamers[0].channels.len(), 2);
    }

    #[test]
    fn list_is_capped() {
        let store = Store::new(2, Database::memory());
        for handle in ["a", "b", "c"] {
            store.streamer_live(twitch(handle));
        }
        assert_eq!(handles(&store), ["c", "b"]);
    }

    #[test]
    fn live_streamers_are_listed_first_and_kept() {
        let store = Store::new(2, Database::memory());
        let live =
            |handle: &str| store.set_live(&channel(Platform::Twitch, handle), Some(stream("IRL")));
        for handle in ["anna", "bob", "carl"] {
            store.streamer_live(twitch(handle));
        }
        assert_eq!(handles(&store), ["carl", "bob"]);
        live("bob");
        assert_eq!(handles(&store), ["bob", "carl"]);
        store.streamer_live(twitch("dan"));
        assert_eq!(handles(&store), ["bob", "dan"]);
        assert_eq!(tracked(&store), ["dan", "carl", "bob"]);
        live("dan");
        assert_eq!(handles(&store), ["dan", "bob"]);
        store.streamer_live(twitch("erin"));
        store.streamer_live(twitch("fred"));
        assert_eq!(handles(&store), ["dan", "bob"]);
        assert_eq!(tracked(&store), ["fred", "erin", "dan", "bob"]);
        store.set_live(&channel(Platform::Twitch, "bob"), None);
        assert_eq!(handles(&store), ["dan", "fred"]);
        store.streamer_live(twitch("gus"));
        assert_eq!(tracked(&store), ["gus", "fred", "dan"]);
    }

    #[test]
    fn newcomers_are_kept_until_checked_and_older_live_streamers_are_capped() {
        let store = Store::new(1, Database::memory());
        for handle in ["anna", "bob", "carl", "dan", "erin"] {
            store.streamer_live(twitch(handle));
            assert_eq!(tracked(&store)[0], handle);
            store.set_live(&channel(Platform::Twitch, handle), Some(stream("IRL")));
        }
        assert_eq!(handles(&store), ["erin"]);
        assert_eq!(tracked(&store), ["erin", "dan"]);
    }

    #[test]
    fn streamers_are_kept_across_restarts() {
        let path = database::temporary_path("streamers");
        let anna = channel(Platform::Twitch, "anna");
        let kick = channel(Platform::Kick, "anna_irl");
        let store = Store::new(2, Database::open(Some(&path)).unwrap());
        store.streamer_live(twitch("bob"));
        store.set_live(&channel(Platform::Twitch, "bob"), Some(stream("IRL")));
        store.streamer_live(twitch("carl"));
        store.streamer_live(twitch("dan"));
        store.streamer_live(vec![anna.clone(), kick.clone()]);
        store.looked_up(&anna, profile("https://a/1.png", "Anna"));
        let before = serde_json::to_value(store.streamers()).unwrap();
        drop(store);
        let store = Store::new(2, Database::open(Some(&path)).unwrap());
        assert_eq!(serde_json::to_value(store.streamers()).unwrap(), before);
        assert_eq!(handles(&store), ["bob", "anna"]);
        assert_eq!(tracked(&store), ["anna", "dan", "bob"]);
        assert_eq!(store.next_lookup().unwrap().0, kick);
        drop(store);
        let store = Store::new(1, Database::open(Some(&path)).unwrap());
        assert_eq!(tracked(&store), ["anna", "bob"]);
        database::remove(&path);
    }

    #[test]
    fn rejects_bad_requests() {
        let request = |channels: Vec<Channel>| Channel::validate_all(&channels);
        let twitch = |handle: &str| channel(Platform::Twitch, handle);
        assert!(request(vec![]).is_err());
        assert!(request((0..6).map(|i| twitch(&format!("anna{i}"))).collect()).is_err());
        for handle in [
            "",
            "anna/../x",
            "anna banana",
            "https://twitch.tv/anna",
            &"x".repeat(41),
        ] {
            assert!(request(vec![twitch(handle)]).is_err(), "{handle:?}");
        }
        assert!(request(vec![twitch("anna")]).is_ok());
    }

    #[test]
    fn a_channel_is_looked_up_once() {
        let store = Store::new(24, Database::memory());
        let anna = channel(Platform::Twitch, "anna");
        assert_eq!(store.next_lookup(), None);
        store.streamer_live(vec![anna.clone()]);
        let (channel, due) = store.next_lookup().unwrap();
        assert_eq!(channel, anna);
        assert!(due <= Instant::now());
        store.looked_up(&anna, profile("https://a/1.png", "Anna"));
        assert_eq!(store.next_lookup(), None);
        assert_eq!(
            lookups(&store),
            [Lookup::Done(profile("https://a/1.png", "Anna"))]
        );
        store.streamer_live(twitch("Anna"));
        assert_eq!(store.next_lookup(), None);
        assert_eq!(
            lookups(&store),
            [Lookup::Done(profile("https://a/1.png", "Anna"))]
        );
    }

    #[test]
    fn no_profile_is_an_answer_too() {
        let store = Store::new(24, Database::memory());
        let bob = channel(Platform::Twitch, "bob");
        store.streamer_live(vec![bob.clone()]);
        assert_eq!(
            store.streamers()[0].channels[0].lookup.profile(),
            &NO_PROFILE
        );
        store.looked_up(&bob, Profile::default());
        assert_eq!(store.next_lookup(), None);
        assert_eq!(lookups(&store), [Lookup::Done(Profile::default())]);
    }

    #[test]
    fn a_new_channel_of_a_listed_streamer_is_looked_up() {
        let store = Store::new(24, Database::memory());
        let anna = channel(Platform::Twitch, "anna");
        let kick = channel(Platform::Kick, "anna_irl");
        store.streamer_live(vec![anna.clone()]);
        store.looked_up(&anna, profile("https://a/1.png", "Anna"));
        store.streamer_live(vec![anna.clone(), kick.clone()]);
        assert_eq!(store.next_lookup().unwrap().0, kick);
        assert_eq!(
            lookups(&store)[0],
            Lookup::Done(profile("https://a/1.png", "Anna"))
        );
        assert_eq!(pending_in(&lookups(&store)[1]), Duration::ZERO);
    }

    #[test]
    fn failed_lookups_are_retried_ever_later() {
        let store = Store::new(24, Database::memory());
        let anna = channel(Platform::Twitch, "anna");
        store.streamer_live(vec![anna.clone()]);
        let mut previous = Duration::ZERO;
        for _ in 0..10 {
            let delay = store.lookup_failed(&anna).unwrap();
            assert!(delay > previous || delay >= RETRY_MAX_DELAY);
            assert!(delay <= RETRY_MAX_DELAY);
            let due = store.next_lookup().unwrap().1;
            assert!(due > Instant::now() + delay - Duration::from_secs(1));
            previous = delay;
        }
        assert_eq!(previous, RETRY_MAX_DELAY);
        store.streamer_live(twitch("Anna"));
        assert!(pending_in(&lookups(&store)[0]) >= RETRY_MAX_DELAY - Duration::from_secs(1));
    }

    #[test]
    fn a_lookup_for_a_channel_no_longer_listed_is_discarded() {
        let store = Store::new(1, Database::memory());
        let anna = channel(Platform::Twitch, "anna");
        let bob = channel(Platform::Twitch, "bob");
        store.streamer_live(vec![anna.clone()]);
        store.streamer_live(vec![bob.clone()]);
        assert_eq!(handles(&store), ["bob"]);
        store.looked_up(&anna, profile("https://a/1.png", "Anna"));
        assert_eq!(store.lookup_failed(&anna), None);
        assert_eq!(handles(&store), ["bob"]);
        assert_eq!(store.next_lookup().unwrap().0, bob);
    }

    #[test]
    fn live_status_is_kept_per_channel() {
        let store = Store::new(24, Database::memory());
        let anna = channel(Platform::Twitch, "Anna");
        let kick = channel(Platform::Kick, "anna_irl");
        store.streamer_live(vec![anna.clone(), kick.clone()]);
        store.streamer_live(twitch("bob"));
        assert_eq!(
            store.channels(Platform::Twitch),
            [channel(Platform::Twitch, "bob"), anna.clone()]
        );
        assert_eq!(store.channels(Platform::Kick), std::slice::from_ref(&kick));
        assert_eq!(lives(&store), [false, false, false]);
        store.set_live(&channel(Platform::Twitch, "anna"), Some(stream("IRL")));
        store.set_live(&kick, Some(Stream::default()));
        assert_eq!(lives(&store), [true, true, false]);
        assert_eq!(categories(&store), [Some("IRL".into()), None, None]);
        store.streamer_live(vec![anna.clone(), channel(Platform::YouTube, "AnnaIRL")]);
        assert_eq!(lives(&store), [true, false, false]);
        assert_eq!(categories(&store), [Some("IRL".into()), None, None]);
        store.set_live(&anna, None);
        store.set_live(&channel(Platform::Twitch, "carl"), Some(stream("IRL")));
        assert_eq!(lives(&store), [false, false, false]);
    }

    #[test]
    fn serializes_the_profile_flat_with_nulls_for_the_unknown() {
        let store = Store::new(24, Database::memory());
        let anna = channel(Platform::Twitch, "anna");
        let kick = channel(Platform::Kick, "anna_irl");
        store.streamer_live(vec![
            anna.clone(),
            kick.clone(),
            channel(Platform::YouTube, "AnnaIRL"),
        ]);
        store.streamer_live(twitch("bob"));
        store.looked_up(&anna, profile("https://a/1.png", "Anna"));
        store.looked_up(
            &kick,
            Profile {
                avatar: None,
                display_name: Some("Anna_IRL".into()),
            },
        );
        store.set_live(
            &anna,
            Some(Stream {
                title: Some("Walking around Stockholm".into()),
                thumbnail: Some("https://a/live.jpg".into()),
                ..stream("Just Chatting")
            }),
        );
        let json = serde_json::to_value(store.streamers()).unwrap();
        assert_eq!(
            json,
            serde_json::json!([
                {"channels": [
                    {"platform": "twitch", "name": "anna", "avatar": "https://a/1.png", "displayName": "Anna", "live": true, "category": "Just Chatting", "title": "Walking around Stockholm", "thumbnail": "https://a/live.jpg"},
                    {"platform": "kick", "name": "anna_irl", "avatar": null, "displayName": "Anna_IRL", "live": false, "category": null, "title": null, "thumbnail": null},
                    {"platform": "youtube", "name": "AnnaIRL", "avatar": null, "displayName": null, "live": false, "category": null, "title": null, "thumbnail": null},
                ],},
                {"channels": [
                    {"platform": "twitch", "name": "bob", "avatar": null, "displayName": null, "live": false, "category": null, "title": null, "thumbnail": null},
                ],},
            ])
        );
    }
}
