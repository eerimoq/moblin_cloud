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
use crate::traffic::{self, SocketTraffic, Totals, Traffic};

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

impl Outcome {
    fn name(&self) -> &'static str {
        match self {
            Self::Ok {} => "ok",
            Self::WrongPassword {} => "wrongPassword",
            Self::UnknownRequest {} => "unknownRequest",
            Self::NotIdentified {} => "notIdentified",
            Self::AlreadyIdentified {} => "alreadyIdentified",
            Self::Error {} => "error",
        }
    }
}

impl MessageToStreamer {
    fn kind(&self) -> String {
        match self {
            Self::Hello { .. } => "hello".to_string(),
            Self::Identified { .. } => "identified".to_string(),
            Self::Request { data, .. } => data.name().to_string(),
            Self::Pong {} => "pong".to_string(),
        }
    }
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

impl MessageToAssistant {
    fn kind(&self) -> String {
        match self {
            Self::Identify { .. } => "identify".to_string(),
            Self::Response { result, .. } => format!("response:{}", result.name()),
            Self::Event { data } => data.name().to_string(),
            Self::Preview { .. } => "preview".to_string(),
            Self::Ping {} => "ping".to_string(),
        }
    }
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

impl Event {
    fn name(&self) -> &'static str {
        match self {
            Self::WheelOfLuck { .. } => "wheelOfLuck",
            Self::Icon { .. } => "icon",
            Self::Status { .. } => "status",
            Self::State { .. } => "state",
        }
    }
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

impl Request {
    fn name(&self) -> &'static str {
        match self {
            Self::Tip { .. } => "tip",
            Self::GetSettings {} => "getSettings",
            Self::GetStatus {} => "getStatus",
            Self::StartStatus { .. } => "startStatus",
            Self::StopStatus {} => "stopStatus",
            Self::StartPreview {} => "startPreview",
            Self::StopPreview {} => "stopPreview",
            Self::Control(control) => control.name(),
        }
    }
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
    SendMessage {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    GetScenes {},
    GetWidgets {},
    GetVideoSources {},
    GetAlertsMediaGallery {},
    GetEditorOptions {},
    CreateScene {
        scene: Value,
    },
    UpdateScene {
        scene: Value,
    },
    DeleteScene {
        id: Uuid,
    },
    CreateWidget {
        widget: Value,
        #[serde(rename = "sceneIds")]
        scene_ids: Vec<Uuid>,
    },
    UpdateWidget {
        widget: Value,
    },
    DeleteWidget {
        id: Uuid,
    },
    SetScoreboardPlayers {
        players: Value,
    },
    GetVideoSourceSnapshot {
        id: Uuid,
    },
    SetMoblinCloudAlertsWidget {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<Uuid>,
    },
    SetMoblinCloudWheelOfLuckWidget {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<Uuid>,
    },
}

fn has_uuid_id(object: &Value) -> bool {
    object
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(|id| Uuid::parse_str(id).is_ok())
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
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::SetRecord { .. } => "setRecord",
            Self::SetLive { .. } => "setLive",
            Self::SetMute { .. } => "setMute",
            Self::SetStealthMode { .. } => "setStealthMode",
            Self::SetTorch { .. } => "setTorch",
            Self::SetZoom { .. } => "setZoom",
            Self::SetZoomPreset { .. } => "setZoomPreset",
            Self::SetScene { .. } => "setScene",
            Self::SetBitratePreset { .. } => "setBitratePreset",
            Self::SetMic { .. } => "setMic",
            Self::SetSrtConnectionPriority { .. } => "setSrtConnectionPriority",
            Self::SetSrtConnectionPrioritiesEnabled { .. } => "setSrtConnectionPrioritiesEnabled",
            Self::ReloadBrowserWidgets {} => "reloadBrowserWidgets",
            Self::InstantReplay {} => "instantReplay",
            Self::SetAutoSceneSwitcher { .. } => "setAutoSceneSwitcher",
            Self::SetPreviewStream { .. } => "setPreviewStream",
            Self::SetFilter { .. } => "setFilter",
            Self::StartMacro { .. } => "startMacro",
            Self::StopMacro { .. } => "stopMacro",
            Self::SetGimbalTracking { .. } => "setGimbalTracking",
            Self::SendMessage { .. } => "sendMessage",
            Self::GetScenes {} => "getScenes",
            Self::GetWidgets {} => "getWidgets",
            Self::GetVideoSources {} => "getVideoSources",
            Self::GetAlertsMediaGallery {} => "getAlertsMediaGallery",
            Self::GetEditorOptions {} => "getEditorOptions",
            Self::CreateScene { .. } => "createScene",
            Self::UpdateScene { .. } => "updateScene",
            Self::DeleteScene { .. } => "deleteScene",
            Self::CreateWidget { .. } => "createWidget",
            Self::UpdateWidget { .. } => "updateWidget",
            Self::DeleteWidget { .. } => "deleteWidget",
            Self::SetScoreboardPlayers { .. } => "setScoreboardPlayers",
            Self::GetVideoSourceSnapshot { .. } => "getVideoSourceSnapshot",
            Self::SetMoblinCloudAlertsWidget { .. } => "setMoblinCloudAlertsWidget",
            Self::SetMoblinCloudWheelOfLuckWidget { .. } => "setMoblinCloudWheelOfLuckWidget",
        }
    }

    pub fn valid(&self) -> bool {
        match self {
            Self::SendMessage { text, .. } => {
                !text.trim().is_empty() && text.chars().count() <= 300
            }
            Self::CreateScene { scene: object }
            | Self::UpdateScene { scene: object }
            | Self::CreateWidget { widget: object, .. }
            | Self::UpdateWidget { widget: object } => has_uuid_id(object),
            Self::SetScoreboardPlayers { players } => players
                .as_array()
                .is_some_and(|players| players.iter().all(has_uuid_id)),
            _ => true,
        }
    }

    pub(crate) fn answers(&self) -> Option<&'static str> {
        match self {
            Self::GetScenes {} => Some("scenes"),
            Self::GetWidgets {} => Some("widgets"),
            Self::GetVideoSources {} => Some("videoSources"),
            Self::GetAlertsMediaGallery {} => Some("gallery"),
            Self::GetEditorOptions {} => Some("options"),
            Self::GetVideoSourceSnapshot { .. } => Some("image"),
            _ => None,
        }
    }

    pub(crate) fn editor(&self) -> bool {
        matches!(
            self,
            Self::GetScenes {}
                | Self::GetWidgets {}
                | Self::GetVideoSources {}
                | Self::GetAlertsMediaGallery {}
                | Self::GetEditorOptions {}
                | Self::CreateScene { .. }
                | Self::UpdateScene { .. }
                | Self::DeleteScene { .. }
                | Self::CreateWidget { .. }
                | Self::UpdateWidget { .. }
                | Self::DeleteWidget { .. }
                | Self::SetScoreboardPlayers { .. }
                | Self::GetVideoSourceSnapshot { .. }
        )
    }

    fn wait(&self, answer: Duration) -> Duration {
        match self {
            Self::GetVideoSourceSnapshot { .. } => Duration::from_secs(60),
            _ => answer,
        }
    }

    fn changes_settings(&self) -> bool {
        matches!(
            self,
            Self::SetSrtConnectionPriority { .. }
                | Self::SetSrtConnectionPrioritiesEnabled { .. }
                | Self::CreateScene { .. }
                | Self::UpdateScene { .. }
                | Self::DeleteScene { .. }
                | Self::SetMoblinCloudAlertsWidget { .. }
                | Self::SetMoblinCloudWheelOfLuckWidget { .. }
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
        traffic: Totals,
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
    pub traffic: Option<Totals>,
}

struct Monitor {
    poll: Option<AbortHandle>,
    watchers: HashMap<String, usize>,
    status: Value,
    state: Value,
    settings: Option<Value>,
    events: broadcast::Sender<Arc<Update>>,
}

impl Default for Monitor {
    fn default() -> Self {
        Self {
            poll: None,
            watchers: HashMap::new(),
            status: empty_status(),
            state: Value::Object(Default::default()),
            settings: None,
            events: broadcast::channel(256).0,
        }
    }
}

fn merge_filters(filters: Option<&Value>, changes: &Value) -> Value {
    let mut merged = filters
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for [filter, on] in changes
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .as_chunks::<2>()
        .0
    {
        match merged
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .find(|[existing, _]| existing == filter)
        {
            Some([_, existing]) => *existing = on.clone(),
            None => merged.extend([filter.clone(), on.clone()]),
        }
    }
    Value::Array(merged)
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
        traffic: Totals,
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
            traffic,
        });
    }

    fn snapshot(&self, traffic: Option<Totals>) -> Snapshot {
        Snapshot {
            connected: traffic.is_some(),
            status: self.status.clone(),
            state: self.state.clone(),
            settings: self.settings.clone(),
            traffic,
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
    page_id: String,
    device: String,
}

impl Drop for PreviewGuard {
    fn drop(&mut self) {
        let mut sessions = self.sessions.lock().unwrap();
        let key = (self.page_id.clone(), self.device.clone());
        let Some(preview) = sessions.previews.get_mut(&key) else {
            return;
        };
        preview.watchers = preview.watchers.saturating_sub(1);
        if preview.watchers > 0 {
            return;
        }
        sessions.previews.remove(&key);
        if let Some(session) = sessions.get_mut(&self.page_id, &self.device, None)
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
    changes: Arc<Changes>,
    page_id: String,
    device: String,
    watcher: String,
}

impl Drop for WatchGuard {
    fn drop(&mut self) {
        let mut sessions = self.sessions.lock().unwrap();
        let Some(monitor) = sessions.monitor_mut(&self.page_id, &self.device) else {
            return;
        };
        if let Some(count) = monitor.watchers.get_mut(&self.watcher) {
            *count -= 1;
            if *count == 0 {
                monitor.watchers.remove(&self.watcher);
                self.changes.notify(&self.page_id, Change::Account);
            }
        }
        if !monitor.watchers.is_empty() {
            return;
        }
        if let Some(poll) = sessions
            .monitors
            .remove(&(self.page_id.clone(), self.device.clone()))
            .and_then(|monitor| monitor.poll)
        {
            poll.abort();
        }
        if let Some(session) = sessions.get_mut(&self.page_id, &self.device, None)
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
    socket_traffic: Arc<SocketTraffic>,
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
    fn get_mut(
        &mut self,
        page_id: &str,
        device: &str,
        number: Option<u64>,
    ) -> Option<&mut Session> {
        self.by_page.get_mut(page_id)?.iter_mut().find(|session| {
            session.device == device && number.is_none_or(|number| session.number == number)
        })
    }

    fn monitor_mut(&mut self, page_id: &str, device: &str) -> Option<&mut Monitor> {
        self.monitors
            .get_mut(&(page_id.to_string(), device.to_string()))
    }

    fn monitor_of(
        &mut self,
        page_id: &str,
        device: &str,
        number: u64,
    ) -> Option<(Totals, &mut Monitor)> {
        let traffic = self
            .get_mut(page_id, device, Some(number))?
            .socket_traffic
            .totals();
        Some((traffic, self.monitor_mut(page_id, device)?))
    }

    fn ended(&mut self, page_id: &str, session: &Session) {
        if !session.reconnecting
            && let Some(monitor) = self.monitor_mut(page_id, &session.device)
        {
            monitor.disconnected();
        }
    }

    fn wheel(&self, page_id: &str) -> Option<&Session> {
        let sessions = self.by_page.get(page_id)?;
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
    page_id: String,
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
    pub changes: Arc<Changes>,
}

pub struct Socket {
    inner: WebSocket,
    traffic: Arc<SocketTraffic>,
}

impl Socket {
    pub fn new(inner: WebSocket) -> Self {
        Self {
            inner,
            traffic: Arc::default(),
        }
    }

    pub fn traffic(&self) -> Arc<SocketTraffic> {
        self.traffic.clone()
    }
}

async fn send(
    socket: &mut (impl Sink<Message, Error = axum::Error> + Unpin),
    message: &MessageToStreamer,
    traffic: &Traffic,
    socket_traffic: &SocketTraffic,
) -> Result<()> {
    let text = serde_json::to_string(message)?;
    traffic.sent(&message.kind(), text.len());
    socket_traffic.sent(text.len());
    socket.send(Message::Text(text.into())).await?;
    Ok(())
}

async fn poll_status(sessions: Arc<Mutex<Sessions>>, page_id: String, device: String) {
    loop {
        sleep(Duration::from_secs(5)).await;
        let mut sessions = sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(&page_id, &device, None)
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
            changes: Arc::default(),
        }
    }

    fn random_bytes<const N: usize>(&self) -> Option<[u8; N]> {
        let mut bytes = [0; N];
        self.random.fill(&mut bytes).ok()?;
        Some(bytes)
    }

    pub fn new_moblin_cloud_key(&self) -> String {
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

    pub async fn refuse(&self, mut socket: Socket) {
        let refused = MessageToStreamer::Identified {
            result: Outcome::Error {},
        };
        if send(&mut socket.inner, &refused, &self.traffic, &socket.traffic)
            .await
            .is_ok()
        {
            let _ = socket.inner.send(Message::Close(None)).await;
        }
    }

    pub fn disconnect(&self, page_id: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        for session in sessions.by_page.remove(page_id).into_iter().flatten() {
            sessions.ended(page_id, &session);
        }
    }

    pub fn is_connected(&self, page_id: &str) -> bool {
        self.sessions.lock().unwrap().by_page.contains_key(page_id)
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

    pub fn devices(&self, page_id: &str) -> Vec<Device> {
        let sessions = self.sessions.lock().unwrap();
        let wheel = sessions
            .wheel(page_id)
            .filter(|session| session.wheel.is_some())
            .map(|session| session.number);
        sessions
            .by_page
            .get(page_id)
            .into_iter()
            .flatten()
            .map(|session| Device {
                id: session.device.clone(),
                wheel: wheel == Some(session.number),
                reconnecting: session.reconnecting,
            })
            .collect()
    }

    pub fn watchers(&self, page_id: &str) -> HashMap<String, Vec<String>> {
        let sessions = self.sessions.lock().unwrap();
        sessions
            .monitors
            .iter()
            .filter(|((monitored, _), _)| monitored == page_id)
            .map(|((_, device), monitor)| {
                (device.clone(), monitor.watchers.keys().cloned().collect())
            })
            .collect()
    }

    pub fn wheel_device(&self, page_id: &str) -> Option<String> {
        let sessions = self.sessions.lock().unwrap();
        sessions
            .wheel(page_id)
            .map(|session| session.device.clone())
    }

    pub async fn identify(&self, socket: &mut Socket) -> Result<Identity> {
        let challenge = self.random_bytes::<64>().context("no randomness")?;
        let salt = self.random_bytes::<64>().context("no randomness")?;
        let hello = MessageToStreamer::Hello {
            api_version: "0.1".to_string(),
            authentication: Authentication {
                challenge: BASE64_STANDARD.encode(challenge),
                salt: BASE64_STANDARD.encode(salt),
            },
        };
        send(&mut socket.inner, &hello, &self.traffic, &socket.traffic).await?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let message = timeout_at(deadline, socket.inner.recv())
                .await
                .context("no identify")?
                .context("closed")??;
            if let Message::Text(text) = message
                && let Some(MessageToAssistant::Identify {
                    streamer_id,
                    app_attest,
                }) = self.receive(text.as_str(), &socket.traffic)
            {
                return Ok(Identity {
                    device: streamer_id.filter(|id| (1..=100).contains(&id.len())),
                    challenge,
                    app_attest,
                });
            }
        }
    }

    pub fn connect(
        &self,
        page_id: &str,
        device: &str,
        socket_traffic: Arc<SocketTraffic>,
    ) -> Option<Connection> {
        let mut sessions = self.sessions.lock().unwrap();
        let total: usize = sessions.by_page.values().map(Vec::len).sum();
        let others = sessions
            .by_page
            .get(page_id)
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
            socket_traffic,
        };
        let page_sessions = sessions.by_page.entry(page_id.to_string()).or_default();
        match page_sessions.iter_mut().find(|old| old.device == device) {
            Some(old) => {
                session.wheel = old.wheel.take();
                *old = session;
            }
            None => page_sessions.push(session),
        }
        Some(Connection {
            page_id: page_id.to_string(),
            device: device.to_string(),
            number,
            receiver,
            replies,
        })
    }

    pub async fn serve(
        &self,
        connection: Connection,
        socket: Socket,
        on_icon: impl Fn(&'static Icon),
    ) {
        let Connection {
            page_id,
            device,
            number,
            mut receiver,
            replies,
        } = connection;
        let socket_traffic = &*socket.traffic;
        let (mut sink, mut stream) = socket.inner.split();
        let identified = MessageToStreamer::Identified {
            result: Outcome::Ok {},
        };
        let mut stopped = false;
        if send(&mut sink, &identified, &self.traffic, socket_traffic)
            .await
            .is_ok()
        {
            info!("remote control streamer {page_id} device {device} connected");
            self.changes.notify(&page_id, Change::Page);
            self.resume_watching(&page_id, &device, number);
            let writer = async {
                while let Some(message) = receiver.recv().await {
                    let sent = send(&mut sink, &message, &self.traffic, socket_traffic);
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
                            if let Some(message) = self.receive(text.as_str(), socket_traffic)
                                && let Some(reply) =
                                    self.handle(&page_id, &device, number, message, &on_icon)
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
        if !stopped && self.left(&page_id, &device, number) {
            info!("remote control streamer {page_id} device {device} reconnecting");
            sleep(self.linger).await;
        }
        self.remove(&page_id, &device, number);
        info!("remote control streamer {page_id} device {device} disconnected");
    }

    fn receive(&self, text: &str, socket_traffic: &SocketTraffic) -> Option<MessageToAssistant> {
        socket_traffic.received(text.len());
        let message = serde_json::from_str::<MessageToAssistant>(text).ok();
        let kind = message
            .as_ref()
            .map_or_else(|| traffic::unread(text), MessageToAssistant::kind);
        self.traffic.received(&kind, text.len());
        message
    }

    fn handle(
        &self,
        page_id: &str,
        device: &str,
        number: u64,
        message: MessageToAssistant,
        on_icon: &impl Fn(&'static Icon),
    ) -> Option<MessageToStreamer> {
        match message {
            MessageToAssistant::Ping {} => return Some(MessageToStreamer::Pong {}),
            MessageToAssistant::Response {
                id: request_id,
                result,
                data,
            } => {
                let mut sessions = self.sessions.lock().unwrap();
                if let Some((traffic, monitor)) = sessions.monitor_of(page_id, device, number) {
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
                            traffic,
                        );
                    }
                }
                if let Some(session) = sessions.get_mut(page_id, device, Some(number))
                    && let Some(pending) = session.pending.remove(&request_id)
                {
                    let _ = pending.send((result, data));
                }
            }
            MessageToAssistant::Event {
                data:
                    Event::Status {
                        general,
                        top_left,
                        top_right,
                    },
            } => {
                let mut sessions = self.sessions.lock().unwrap();
                if let Some((traffic, monitor)) = sessions.monitor_of(page_id, device, number) {
                    monitor.merge_status(general, top_left, top_right, true, traffic);
                }
            }
            MessageToAssistant::Event {
                data: Event::State { data },
            } => {
                let mut data = data;
                let mut sessions = self.sessions.lock().unwrap();
                if let Some((_, monitor)) = sessions.monitor_of(page_id, device, number)
                    && let Some(changes) = data.as_object_mut()
                {
                    if let Some(state) = monitor.state.as_object_mut() {
                        if let Some(filters) = changes.get_mut("filters") {
                            *filters = merge_filters(state.get("filters"), filters);
                        }
                        for (key, value) in changes.iter() {
                            state.insert(key.clone(), value.clone());
                        }
                    }
                    monitor.publish(Update::State { data });
                }
            }
            MessageToAssistant::Preview { preview } => {
                let mut sessions = self.sessions.lock().unwrap();
                if preview.len() <= 2_000_000
                    && sessions.get_mut(page_id, device, Some(number)).is_some()
                    && let Some(watched) = sessions
                        .previews
                        .get_mut(&(page_id.to_string(), device.to_string()))
                {
                    let frame: Arc<str> = preview.into();
                    watched.frame = Some(frame.clone());
                    let _ = watched.frames.send(frame);
                }
            }
            MessageToAssistant::Event {
                data: Event::WheelOfLuck { wheel },
            } => {
                let mut sessions = self.sessions.lock().unwrap();
                if let Some(session) = sessions.get_mut(page_id, device, Some(number)) {
                    session.wheel = wheel.and_then(Wheel::normalize);
                    self.changes.notify(page_id, Change::Page);
                }
            }
            MessageToAssistant::Event {
                data: Event::Icon { id },
            } => {
                if let Some(icon) = icons::of_moblin(&id) {
                    on_icon(icon);
                    self.changes.notify(page_id, Change::Page);
                }
            }
            MessageToAssistant::Identify { .. } => {}
        }
        None
    }

    fn left(&self, page_id: &str, device: &str, number: u64) -> bool {
        let mut sessions = self.sessions.lock().unwrap();
        let Some(session) = sessions.get_mut(page_id, device, Some(number)) else {
            return false;
        };
        session.reconnecting = true;
        session.status_requested = false;
        session.preview_requested = false;
        session.pending.clear();
        if let Some(monitor) = sessions.monitor_mut(page_id, device) {
            monitor.disconnected();
        }
        self.changes.notify(page_id, Change::Page);
        true
    }

    fn resume_watching(&self, page_id: &str, device: &str, number: u64) {
        let mut sessions = self.sessions.lock().unwrap();
        if sessions
            .previews
            .contains_key(&(page_id.to_string(), device.to_string()))
            && let Some(session) = sessions.get_mut(page_id, device, Some(number))
        {
            session.start_preview();
        }
        if sessions.monitor_mut(page_id, device).is_none() {
            return;
        }
        if let Some(session) = sessions.get_mut(page_id, device, Some(number)) {
            session.start_status();
        }
        if let Some(monitor) = sessions.monitor_mut(page_id, device) {
            monitor.publish(Update::Connection { connected: true });
        }
    }

    pub fn watch(&self, (page_id, device): (&str, &str), watcher: &str) -> Watch {
        let mut sessions = self.sessions.lock().unwrap();
        let traffic = match sessions.get_mut(page_id, device, None) {
            Some(session) => {
                session.start_status();
                (!session.reconnecting).then(|| session.socket_traffic.totals())
            }
            None => None,
        };
        let monitor = sessions
            .monitors
            .entry((page_id.to_string(), device.to_string()))
            .or_insert_with(|| Monitor {
                poll: Some(
                    tokio::spawn(poll_status(
                        self.sessions.clone(),
                        page_id.to_string(),
                        device.to_string(),
                    ))
                    .abort_handle(),
                ),
                ..Monitor::default()
            });
        let count = monitor.watchers.entry(watcher.to_string()).or_default();
        *count += 1;
        if *count == 1 {
            self.changes.notify(page_id, Change::Account);
        }
        Watch {
            snapshot: monitor.snapshot(traffic),
            events: monitor.events.subscribe(),
            _guard: WatchGuard {
                sessions: self.sessions.clone(),
                changes: self.changes.clone(),
                page_id: page_id.to_string(),
                device: device.to_string(),
                watcher: watcher.to_string(),
            },
        }
    }

    pub fn watch_preview(&self, (page_id, device): (&str, &str)) -> PreviewWatch {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(page_id, device, None) {
            session.start_preview();
        }
        let preview = sessions
            .previews
            .entry((page_id.to_string(), device.to_string()))
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
                page_id: page_id.to_string(),
                device: device.to_string(),
            },
        }
    }

    pub async fn control(
        &self,
        (page_id, device): (&str, &str),
        control: Control,
        answer: Duration,
    ) -> Result<Option<Value>, RequestError> {
        let changes_settings = control.changes_settings();
        let wait = control.wait(answer);
        let data = self
            .request((page_id, device), Request::Control(control), wait)
            .await?;
        let mut sessions = self.sessions.lock().unwrap();
        if changes_settings
            && sessions.monitor_mut(page_id, device).is_some()
            && let Some(session) = sessions.get_mut(page_id, device, None)
        {
            session.send(Request::GetSettings {});
        }
        Ok(data)
    }

    fn remove(&self, page_id: &str, device: &str, number: u64) {
        let mut sessions = self.sessions.lock().unwrap();
        let Some(page_sessions) = sessions.by_page.get_mut(page_id) else {
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
            sessions.by_page.remove(page_id);
        }
        sessions.ended(page_id, &session);
    }

    pub fn wheel(&self, page_id: &str) -> Option<Wheel> {
        self.sessions
            .lock()
            .unwrap()
            .wheel(page_id)
            .and_then(|session| session.wheel.clone())
    }

    pub async fn tip(
        &self,
        (page_id, device): (&str, &str),
        name: String,
        message: String,
        kind: TipKind,
        request_id: String,
    ) -> Result<(), RequestError> {
        self.request(
            (page_id, device),
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
        (page_id, device): (&str, &str),
        data: Request,
        wait: Duration,
    ) -> Result<Option<Value>, RequestError> {
        let (number, request_id, receiver) = {
            let mut sessions = self.sessions.lock().unwrap();
            let session = sessions
                .get_mut(page_id, device, None)
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
                if let Some(session) = sessions.get_mut(page_id, device, Some(number)) {
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
#[path = "tests/remote_control.rs"]
mod tests;
