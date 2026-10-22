use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::extract::ws::{Message, WebSocket};
use base64::Engine;
use base64::prelude::{BASE64_STANDARD, BASE64_URL_SAFE_NO_PAD};
use log::info;
use reqwest::Url;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{Notify, mpsc, oneshot};
use tokio::time::{Instant, sleep_until, timeout, timeout_at};
use uuid::Uuid;

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
    #[serde(rename_all = "camelCase")]
    Identify {
        streamer_id: Option<String>,
    },
    Response {
        id: u64,
        result: Outcome,
        data: Option<Value>,
    },
    Event {
        data: Event,
    },
    Ping {},
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Event {
    WheelOfLuck { wheel: Option<Wheel> },
    SongRequests { enabled: bool },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wheel {
    pub options: Vec<WheelOption>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WheelOption {
    pub text: String,
    pub weight: u32,
}

impl Wheel {
    fn normalize(self) -> Option<Self> {
        let options: Vec<WheelOption> = self
            .options
            .into_iter()
            .filter(|option| option.weight > 0)
            .take(100)
            .map(|option| WheelOption {
                text: option.text.trim().chars().take(100).collect(),
                weight: option.weight.min(1000),
            })
            .collect();
        (!options.is_empty()).then_some(Self { options })
    }
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum Request {
    Music {
        command: MusicCommand,
    },
    #[serde(rename_all = "camelCase")]
    SpinWheelOfLuck {
        name: String,
        message: String,
        count: u32,
        request_id: String,
    },
    #[serde(rename_all = "camelCase")]
    Tip {
        name: String,
        message: String,
        amount: String,
        currency: String,
        request_id: String,
    },
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
        message: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
    },
    Find {
        title: String,
    },
    Play {},
    Pause {},
    Clear {},
    Remove {
        id: String,
    },
    Select {
        id: String,
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
    pub duration: Option<u32>,
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
    pub duration: Option<u32>,
    pub user: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RequestError {
    Offline,
    Timeout,
    Failed,
}

struct Session {
    device: String,
    number: u64,
    sender: mpsc::UnboundedSender<MessageToStreamer>,
    next_request_id: u64,
    pending: HashMap<u64, oneshot::Sender<(Outcome, Option<Value>)>>,
    status: Option<(Instant, Result<MusicStatus, RequestError>)>,
    status_waiting: Option<Vec<oneshot::Sender<Result<MusicStatus, RequestError>>>>,
    wheel: Option<Wheel>,
    song_requests: bool,
}

#[derive(Default)]
struct Sessions {
    next_number: u64,
    by_page: HashMap<String, Vec<Session>>,
}

impl Sessions {
    fn get_mut(&mut self, page: &str, device: &str, number: Option<u64>) -> Option<&mut Session> {
        self.by_page.get_mut(page)?.iter_mut().find(|session| {
            session.device == device && number.is_none_or(|number| session.number == number)
        })
    }

    fn music(&self, page: &str) -> Option<&Session> {
        let sessions = self.by_page.get(page)?;
        sessions
            .iter()
            .find(|session| session.song_requests)
            .or(sessions.first())
    }

    fn wheel(&self, page: &str) -> Option<&Session> {
        let sessions = self.by_page.get(page)?;
        sessions
            .iter()
            .find(|session| session.wheel.is_some())
            .or(sessions.first())
    }
}

#[derive(Debug, PartialEq)]
pub struct Device {
    pub id: String,
    pub music: bool,
    pub wheel: bool,
}

pub struct Connection {
    page: String,
    device: String,
    number: u64,
    receiver: mpsc::UnboundedReceiver<MessageToStreamer>,
}

pub struct Streamers {
    random: SystemRandom,
    sessions: Mutex<Sessions>,
    pub connected: Notify,
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

pub async fn refuse(mut socket: WebSocket) {
    let refused = MessageToStreamer::Identified {
        result: Outcome::Error {},
    };
    if send(&mut socket, &refused).await.is_ok() {
        let _ = socket.send(Message::Close(None)).await;
    }
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

    pub fn new_key(&self) -> String {
        Uuid::new_v4().to_string()
    }

    pub fn new_page_id(&self) -> Option<String> {
        Some(BASE64_URL_SAFE_NO_PAD.encode(self.random_bytes::<12>()?))
    }

    pub fn new_request_id(&self) -> Option<String> {
        Some(BASE64_URL_SAFE_NO_PAD.encode(self.random_bytes::<16>()?))
    }

    pub fn new_account_id(&self) -> Option<String> {
        Some(
            self.random_bytes::<16>()?
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        )
    }

    pub fn disconnect(&self, page: &str) {
        self.sessions.lock().unwrap().by_page.remove(page);
    }

    pub fn is_connected(&self, page: &str) -> bool {
        self.sessions.lock().unwrap().by_page.contains_key(page)
    }

    pub fn connected_count(&self) -> usize {
        self.sessions
            .lock()
            .unwrap()
            .by_page
            .values()
            .map(Vec::len)
            .sum()
    }

    pub fn devices(&self, page: &str) -> Vec<Device> {
        let sessions = self.sessions.lock().unwrap();
        let music = sessions
            .music(page)
            .filter(|session| session.song_requests)
            .map(|session| session.number);
        let wheel = sessions
            .wheel(page)
            .filter(|session| session.wheel.is_some())
            .map(|session| session.number);
        sessions
            .by_page
            .get(page)
            .into_iter()
            .flatten()
            .map(|session| Device {
                id: session.device.clone(),
                music: music == Some(session.number),
                wheel: wheel == Some(session.number),
            })
            .collect()
    }

    pub fn music_device(&self, page: &str) -> Option<String> {
        let sessions = self.sessions.lock().unwrap();
        sessions.music(page).map(|session| session.device.clone())
    }

    pub fn wheel_device(&self, page: &str) -> Option<String> {
        let sessions = self.sessions.lock().unwrap();
        sessions.wheel(page).map(|session| session.device.clone())
    }

    pub async fn identify(&self, socket: &mut WebSocket) -> Result<Option<String>> {
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
                && let Ok(MessageToAssistant::Identify { streamer_id }) =
                    serde_json::from_str(text.as_str())
            {
                return Ok(streamer_id.filter(|id| (1..=100).contains(&id.len())));
            }
        }
    }

    pub fn connect(&self, page: &str, device: &str) -> Option<Connection> {
        let mut sessions = self.sessions.lock().unwrap();
        let total: usize = sessions.by_page.values().map(Vec::len).sum();
        let others = sessions
            .by_page
            .get(page)
            .into_iter()
            .flatten()
            .filter(|session| session.device != device)
            .count();
        if others >= 5 || total >= 5000 {
            return None;
        }
        sessions.next_number += 1;
        let number = sessions.next_number;
        let (sender, receiver) = mpsc::unbounded_channel();
        let page_sessions = sessions.by_page.entry(page.to_string()).or_default();
        page_sessions.retain(|session| session.device != device);
        page_sessions.push(Session {
            device: device.to_string(),
            number,
            sender,
            next_request_id: 0,
            pending: HashMap::new(),
            status: None,
            status_waiting: None,
            wheel: None,
            song_requests: true,
        });
        Some(Connection {
            page: page.to_string(),
            device: device.to_string(),
            number,
            receiver,
        })
    }

    pub async fn serve(&self, connection: Connection, mut socket: WebSocket) {
        let Connection {
            page,
            device,
            number,
            mut receiver,
        } = connection;
        let identified = MessageToStreamer::Identified {
            result: Outcome::Ok {},
        };
        if send(&mut socket, &identified).await.is_ok() {
            info!("remote control streamer {page} device {device} connected");
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
                                if self.handle(&page, &device, number, text.as_str(), &mut socket).await.is_err() {
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
        }
        self.remove(&page, &device, number);
        info!("remote control streamer {page} device {device} disconnected");
    }

    async fn handle(
        &self,
        page: &str,
        device: &str,
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
                if let Some(session) = sessions.get_mut(page, device, Some(number))
                    && let Some(pending) = session.pending.remove(&request_id)
                {
                    let _ = pending.send((result, data));
                }
            }
            Ok(MessageToAssistant::Event {
                data: Event::WheelOfLuck { wheel },
            }) => {
                let mut sessions = self.sessions.lock().unwrap();
                if let Some(session) = sessions.get_mut(page, device, Some(number)) {
                    session.wheel = wheel.and_then(Wheel::normalize);
                }
            }
            Ok(MessageToAssistant::Event {
                data: Event::SongRequests { enabled },
            }) => {
                let mut sessions = self.sessions.lock().unwrap();
                if let Some(session) = sessions.get_mut(page, device, Some(number)) {
                    session.song_requests = enabled;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn remove(&self, page: &str, device: &str, number: u64) {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(page_sessions) = sessions.by_page.get_mut(page) {
            page_sessions.retain(|session| session.device != device || session.number != number);
            if page_sessions.is_empty() {
                sessions.by_page.remove(page);
            }
        }
    }

    pub fn wheel(&self, page: &str) -> Option<Wheel> {
        self.sessions
            .lock()
            .unwrap()
            .wheel(page)
            .and_then(|session| session.wheel.clone())
    }

    pub fn takes_song_requests(&self, page: &str) -> bool {
        self.sessions
            .lock()
            .unwrap()
            .music(page)
            .is_some_and(|session| session.song_requests)
    }

    pub async fn music(
        &self,
        (page, device): (&str, &str),
        command: MusicCommand,
    ) -> Result<MusicStatus, RequestError> {
        if !matches!(command, MusicCommand::Status {}) {
            return self.music_uncached((page, device), command).await;
        }
        let number = loop {
            let fetching = {
                let mut sessions = self.sessions.lock().unwrap();
                let session = sessions
                    .get_mut(page, device, None)
                    .ok_or(RequestError::Offline)?;
                if let Some((fetched, status)) = &session.status
                    && fetched.elapsed() < Duration::from_secs(10)
                {
                    return status.clone();
                }
                match &mut session.status_waiting {
                    Some(waiting) => {
                        let (sender, receiver) = oneshot::channel();
                        waiting.push(sender);
                        Err(receiver)
                    }
                    None => {
                        session.status_waiting = Some(Vec::new());
                        Ok(session.number)
                    }
                }
            };
            match fetching {
                Ok(number) => break number,
                Err(receiver) => {
                    if let Ok(result) = receiver.await {
                        return result;
                    }
                }
            }
        };
        let mut fetch = StatusFetch {
            streamers: self,
            page,
            device,
            number,
            result: None,
        };
        let result = self
            .music_uncached((page, device), MusicCommand::Status {})
            .await;
        fetch.result = Some(result.clone());
        result
    }

    pub async fn music_uncached(
        &self,
        (page, device): (&str, &str),
        command: MusicCommand,
    ) -> Result<MusicStatus, RequestError> {
        let (number, data) = self
            .request(
                (page, device),
                Request::Music { command },
                Duration::from_secs(20),
            )
            .await?;
        let Some(ResponseData::Music { status }) = parse(data) else {
            return Err(RequestError::Failed);
        };
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(page, device, Some(number)) {
            let mut cached = status.clone();
            cached.song_not_found = None;
            session.status = Some((Instant::now(), Ok(cached)));
        }
        Ok(status)
    }

    pub async fn find(
        &self,
        (page, device): (&str, &str),
        title: String,
    ) -> Result<Option<FoundSong>, RequestError> {
        let (_, data) = self
            .request(
                (page, device),
                Request::Music {
                    command: MusicCommand::Find { title },
                },
                Duration::from_secs(10),
            )
            .await?;
        match parse(data) {
            Some(ResponseData::MusicFind { song }) => Ok(song),
            _ => Err(RequestError::Failed),
        }
    }

    pub async fn spin_wheel(
        &self,
        (page, device): (&str, &str),
        name: String,
        message: String,
        count: u32,
        request_id: String,
    ) -> Result<(), RequestError> {
        self.request(
            (page, device),
            Request::SpinWheelOfLuck {
                name,
                message,
                count,
                request_id,
            },
            Duration::from_secs(10),
        )
        .await?;
        Ok(())
    }

    pub async fn tip(
        &self,
        (page, device): (&str, &str),
        name: String,
        message: String,
        amount: String,
        currency: String,
        request_id: String,
    ) -> Result<(), RequestError> {
        self.request(
            (page, device),
            Request::Tip {
                name,
                message,
                amount,
                currency,
                request_id,
            },
            Duration::from_secs(10),
        )
        .await?;
        Ok(())
    }

    async fn request(
        &self,
        (page, device): (&str, &str),
        data: Request,
        wait: Duration,
    ) -> Result<(u64, Option<Value>), RequestError> {
        let (number, request_id, receiver) = {
            let mut sessions = self.sessions.lock().unwrap();
            let session = sessions
                .get_mut(page, device, None)
                .ok_or(RequestError::Offline)?;
            session.next_request_id += 1;
            let request_id = session.next_request_id;
            let (sender, receiver) = oneshot::channel();
            session.pending.insert(request_id, sender);
            session
                .sender
                .send(MessageToStreamer::Request {
                    id: request_id,
                    data,
                })
                .map_err(|_| RequestError::Offline)?;
            (session.number, request_id, receiver)
        };
        let (result, data) = match timeout(wait, receiver).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => return Err(RequestError::Offline),
            Err(_) => {
                let mut sessions = self.sessions.lock().unwrap();
                if let Some(session) = sessions.get_mut(page, device, Some(number)) {
                    session.pending.remove(&request_id);
                }
                return Err(RequestError::Timeout);
            }
        };
        if result != (Outcome::Ok {}) {
            return Err(RequestError::Failed);
        }
        Ok((number, data))
    }
}

struct StatusFetch<'a> {
    streamers: &'a Streamers,
    page: &'a str,
    device: &'a str,
    number: u64,
    result: Option<Result<MusicStatus, RequestError>>,
}

impl Drop for StatusFetch<'_> {
    fn drop(&mut self) {
        let mut sessions = self.streamers.sessions.lock().unwrap();
        let Some(session) = sessions.get_mut(self.page, self.device, Some(self.number)) else {
            return;
        };
        let waiting = session.status_waiting.take().unwrap_or_default();
        let Some(result) = self.result.take() else {
            return;
        };
        let result = result.map(|mut status| {
            status.song_not_found = None;
            status
        });
        session.status = Some((Instant::now(), result.clone()));
        for sender in waiting {
            let _ = sender.send(result.clone());
        }
    }
}

fn parse(data: Option<Value>) -> Option<ResponseData> {
    serde_json::from_value(data?).ok()
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
                        message: None,
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
                        message: Some("Play it loud!".to_string()),
                        request_id: Some("ORDER1".to_string()),
                    },
                },
            }),
            r#"{"request":{"id":5,"data":{"music":{"command":{"add":{"title":"Song","user":"Erik","message":"Play it loud!","requestId":"ORDER1"}}}}}}"#
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
        assert_eq!(
            encode(&MessageToStreamer::Request {
                id: 7,
                data: Request::SpinWheelOfLuck {
                    name: "Erik".to_string(),
                    message: "Go!".to_string(),
                    count: 3,
                    request_id: "ORDER1".to_string(),
                },
            }),
            r#"{"request":{"id":7,"data":{"spinWheelOfLuck":{"name":"Erik","message":"Go!","count":3,"requestId":"ORDER1"}}}}"#
        );
        assert_eq!(
            encode(&MessageToStreamer::Request {
                id: 8,
                data: Request::Tip {
                    name: "Erik".to_string(),
                    message: "Hi".to_string(),
                    amount: "7.00".to_string(),
                    currency: "USD".to_string(),
                    request_id: "ORDER2".to_string(),
                },
            }),
            r#"{"request":{"id":8,"data":{"tip":{"name":"Erik","message":"Hi","amount":"7.00","currency":"USD","requestId":"ORDER2"}}}}"#
        );
    }

    #[test]
    fn messages_to_assistant_match_moblin() {
        assert_eq!(decode(r#"{"ping":{}}"#), MessageToAssistant::Ping {});
        assert_eq!(
            decode(r#"{"identify":{"streamerId":"ABC","authentication":"x"}}"#),
            MessageToAssistant::Identify {
                streamer_id: Some("ABC".to_string())
            }
        );
        assert_eq!(
            decode(r#"{"identify":{"authentication":"x"}}"#),
            MessageToAssistant::Identify { streamer_id: None }
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
            r#"{"response":{"id":2,"result":{"ok":{}},"data":{"music":{"status":{"playing":true,"currentSongIndex":0,"songs":[{"id":"q1","title":"Song","artist":"Band","duration":213}],"songNotFound":false}}}}}"#,
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
                    duration: Some(213),
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
                r#"{"response":{"id":3,"result":{"ok":{}},"data":{"musicFind":{"song":{"id":"1440818839","title":"Song","artist":"Band","duration":213}}}}}"#
            ),
            Some(FoundSong {
                id: "1440818839".to_string(),
                title: "Song".to_string(),
                artist: Some("Band".to_string()),
                duration: Some(213),
            })
        );
        assert_eq!(
            found(r#"{"response":{"id":4,"result":{"ok":{}},"data":{"musicFind":{}}}}"#),
            None
        );
    }

    #[test]
    fn wheel_events_match_moblin() {
        assert_eq!(
            decode(
                r#"{"event":{"data":{"wheelOfLuck":{"wheel":{"options":[{"text":"Yes","weight":1},{"text":" No ","weight":3}]}}}}}"#
            ),
            MessageToAssistant::Event {
                data: Event::WheelOfLuck {
                    wheel: Some(Wheel {
                        options: vec![
                            WheelOption {
                                text: "Yes".to_string(),
                                weight: 1,
                            },
                            WheelOption {
                                text: " No ".to_string(),
                                weight: 3,
                            },
                        ],
                    }),
                },
            }
        );
        assert_eq!(
            decode(r#"{"event":{"data":{"wheelOfLuck":{}}}}"#),
            MessageToAssistant::Event {
                data: Event::WheelOfLuck { wheel: None },
            }
        );
        assert_eq!(
            decode(r#"{"event":{"data":{"songRequests":{"enabled":false}}}}"#),
            MessageToAssistant::Event {
                data: Event::SongRequests { enabled: false },
            }
        );
        assert!(
            serde_json::from_str::<MessageToAssistant>(
                r#"{"event":{"data":{"log":{"entry":"x"}}}}"#
            )
            .is_err()
        );
    }

    #[test]
    fn wheels_are_normalized() {
        let option = |text: &str, weight| WheelOption {
            text: text.to_string(),
            weight,
        };
        assert_eq!(
            Wheel {
                options: vec![
                    option(" Yes ", 1),
                    option("Never", 0),
                    option(&"a".repeat(101), 5000),
                ],
            }
            .normalize(),
            Some(Wheel {
                options: vec![option("Yes", 1), option(&"a".repeat(100), 1000)],
            })
        );
        assert_eq!(
            Wheel {
                options: vec![option("Never", 0)],
            }
            .normalize(),
            None
        );
        assert_eq!(
            Wheel {
                options: (0..150).map(|_| option("Yes", 1)).collect(),
            }
            .normalize()
            .unwrap()
            .options
            .len(),
            100
        );
    }

    #[test]
    fn keys_and_page_ids_are_random() {
        let streamers = Streamers::new();
        let key = streamers.new_key();
        assert_eq!(key.len(), 36);
        assert_eq!(
            key.split('-').map(str::len).collect::<Vec<_>>(),
            [8, 4, 4, 4, 12]
        );
        assert_eq!(&key[14..15], "4");
        assert!("89ab".contains(&key[19..20]));
        assert!(
            key.chars()
                .all(|c| c == '-' || c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert_ne!(streamers.new_key(), key);
        let id = streamers.new_page_id().unwrap();
        assert_eq!(id.len(), 16);
        assert_ne!(streamers.new_page_id().unwrap(), id);
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
