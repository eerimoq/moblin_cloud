use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::extract::ws::{Message, WebSocket};
use base64::Engine;
use base64::prelude::{BASE64_STANDARD, BASE64_URL_SAFE_NO_PAD};
use log::info;
use reqwest::Url;
use ring::digest::{SHA256, digest};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{Notify, mpsc, oneshot};
use tokio::time::{Instant, sleep_until, timeout, timeout_at};

const KEY_LENGTH: usize = 32;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Outcome {
    Ok {},
    WrongPassword {},
    UnknownRequest {},
    NotIdentified {},
    AlreadyIdentified {},
    Error {},
}

#[derive(Debug, PartialEq, Serialize)]
struct Authentication {
    challenge: String,
    salt: String,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum MessageToStreamer {
    #[serde(rename_all = "camelCase")]
    Hello {
        api_version: String,
        authentication: Authentication,
    },
    Identified {
        result: Outcome,
    },
    Request {
        id: u64,
        data: Request,
    },
    Pong {},
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum MessageToAssistant {
    Identify {},
    Response {
        id: u64,
        result: Outcome,
        data: Option<Value>,
    },
    Ping {},
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum Request {
    Music { command: MusicCommand },
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MusicCommand {
    Status {},
    #[serde(rename_all = "camelCase")]
    Add {
        title: String,
        user: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
    },
    Find {
        title: String,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum ResponseData {
    Music { status: MusicStatus },
    MusicFind { song: Option<FoundSong> },
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct FoundSong {
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
}

impl FoundSong {
    pub fn name(&self) -> String {
        match &self.artist {
            Some(artist) => format!("{artist} - {}", self.title),
            None => self.title.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicStatus {
    pub playing: bool,
    pub current_song_index: Option<usize>,
    pub songs: Vec<Song>,
    pub song_not_found: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Song {
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub user: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum RequestError {
    Offline,
    TooFast,
    Timeout,
    Failed,
}

struct Session {
    number: u64,
    sender: mpsc::UnboundedSender<MessageToStreamer>,
    next_request_id: u64,
    pending: HashMap<u64, oneshot::Sender<(Outcome, Option<Value>)>>,
    last_request: Option<Instant>,
    status: Option<(Instant, MusicStatus)>,
}

#[derive(Default)]
struct Sessions {
    next_number: u64,
    by_id: HashMap<String, Session>,
}

pub struct Streamers {
    random: SystemRandom,
    sessions: Mutex<Sessions>,
    pub connected: Notify,
}

pub fn id_of(key: &str) -> Option<String> {
    let decoded = BASE64_URL_SAFE_NO_PAD.decode(key).ok()?;
    if decoded.len() != KEY_LENGTH {
        return None;
    }
    Some(BASE64_URL_SAFE_NO_PAD.encode(&digest(&SHA256, key.as_bytes()).as_ref()[..12]))
}

pub fn song_link(text: &str) -> Option<String> {
    let url = Url::parse(text.trim()).ok()?;
    if url.scheme() != "https" || url.host_str() != Some("music.apple.com") {
        return None;
    }
    let song_id = match url.query_pairs().find(|(name, _)| name == "i") {
        Some((_, song_id)) => song_id.into_owned(),
        None => {
            let segments: Vec<&str> = url.path_segments()?.collect();
            if !segments.contains(&"song") {
                return None;
            }
            segments.last()?.to_string()
        }
    };
    if !(1..=20).contains(&song_id.len()) || !song_id.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!(
        "https://music.apple.com/song/{song_id}?i={song_id}"
    ))
}

async fn send(socket: &mut WebSocket, message: &MessageToStreamer) -> Result<()> {
    let text = serde_json::to_string(message)?;
    socket.send(Message::Text(text.into())).await?;
    Ok(())
}

impl Streamers {
    pub fn new() -> Self {
        Self {
            random: SystemRandom::new(),
            sessions: Mutex::new(Sessions::default()),
            connected: Notify::new(),
        }
    }

    fn random_bytes<const N: usize>(&self) -> Option<[u8; N]> {
        let mut bytes = [0; N];
        self.random.fill(&mut bytes).ok()?;
        Some(bytes)
    }

    pub fn new_key(&self) -> Option<(String, String)> {
        let key = BASE64_URL_SAFE_NO_PAD.encode(self.random_bytes::<KEY_LENGTH>()?);
        let id = id_of(&key)?;
        Some((key, id))
    }

    pub fn new_request_id(&self) -> Option<String> {
        Some(BASE64_URL_SAFE_NO_PAD.encode(self.random_bytes::<16>()?))
    }

    pub fn limit_requests(&self, id: &str) -> Result<(), RequestError> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions.by_id.get_mut(id).ok_or(RequestError::Offline)?;
        if session
            .last_request
            .is_some_and(|last_request| last_request.elapsed() < Duration::from_secs(3))
        {
            return Err(RequestError::TooFast);
        }
        session.last_request = Some(Instant::now());
        Ok(())
    }

    pub fn is_connected(&self, id: &str) -> bool {
        self.sessions.lock().unwrap().by_id.contains_key(id)
    }

    pub async fn serve(&self, id: String, mut socket: WebSocket) {
        if let Err(error) = self.handshake(&mut socket).await {
            info!("remote control streamer {id} failed to identify: {error:#}");
            return;
        }
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let Some(number) = self.add(&id, sender) else {
            info!("remote control streamer {id} rejected, too many streamers");
            return;
        };
        info!("remote control streamer {id} connected");
        self.connected.notify_one();
        let mut deadline = Instant::now() + Duration::from_secs(60);
        loop {
            tokio::select! {
                message = receiver.recv() => {
                    let Some(message) = message else {
                        break;
                    };
                    if send(&mut socket, &message).await.is_err() {
                        break;
                    }
                }
                message = socket.recv() => {
                    let Some(Ok(message)) = message else {
                        break;
                    };
                    deadline = Instant::now() + Duration::from_secs(60);
                    match message {
                        Message::Text(text) => {
                            if self.handle(&id, number, text.as_str(), &mut socket).await.is_err() {
                                break;
                            }
                        }
                        Message::Close(_) => break,
                        _ => {}
                    }
                }
                _ = sleep_until(deadline) => break,
            }
        }
        self.remove(&id, number);
        info!("remote control streamer {id} disconnected");
    }

    async fn handshake(&self, socket: &mut WebSocket) -> Result<()> {
        let challenge = self.random_bytes::<64>().context("no randomness")?;
        let salt = self.random_bytes::<64>().context("no randomness")?;
        let hello = MessageToStreamer::Hello {
            api_version: "0.1".to_string(),
            authentication: Authentication {
                challenge: BASE64_STANDARD.encode(challenge),
                salt: BASE64_STANDARD.encode(salt),
            },
        };
        send(socket, &hello).await?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let message = timeout_at(deadline, socket.recv())
                .await
                .context("no identify")?
                .context("closed")??;
            if let Message::Text(text) = message
                && let Ok(MessageToAssistant::Identify {}) = serde_json::from_str(text.as_str())
            {
                break;
            }
        }
        send(
            socket,
            &MessageToStreamer::Identified {
                result: Outcome::Ok {},
            },
        )
        .await
    }

    async fn handle(
        &self,
        id: &str,
        number: u64,
        text: &str,
        socket: &mut WebSocket,
    ) -> Result<()> {
        match serde_json::from_str(text) {
            Ok(MessageToAssistant::Ping {}) => send(socket, &MessageToStreamer::Pong {}).await?,
            Ok(MessageToAssistant::Response {
                id: request_id,
                result,
                data,
            }) => {
                let mut sessions = self.sessions.lock().unwrap();
                if let Some(session) = sessions.by_id.get_mut(id)
                    && session.number == number
                    && let Some(pending) = session.pending.remove(&request_id)
                {
                    let _ = pending.send((result, data));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn add(&self, id: &str, sender: mpsc::UnboundedSender<MessageToStreamer>) -> Option<u64> {
        let mut sessions = self.sessions.lock().unwrap();
        if sessions.by_id.len() >= 1000 && !sessions.by_id.contains_key(id) {
            return None;
        }
        sessions.next_number += 1;
        let number = sessions.next_number;
        sessions.by_id.insert(
            id.to_string(),
            Session {
                number,
                sender,
                next_request_id: 0,
                pending: HashMap::new(),
                last_request: None,
                status: None,
            },
        );
        Some(number)
    }

    fn remove(&self, id: &str, number: u64) {
        let mut sessions = self.sessions.lock().unwrap();
        if sessions
            .by_id
            .get(id)
            .is_some_and(|session| session.number == number)
        {
            sessions.by_id.remove(id);
        }
    }

    pub async fn music(
        &self,
        id: &str,
        command: MusicCommand,
    ) -> Result<MusicStatus, RequestError> {
        if matches!(command, MusicCommand::Status {}) {
            let sessions = self.sessions.lock().unwrap();
            if let Some(session) = sessions.by_id.get(id)
                && let Some((fetched, status)) = &session.status
                && fetched.elapsed() < Duration::from_secs(10)
            {
                return Ok(status.clone());
            }
        }
        self.music_uncached(id, command).await
    }

    pub async fn music_uncached(
        &self,
        id: &str,
        command: MusicCommand,
    ) -> Result<MusicStatus, RequestError> {
        let (number, ResponseData::Music { status }) =
            self.request(id, command, Duration::from_secs(20)).await?
        else {
            return Err(RequestError::Failed);
        };
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.by_id.get_mut(id)
            && session.number == number
        {
            let mut cached = status.clone();
            cached.song_not_found = None;
            session.status = Some((Instant::now(), cached));
        }
        Ok(status)
    }

    pub async fn find(&self, id: &str, title: String) -> Result<Option<FoundSong>, RequestError> {
        match self
            .request(id, MusicCommand::Find { title }, Duration::from_secs(10))
            .await?
        {
            (_, ResponseData::MusicFind { song }) => Ok(song),
            _ => Err(RequestError::Failed),
        }
    }

    async fn request(
        &self,
        id: &str,
        command: MusicCommand,
        wait: Duration,
    ) -> Result<(u64, ResponseData), RequestError> {
        let (number, request_id, receiver) = {
            let mut sessions = self.sessions.lock().unwrap();
            let session = sessions.by_id.get_mut(id).ok_or(RequestError::Offline)?;
            session.next_request_id += 1;
            let request_id = session.next_request_id;
            let (sender, receiver) = oneshot::channel();
            session.pending.insert(request_id, sender);
            session
                .sender
                .send(MessageToStreamer::Request {
                    id: request_id,
                    data: Request::Music { command },
                })
                .map_err(|_| RequestError::Offline)?;
            (session.number, request_id, receiver)
        };
        let (result, data) = match timeout(wait, receiver).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => return Err(RequestError::Offline),
            Err(_) => {
                let mut sessions = self.sessions.lock().unwrap();
                if let Some(session) = sessions.by_id.get_mut(id)
                    && session.number == number
                {
                    session.pending.remove(&request_id);
                }
                return Err(RequestError::Timeout);
            }
        };
        if result != (Outcome::Ok {}) {
            return Err(RequestError::Failed);
        }
        let Some(Ok(data)) = data.map(serde_json::from_value) else {
            return Err(RequestError::Failed);
        };
        Ok((number, data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(message: &MessageToStreamer) -> String {
        serde_json::to_string(message).unwrap()
    }

    fn decode(text: &str) -> MessageToAssistant {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn messages_to_streamer_match_moblin() {
        assert_eq!(encode(&MessageToStreamer::Pong {}), r#"{"pong":{}}"#);
        assert_eq!(
            encode(&MessageToStreamer::Identified {
                result: Outcome::Ok {}
            }),
            r#"{"identified":{"result":{"ok":{}}}}"#
        );
        assert_eq!(
            encode(&MessageToStreamer::Hello {
                api_version: "0.1".to_string(),
                authentication: Authentication {
                    challenge: "c".to_string(),
                    salt: "s".to_string(),
                },
            }),
            r#"{"hello":{"apiVersion":"0.1","authentication":{"challenge":"c","salt":"s"}}}"#
        );
        assert_eq!(
            encode(&MessageToStreamer::Request {
                id: 3,
                data: Request::Music {
                    command: MusicCommand::Add {
                        title: "Song".to_string(),
                        user: "Erik".to_string(),
                        request_id: None,
                    },
                },
            }),
            r#"{"request":{"id":3,"data":{"music":{"command":{"add":{"title":"Song","user":"Erik"}}}}}}"#
        );
        assert_eq!(
            encode(&MessageToStreamer::Request {
                id: 5,
                data: Request::Music {
                    command: MusicCommand::Add {
                        title: "Song".to_string(),
                        user: "Erik".to_string(),
                        request_id: Some("ORDER1".to_string()),
                    },
                },
            }),
            r#"{"request":{"id":5,"data":{"music":{"command":{"add":{"title":"Song","user":"Erik","requestId":"ORDER1"}}}}}}"#
        );
        assert_eq!(
            encode(&MessageToStreamer::Request {
                id: 6,
                data: Request::Music {
                    command: MusicCommand::Find {
                        title: "Song".to_string(),
                    },
                },
            }),
            r#"{"request":{"id":6,"data":{"music":{"command":{"find":{"title":"Song"}}}}}}"#
        );
        assert_eq!(
            encode(&MessageToStreamer::Request {
                id: 4,
                data: Request::Music {
                    command: MusicCommand::Status {},
                },
            }),
            r#"{"request":{"id":4,"data":{"music":{"command":{"status":{}}}}}}"#
        );
    }

    #[test]
    fn messages_to_assistant_match_moblin() {
        assert_eq!(decode(r#"{"ping":{}}"#), MessageToAssistant::Ping {});
        assert_eq!(
            decode(r#"{"identify":{"streamerId":"ABC","authentication":"x"}}"#),
            MessageToAssistant::Identify {}
        );
        assert_eq!(
            decode(r#"{"response":{"id":1,"result":{"ok":{}}}}"#),
            MessageToAssistant::Response {
                id: 1,
                result: Outcome::Ok {},
                data: None,
            }
        );
        let MessageToAssistant::Response {
            data: Some(data), ..
        } = decode(
            r#"{"response":{"id":2,"result":{"ok":{}},"data":{"music":{"status":{"playing":true,"currentSongIndex":0,"songs":[{"id":"q1","title":"Song","artist":"Band"}],"songNotFound":false}}}}}"#,
        )
        else {
            panic!();
        };
        let Ok(ResponseData::Music { status }) = serde_json::from_value(data) else {
            panic!();
        };
        assert_eq!(
            status,
            MusicStatus {
                playing: true,
                current_song_index: Some(0),
                songs: vec![Song {
                    id: "q1".to_string(),
                    title: "Song".to_string(),
                    artist: Some("Band".to_string()),
                    user: None,
                }],
                song_not_found: Some(false),
            }
        );
        let found = |text: &str| -> Option<FoundSong> {
            let MessageToAssistant::Response {
                data: Some(data), ..
            } = decode(text)
            else {
                panic!();
            };
            let Ok(ResponseData::MusicFind { song }) = serde_json::from_value(data) else {
                panic!();
            };
            song
        };
        assert_eq!(
            found(
                r#"{"response":{"id":3,"result":{"ok":{}},"data":{"musicFind":{"song":{"id":"1440818839","title":"Song","artist":"Band"}}}}}"#
            ),
            Some(FoundSong {
                id: "1440818839".to_string(),
                title: "Song".to_string(),
                artist: Some("Band".to_string()),
            })
        );
        assert_eq!(
            found(r#"{"response":{"id":4,"result":{"ok":{}},"data":{"musicFind":{}}}}"#),
            None
        );
    }

    #[test]
    fn ids_are_derived_from_keys() {
        let streamers = Streamers::new();
        let (key, id) = streamers.new_key().unwrap();
        assert_eq!(key.len(), 43);
        assert_eq!(id.len(), 16);
        assert_eq!(id_of(&key), Some(id.clone()));
        assert!(!key.contains(&id));
        assert_ne!(streamers.new_key().unwrap().0, key);
        assert_eq!(id_of("short"), None);
        assert_eq!(id_of(&format!("{key}A")), None);
    }

    #[test]
    fn only_song_links_are_accepted() {
        let link = Some("https://music.apple.com/song/1440818839?i=1440818839");
        assert_eq!(
            song_link(" https://music.apple.com/us/album/x/1440818584?i=1440818839 ").as_deref(),
            link
        );
        assert_eq!(
            song_link("https://music.apple.com/us/song/never-gonna/1440818839").as_deref(),
            link
        );
        assert_eq!(
            song_link("https://music.apple.com/se/song/x/1440818839?l=en").as_deref(),
            link
        );
        assert_eq!(
            song_link("https://music.apple.com/us/album/x/1440818584"),
            None
        );
        assert_eq!(song_link("https://music.apple.com/us/album/x/1?i=x1"), None);
        assert_eq!(song_link("https://example.com/us/song/x/1440818839"), None);
        assert_eq!(
            song_link("http://music.apple.com/us/song/x/1440818839"),
            None
        );
        assert_eq!(song_link("1440818839"), None);
        assert_eq!(song_link("Rick Astley Never Gonna"), None);
        assert_eq!(song_link("  "), None);
    }
}
