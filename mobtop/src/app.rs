use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use image::{DynamicImage, RgbImage};
use ratatui::layout::{Rect, Size};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::{StatefulProtocol, StatefulProtocolType};
use ratatui_image::{FontSize, ResizeEncodeRender};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

use crate::account::{self, Account, Assisting, Connect, Device, Item, Kind, Recent};
use crate::backend::{Backend, BackendError};
use crate::controls::{self, Control};
use crate::feed::Feed;
use crate::layout::{self, Direction, Divider, Leaf, Pane, Panel, Zone};
use crate::menu::{self, Action, Menu, MenuKind};
use crate::preview;
use crate::session;
use crate::sign_in::{self, Platform};
use crate::sse::{self, Delivery};

pub enum Message {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Redraw,
    Tick,
    SignInUrl(String),
    Connect(Connect),
    SignedIn(Result<String, String>),
    Account(String, Result<Box<Account>, BackendError>),
    Layout(String, Result<Value, BackendError>),
    Recent(String, Kind, Load, Result<Recent, BackendError>),
    Changed(String, Change),
    Device(String, String, String, Value),
    Frame(String, String, Box<RgbImage>),
    Encoded(String, String, Box<StatefulProtocol>, Size, u64),
    Controlled(String, Result<(), BackendError>),
    Copied(String, Result<(), BackendError>),
    Farted(Result<(), BackendError>),
    Saved(Result<(), BackendError>),
    Done(Result<String, BackendError>),
    SignedOut,
}

#[derive(Clone, Copy)]
pub enum Load {
    Refresh,
    More,
}

#[derive(Clone, Copy)]
pub enum Change {
    Account,
    Recent,
}

pub enum Screen {
    SignedOut {
        error: Option<String>,
    },
    SigningIn {
        platform: Platform,
        url: Option<String>,
    },
    Connecting {
        connect: Option<Connect>,
    },
    Loading {
        error: Option<String>,
    },
    SignedIn(Box<Monitor>),
}

pub struct Confirm {
    pub text: &'static str,
    pub yes: &'static str,
    pub device: String,
    pub control: Control,
}

pub struct Input {
    pub leaf: u64,
    pub text: String,
    pub error: Option<String>,
}

pub enum Modal {
    Menu(Menu),
    Confirm(Confirm),
    Input(Input),
}

pub struct Encoded {
    pub protocol: StatefulProtocol,
    pub size: Size,
    seq: u64,
}

pub struct Picture {
    pub latest: DynamicImage,
    pub font_size: FontSize,
    pub shown: Option<Encoded>,
    pub wanted: Option<Size>,
    seq: u64,
    spare: Option<StatefulProtocolType>,
    encoding: bool,
}

pub struct Moving {
    pub leaf: u64,
    pub target: u64,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Target {
    Leaf(u64, Zone),
    Edge(Zone),
}

pub enum Drag {
    Divider(Divider),
    Pane {
        leaf: u64,
        target: Option<Target>,
        moved: bool,
    },
}

#[derive(Default)]
pub struct Hits {
    pub panes: Rect,
    pub leaves: Vec<(u64, Rect)>,
    pub dividers: Vec<Divider>,
    pub menu: Option<Rect>,
    pub menu_items: Vec<(Rect, usize)>,
    pub rows: Vec<(u64, u16, usize)>,
}

pub struct Monitor {
    pub own: Account,
    pub page: Option<Account>,
    pub channel: Option<String>,
    pub layout: Option<Pane>,
    unsaved: Option<Instant>,
    pub focus: Option<u64>,
    pub maximized: Option<u64>,
    pub feeds: HashMap<String, Feed>,
    pub frames: HashMap<String, RgbImage>,
    pub images: RefCell<HashMap<String, Picture>>,
    pub latest: [Option<Recent>; 3],
    pub arrived: HashMap<String, Instant>,
    pub selected: HashMap<u64, usize>,
    pub drafts: HashMap<u64, String>,
    pub modal: Option<Modal>,
    pub moving: Option<Moving>,
    pub drag: Option<Drag>,
    pub pending: HashSet<String>,
    pub fart: Option<Instant>,
    pub queued: Option<Instant>,
    pub hits: RefCell<Hits>,
    pub narrow: Cell<bool>,
}

impl Monitor {
    fn new(own: Account) -> Self {
        Self {
            page: Some(own.clone()),
            own,
            channel: None,
            layout: None,
            unsaved: None,
            focus: None,
            maximized: None,
            feeds: HashMap::new(),
            frames: HashMap::new(),
            images: RefCell::default(),
            latest: Default::default(),
            arrived: HashMap::new(),
            selected: HashMap::new(),
            drafts: HashMap::new(),
            modal: None,
            moving: None,
            drag: None,
            pending: HashSet::new(),
            fart: None,
            queued: None,
            hits: RefCell::default(),
            narrow: Cell::new(false),
        }
    }

    pub fn page_id(&self) -> String {
        self.channel
            .clone()
            .unwrap_or_else(|| self.own.settings.id.clone())
    }

    pub fn base(&self) -> String {
        match &self.channel {
            Some(id) => format!("/account/pages/{id}"),
            None => "/account".to_string(),
        }
    }

    pub fn assisted(&self) -> Option<&Assisting> {
        let id = self.channel.as_ref()?;
        self.own.assisting.iter().find(|channel| &channel.id == id)
    }

    pub fn device_of(&self, leaf: &Leaf) -> Option<&Device> {
        let devices = &self.page.as_ref()?.devices;
        leaf.device
            .as_ref()
            .and_then(|id| devices.iter().find(|device| &device.id == id))
            .or_else(|| devices.iter().find(|device| device.connected))
            .or_else(|| devices.first())
    }

    pub fn may_watch(&self) -> bool {
        self.page
            .as_ref()
            .is_some_and(|page| page.remote_control != "none")
    }

    pub fn connected(&self, device: &str) -> bool {
        self.feeds.get(device).is_some_and(|feed| feed.connected)
    }

    pub fn may_control(&self, device: &str) -> bool {
        self.page
            .as_ref()
            .is_some_and(|page| page.remote_control == "control")
            && self.connected(device)
    }

    pub fn can_maximize(&self) -> bool {
        !self.narrow.get() && matches!(self.layout, Some(Pane::Split(_)))
    }

    pub fn focused(&self) -> Option<&Leaf> {
        let layout = self.layout.as_ref()?;
        let focus = self.focus?;
        layout.leaf(focus).map(|(_, leaf)| leaf)
    }

    pub fn copy_targets(&self) -> Vec<(String, String)> {
        let page = self.page_id();
        let mut targets = vec![(self.own.settings.id.clone(), "You".to_string())];
        for channel in &self.own.assisting {
            if targets.iter().all(|(id, _)| id != &channel.id) {
                targets.push((
                    channel.id.clone(),
                    if channel.platform == "kick" {
                        format!("{} on Kick", channel.name)
                    } else {
                        channel.name.clone()
                    },
                ));
            }
        }
        targets.retain(|(id, _)| *id != page);
        targets
    }

    pub fn activity_of(&self, leaf: &Leaf) -> Vec<(Kind, &Item)> {
        activity(&self.latest, kinds_of(leaf))
    }

    pub fn selected_of(&self, leaf: u64) -> usize {
        self.selected.get(&leaf).copied().unwrap_or_default()
    }

    pub fn new_arrival(&self, item: &Item) -> bool {
        self.arrived
            .get(&item.id)
            .is_some_and(|at| at.elapsed() < Duration::from_secs(8))
    }

    fn ensure_focus(&mut self) {
        let Some(layout) = &self.layout else {
            return;
        };
        let leaves = layout.leaves();
        if self
            .focus
            .is_none_or(|focus| leaves.iter().all(|(_, leaf)| leaf.id != focus))
        {
            self.focus = leaves.first().map(|(_, leaf)| leaf.id);
        }
        if self
            .maximized
            .is_some_and(|maximized| leaves.iter().all(|(_, leaf)| leaf.id != maximized))
        {
            self.maximized = None;
        }
    }

    fn leaf_areas(&self) -> Vec<(u64, Rect)> {
        let Some(layout) = &self.layout else {
            return Vec::new();
        };
        layout::areas(layout, self.hits.borrow().panes).0
    }

    fn leaf_at(&self, column: u16, row: u16) -> Option<(u64, Rect)> {
        self.hits
            .borrow()
            .leaves
            .iter()
            .copied()
            .find(|(_, rect)| rect.contains((column, row).into()))
    }

    fn drop_target(&self, from: u64, column: u16, row: u16) -> Option<Target> {
        let layout = self.layout.as_ref()?;
        let panes = self.hits.borrow().panes;
        let edge = if column == panes.x {
            Some(Zone::Left)
        } else if column + 1 == panes.right() {
            Some(Zone::Right)
        } else if row == panes.y {
            Some(Zone::Top)
        } else if row + 1 == panes.bottom() {
            Some(Zone::Bottom)
        } else {
            None
        };
        let (target, moved) = match edge {
            Some(edge) => (Target::Edge(edge), layout.move_to_edge(from, edge)),
            None => {
                let (leaf, rect) = self.leaf_at(column, row)?;
                let zone = layout::zone_at(rect, column, row);
                (Target::Leaf(leaf, zone), layout.move_to(from, leaf, zone))
            }
        };
        moved
            .filter(|moved| !moved.same_arrangement(layout))
            .map(|_| target)
    }
}

pub fn kinds_of(leaf: &Leaf) -> [bool; 3] {
    [leaf.shows("tip"), leaf.shows("spin"), leaf.shows("effect")]
}

pub fn activity(latest: &[Option<Recent>; 3], kinds: [bool; 3]) -> Vec<(Kind, &Item)> {
    let shown = || {
        Kind::ALL
            .into_iter()
            .filter(|kind| kinds[*kind as usize])
            .filter_map(|kind| latest[kind as usize].as_ref().map(|recent| (kind, recent)))
    };
    let cutoff = shown()
        .filter(|(_, recent)| recent.more)
        .filter_map(|(_, recent)| recent.items.last().map(|item| item.at))
        .max();
    let mut items: Vec<(Kind, &Item)> = shown()
        .flat_map(|(kind, recent)| recent.items.iter().map(move |item| (kind, item)))
        .filter(|(_, item)| cutoff.is_none_or(|cutoff| item.at >= cutoff))
        .collect();
    items.sort_by(|(_, a), (_, b)| b.at.cmp(&a.at).then_with(|| b.id.cmp(&a.id)));
    items
}

pub fn wanted_streams(monitor: &Monitor) -> (HashSet<String>, HashSet<String>) {
    let mut devices = HashSet::new();
    let mut previews = HashSet::new();
    let (Some(layout), true) = (&monitor.layout, monitor.may_watch()) else {
        return (devices, previews);
    };
    let preview = monitor.page.as_ref().is_some_and(|page| page.preview);
    for (_, leaf) in layout.leaves() {
        if !leaf.panel.has_device() {
            continue;
        }
        if let Some(device) = monitor.device_of(leaf) {
            devices.insert(device.id.clone());
            if leaf.panel == Panel::Phone && !leaf.hide_preview && preview {
                previews.insert(device.id.clone());
            }
        }
    }
    (devices, previews)
}

fn neighbor(rects: &[(u64, Rect)], from: u64, direction: Zone) -> Option<u64> {
    let (_, from_rect) = rects.iter().find(|(id, _)| *id == from)?;
    let center = |rect: &Rect| {
        (
            i32::from(rect.x) * 2 + i32::from(rect.width),
            i32::from(rect.y) * 2 + i32::from(rect.height),
        )
    };
    let (x, y) = center(from_rect);
    rects
        .iter()
        .filter(|(id, rect)| {
            *id != from
                && match direction {
                    Zone::Left => rect.right() <= from_rect.x,
                    Zone::Right => rect.x >= from_rect.right(),
                    Zone::Top => rect.bottom() <= from_rect.y,
                    Zone::Bottom => rect.y >= from_rect.bottom(),
                    Zone::Center => false,
                }
        })
        .min_by_key(|(_, rect)| {
            let (other_x, other_y) = center(rect);
            (x - other_x).abs() + (y - other_y).abs()
        })
        .map(|(id, _)| *id)
}

pub struct Status {
    pub text: String,
    pub error: bool,
}

pub struct App {
    pub backend: Backend,
    pub screen: Screen,
    pub status: Option<Status>,
    sender: UnboundedSender<Message>,
    sign_in_task: Option<JoinHandle<()>>,
    page_stream: Option<JoinHandle<()>>,
    device_streams: HashMap<String, JoinHandle<()>>,
    preview_streams: HashMap<String, JoinHandle<()>>,
    picker: Option<Picker>,
}

fn replace(slot: &mut Option<JoinHandle<()>>, task: Option<JoinHandle<()>>) {
    if let Some(old) = std::mem::replace(slot, task) {
        old.abort();
    }
}

impl App {
    pub fn new(
        backend_url: &str,
        sender: UnboundedSender<Message>,
        picker: Option<Picker>,
    ) -> Self {
        let mut backend = Backend::new(backend_url);
        backend.session = session::read(&backend.url);
        let mut app = Self {
            screen: Screen::SignedOut { error: None },
            status: None,
            backend,
            sender,
            sign_in_task: None,
            page_stream: None,
            device_streams: HashMap::new(),
            preview_streams: HashMap::new(),
            picker,
        };
        if app.backend.session.is_some() {
            app.load_account();
        }
        app
    }

    pub fn update(&mut self, message: Message) -> bool {
        match message {
            Message::Key(key) => return self.key(key),
            Message::Mouse(mouse) => self.mouse(mouse),
            Message::Redraw => {}
            Message::Tick => {
                if self
                    .monitor()
                    .and_then(|monitor| monitor.unsaved)
                    .is_some_and(|at| at.elapsed() >= Duration::from_secs(1))
                {
                    self.save();
                }
            }
            Message::SignInUrl(url) => {
                if let Screen::SigningIn { url: shown, .. } = &mut self.screen {
                    *shown = Some(url);
                }
            }
            Message::Connect(connect) => {
                if let Screen::Connecting { connect: shown } = &mut self.screen {
                    *shown = Some(connect);
                }
            }
            Message::SignedIn(Ok(session)) => {
                if let Err(error) = session::write(&self.backend.url, Some(&session)) {
                    self.set_status(format!("Couldn't save the session: {error}"), true);
                }
                self.backend.session = Some(session);
                self.load_account();
            }
            Message::SignedIn(Err(error)) => {
                self.screen = Screen::SignedOut { error: Some(error) };
            }
            Message::Account(base, Ok(account)) => self.account(&base, *account),
            Message::Account(_, Err(error)) => {
                if let Screen::Loading { error: shown } = &mut self.screen
                    && !error.signed_out()
                {
                    *shown = Some(error.to_string());
                } else {
                    self.failed(error);
                }
            }
            Message::Layout(page, result) => self.layout_loaded(&page, result),
            Message::Recent(page, kind, load, result) => match result {
                Ok(recent) => self.recent(&page, kind, load, recent),
                Err(error) => self.failed(error),
            },
            Message::Changed(page, change) => {
                if self
                    .monitor()
                    .is_some_and(|monitor| monitor.page_id() == page)
                {
                    match change {
                        Change::Account => self.refresh_page(),
                        Change::Recent => self.refresh_recent(),
                    }
                }
            }
            Message::Device(page, device, event, data) => {
                if let Some(monitor) = self.monitor_mut()
                    && monitor.page_id() == page
                    && let Some(feed) = monitor.feeds.get_mut(&device)
                {
                    feed.apply(&event, &data);
                }
            }
            Message::Frame(page, device, frame) => {
                if self.preview_streams.contains_key(&device)
                    && let Screen::SignedIn(monitor) = &mut self.screen
                    && monitor.page_id() == page
                {
                    if let Some(picker) = &self.picker {
                        let latest = DynamicImage::ImageRgb8((*frame).clone());
                        let mut images = monitor.images.borrow_mut();
                        let picture = images.entry(device.clone()).or_insert_with(|| Picture {
                            latest: latest.clone(),
                            font_size: picker.font_size(),
                            shown: None,
                            wanted: None,
                            seq: 0,
                            spare: None,
                            encoding: false,
                        });
                        picture.latest = latest;
                        picture.seq += 1;
                    }
                    monitor.frames.insert(device, *frame);
                }
            }
            Message::Encoded(page, device, protocol, size, seq) => {
                if let Some(monitor) = self.monitor_mut()
                    && monitor.page_id() == page
                    && let Some(picture) = monitor.images.borrow_mut().get_mut(&device)
                {
                    picture.encoding = false;
                    let old = picture.shown.replace(Encoded {
                        protocol: *protocol,
                        size,
                        seq,
                    });
                    if let Some(old) = old {
                        picture.spare = Some(old.protocol.protocol_type_owned());
                    }
                }
            }
            Message::Controlled(key, result) => {
                if let Some(monitor) = self.monitor_mut() {
                    monitor.pending.remove(&key);
                }
                if let Err(error) = result {
                    self.failed(error);
                }
            }
            Message::Copied(page, result) => match result {
                Ok(()) => {
                    if let Some(Modal::Menu(menu)) = self
                        .monitor_mut()
                        .and_then(|monitor| monitor.modal.as_mut())
                    {
                        menu.copied.push(page);
                    }
                }
                Err(error) => self.failed(error),
            },
            Message::Farted(result) => match result {
                Ok(()) => {
                    if let Some(monitor) = self.monitor_mut() {
                        monitor.queued = Some(Instant::now());
                    }
                }
                Err(error) => self.failed(error),
            },
            Message::Saved(Err(error)) => self.failed(error),
            Message::Saved(Ok(())) => {}
            Message::Done(Ok(text)) => self.set_status(text, false),
            Message::Done(Err(error)) => self.failed(error),
            Message::SignedOut => self.sign_out(Some("Your session ended. Sign in again.")),
        }
        false
    }

    fn monitor(&self) -> Option<&Monitor> {
        match &self.screen {
            Screen::SignedIn(monitor) => Some(monitor),
            _ => None,
        }
    }

    fn monitor_mut(&mut self) -> Option<&mut Monitor> {
        match &mut self.screen {
            Screen::SignedIn(monitor) => Some(monitor),
            _ => None,
        }
    }

    fn set_status(&mut self, text: String, error: bool) {
        self.status = Some(Status { text, error });
    }

    fn failed(&mut self, error: BackendError) {
        if error.signed_out() {
            self.sign_out(Some("Your session ended. Sign in again."));
        } else {
            self.set_status(error.to_string(), true);
        }
    }

    fn spawn<F>(&self, task: F) -> JoinHandle<()>
    where
        F: Future<Output = Message> + Send + 'static,
    {
        let sender = self.sender.clone();
        tokio::spawn(async move {
            let _ = sender.send(task.await);
        })
    }

    fn load_account(&mut self) {
        self.screen = Screen::Loading { error: None };
        let backend = self.backend.clone();
        self.spawn(async move {
            let account = backend.account("/account").await.map(Box::new);
            Message::Account("/account".to_string(), account)
        });
    }

    fn refresh_page(&self) {
        let Some(monitor) = self.monitor() else {
            return;
        };
        let base = monitor.base();
        let backend = self.backend.clone();
        self.spawn(async move {
            let account = backend.account(&base).await.map(Box::new);
            Message::Account(base, account)
        });
    }

    fn account(&mut self, base: &str, account: Account) {
        let Screen::SignedIn(monitor) = &mut self.screen else {
            if base == "/account" {
                self.screen = Screen::SignedIn(Box::new(Monitor::new(account)));
                self.open_page();
            }
            return;
        };
        if base == "/account" {
            monitor.own = account.clone();
            if monitor.channel.is_none() {
                monitor.page = Some(account);
            } else if monitor.assisted().is_none() {
                self.switch_channel(None);
                return;
            }
        } else if base == monitor.base() {
            monitor.page = Some(account);
        }
        self.reconcile();
    }

    fn open_page(&mut self) {
        let Some(monitor) = self.monitor() else {
            return;
        };
        let page = monitor.page_id();
        let path = format!("/account/pages/{page}/events");
        let changes = page.clone();
        let stream = tokio::spawn(sse::subscribe(
            self.backend.clone(),
            path,
            self.sender.clone(),
            move |delivery| {
                let change = |change| Message::Changed(changes.clone(), change);
                match delivery {
                    Delivery::Opened { again: true } => {
                        vec![change(Change::Account), change(Change::Recent)]
                    }
                    Delivery::Opened { again: false } => Vec::new(),
                    Delivery::Event(event) => match event.name.as_str() {
                        "account" => vec![change(Change::Account)],
                        "recent" => vec![change(Change::Recent)],
                        _ => Vec::new(),
                    },
                }
            },
        ));
        replace(&mut self.page_stream, Some(stream));
        let backend = self.backend.clone();
        self.spawn(async move {
            let layout = backend.layout(&page).await;
            Message::Layout(page, layout)
        });
        self.refresh_recent();
    }

    fn layout_loaded(&mut self, page: &str, result: Result<Value, BackendError>) {
        let layout = match result {
            Ok(value) => layout::sanitize(&value),
            Err(error) if error.signed_out() => return self.failed(error),
            Err(_) => None,
        };
        let Some(monitor) = self.monitor_mut() else {
            return;
        };
        if monitor.page_id() != page || monitor.unsaved.is_some() {
            return;
        }
        monitor.layout = Some(layout.unwrap_or_else(layout::default_layout));
        monitor.maximized = None;
        monitor.ensure_focus();
        self.reconcile();
    }

    fn reconcile(&mut self) {
        let Some(monitor) = self.monitor() else {
            return;
        };
        let page = monitor.page_id();
        let (devices, previews) = wanted_streams(monitor);
        self.device_streams.retain(|device, task| {
            let keep = devices.contains(device);
            if !keep {
                task.abort();
            }
            keep
        });
        self.preview_streams.retain(|device, task| {
            let keep = previews.contains(device);
            if !keep {
                task.abort();
            }
            keep
        });
        for device in &devices {
            if self.device_streams.contains_key(device) {
                continue;
            }
            let (stream_page, stream_device) = (page.clone(), device.clone());
            let task = tokio::spawn(sse::subscribe(
                self.backend.clone(),
                format!("/account/pages/{page}/devices/{device}/events"),
                self.sender.clone(),
                move |delivery| match delivery {
                    Delivery::Event(event) => serde_json::from_str(&event.data)
                        .map(|data| {
                            vec![Message::Device(
                                stream_page.clone(),
                                stream_device.clone(),
                                event.name,
                                data,
                            )]
                        })
                        .unwrap_or_default(),
                    Delivery::Opened { .. } => Vec::new(),
                },
            ));
            self.device_streams.insert(device.clone(), task);
        }
        for device in &previews {
            if self.preview_streams.contains_key(device) {
                continue;
            }
            let (stream_page, stream_device) = (page.clone(), device.clone());
            let task = tokio::spawn(sse::subscribe(
                self.backend.clone(),
                format!("/account/pages/{page}/devices/{device}/preview"),
                self.sender.clone(),
                move |delivery| match delivery {
                    Delivery::Event(event) if event.name == "frame" => {
                        serde_json::from_str::<Value>(&event.data)
                            .ok()
                            .and_then(|data| preview::decode(data["image"].as_str()?))
                            .map(|frame| {
                                vec![Message::Frame(
                                    stream_page.clone(),
                                    stream_device.clone(),
                                    Box::new(frame),
                                )]
                            })
                            .unwrap_or_default()
                    }
                    _ => Vec::new(),
                },
            ));
            self.preview_streams.insert(device.clone(), task);
        }
        if let Some(monitor) = self.monitor_mut() {
            monitor.feeds.retain(|device, _| devices.contains(device));
            for device in devices {
                monitor.feeds.entry(device).or_default();
            }
            monitor.frames.retain(|device, _| previews.contains(device));
            monitor
                .images
                .borrow_mut()
                .retain(|device, _| previews.contains(device));
        }
    }

    fn changed(&mut self, structural: bool) {
        if let Some(monitor) = self.monitor_mut() {
            monitor.unsaved = Some(Instant::now());
            if structural {
                monitor.maximized = None;
            }
            monitor.ensure_focus();
        }
        self.reconcile();
    }

    pub fn encode_previews(&mut self) {
        let sender = self.sender.clone();
        let (Some(picker), Screen::SignedIn(monitor)) = (&self.picker, &mut self.screen) else {
            return;
        };
        let page = monitor.page_id();
        for (device, picture) in monitor.images.borrow_mut().iter_mut() {
            let Some(size) = picture.wanted else {
                continue;
            };
            let current = picture
                .shown
                .as_ref()
                .is_some_and(|shown| shown.size == size && shown.seq == picture.seq);
            if picture.encoding || current {
                continue;
            }
            picture.encoding = true;
            let (image, seq, kind) = (picture.latest.clone(), picture.seq, picture.spare.take());
            let (picker, sender, page, device) =
                (picker.clone(), sender.clone(), page.clone(), device.clone());
            tokio::task::spawn_blocking(move || {
                let mut protocol = match kind {
                    Some(kind) => StatefulProtocol::new(image, picker.font_size(), None, kind),
                    None => picker.new_resize_protocol(image),
                };
                protocol.resize_encode(&preview::resize(), size);
                let _ = sender.send(Message::Encoded(
                    page,
                    device,
                    Box::new(protocol),
                    size,
                    seq,
                ));
            });
        }
    }

    pub fn save(&mut self) -> Option<JoinHandle<()>> {
        let monitor = self.monitor_mut()?;
        monitor.unsaved.take()?;
        let layout = serde_json::to_value(monitor.layout.as_ref()?).ok()?;
        let page = monitor.page_id();
        let backend = self.backend.clone();
        Some(self.spawn(async move { Message::Saved(backend.save_layout(&page, layout).await) }))
    }

    fn switch_channel(&mut self, channel: Option<String>) {
        self.save();
        for (_, task) in self.device_streams.drain() {
            task.abort();
        }
        for (_, task) in self.preview_streams.drain() {
            task.abort();
        }
        let Some(monitor) = self.monitor_mut() else {
            return;
        };
        monitor.page = channel.is_none().then(|| monitor.own.clone());
        monitor.channel = channel;
        monitor.layout = None;
        monitor.focus = None;
        monitor.maximized = None;
        monitor.modal = None;
        monitor.moving = None;
        monitor.feeds.clear();
        monitor.frames.clear();
        monitor.latest = Default::default();
        monitor.selected.clear();
        monitor.arrived.clear();
        let assisted = monitor.channel.is_some();
        self.open_page();
        if assisted {
            self.refresh_page();
        }
    }

    fn refresh_recent(&self) {
        let Some(monitor) = self.monitor() else {
            return;
        };
        for kind in Kind::ALL {
            let shown = monitor.latest[kind as usize]
                .as_ref()
                .map_or(0, |recent| recent.items.len());
            let (page, base) = (monitor.page_id(), monitor.base());
            let backend = self.backend.clone();
            self.spawn(async move {
                let recent = backend.recent(&base, kind, shown.clamp(5, 50), None).await;
                Message::Recent(page, kind, Load::Refresh, recent)
            });
        }
    }

    fn recent(&mut self, page: &str, kind: Kind, load: Load, recent: Recent) {
        let Some(monitor) = self.monitor_mut() else {
            return;
        };
        if monitor.page_id() != page {
            return;
        }
        let old = monitor.latest[kind as usize].take();
        let recent = match (load, old) {
            (Load::Refresh, Some(old)) => {
                monitor
                    .arrived
                    .retain(|_, at| at.elapsed() < Duration::from_secs(8));
                for item in &recent.items {
                    if !old.items.iter().any(|known| known.id == item.id) {
                        monitor.arrived.insert(item.id.clone(), Instant::now());
                    }
                }
                account::merge(recent, &old)
            }
            (Load::More, Some(mut old)) => {
                account::append(&mut old, recent);
                old
            }
            (_, None) => recent,
        };
        monitor.latest[kind as usize] = Some(recent);
        let counts: Vec<(u64, usize)> = monitor
            .layout
            .iter()
            .flat_map(|layout| layout.leaves())
            .filter(|(_, leaf)| leaf.panel == Panel::Activity)
            .map(|(_, leaf)| (leaf.id, monitor.activity_of(leaf).len()))
            .collect();
        for (leaf, count) in counts {
            if let Some(selected) = monitor.selected.get_mut(&leaf) {
                *selected = (*selected).min(count.saturating_sub(1));
            }
        }
    }

    fn sign_out(&mut self, error: Option<&str>) {
        self.save();
        for slot in [&mut self.sign_in_task, &mut self.page_stream] {
            replace(slot, None);
        }
        for (_, task) in self
            .device_streams
            .drain()
            .chain(self.preview_streams.drain())
        {
            task.abort();
        }
        self.backend.session = None;
        let _ = session::write(&self.backend.url, None);
        self.status = None;
        self.screen = Screen::SignedOut {
            error: error.map(str::to_string),
        };
    }

    fn send_control(&mut self, device: &str, control: Control) {
        let Some(monitor) = self.monitor_mut() else {
            return;
        };
        let key = format!("{device}:{}", control.key);
        if !monitor.pending.insert(key.clone()) {
            return;
        }
        let page = monitor.page_id();
        let (backend, device) = (self.backend.clone(), device.to_string());
        self.spawn(async move {
            let sent = backend.control(&page, &device, control.body).await;
            Message::Controlled(key, sent)
        });
    }

    fn fart(&mut self) {
        let Some(monitor) = self.monitor_mut() else {
            return;
        };
        monitor.fart = Some(Instant::now());
        let base = monitor.base();
        let backend = self.backend.clone();
        self.spawn(async move { Message::Farted(backend.fart(&base).await) });
    }

    fn start_sign_in(&mut self, platform: Platform) {
        self.screen = Screen::SigningIn {
            platform,
            url: None,
        };
        let sender = self.sender.clone();
        let backend = self.backend.clone();
        let task = self.spawn(async move {
            let listening = match sign_in::listen(&backend.url, platform).await {
                Ok(listening) => listening,
                Err(error) => return Message::SignedIn(Err(format!("Couldn't sign in: {error}"))),
            };
            let _ = open::that_detached(&listening.url);
            let _ = sender.send(Message::SignInUrl(listening.url));
            let verifier = listening.verifier;
            Message::SignedIn(
                sign_in::wait(listening.listener, |code| {
                    let backend = backend.clone();
                    let verifier = verifier.clone();
                    Box::pin(async move {
                        let session = backend.session(&code, &verifier).await;
                        session.map_err(|error| error.to_string())
                    })
                })
                .await,
            )
        });
        replace(&mut self.sign_in_task, Some(task));
    }

    fn start_connect(&mut self) {
        self.screen = Screen::Connecting { connect: None };
        let sender = self.sender.clone();
        let backend = self.backend.clone();
        let task = self.spawn(async move {
            let connect = match backend.connect().await {
                Ok(connect) => connect,
                Err(error) => return Message::SignedIn(Err(error.to_string())),
            };
            let interval = Duration::from_secs(connect.interval.max(1));
            let device_code = connect.device_code.clone();
            let _ = sender.send(Message::Connect(connect));
            loop {
                tokio::time::sleep(interval).await;
                match backend.connect_session(&device_code).await {
                    Ok(Some(session)) => return Message::SignedIn(Ok(session)),
                    Ok(None) => {}
                    Err(error) if error.status.is_some_and(|status| status.is_client_error()) => {
                        return Message::SignedIn(Err(error.to_string()));
                    }
                    Err(_) => {}
                }
            }
        });
        replace(&mut self.sign_in_task, Some(task));
    }

    fn key(&mut self, key: KeyEvent) -> bool {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return true;
        }
        match &mut self.screen {
            Screen::SignedOut { .. } => match key.code {
                KeyCode::Char('q') => return true,
                KeyCode::Char('t') => self.start_sign_in(Platform::Twitch),
                KeyCode::Char('k') => self.start_sign_in(Platform::Kick),
                KeyCode::Char('c') => self.start_connect(),
                _ => {}
            },
            Screen::SigningIn { .. } | Screen::Connecting { .. } => match key.code {
                KeyCode::Char('q') => return true,
                KeyCode::Esc => self.sign_out(None),
                _ => {}
            },
            Screen::Loading { .. } => match key.code {
                KeyCode::Char('q') => return true,
                KeyCode::Char('r') => self.load_account(),
                KeyCode::Char('s') => self.sign_out(None),
                _ => {}
            },
            Screen::SignedIn(_) => return self.monitor_key(key),
        }
        false
    }

    fn monitor_key(&mut self, key: KeyEvent) -> bool {
        let Some(monitor) = self.monitor_mut() else {
            return false;
        };
        if monitor.modal.is_some() {
            self.modal_key(key);
            return false;
        }
        if monitor.moving.is_some() {
            self.moving_key(key.code);
            return false;
        }
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        if let Some(leaf) = monitor
            .focused()
            .filter(|leaf| leaf.panel == Panel::Message)
            && !alt
            && !matches!(key.code, KeyCode::Tab | KeyCode::BackTab)
        {
            let leaf = leaf.clone();
            self.message_key(&leaf, key);
            return false;
        }
        match key.code {
            KeyCode::Char('q') => return true,
            KeyCode::Char('s') => self.sign_out(None),
            KeyCode::Char('c') => self.pick_channel(),
            KeyCode::Tab | KeyCode::BackTab => {
                let leaves: Vec<u64> = monitor
                    .layout
                    .iter()
                    .flat_map(|layout| layout.leaves())
                    .map(|(_, leaf)| leaf.id)
                    .collect();
                if let Some(index) = leaves.iter().position(|id| Some(*id) == monitor.focus) {
                    let step = if key.code == KeyCode::Tab {
                        1
                    } else {
                        leaves.len() - 1
                    };
                    monitor.focus = Some(leaves[(index + step) % leaves.len()]);
                    if monitor.maximized.is_some() {
                        monitor.maximized = monitor.focus;
                    }
                }
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down if alt => {
                let direction = match key.code {
                    KeyCode::Left => Zone::Left,
                    KeyCode::Right => Zone::Right,
                    KeyCode::Up => Zone::Top,
                    _ => Zone::Bottom,
                };
                if let Some(focus) = monitor.focus
                    && let Some(next) = neighbor(&monitor.leaf_areas(), focus, direction)
                {
                    monitor.focus = Some(next);
                }
            }
            KeyCode::Char(' ') => {
                if let Some(focus) = monitor.focus {
                    monitor.modal = Some(Modal::Menu(Menu::new(focus, None, MenuKind::Pane)));
                }
            }
            KeyCode::Char('z') => {
                if monitor.can_maximize() {
                    monitor.maximized = if monitor.maximized.is_some() {
                        None
                    } else {
                        monitor.focus
                    };
                }
            }
            KeyCode::Char('<') | KeyCode::Char('>') => {
                let step = if key.code == KeyCode::Char('>') {
                    0.02
                } else {
                    -0.02
                };
                if let (Some(layout), Some(focus)) = (&mut monitor.layout, monitor.focus) {
                    layout.grow(focus, step);
                    self.changed(true);
                }
            }
            KeyCode::Char('M') => {
                if let Some(focus) = monitor.focus
                    && matches!(monitor.layout, Some(Pane::Split(_)))
                {
                    monitor.moving = Some(Moving {
                        leaf: focus,
                        target: focus,
                    });
                }
            }
            code => self.pane_key(code),
        }
        false
    }

    fn pane_key(&mut self, code: KeyCode) {
        let Some(monitor) = self.monitor_mut() else {
            return;
        };
        let Some(leaf) = monitor.focused().cloned() else {
            return;
        };
        let device = monitor.device_of(&leaf).map(|device| device.id.clone());
        match (leaf.panel, code) {
            (Panel::Phone, KeyCode::Enter) => match device {
                Some(device) if monitor.may_watch() => {
                    monitor.modal = Some(Modal::Menu(Menu::new(
                        leaf.id,
                        Some(device),
                        MenuKind::Controls,
                    )));
                }
                _ => self.set_status("No Moblin to control.".to_string(), true),
            },
            (Panel::Phone, KeyCode::Char('p')) => {
                if monitor.page.as_ref().is_some_and(|page| page.preview) {
                    self.edit_leaf(leaf.id, |leaf| leaf.hide_preview = !leaf.hide_preview);
                }
            }
            (Panel::Activity, KeyCode::Up | KeyCode::Char('k')) => {
                let selected = monitor.selected_of(leaf.id).saturating_sub(1);
                monitor.selected.insert(leaf.id, selected);
            }
            (Panel::Activity, KeyCode::Down | KeyCode::Char('j')) => {
                let count = monitor.activity_of(&leaf).len();
                let selected = (monitor.selected_of(leaf.id) + 1).min(count.saturating_sub(1));
                monitor.selected.insert(leaf.id, selected);
            }
            (Panel::Activity, KeyCode::Char(digit @ '1'..='3')) => {
                let index = digit as usize - '1' as usize;
                let mut kinds = kinds_of(&leaf);
                kinds[index] = !kinds[index];
                if !kinds.contains(&true) {
                    return;
                }
                monitor.selected.insert(leaf.id, 0);
                self.edit_leaf(leaf.id, |leaf| {
                    leaf.kinds = (!kinds.iter().all(|shown| *shown)).then(|| {
                        Kind::ALL
                            .into_iter()
                            .filter(|kind| kinds[*kind as usize])
                            .map(|kind| kind.name().to_string())
                            .collect()
                    });
                });
            }
            (Panel::Activity, KeyCode::Char('r')) => {
                let Some((_, item)) = monitor
                    .activity_of(&leaf)
                    .get(monitor.selected_of(leaf.id))
                    .copied()
                else {
                    return;
                };
                let (base, order) = (monitor.base(), item.id.clone());
                let backend = self.backend.clone();
                self.set_status("Sending again…".to_string(), false);
                self.spawn(async move {
                    let sent = backend.resend(&base, &order).await;
                    Message::Done(sent.map(|()| "Sent again.".to_string()))
                });
            }
            (Panel::Activity, KeyCode::Char('m')) => {
                let kinds = kinds_of(&leaf);
                let (page, base) = (monitor.page_id(), monitor.base());
                let more: Vec<(Kind, Option<String>)> = Kind::ALL
                    .into_iter()
                    .filter(|kind| kinds[*kind as usize])
                    .filter_map(|kind| {
                        let recent = monitor.latest[kind as usize].as_ref()?;
                        recent
                            .more
                            .then(|| (kind, recent.items.last().map(|item| item.id.clone())))
                    })
                    .collect();
                for (kind, before) in more {
                    let (page, base) = (page.clone(), base.clone());
                    let backend = self.backend.clone();
                    self.spawn(async move {
                        let recent = backend.recent(&base, kind, 20, before.as_deref()).await;
                        Message::Recent(page, kind, Load::More, recent)
                    });
                }
            }
            (Panel::Toybox, KeyCode::Enter | KeyCode::Char('f')) => self.fart(),
            (Panel::Web, KeyCode::Enter) => {
                monitor.modal = Some(Modal::Input(Input {
                    leaf: leaf.id,
                    text: leaf.url.clone().unwrap_or_else(|| "https://".to_string()),
                    error: None,
                }));
            }
            (Panel::Web, KeyCode::Char('t')) => {
                if monitor
                    .page
                    .as_ref()
                    .is_some_and(|page| page.twitch.is_some())
                {
                    self.edit_leaf(leaf.id, |leaf| {
                        leaf.url = None;
                        leaf.twitch_chat = true;
                    });
                }
            }
            (Panel::Web, KeyCode::Char('o')) => {
                let address = if leaf.twitch_chat {
                    monitor
                        .page
                        .as_ref()
                        .and_then(|page| page.twitch.as_ref())
                        .map(|twitch| format!("https://www.twitch.tv/popout/{}/chat", twitch.login))
                } else {
                    leaf.url.clone()
                };
                if let Some(address) = address {
                    let _ = open::that_detached(address);
                }
            }
            _ => {}
        }
    }

    fn edit_leaf(&mut self, id: u64, edit: impl FnOnce(&mut Leaf)) {
        if let Some(leaf) = self
            .monitor_mut()
            .and_then(|monitor| monitor.layout.as_mut())
            .and_then(|layout| layout.leaf_mut(id))
        {
            edit(leaf);
            self.changed(false);
        }
    }

    fn modal_key(&mut self, key: KeyEvent) {
        let Some(monitor) = self.monitor_mut() else {
            return;
        };
        match monitor.modal.take() {
            Some(Modal::Menu(menu)) => self.menu_key(menu, key.code),
            Some(Modal::Confirm(confirm)) => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.send_control(&confirm.device, confirm.control)
                }
                KeyCode::Char('n') | KeyCode::Esc => {}
                _ => monitor.modal = Some(Modal::Confirm(confirm)),
            },
            Some(Modal::Input(mut input)) => match key.code {
                KeyCode::Esc => {}
                KeyCode::Backspace => {
                    input.text.pop();
                    monitor.modal = Some(Modal::Input(input));
                }
                KeyCode::Char(letter) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    input.text.push(letter);
                    input.error = None;
                    monitor.modal = Some(Modal::Input(input));
                }
                KeyCode::Enter => self.submit(input),
                _ => monitor.modal = Some(Modal::Input(input)),
            },
            None => {}
        }
    }

    fn submit(&mut self, mut input: Input) {
        let Some(monitor) = self.monitor_mut() else {
            return;
        };
        let text = input.text.trim().to_string();
        if url::Url::parse(&text).is_ok_and(|url| url.scheme() == "https") {
            self.edit_leaf(input.leaf, |leaf| {
                leaf.url = Some(text);
                leaf.twitch_chat = false;
            });
        } else {
            input.error = Some("Enter an https:// address.".to_string());
            monitor.modal = Some(Modal::Input(input));
        }
    }

    fn message_key(&mut self, leaf: &Leaf, key: KeyEvent) {
        let Some(monitor) = self.monitor_mut() else {
            return;
        };
        let draft = monitor.drafts.entry(leaf.id).or_default();
        match key.code {
            KeyCode::Char(letter)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && draft.chars().count() < 300 =>
            {
                draft.push(letter)
            }
            KeyCode::Backspace => {
                draft.pop();
            }
            KeyCode::Esc => draft.clear(),
            KeyCode::Enter => {
                let text = draft.trim().to_string();
                if text.is_empty() {
                    return;
                }
                let device = monitor
                    .device_of(leaf)
                    .map(|device| device.id.clone())
                    .filter(|device| monitor.may_watch() && monitor.connected(device));
                let Some(device) = device else {
                    return self.set_status(
                        "No connected Moblin to send a message to.".to_string(),
                        true,
                    );
                };
                monitor.drafts.remove(&leaf.id);
                self.send_control(&device, controls::message(&text));
            }
            _ => {}
        }
    }

    fn menu_key(&mut self, mut menu: Menu, code: KeyCode) {
        let Some(monitor) = self.monitor_mut() else {
            return;
        };
        let shown = menu::shown(monitor, &menu);
        let count = shown.items.len();
        let selected = menu.selected().min(count.saturating_sub(1));
        let item = shown.items.get(selected).cloned();
        let priorities = menu.kind() == MenuKind::Priorities;
        match code {
            KeyCode::Up | KeyCode::Char('k') => menu.select(selected.saturating_sub(1)),
            KeyCode::Down | KeyCode::Char('j') => {
                menu.select((selected + 1).min(count.saturating_sub(1)))
            }
            KeyCode::Left | KeyCode::Right if priorities => {
                if let Some(item) = item.filter(|item| item.enabled)
                    && let Action::Priority {
                        id,
                        priority,
                        enabled,
                    } = item.action
                {
                    let changed = if code == KeyCode::Right {
                        priority + 1
                    } else {
                        priority - 1
                    }
                    .clamp(1, 10);
                    if changed != priority
                        && let Some(device) = menu.device.clone()
                    {
                        self.send_control(&device, controls::priority(&id, changed, enabled));
                    }
                }
            }
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => {
                menu.stack.pop();
                if menu.stack.is_empty() {
                    return;
                }
            }
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') | KeyCode::Char(' ') => {
                if let Some(item) = item.filter(|item| item.enabled) {
                    return self.activate(menu, item.action);
                }
            }
            _ => {}
        }
        if let Some(monitor) = self.monitor_mut() {
            monitor.modal = Some(Modal::Menu(menu));
        }
    }

    fn pick_channel(&mut self) {
        let Some(monitor) = self.monitor_mut() else {
            return;
        };
        if monitor.own.assisting.is_empty() {
            return;
        }
        let current = menu::channels(monitor)
            .iter()
            .position(|(id, _, _)| *id == monitor.channel)
            .unwrap_or_default();
        let mut menu = Menu::new(monitor.focus.unwrap_or_default(), None, MenuKind::Channels);
        menu.select(current);
        monitor.modal = Some(Modal::Menu(menu));
    }

    fn activate(&mut self, mut menu: Menu, action: Action) {
        let Some(monitor) = self.monitor_mut() else {
            return;
        };
        let leaf = menu.leaf;
        let path = monitor
            .layout
            .as_ref()
            .and_then(|layout| layout.leaf(leaf))
            .map(|(path, _)| path);
        let device = menu.device.clone().unwrap_or_default();
        match action {
            Action::Nothing => monitor.modal = Some(Modal::Menu(menu)),
            Action::Open(kind) => {
                menu.stack.push((kind, 0));
                let checked = menu::shown(monitor, &menu)
                    .items
                    .iter()
                    .position(|item| item.mark == menu::Mark::Check)
                    .unwrap_or_default();
                menu.select(checked);
                monitor.modal = Some(Modal::Menu(menu));
            }
            Action::Send(control) => {
                monitor.modal = Some(Modal::Menu(menu));
                self.send_control(&device, control);
            }
            Action::Priority {
                id,
                priority,
                enabled,
            } => {
                monitor.modal = Some(Modal::Menu(menu));
                self.send_control(&device, controls::priority(&id, priority, !enabled));
            }
            Action::Confirm { text, yes, control } => {
                monitor.modal = Some(Modal::Confirm(Confirm {
                    text,
                    yes,
                    device,
                    control,
                }));
            }
            Action::SetPanel(panel) => self.edit_leaf(leaf, |leaf| {
                *leaf = Leaf {
                    id: leaf.id,
                    ..Leaf::new(panel)
                }
            }),
            Action::SetDevice(device) => self.edit_leaf(leaf, |leaf| leaf.device = device),
            Action::TogglePreview => {
                self.edit_leaf(leaf, |leaf| leaf.hide_preview = !leaf.hide_preview)
            }
            Action::ChangeWeb => self.edit_leaf(leaf, |leaf| {
                leaf.url = None;
                leaf.twitch_chat = false;
            }),
            Action::Maximize => {
                monitor.maximized = if monitor.maximized == Some(leaf) {
                    None
                } else {
                    Some(leaf)
                };
            }
            Action::Split(direction) => {
                if let (Some(layout), Some(path)) = (&mut monitor.layout, path) {
                    let added = layout.added();
                    monitor.focus = Some(added.id);
                    layout.split_at(&path, direction, added, false);
                    self.changed(true);
                }
            }
            Action::Move => monitor.moving = Some(Moving { leaf, target: leaf }),
            Action::Remove => {
                if let (Some(layout), Some(path)) = (&mut monitor.layout, path) {
                    let leaves: Vec<u64> =
                        layout.leaves().iter().map(|(_, leaf)| leaf.id).collect();
                    let index = leaves.iter().position(|id| *id == leaf).unwrap_or_default();
                    layout.remove_at(&path);
                    monitor.focus = leaves
                        .get(index.saturating_sub(1))
                        .filter(|id| **id != leaf)
                        .or_else(|| leaves.get(index + 1))
                        .copied();
                    self.changed(true);
                }
            }
            Action::Reset => {
                monitor.layout = Some(layout::default_layout());
                monitor.focus = None;
                self.changed(true);
            }
            Action::Channel(channel) => {
                if channel != monitor.channel {
                    self.switch_channel(channel);
                }
            }
            Action::Copy(page) => {
                let layout = monitor
                    .layout
                    .as_ref()
                    .and_then(|layout| serde_json::to_value(layout).ok());
                monitor.modal = Some(Modal::Menu(menu));
                if let Some(layout) = layout {
                    let backend = self.backend.clone();
                    self.spawn(async move {
                        let copied = backend.save_layout(&page, layout).await;
                        Message::Copied(page, copied)
                    });
                }
            }
        }
    }

    fn moving_key(&mut self, code: KeyCode) {
        let Some(monitor) = self.monitor_mut() else {
            return;
        };
        let Some(moving) = monitor.moving.take() else {
            return;
        };
        let direction = |code: KeyCode| match code {
            KeyCode::Left => Some(Zone::Left),
            KeyCode::Right => Some(Zone::Right),
            KeyCode::Up => Some(Zone::Top),
            KeyCode::Down => Some(Zone::Bottom),
            _ => None,
        };
        if let Some(direction) = direction(code) {
            let target =
                neighbor(&monitor.leaf_areas(), moving.target, direction).unwrap_or(moving.target);
            monitor.moving = Some(Moving { target, ..moving });
            return;
        }
        let Some(layout) = &monitor.layout else {
            return;
        };
        let moved = match code {
            KeyCode::Enter => layout.move_to(moving.leaf, moving.target, Zone::Center),
            KeyCode::Char('h') => layout.move_to(moving.leaf, moving.target, Zone::Left),
            KeyCode::Char('j') => layout.move_to(moving.leaf, moving.target, Zone::Bottom),
            KeyCode::Char('k') => layout.move_to(moving.leaf, moving.target, Zone::Top),
            KeyCode::Char('l') => layout.move_to(moving.leaf, moving.target, Zone::Right),
            KeyCode::Char('H') => layout.move_to_edge(moving.leaf, Zone::Left),
            KeyCode::Char('J') => layout.move_to_edge(moving.leaf, Zone::Bottom),
            KeyCode::Char('K') => layout.move_to_edge(moving.leaf, Zone::Top),
            KeyCode::Char('L') => layout.move_to_edge(moving.leaf, Zone::Right),
            KeyCode::Esc => return,
            _ => {
                monitor.moving = Some(moving);
                return;
            }
        };
        match moved.filter(|moved| !moved.same_arrangement(layout)) {
            Some(moved) => {
                monitor.layout = Some(moved);
                monitor.focus = Some(moving.leaf);
                self.changed(true);
            }
            None => {
                monitor.moving = Some(moving);
                self.set_status("That leaves the panes as they are.".to_string(), true);
            }
        }
    }

    fn mouse(&mut self, mouse: MouseEvent) {
        let Some(monitor) = self.monitor_mut() else {
            return;
        };
        let (column, row) = (mouse.column, mouse.row);
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(Modal::Menu(menu)) = &mut monitor.modal {
                    let hits = monitor.hits.borrow();
                    if let Some((_, index)) = hits
                        .menu_items
                        .iter()
                        .find(|(rect, _)| rect.contains((column, row).into()))
                    {
                        menu.select(*index);
                        drop(hits);
                        let Some(Modal::Menu(menu)) = monitor.modal.take() else {
                            return;
                        };
                        return self.menu_key(menu, KeyCode::Enter);
                    }
                    let outside = hits
                        .menu
                        .is_none_or(|rect| !rect.contains((column, row).into()));
                    drop(hits);
                    if outside {
                        monitor.modal = None;
                    }
                    return;
                }
                if monitor.modal.is_some() {
                    return;
                }
                if row < monitor.hits.borrow().panes.y {
                    return self.pick_channel();
                }
                monitor.moving = None;
                let divider = monitor
                    .hits
                    .borrow()
                    .dividers
                    .iter()
                    .find(|divider| {
                        let along = match divider.direction {
                            Direction::Row => column,
                            Direction::Column => row,
                        };
                        let across = match divider.direction {
                            Direction::Row => (row, divider.area.y, divider.area.bottom()),
                            Direction::Column => (column, divider.area.x, divider.area.right()),
                        };
                        (along + 1 == divider.at
                            || (along == divider.at && divider.direction == Direction::Row))
                            && across.0 > across.1
                            && across.0 + 1 < across.2
                    })
                    .cloned();
                if let Some(divider) = divider {
                    monitor.drag = Some(Drag::Divider(divider));
                    return;
                }
                let Some((leaf, rect)) = monitor.leaf_at(column, row) else {
                    return;
                };
                monitor.focus = Some(leaf);
                if row == rect.y {
                    monitor.drag = Some(Drag::Pane {
                        leaf,
                        target: None,
                        moved: false,
                    });
                }
                let picked = monitor
                    .hits
                    .borrow()
                    .rows
                    .iter()
                    .find(|(id, y, _)| *id == leaf && *y == row)
                    .map(|(_, _, index)| *index);
                if let Some(index) = picked {
                    monitor.selected.insert(leaf, index);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => match &mut monitor.drag {
                Some(Drag::Divider(divider)) => {
                    let (position, start, length) = match divider.direction {
                        Direction::Row => (column, divider.area.x, divider.area.width),
                        Direction::Column => (row, divider.area.y, divider.area.height),
                    };
                    let divider = divider.clone();
                    if let Some(Pane::Split(split)) = monitor
                        .layout
                        .as_ref()
                        .and_then(|layout| layout.at(&divider.split))
                    {
                        let before: f64 = split.sizes[..divider.index - 1].iter().sum();
                        let fraction =
                            f64::from(position.saturating_sub(start)) / f64::from(length.max(1));
                        if let Some(layout) = &mut monitor.layout {
                            layout.resize(&divider.split, divider.index, fraction - before);
                        }
                    }
                }
                Some(Drag::Pane { leaf, .. }) => {
                    let leaf = *leaf;
                    let target = monitor.drop_target(leaf, column, row);
                    monitor.drag = Some(Drag::Pane {
                        leaf,
                        target,
                        moved: true,
                    });
                }
                None => {}
            },
            MouseEventKind::Up(MouseButton::Left) => match monitor.drag.take() {
                Some(Drag::Divider(_)) => self.changed(true),
                Some(Drag::Pane {
                    leaf,
                    target: None,
                    moved: false,
                }) => {
                    monitor.modal = Some(Modal::Menu(Menu::new(leaf, None, MenuKind::Pane)));
                }
                Some(Drag::Pane {
                    leaf,
                    target: Some(target),
                    ..
                }) => {
                    let moved = monitor.layout.as_ref().and_then(|layout| match target {
                        Target::Leaf(to, zone) => layout.move_to(leaf, to, zone),
                        Target::Edge(edge) => layout.move_to_edge(leaf, edge),
                    });
                    if let Some(moved) = moved {
                        monitor.layout = Some(moved);
                        monitor.focus = Some(leaf);
                        self.changed(true);
                    }
                }
                _ => {}
            },
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                let code = if mouse.kind == MouseEventKind::ScrollDown {
                    KeyCode::Down
                } else {
                    KeyCode::Up
                };
                if let Some(Modal::Menu(_)) = &monitor.modal {
                    let Some(Modal::Menu(menu)) = monitor.modal.take() else {
                        return;
                    };
                    return self.menu_key(menu, code);
                }
                if monitor.modal.is_some() {
                    return;
                }
                if let Some((leaf, _)) = monitor.leaf_at(column, row) {
                    monitor.focus = Some(leaf);
                    if monitor
                        .focused()
                        .is_some_and(|leaf| leaf.panel == Panel::Activity)
                    {
                        self.pane_key(code);
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, at: u64) -> Item {
        serde_json::from_value(serde_json::json!({
            "id": id, "at": at, "user": "Erik", "amount": 500, "currency": "USD", "status": "delivered",
        }))
        .unwrap()
    }

    fn recent(items: Vec<Item>, more: bool) -> Option<Recent> {
        Some(Recent { items, more })
    }

    #[test]
    fn activity_merges_kinds_by_time_without_gaps() {
        let latest = [
            recent(vec![item("t3", 30), item("t1", 10)], false),
            recent(vec![item("s4", 40), item("s2", 20)], true),
            recent(vec![item("e3", 30)], false),
        ];
        let ids = |kinds| {
            activity(&latest, kinds)
                .into_iter()
                .map(|(_, item)| item.id.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids([true; 3]), ["s4", "t3", "e3", "s2"]);
        assert_eq!(ids([true, false, true]), ["t3", "e3", "t1"]);
        assert_eq!(activity(&latest, [false, true, false])[0].0, Kind::Spin);
        let mut leaf = Leaf::new(Panel::Activity);
        assert_eq!(kinds_of(&leaf), [true; 3]);
        leaf.kinds = Some(vec!["spin".to_string()]);
        assert_eq!(kinds_of(&leaf), [false, true, false]);
    }

    fn account(devices: serde_json::Value) -> Account {
        serde_json::from_value(serde_json::json!({
            "name": "Erik", "twitch": null, "kick": null, "devices": devices,
            "settings": {"id": "page"}, "remoteControl": "control", "preview": true,
        }))
        .unwrap()
    }

    #[test]
    fn streams_follow_the_panes_devices() {
        let devices = serde_json::json!([
            {"id": "a", "name": "Moblin 1", "connected": false, "reconnecting": false},
            {"id": "b", "name": "Moblin 2", "connected": true, "reconnecting": false},
        ]);
        let mut monitor = Monitor::new(account(devices));
        monitor.layout = layout::sanitize(&serde_json::json!({
            "split": "row",
            "children": [
                {"panel": "phone"},
                {"panel": "status", "device": "a"},
                {"panel": "phone", "device": "a", "hidePreview": true},
                {"panel": "activity", "device": "a"},
            ],
        }));
        let (devices, previews) = wanted_streams(&monitor);
        assert_eq!(devices, HashSet::from(["a".to_string(), "b".to_string()]));
        assert_eq!(previews, HashSet::from(["b".to_string()]));
        monitor.layout =
            layout::sanitize(&serde_json::json!({"panel": "graphs", "device": "gone"}));
        let (devices, previews) = wanted_streams(&monitor);
        assert_eq!(devices, HashSet::from(["b".to_string()]));
        assert!(previews.is_empty());
        monitor.page.as_mut().unwrap().remote_control = "none".to_string();
        assert!(wanted_streams(&monitor).0.is_empty());
        let rects = [
            (1, Rect::new(0, 0, 10, 10)),
            (2, Rect::new(10, 0, 10, 5)),
            (3, Rect::new(10, 5, 10, 5)),
        ];
        assert_eq!(neighbor(&rects, 1, Zone::Right), Some(2));
        assert_eq!(neighbor(&rects, 2, Zone::Bottom), Some(3));
        assert_eq!(neighbor(&rects, 3, Zone::Left), Some(1));
        assert_eq!(neighbor(&rects, 1, Zone::Left), None);
    }

    #[test]
    fn the_channel_picker_lines_up_its_addresses() {
        let own: Account = serde_json::from_value(serde_json::json!({
            "name": "Erik", "twitch": {"login": "eerimoq"}, "kick": null, "devices": [],
            "settings": {"id": "page"}, "remoteControl": "control", "preview": true,
            "assisting": [
                {"id": "a", "platform": "twitch", "name": "Streamer with a long name", "login": "streamer"},
                {"id": "b", "platform": "kick", "name": "Bo", "login": "bo"},
            ],
        }))
        .unwrap();
        let mut monitor = Monitor::new(own);
        monitor.modal = Some(Modal::Menu(Menu::new(0, None, MenuKind::Channels)));
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 20)).unwrap();
        terminal
            .draw(|frame| crate::panes::draw(frame, frame.area(), &monitor))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rows: Vec<String> = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect()
            })
            .collect();
        let columns: Vec<usize> = ["twitch/eerimoq", "twitch/streamer", "kick/bo"]
            .iter()
            .map(|address| {
                let row = rows.iter().find(|row| row.contains(address)).unwrap();
                row[..row.find(address).unwrap()].chars().count()
            })
            .collect();
        assert_eq!(columns[0], columns[1]);
        assert_eq!(columns[1], columns[2]);
    }
}
