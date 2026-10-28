use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use axum::extract::ws::{Message, WebSocket};
use base64::Engine;
use base64::prelude::{BASE64_STANDARD, BASE64_URL_SAFE_NO_PAD};
use futures_util::{Sink, SinkExt, StreamExt};
use log::info;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::AbortHandle;
use tokio::time::{Instant, sleep, timeout, timeout_at};
use uuid::Uuid;

use crate::app_attest::Attestation;
use crate::changes::{Change, Changes};
use crate::icons::{self, Icon};
use crate::traffic::Traffic;

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
        #[serde(default, deserialize_with = "lenient_attestation")]
        app_attest: Option<Attestation>,
    },
    Response {
        id: u64,
        result: Outcome,
        data: Option<Value>,
    },
    Event {
        data: Event,
    },
    Preview {
        preview: String,
    },
    Ping {},
}

fn lenient_attestation<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Attestation>, D::Error> {
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.map(|value| serde_json::from_value(value).unwrap_or_default()))
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Event {
    WheelOfLuck {
        wheel: Option<Wheel>,
    },
    Icon {
        id: String,
    },
    #[serde(rename_all = "camelCase")]
    Status {
        general: Option<Value>,
        top_left: Option<Value>,
        top_right: Option<Value>,
    },
    State {
        data: Value,
    },
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
    GetSettings {},
    GetStatus {},
    StartStatus {
        interval: u32,
        filter: StatusFilter,
    },
    StopStatus {},
    StartPreview {},
    StopPreview {},
    #[serde(untagged)]
    Control(Control),
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusFilter {
    top_right: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Control {
    SetRecord {
        on: bool,
    },
    SetLive {
        on: bool,
    },
    SetMute {
        on: bool,
    },
    SetStealthMode {
        on: bool,
    },
    SetTorch {
        on: bool,
    },
    SetZoom {
        x: f32,
    },
    SetZoomPreset {
        id: Uuid,
    },
    SetScene {
        id: Uuid,
    },
    SetBitratePreset {
        id: Uuid,
    },
    SetMic {
        id: String,
    },
    SetSrtConnectionPriority {
        id: Uuid,
        priority: i32,
        enabled: bool,
    },
    SetSrtConnectionPrioritiesEnabled {
        enabled: bool,
    },
    ReloadBrowserWidgets {},
    InstantReplay {},
    SetAutoSceneSwitcher {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<Uuid>,
    },
    SetPreviewStream {
        on: bool,
    },
    SetFilter {
        filter: Filter,
        on: bool,
    },
    StartMacro {
        id: Uuid,
    },
    StopMacro {
        id: Uuid,
    },
    SetGimbalTracking {
        on: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Filter {
    Pixellate {},
    Movie {},
    GrayScale {},
    Sepia {},
    Triple {},
    Twin {},
    FourThree {},
    Crt {},
    Pinch {},
    Whirlpool {},
    Poll {},
    BlurFaces {},
    Privacy {},
    Beauty {},
    MoblinInMouth {},
    CameraMan {},
}

impl Control {
    fn changes_settings(&self) -> bool {
        matches!(
            self,
            Self::SetSrtConnectionPriority { .. } | Self::SetSrtConnectionPrioritiesEnabled { .. }
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BondingSample {
    name: String,
    bytes_per_second: u64,
    rtt: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    at: u64,
    bitrate: Option<i64>,
    total_bytes: Option<i64>,
    bonding: Vec<BondingSample>,
}

impl Sample {
    fn of(top_right: &Value) -> Self {
        Self {
            at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|since| since.as_millis() as u64)
                .unwrap_or(0),
            bitrate: top_right["bitrateInfo"]["bitsPerSecond"].as_i64(),
            total_bytes: top_right["bitrateInfo"]["totalBytes"].as_i64(),
            bonding: top_right["bondingInfo"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|connection| {
                    Some(BondingSample {
                        name: connection["name"].as_str()?.to_string(),
                        bytes_per_second: connection["bytesPerSecond"].as_u64()?,
                        rtt: connection["rtt"].as_i64(),
                    })
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Update {
    #[serde(rename_all = "camelCase")]
    Status {
        general: Option<Value>,
        top_left: Option<Value>,
        top_right: Option<Value>,
        sample: Option<Sample>,
    },
    State {
        data: Value,
    },
    Settings {
        data: Value,
    },
    Connection {
        connected: bool,
    },
}

impl Update {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Status { .. } => "status",
            Self::State { .. } => "state",
            Self::Settings { .. } => "settings",
            Self::Connection { .. } => "connection",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub connected: bool,
    pub status: Value,
    pub state: Value,
    pub settings: Option<Value>,
}

struct Monitor {
    poll: Option<AbortHandle>,
    watchers: usize,
    status: Value,
    state: Value,
    settings: Option<Value>,
    events: broadcast::Sender<Arc<Update>>,
}

impl Default for Monitor {
    fn default() -> Self {
        Self {
            poll: None,
            watchers: 0,
            status: empty_status(),
            state: Value::Object(Default::default()),
            settings: None,
            events: broadcast::channel(256).0,
        }
    }
}

fn empty_status() -> Value {
    serde_json::json!({"general": {}, "topLeft": {}, "topRight": {}})
}

impl Monitor {
    fn publish(&self, update: Update) {
        let _ = self.events.send(Arc::new(update));
    }

    fn disconnected(&mut self) {
        self.status = empty_status();
        self.state = Value::Object(Default::default());
        self.publish(Update::Connection { connected: false });
    }

    fn merge_status(
        &mut self,
        general: Option<Value>,
        top_left: Option<Value>,
        top_right: Option<Value>,
        sampled: bool,
    ) {
        for (name, part) in [
            ("general", &general),
            ("topLeft", &top_left),
            ("topRight", &top_right),
        ] {
            if let Some(part) = part.as_ref().filter(|part| part.is_object()) {
                self.status[name] = part.clone();
            }
        }
        let sample = top_right
            .as_ref()
            .filter(|part| sampled && part.is_object())
            .map(Sample::of);
        self.publish(Update::Status {
            general,
            top_left,
            top_right,
            sample,
        });
    }

    fn snapshot(&self, connected: bool) -> Snapshot {
        Snapshot {
            connected,
            status: self.status.clone(),
            state: self.state.clone(),
            settings: self.settings.clone(),
        }
    }
}

struct PreviewMonitor {
    watchers: usize,
    frame: Option<Arc<str>>,
    frames: broadcast::Sender<Arc<str>>,
}

pub struct PreviewWatch {
    pub frame: Option<Arc<str>>,
    pub frames: broadcast::Receiver<Arc<str>>,
    _guard: PreviewGuard,
}

struct PreviewGuard {
    sessions: Arc<Mutex<Sessions>>,
    page: String,
    device: String,
}

impl Drop for PreviewGuard {
    fn drop(&mut self) {
        let mut sessions = self.sessions.lock().unwrap();
        let key = (self.page.clone(), self.device.clone());
        let Some(preview) = sessions.previews.get_mut(&key) else {
            return;
        };
        preview.watchers = preview.watchers.saturating_sub(1);
        if preview.watchers > 0 {
            return;
        }
        sessions.previews.remove(&key);
        if let Some(session) = sessions.get_mut(&self.page, &self.device, None)
            && session.preview_requested
        {
            session.preview_requested = false;
            session.send(Request::StopPreview {});
        }
    }
}

pub struct Watch {
    pub snapshot: Snapshot,
    pub events: broadcast::Receiver<Arc<Update>>,
    _guard: WatchGuard,
}

struct WatchGuard {
    sessions: Arc<Mutex<Sessions>>,
    page: String,
    device: String,
}

impl Drop for WatchGuard {
    fn drop(&mut self) {
        let mut sessions = self.sessions.lock().unwrap();
        let Some(monitor) = sessions.monitor_mut(&self.page, &self.device) else {
            return;
        };
        monitor.watchers = monitor.watchers.saturating_sub(1);
        if monitor.watchers > 0 {
            return;
        }
        if let Some(poll) = sessions
            .monitors
            .remove(&(self.page.clone(), self.device.clone()))
            .and_then(|monitor| monitor.poll)
        {
            poll.abort();
        }
        if let Some(session) = sessions.get_mut(&self.page, &self.device, None)
            && session.status_requested
        {
            session.status_requested = false;
            session.send(Request::StopStatus {});
        }
    }
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
    pending: HashMap<u64, oneshot::Sender<(Outcome, Option<Value>)>>,
    wheel: Option<Wheel>,
    reconnecting: bool,
    status_requested: bool,
    preview_requested: bool,
}

impl Session {
    fn send(&mut self, data: Request) -> u64 {
        self.next_request_id += 1;
        let request_id = self.next_request_id;
        let _ = self.sender.send(MessageToStreamer::Request {
            id: request_id,
            data,
        });
        request_id
    }

    fn start_status(&mut self) {
        if self.reconnecting {
            return;
        }
        if !self.status_requested {
            self.status_requested = true;
            self.send(Request::StartStatus {
                interval: 1,
                filter: StatusFilter { top_right: true },
            });
            self.send(Request::GetStatus {});
        }
        self.send(Request::GetSettings {});
    }

    fn start_preview(&mut self) {
        if !self.reconnecting && !self.preview_requested {
            self.preview_requested = true;
            self.send(Request::StartPreview {});
        }
    }
}

#[derive(Default)]
struct Sessions {
    next_number: u64,
    by_page: HashMap<String, Vec<Session>>,
    monitors: HashMap<(String, String), Monitor>,
    previews: HashMap<(String, String), PreviewMonitor>,
}

impl Sessions {
    fn get_mut(&mut self, page: &str, device: &str, number: Option<u64>) -> Option<&mut Session> {
        self.by_page.get_mut(page)?.iter_mut().find(|session| {
            session.device == device && number.is_none_or(|number| session.number == number)
        })
    }

    fn monitor_mut(&mut self, page: &str, device: &str) -> Option<&mut Monitor> {
        self.monitors
            .get_mut(&(page.to_string(), device.to_string()))
    }

    fn monitor_of(&mut self, page: &str, device: &str, number: u64) -> Option<&mut Monitor> {
        self.get_mut(page, device, Some(number))?;
        self.monitor_mut(page, device)
    }

    fn ended(&mut self, page: &str, session: &Session) {
        if !session.reconnecting
            && let Some(monitor) = self.monitor_mut(page, &session.device)
        {
            monitor.disconnected();
        }
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

pub struct Identity {
    pub device: Option<String>,
    pub challenge: [u8; 64],
    pub app_attest: Option<Attestation>,
}

pub struct Connection {
    page: String,
    device: String,
    number: u64,
    receiver: mpsc::UnboundedReceiver<MessageToStreamer>,
    replies: mpsc::WeakUnboundedSender<MessageToStreamer>,
}

pub struct Streamers {
    random: SystemRandom,
    sessions: Arc<Mutex<Sessions>>,
    linger: Duration,
    pub traffic: Traffic,
    pub changes: Changes,
}

async fn send(
    socket: &mut (impl Sink<Message, Error = axum::Error> + Unpin),
    message: &MessageToStreamer,
    traffic: &Traffic,
) -> Result<()> {
    let message = serde_json::to_value(message)?;
    let text = message.to_string();
    traffic.sent(&message, text.len());
    socket.send(Message::Text(text.into())).await?;
    Ok(())
}

async fn poll_status(sessions: Arc<Mutex<Sessions>>, page: String, device: String) {
    loop {
        sleep(Duration::from_secs(5)).await;
        let mut sessions = sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(&page, &device, None)
            && !session.reconnecting
        {
            session.send(Request::GetStatus {});
        }
    }
}

impl Streamers {
    pub fn new(linger: Duration) -> Self {
        Self {
            random: SystemRandom::new(),
            sessions: Arc::new(Mutex::new(Sessions::default())),
            linger,
            traffic: Traffic::default(),
            changes: Changes::default(),
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

    pub async fn refuse(&self, mut socket: WebSocket) {
        let refused = MessageToStreamer::Identified {
            result: Outcome::Error {},
        };
        if send(&mut socket, &refused, &self.traffic).await.is_ok() {
            let _ = socket.send(Message::Close(None)).await;
        }
    }

    pub fn disconnect(&self, page: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        for session in sessions.by_page.remove(page).into_iter().flatten() {
            sessions.ended(page, &session);
        }
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

    pub async fn identify(&self, socket: &mut WebSocket) -> Result<Identity> {
        let challenge = self.random_bytes::<64>().context("no randomness")?;
        let salt = self.random_bytes::<64>().context("no randomness")?;
        let hello = MessageToStreamer::Hello {
            api_version: "0.1".to_string(),
            authentication: Authentication {
                challenge: BASE64_STANDARD.encode(challenge),
                salt: BASE64_STANDARD.encode(salt),
            },
        };
        send(socket, &hello, &self.traffic).await?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let message = timeout_at(deadline, socket.recv())
                .await
                .context("no identify")?
                .context("closed")??;
            if let Message::Text(text) = &message {
                self.traffic.received(text.as_str());
            }
            if let Message::Text(text) = message
                && let Ok(MessageToAssistant::Identify {
                    streamer_id,
                    app_attest,
                }) = serde_json::from_str(text.as_str())
            {
                return Ok(Identity {
                    device: streamer_id.filter(|id| (1..=100).contains(&id.len())),
                    challenge,
                    app_attest,
                });
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
        let replies = sender.downgrade();
        let mut session = Session {
            device: device.to_string(),
            number,
            sender,
            next_request_id: 0,
            pending: HashMap::new(),
            wheel: None,
            reconnecting: false,
            status_requested: false,
            preview_requested: false,
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
            replies,
        })
    }

    pub async fn serve(
        &self,
        connection: Connection,
        socket: WebSocket,
        on_icon: impl Fn(&'static Icon),
    ) {
        let Connection {
            page,
            device,
            number,
            mut receiver,
            replies,
        } = connection;
        let (mut sink, mut stream) = socket.split();
        let identified = MessageToStreamer::Identified {
            result: Outcome::Ok {},
        };
        let mut stopped = false;
        if send(&mut sink, &identified, &self.traffic).await.is_ok() {
            info!("remote control streamer {page} device {device} connected");
            self.changes.notify(&page, Change::Page);
            self.resume_watching(&page, &device, number);
            let writer = async {
                while let Some(message) = receiver.recv().await {
                    let sent = send(&mut sink, &message, &self.traffic);
                    if !matches!(timeout(Duration::from_secs(10), sent).await, Ok(Ok(()))) {
                        break;
                    }
                }
            };
            let reader = async {
                while let Ok(Some(Ok(message))) =
                    timeout(Duration::from_secs(60), stream.next()).await
                {
                    match message {
                        Message::Text(text) => {
                            self.traffic.received(text.as_str());
                            if let Some(reply) =
                                self.handle(&page, &device, number, text.as_str(), &on_icon)
                                && let Some(replies) = replies.upgrade()
                            {
                                let _ = replies.send(reply);
                            }
                        }
                        Message::Close(frame) => {
                            return frame.is_some_and(|frame| frame.code == 1000);
                        }
                        _ => {}
                    }
                }
                false
            };
            tokio::select! {
                _ = writer => {}
                closed = reader => stopped = closed,
            }
        }
        drop(receiver);
        let _ = timeout(Duration::from_secs(1), sink.close()).await;
        drop(sink);
        drop(stream);
        if !stopped && self.left(&page, &device, number) {
            info!("remote control streamer {page} device {device} reconnecting");
            sleep(self.linger).await;
        }
        self.remove(&page, &device, number);
        info!("remote control streamer {page} device {device} disconnected");
    }

    fn handle(
        &self,
        page: &str,
        device: &str,
        number: u64,
        text: &str,
        on_icon: &impl Fn(&'static Icon),
    ) -> Option<MessageToStreamer> {
        match serde_json::from_str(text) {
            Ok(MessageToAssistant::Ping {}) => return Some(MessageToStreamer::Pong {}),
            Ok(MessageToAssistant::Response {
                id: request_id,
                result,
                data,
            }) => {
                let mut sessions = self.sessions.lock().unwrap();
                if let Some(monitor) = sessions.monitor_of(page, device, number) {
                    if let Some(settings) = data
                        .as_ref()
                        .and_then(|data| data.get("getSettings"))
                        .and_then(|settings| settings.get("data"))
                        .filter(|settings| settings.is_object())
                    {
                        monitor.settings = Some(settings.clone());
                        monitor.publish(Update::Settings {
                            data: settings.clone(),
                        });
                    }
                    if let Some(status) = data
                        .as_ref()
                        .and_then(|data| data.get("getStatus"))
                        .filter(|status| status.is_object())
                    {
                        monitor.merge_status(
                            status.get("general").cloned(),
                            status.get("topLeft").cloned(),
                            status.get("topRight").cloned(),
                            false,
                        );
                    }
                }
                if let Some(session) = sessions.get_mut(page, device, Some(number))
                    && let Some(pending) = session.pending.remove(&request_id)
                {
                    let _ = pending.send((result, data));
                }
            }
            Ok(MessageToAssistant::Event {
                data:
                    Event::Status {
                        general,
                        top_left,
                        top_right,
                    },
            }) => {
                let mut sessions = self.sessions.lock().unwrap();
                if let Some(monitor) = sessions.monitor_of(page, device, number) {
                    monitor.merge_status(general, top_left, top_right, true);
                }
            }
            Ok(MessageToAssistant::Event {
                data: Event::State { data },
            }) => {
                let mut sessions = self.sessions.lock().unwrap();
                if let Some(monitor) = sessions.monitor_of(page, device, number)
                    && let Some(changes) = data.as_object()
                {
                    if let Some(state) = monitor.state.as_object_mut() {
                        for (key, value) in changes {
                            state.insert(key.clone(), value.clone());
                        }
                    }
                    monitor.publish(Update::State { data });
                }
            }
            Ok(MessageToAssistant::Preview { preview }) => {
                let mut sessions = self.sessions.lock().unwrap();
                if preview.len() <= 2_000_000
                    && sessions.get_mut(page, device, Some(number)).is_some()
                    && let Some(watched) = sessions
                        .previews
                        .get_mut(&(page.to_string(), device.to_string()))
                {
                    let frame: Arc<str> = preview.into();
                    watched.frame = Some(frame.clone());
                    let _ = watched.frames.send(frame);
                }
            }
            Ok(MessageToAssistant::Event {
                data: Event::WheelOfLuck { wheel },
            }) => {
                let mut sessions = self.sessions.lock().unwrap();
                if let Some(session) = sessions.get_mut(page, device, Some(number)) {
                    session.wheel = wheel.and_then(Wheel::normalize);
                    self.changes.notify(page, Change::Page);
                }
            }
            Ok(MessageToAssistant::Event {
                data: Event::Icon { id },
            }) => {
                if let Some(icon) = icons::of_moblin(&id) {
                    on_icon(icon);
                    self.changes.notify(page, Change::Page);
                }
            }
            _ => {}
        }
        None
    }

    fn left(&self, page: &str, device: &str, number: u64) -> bool {
        let mut sessions = self.sessions.lock().unwrap();
        let Some(session) = sessions.get_mut(page, device, Some(number)) else {
            return false;
        };
        session.reconnecting = true;
        session.status_requested = false;
        session.preview_requested = false;
        session.pending.clear();
        if let Some(monitor) = sessions.monitor_mut(page, device) {
            monitor.disconnected();
        }
        self.changes.notify(page, Change::Page);
        true
    }

    fn resume_watching(&self, page: &str, device: &str, number: u64) {
        let mut sessions = self.sessions.lock().unwrap();
        if sessions
            .previews
            .contains_key(&(page.to_string(), device.to_string()))
            && let Some(session) = sessions.get_mut(page, device, Some(number))
        {
            session.start_preview();
        }
        if sessions.monitor_mut(page, device).is_none() {
            return;
        }
        if let Some(session) = sessions.get_mut(page, device, Some(number)) {
            session.start_status();
        }
        if let Some(monitor) = sessions.monitor_mut(page, device) {
            monitor.publish(Update::Connection { connected: true });
        }
    }

    pub fn watch(&self, (page, device): (&str, &str)) -> Watch {
        let mut sessions = self.sessions.lock().unwrap();
        let connected = match sessions.get_mut(page, device, None) {
            Some(session) => {
                session.start_status();
                !session.reconnecting
            }
            None => false,
        };
        let monitor = sessions
            .monitors
            .entry((page.to_string(), device.to_string()))
            .or_insert_with(|| Monitor {
                poll: Some(
                    tokio::spawn(poll_status(
                        self.sessions.clone(),
                        page.to_string(),
                        device.to_string(),
                    ))
                    .abort_handle(),
                ),
                ..Monitor::default()
            });
        monitor.watchers += 1;
        Watch {
            snapshot: monitor.snapshot(connected),
            events: monitor.events.subscribe(),
            _guard: WatchGuard {
                sessions: self.sessions.clone(),
                page: page.to_string(),
                device: device.to_string(),
            },
        }
    }

    pub fn watch_preview(&self, (page, device): (&str, &str)) -> PreviewWatch {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(page, device, None) {
            session.start_preview();
        }
        let preview = sessions
            .previews
            .entry((page.to_string(), device.to_string()))
            .or_insert_with(|| PreviewMonitor {
                watchers: 0,
                frame: None,
                frames: broadcast::channel(4).0,
            });
        preview.watchers += 1;
        PreviewWatch {
            frame: preview.frame.clone(),
            frames: preview.frames.subscribe(),
            _guard: PreviewGuard {
                sessions: self.sessions.clone(),
                page: page.to_string(),
                device: device.to_string(),
            },
        }
    }

    pub async fn control(
        &self,
        (page, device): (&str, &str),
        control: Control,
    ) -> Result<(), RequestError> {
        let changes_settings = control.changes_settings();
        self.request(
            (page, device),
            Request::Control(control),
            Duration::from_secs(5),
        )
        .await?;
        let mut sessions = self.sessions.lock().unwrap();
        if changes_settings
            && sessions.monitor_mut(page, device).is_some()
            && let Some(session) = sessions.get_mut(page, device, None)
        {
            session.send(Request::GetSettings {});
        }
        Ok(())
    }

    fn remove(&self, page: &str, device: &str, number: u64) {
        let mut sessions = self.sessions.lock().unwrap();
        let Some(page_sessions) = sessions.by_page.get_mut(page) else {
            return;
        };
        let Some(index) = page_sessions
            .iter()
            .position(|session| session.device == device && session.number == number)
        else {
            return;
        };
        let session = page_sessions.remove(index);
        if page_sessions.is_empty() {
            sessions.by_page.remove(page);
        }
        sessions.ended(page, &session);
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
    ) -> Result<Option<Value>, RequestError> {
        let (number, request_id, receiver) = {
            let mut sessions = self.sessions.lock().unwrap();
            let session = sessions
                .get_mut(page, device, None)
                .filter(|session| !session.reconnecting)
                .ok_or(RequestError::Offline)?;
            if session.sender.is_closed() {
                return Err(RequestError::Offline);
            }
            let (sender, receiver) = oneshot::channel();
            let request_id = session.send(data);
            session.pending.insert(request_id, sender);
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
        Ok(data)
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
        let request = |id, data| encode(&MessageToStreamer::Request { id, data });
        assert_eq!(
            request(10, Request::GetSettings {}),
            r#"{"request":{"id":10,"data":{"getSettings":{}}}}"#
        );
        assert_eq!(
            request(
                11,
                Request::StartStatus {
                    interval: 1,
                    filter: StatusFilter { top_right: true }
                }
            ),
            r#"{"request":{"id":11,"data":{"startStatus":{"interval":1,"filter":{"topRight":true}}}}}"#
        );
        assert_eq!(
            request(12, Request::StopStatus {}),
            r#"{"request":{"id":12,"data":{"stopStatus":{}}}}"#
        );
        assert_eq!(
            request(19, Request::GetStatus {}),
            r#"{"request":{"id":19,"data":{"getStatus":{}}}}"#
        );
        let control = |id, control| request(id, Request::Control(control));
        assert_eq!(
            control(13, Control::SetMute { on: true }),
            r#"{"request":{"id":13,"data":{"setMute":{"on":true}}}}"#
        );
        assert_eq!(
            control(14, Control::SetZoom { x: 2.5 }),
            r#"{"request":{"id":14,"data":{"setZoom":{"x":2.5}}}}"#
        );
        assert_eq!(
            control(
                15,
                Control::SetScene {
                    id: "E621E1F8-C36C-495A-93FC-0C247A3E6E5F".parse().unwrap()
                }
            ),
            r#"{"request":{"id":15,"data":{"setScene":{"id":"e621e1f8-c36c-495a-93fc-0c247a3e6e5f"}}}}"#
        );
        assert_eq!(
            control(
                17,
                Control::SetSrtConnectionPriority {
                    id: "0c4a1f9e-6b1d-4d8a-9f3e-2a7c5b8d1e60".parse().unwrap(),
                    priority: 3,
                    enabled: false
                }
            ),
            r#"{"request":{"id":17,"data":{"setSrtConnectionPriority":{"id":"0c4a1f9e-6b1d-4d8a-9f3e-2a7c5b8d1e60","priority":3,"enabled":false}}}}"#
        );
        assert_eq!(
            control(18, Control::ReloadBrowserWidgets {}),
            r#"{"request":{"id":18,"data":{"reloadBrowserWidgets":{}}}}"#
        );
        assert_eq!(
            control(
                19,
                serde_json::from_str(r#"{"setAutoSceneSwitcher":{}}"#).unwrap()
            ),
            r#"{"request":{"id":19,"data":{"setAutoSceneSwitcher":{}}}}"#
        );
        assert_eq!(
            control(
                20,
                Control::SetFilter {
                    filter: Filter::GrayScale {},
                    on: true
                }
            ),
            r#"{"request":{"id":20,"data":{"setFilter":{"filter":{"grayScale":{}},"on":true}}}}"#
        );
        assert_eq!(
            encode(&MessageToStreamer::Request {
                id: 24,
                data: Request::StartPreview {},
            }),
            r#"{"request":{"id":24,"data":{"startPreview":{}}}}"#
        );
        assert!(
            serde_json::from_str::<Control>(r#"{"setFilter":{"filter":"movie","on":true}}"#)
                .is_err()
        );
        assert!(serde_json::from_str::<Control>(r#"{"setStream":{"id":"A"}}"#).is_err());
        assert!(serde_json::from_str::<Control>(r#"{"setScene":{"id":"C2"}}"#).is_err());
        assert!(serde_json::from_str::<Control>(r#"{"setMic":{"id":"bt"}}"#).is_ok());
        assert!(serde_json::from_str::<Control>(r#"{"getStatus":{}}"#).is_err());
        assert!(serde_json::from_str::<Control>(r#"{"tip":{}}"#).is_err());
    }

    #[test]
    fn messages_to_assistant_match_moblin() {
        assert_eq!(decode(r#"{"ping":{}}"#), MessageToAssistant::Ping {});
        assert_eq!(
            decode(r#"{"preview":{"preview":"/9j/"}}"#),
            MessageToAssistant::Preview {
                preview: "/9j/".to_string()
            }
        );
        assert_eq!(
            decode(r#"{"identify":{"streamerId":"ABC","authentication":"x"}}"#),
            MessageToAssistant::Identify {
                streamer_id: Some("ABC".to_string()),
                app_attest: None,
            }
        );
        assert_eq!(
            decode(r#"{"identify":{"authentication":"x"}}"#),
            MessageToAssistant::Identify {
                streamer_id: None,
                app_attest: None,
            }
        );
        assert_eq!(
            decode(
                r#"{"identify":{"streamerId":"ABC","authentication":"x","appAttest":{"keyId":"k","attestation":"a","attestationChallenge":"c","assertion":"s"}}}"#
            ),
            MessageToAssistant::Identify {
                streamer_id: Some("ABC".to_string()),
                app_attest: Some(Attestation {
                    key_id: Some("k".to_string()),
                    attestation: Some("a".to_string()),
                    attestation_challenge: Some("c".to_string()),
                    assertion: Some("s".to_string()),
                }),
            }
        );
        for malformed in [
            r#"{"identify":{"streamerId":"ABC","authentication":"x","appAttest":{}}}"#,
            r#"{"identify":{"streamerId":"ABC","authentication":"x","appAttest":{"keyId":1}}}"#,
            r#"{"identify":{"streamerId":"ABC","authentication":"x","appAttest":"x"}}"#,
        ] {
            assert_eq!(
                decode(malformed),
                MessageToAssistant::Identify {
                    streamer_id: Some("ABC".to_string()),
                    app_attest: Some(Attestation::default()),
                }
            );
        }
        assert_eq!(
            decode(r#"{"identify":{"streamerId":"ABC","authentication":"x","appAttest":null}}"#),
            MessageToAssistant::Identify {
                streamer_id: Some("ABC".to_string()),
                app_attest: None,
            }
        );
        assert_eq!(
            decode(r#"{"response":{"id":1,"result":{"ok":{}}}}"#),
            MessageToAssistant::Response {
                id: 1,
                result: Outcome::Ok {},
                data: None,
            }
        );
        assert_eq!(
            decode(
                r#"{"response":{"id":2,"result":{"ok":{}},"data":{"getSettings":{"data":{"streams":[{"id":"A","name":"Twitch"}],"scenes":[],"bitratePresets":[],"mics":[],"srt":{"connectionPrioritiesEnabled":false,"connectionPriorities":[]}}}}}}"#
            ),
            MessageToAssistant::Response {
                id: 2,
                result: Outcome::Ok {},
                data: Some(serde_json::json!({
                    "getSettings": {
                        "data": {
                            "streams": [{"id": "A", "name": "Twitch"}],
                            "scenes": [],
                            "bitratePresets": [],
                            "mics": [],
                            "srt": {"connectionPrioritiesEnabled": false, "connectionPriorities": []},
                        }
                    }
                })),
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
        assert_eq!(
            decode(
                r#"{"event":{"data":{"status":{"topRight":{"bitrate":{"message":"5.2 Mbps (1.2 GB)","ok":true},"bitrateInfo":{"bitsPerSecond":5200000,"totalBytes":1200000000},"bondingInfo":[{"name":"Cellular","bytesPerSecond":400000,"rtt":45},{"name":"WiFi","bytesPerSecond":250000}]}}}}}"#
            ),
            MessageToAssistant::Event {
                data: Event::Status {
                    general: None,
                    top_left: None,
                    top_right: Some(serde_json::json!({
                        "bitrate": {"message": "5.2 Mbps (1.2 GB)", "ok": true},
                        "bitrateInfo": {"bitsPerSecond": 5200000, "totalBytes": 1200000000u64},
                        "bondingInfo": [
                            {"name": "Cellular", "bytesPerSecond": 400000, "rtt": 45},
                            {"name": "WiFi", "bytesPerSecond": 250000},
                        ],
                    })),
                },
            }
        );
        assert_eq!(
            decode(
                r#"{"event":{"data":{"state":{"data":{"muted":true,"autoSceneSwitcher":{}}}}}}"#
            ),
            MessageToAssistant::Event {
                data: Event::State {
                    data: serde_json::json!({"muted": true, "autoSceneSwitcher": {}}),
                },
            }
        );
        assert!(
            serde_json::from_str::<MessageToAssistant>(
                r#"{"event":{"data":{"stats":{"data":{}}}}}"#
            )
            .is_err()
        );
    }

    #[test]
    fn samples_are_taken_from_the_top_right_status() {
        let sample = Sample::of(&serde_json::json!({
            "bitrateInfo": {"bitsPerSecond": 5200000, "totalBytes": 1200000000u64},
            "bondingInfo": [
                {"name": "Cellular", "bytesPerSecond": 400000, "rtt": 45},
                {"name": "WiFi", "bytesPerSecond": 250000},
                {"bytesPerSecond": 1},
            ],
        }));
        assert_eq!(sample.bitrate, Some(5_200_000));
        assert_eq!(sample.total_bytes, Some(1_200_000_000));
        assert_eq!(
            sample.bonding,
            vec![
                BondingSample {
                    name: "Cellular".to_string(),
                    bytes_per_second: 400_000,
                    rtt: Some(45),
                },
                BondingSample {
                    name: "WiFi".to_string(),
                    bytes_per_second: 250_000,
                    rtt: None,
                },
            ]
        );
        let empty = Sample::of(&serde_json::json!({"uptime": {"message": "1:00:00"}}));
        assert_eq!(empty.bitrate, None);
        assert!(empty.bonding.is_empty());
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

    #[tokio::test]
    async fn watches_outlive_sessions() {
        let streamers = Streamers::new(Duration::ZERO);
        let offline = streamers.watch(("page", "device"));
        assert!(!offline.snapshot.connected);
        let old = streamers.connect("page", "device").unwrap();
        let mut watch = streamers.watch(("page", "device"));
        assert!(watch.snapshot.connected);
        streamers.remove("page", "device", old.number);
        assert_eq!(
            *watch.events.recv().await.unwrap(),
            Update::Connection { connected: false }
        );
        drop(offline);
        let mut new = streamers.connect("page", "device").unwrap();
        let watchers = |streamers: &Streamers| {
            streamers
                .sessions
                .lock()
                .unwrap()
                .monitor_mut("page", "device")
                .map(|monitor| monitor.watchers)
        };
        assert_eq!(watchers(&streamers), Some(1));
        assert!(new.receiver.try_recv().is_err());
        drop(watch);
        assert_eq!(watchers(&streamers), None);
        assert!(new.receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_disconnect_forgets_the_status_and_state() {
        let streamers = Streamers::new(Duration::ZERO);
        let old = streamers.connect("page", "device").unwrap();
        let watch = streamers.watch(("page", "device"));
        let on_icon = |_: &'static Icon| {};
        streamers.handle(
            "page",
            "device",
            old.number,
            r#"{"event":{"data":{"status":{"general":{"batteryLevel":50},"topRight":{}}}}}"#,
            &on_icon,
        );
        streamers.handle(
            "page",
            "device",
            old.number,
            r#"{"event":{"data":{"state":{"data":{"streaming":true}}}}}"#,
            &on_icon,
        );
        let snapshot = streamers.watch(("page", "device")).snapshot;
        assert_eq!(snapshot.status["general"]["batteryLevel"], 50);
        assert_eq!(snapshot.state["streaming"], true);
        streamers.remove("page", "device", old.number);
        let snapshot = streamers.watch(("page", "device")).snapshot;
        assert_eq!(snapshot.status, empty_status());
        assert_eq!(snapshot.state, serde_json::json!({}));
        drop(watch);
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
