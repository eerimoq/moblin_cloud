use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::extract::ws::{Message, WebSocket};
use base64::Engine;
use base64::prelude::{BASE64_STANDARD, BASE64_URL_SAFE_NO_PAD};
use log::info;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{Notify, mpsc, oneshot};
use tokio::time::{Instant, sleep, sleep_until, timeout, timeout_at};
use uuid::Uuid;

use crate::icons::{self, Icon};

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
    Icon { id: String },
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
    #[serde(rename_all = "camelCase")]
    Tip {
        name: String,
        message: String,
        kind: TipKind,
        request_id: String,
    },
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TipKind {
    Message { amount: String, currency: String },
    Effect { sound: String },
    WheelOfLuck { count: u32 },
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
    pending: HashMap<u64, oneshot::Sender<Outcome>>,
    wheel: Option<Wheel>,
    reconnecting: bool,
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
    pub wheel: bool,
    pub reconnecting: bool,
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
    linger: Duration,
    pub connected: Notify,
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
    pub fn new(linger: Duration) -> Self {
        Self {
            random: SystemRandom::new(),
            sessions: Mutex::new(Sessions::default()),
            linger,
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
            .flatten()
            .filter(|session| !session.reconnecting)
            .count()
    }

    pub fn devices(&self, page: &str) -> Vec<Device> {
        let sessions = self.sessions.lock().unwrap();
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
                wheel: wheel == Some(session.number),
                reconnecting: session.reconnecting,
            })
            .collect()
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
        let mut session = Session {
            device: device.to_string(),
            number,
            sender,
            next_request_id: 0,
            pending: HashMap::new(),
            wheel: None,
            reconnecting: false,
        };
        let page_sessions = sessions.by_page.entry(page.to_string()).or_default();
        match page_sessions.iter_mut().find(|old| old.device == device) {
            Some(old) => {
                session.wheel = old.wheel.take();
                *old = session;
            }
            None => page_sessions.push(session),
        }
        Some(Connection {
            page: page.to_string(),
            device: device.to_string(),
            number,
            receiver,
        })
    }

    pub async fn serve(
        &self,
        connection: Connection,
        mut socket: WebSocket,
        on_icon: impl Fn(&'static Icon),
    ) {
        let Connection {
            page,
            device,
            number,
            mut receiver,
        } = connection;
        let identified = MessageToStreamer::Identified {
            result: Outcome::Ok {},
        };
        let mut stopped = false;
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
                                if self.handle(&page, &device, number, text.as_str(), &mut socket, &on_icon).await.is_err() {
                                    break;
                                }
                            }
                            Message::Close(frame) => {
                                stopped = frame.is_some_and(|frame| frame.code == 1000);
                                break;
                            }
                            _ => {}
                        }
                    }
                    _ = sleep_until(deadline) => break,
                }
            }
        }
        drop(receiver);
        if !stopped && self.left(&page, &device, number) {
            info!("remote control streamer {page} device {device} reconnecting");
            sleep(self.linger).await;
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
        on_icon: &impl Fn(&'static Icon),
    ) -> Result<()> {
        match serde_json::from_str(text) {
            Ok(MessageToAssistant::Ping {}) => send(socket, &MessageToStreamer::Pong {}).await?,
            Ok(MessageToAssistant::Response {
                id: request_id,
                result,
                ..
            }) => {
                let mut sessions = self.sessions.lock().unwrap();
                if let Some(session) = sessions.get_mut(page, device, Some(number))
                    && let Some(pending) = session.pending.remove(&request_id)
                {
                    let _ = pending.send(result);
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
                data: Event::Icon { id },
            }) => {
                if let Some(icon) = icons::of_moblin(&id) {
                    on_icon(icon);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn left(&self, page: &str, device: &str, number: u64) -> bool {
        let mut sessions = self.sessions.lock().unwrap();
        let Some(session) = sessions.get_mut(page, device, Some(number)) else {
            return false;
        };
        session.reconnecting = true;
        session.pending.clear();
        true
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

    pub async fn tip(
        &self,
        (page, device): (&str, &str),
        name: String,
        message: String,
        kind: TipKind,
        request_id: String,
    ) -> Result<(), RequestError> {
        self.request(
            (page, device),
            Request::Tip {
                name,
                message,
                kind,
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
    ) -> Result<(), RequestError> {
        let (number, request_id, receiver) = {
            let mut sessions = self.sessions.lock().unwrap();
            let session = sessions
                .get_mut(page, device, None)
                .filter(|session| !session.reconnecting)
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
        let result = match timeout(wait, receiver).await {
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
        Ok(())
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
        let tip = |id, kind| {
            encode(&MessageToStreamer::Request {
                id,
                data: Request::Tip {
                    name: "Erik".to_string(),
                    message: "Hi".to_string(),
                    kind,
                    request_id: "ORDER1".to_string(),
                },
            })
        };
        assert_eq!(
            tip(
                7,
                TipKind::Message {
                    amount: "7.00".to_string(),
                    currency: "USD".to_string(),
                }
            ),
            r#"{"request":{"id":7,"data":{"tip":{"name":"Erik","message":"Hi","kind":{"message":{"amount":"7.00","currency":"USD"}},"requestId":"ORDER1"}}}}"#
        );
        assert_eq!(
            tip(
                8,
                TipKind::Effect {
                    sound: "Bad chili fart".to_string(),
                }
            ),
            r#"{"request":{"id":8,"data":{"tip":{"name":"Erik","message":"Hi","kind":{"effect":{"sound":"Bad chili fart"}},"requestId":"ORDER1"}}}}"#
        );
        assert_eq!(
            tip(9, TipKind::WheelOfLuck { count: 3 }),
            r#"{"request":{"id":9,"data":{"tip":{"name":"Erik","message":"Hi","kind":{"wheelOfLuck":{"count":3}},"requestId":"ORDER1"}}}}"#
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
    }

    #[test]
    fn events_match_moblin() {
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
            decode(r#"{"event":{"data":{"icon":{"id":"AppIconKing"}}}}"#),
            MessageToAssistant::Event {
                data: Event::Icon {
                    id: "AppIconKing".to_string()
                },
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
        let streamers = Streamers::new(Duration::ZERO);
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
}
