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
use tokio::sync::{mpsc, oneshot};
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
    Add { title: String, user: String },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum ResponseData {
    Music { status: MusicStatus },
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
    last_add: Option<Instant>,
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
}

pub fn id_of(key: &str) -> Option<String> {
    let decoded = BASE64_URL_SAFE_NO_PAD.decode(key).ok()?;
    if decoded.len() != KEY_LENGTH {
        return None;
    }
    Some(BASE64_URL_SAFE_NO_PAD.encode(&digest(&SHA256, key.as_bytes()).as_ref()[..12]))
}

pub fn normalize_song(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > 200 {
        return None;
    }
    if text.chars().all(|c| c.is_ascii_digit()) {
        return Some(format!("https://music.apple.com/song/{text}?i={text}"));
    }
    if let Ok(mut url) = Url::parse(text)
        && url.host_str() == Some("music.apple.com")
        && !url.query_pairs().any(|(name, _)| name == "i")
    {
        let segments: Vec<String> = url
            .path_segments()
            .map(|segments| segments.map(str::to_string).collect())
            .unwrap_or_default();
        if segments.iter().any(|segment| segment == "song")
            && let Some(song_id) = segments.last()
            && !song_id.is_empty()
            && song_id.chars().all(|c| c.is_ascii_digit())
        {
            url.query_pairs_mut().append_pair("i", song_id);
            return Some(url.to_string());
        }
    }
    Some(text.to_string())
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
                last_add: None,
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
        let is_add = matches!(command, MusicCommand::Add { .. });
        let (number, request_id, receiver) = {
            let mut sessions = self.sessions.lock().unwrap();
            let session = sessions.by_id.get_mut(id).ok_or(RequestError::Offline)?;
            if is_add {
                if session
                    .last_add
                    .is_some_and(|last_add| last_add.elapsed() < Duration::from_secs(3))
                {
                    return Err(RequestError::TooFast);
                }
                session.last_add = Some(Instant::now());
            } else if let Some((fetched, status)) = &session.status
                && fetched.elapsed() < Duration::from_secs(10)
            {
                return Ok(status.clone());
            }
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
        let response = timeout(Duration::from_secs(20), receiver).await;
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .by_id
            .get_mut(id)
            .filter(|session| session.number == number);
        let (result, data) = match response {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => return Err(RequestError::Offline),
            Err(_) => {
                if let Some(session) = session {
                    session.pending.remove(&request_id);
                }
                return Err(RequestError::Timeout);
            }
        };
        if result != (Outcome::Ok {}) {
            return Err(RequestError::Failed);
        }
        let Some(Ok(ResponseData::Music { status })) = data.map(serde_json::from_value) else {
            return Err(RequestError::Failed);
        };
        if let Some(session) = session {
            let mut cached = status.clone();
            cached.song_not_found = None;
            session.status = Some((Instant::now(), cached));
        }
        Ok(status)
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
                    },
                },
            }),
            r#"{"request":{"id":3,"data":{"music":{"command":{"add":{"title":"Song","user":"Erik"}}}}}}"#
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
        let ResponseData::Music { status } = serde_json::from_value(data).unwrap();
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
    fn songs_are_normalized() {
        assert_eq!(
            normalize_song(" 1440818839 ").as_deref(),
            Some("https://music.apple.com/song/1440818839?i=1440818839")
        );
        assert_eq!(
            normalize_song("https://music.apple.com/us/album/x/1440818584?i=1440818839").as_deref(),
            Some("https://music.apple.com/us/album/x/1440818584?i=1440818839")
        );
        assert_eq!(
            normalize_song("https://music.apple.com/us/song/never-gonna/1440818839").as_deref(),
            Some("https://music.apple.com/us/song/never-gonna/1440818839?i=1440818839")
        );
        assert_eq!(
            normalize_song("https://music.apple.com/se/song/x/1440818839?l=en").as_deref(),
            Some("https://music.apple.com/se/song/x/1440818839?l=en&i=1440818839")
        );
        assert_eq!(
            normalize_song("Rick Astley Never Gonna").as_deref(),
            Some("Rick Astley Never Gonna")
        );
        assert_eq!(normalize_song("  "), None);
        assert_eq!(normalize_song(&"a".repeat(201)), None);
    }
}
