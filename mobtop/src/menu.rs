use serde_json::Value;

use crate::app::Monitor;
use crate::controls::{self, Control, FILTERS};
use crate::layout::{Direction, Leaf, Pane, Panel};
use crate::panes::format_bits;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuKind {
    Pane,
    Panels,
    Devices,
    Copy,
    Controls,
    Scenes,
    Zooms,
    Mics,
    Bitrates,
    Priorities,
    Switchers,
    Macros,
    Filters,
}

pub struct Menu {
    pub leaf: u64,
    pub device: Option<String>,
    pub stack: Vec<(MenuKind, usize)>,
    pub copied: Vec<String>,
}

impl Menu {
    pub fn new(leaf: u64, device: Option<String>, kind: MenuKind) -> Self {
        Self {
            leaf,
            device,
            stack: vec![(kind, 0)],
            copied: Vec::new(),
        }
    }

    pub fn kind(&self) -> MenuKind {
        self.stack.last().map_or(MenuKind::Pane, |(kind, _)| *kind)
    }

    pub fn selected(&self) -> usize {
        self.stack.last().map_or(0, |(_, selected)| *selected)
    }

    pub fn select(&mut self, index: usize) {
        if let Some((_, selected)) = self.stack.last_mut() {
            *selected = index;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mark {
    None,
    On,
    Off,
    Check,
    Running,
    Open,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Nothing,
    Open(MenuKind),
    Send(Control),
    Confirm {
        text: &'static str,
        yes: &'static str,
        control: Control,
    },
    Priority {
        id: String,
        priority: i64,
        enabled: bool,
    },
    SetPanel(Panel),
    SetDevice(Option<String>),
    TogglePreview,
    Maximize,
    Split(Direction),
    ChangeWeb,
    Move,
    Remove,
    Reset,
    Copy(String),
}

#[derive(Clone, Debug)]
pub struct Item {
    pub label: String,
    pub detail: Option<String>,
    pub mark: Mark,
    pub enabled: bool,
    pub action: Action,
}

impl Item {
    fn new(label: impl Into<String>, action: Action) -> Self {
        Self {
            label: label.into(),
            detail: None,
            mark: Mark::None,
            enabled: true,
            action,
        }
    }

    fn detail(mut self, detail: Option<String>) -> Self {
        self.detail = detail;
        self
    }

    fn mark(mut self, mark: Mark) -> Self {
        self.mark = mark;
        self
    }

    fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

pub struct Shown {
    pub title: String,
    pub items: Vec<Item>,
    pub empty: Option<&'static str>,
    pub footer: Option<&'static str>,
}

fn named(list: Option<&Value>) -> Vec<(&str, &str)> {
    list.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| Some((item["id"].as_str()?, item["name"].as_str()?)))
        .collect()
}

fn on_off(on: bool) -> Mark {
    if on { Mark::On } else { Mark::Off }
}

pub fn shown(monitor: &Monitor, menu: &Menu) -> Shown {
    let leaf = monitor
        .layout
        .as_ref()
        .and_then(|layout| layout.leaf(menu.leaf))
        .map(|(path, leaf)| (path, leaf.clone()));
    match menu.kind() {
        MenuKind::Pane | MenuKind::Panels | MenuKind::Devices | MenuKind::Copy => {
            let Some((path, leaf)) = leaf else {
                return Shown {
                    title: "Pane".to_string(),
                    items: Vec::new(),
                    empty: Some("Nothing here."),
                    footer: None,
                };
            };
            pane_menu(monitor, menu, &path, &leaf)
        }
        kind => controls_menu(monitor, menu, kind),
    }
}

fn pane_menu(monitor: &Monitor, menu: &Menu, path: &[usize], leaf: &Leaf) -> Shown {
    let devices = monitor
        .page
        .as_ref()
        .map_or(&[][..], |page| &page.devices[..]);
    let shown_device = monitor.device_of(leaf);
    let automatic = format!(
        "Automatic ({})",
        monitor
            .device_of(&Leaf::new(leaf.panel))
            .map_or("none", |device| device.name.as_str())
    );
    let items = match menu.kind() {
        MenuKind::Panels => Panel::ALL
            .into_iter()
            .map(|panel| {
                Item::new(panel.title(), Action::SetPanel(panel)).mark(if panel == leaf.panel {
                    Mark::Check
                } else {
                    Mark::None
                })
            })
            .collect(),
        MenuKind::Devices => std::iter::once(
            Item::new(automatic.clone(), Action::SetDevice(None)).mark(if leaf.device.is_none() {
                Mark::Check
            } else {
                Mark::None
            }),
        )
        .chain(devices.iter().map(|device| {
            Item::new(
                device.name.clone(),
                Action::SetDevice(Some(device.id.clone())),
            )
            .detail(Some(
                if device.connected {
                    "connected"
                } else if device.reconnecting {
                    "reconnecting"
                } else {
                    "not connected"
                }
                .to_string(),
            ))
            .mark(if leaf.device.as_ref() == Some(&device.id) {
                Mark::Check
            } else {
                Mark::None
            })
        }))
        .collect(),
        MenuKind::Copy => monitor
            .copy_targets()
            .into_iter()
            .map(|(id, label)| {
                let copied = menu.copied.contains(&id);
                Item::new(label, Action::Copy(id))
                    .mark(if copied { Mark::Check } else { Mark::None })
                    .enabled(!copied)
            })
            .collect(),
        _ => {
            let mut items = vec![
                Item::new("Panel", Action::Open(MenuKind::Panels))
                    .detail(Some(leaf.panel.title().to_string()))
                    .mark(Mark::Open),
            ];
            if leaf.panel.has_device() && devices.len() > 1 {
                items.push(
                    Item::new("Device", Action::Open(MenuKind::Devices))
                        .detail(Some(match &leaf.device {
                            Some(_) => shown_device
                                .map_or("none", |device| device.name.as_str())
                                .to_string(),
                            None => automatic,
                        }))
                        .mark(Mark::Open),
                );
            }
            if leaf.panel == Panel::Phone && monitor.page.as_ref().is_some_and(|page| page.preview)
            {
                items.push(Item::new(
                    if leaf.hide_preview {
                        "Show stream preview"
                    } else {
                        "Hide stream preview"
                    },
                    Action::TogglePreview,
                ));
            }
            if monitor.can_maximize() {
                items.push(Item::new(
                    if monitor.maximized == Some(leaf.id) {
                        "Restore pane"
                    } else {
                        "Maximize pane"
                    },
                    Action::Maximize,
                ));
            }
            items.push(Item::new("Split right", Action::Split(Direction::Row)));
            items.push(Item::new("Split down", Action::Split(Direction::Column)));
            if leaf.panel == Panel::Web && (leaf.url.is_some() || leaf.twitch_chat) {
                items.push(Item::new("Change web page", Action::ChangeWeb));
            }
            let several = monitor
                .layout
                .as_ref()
                .is_some_and(|layout| matches!(layout, Pane::Split(_)));
            items.push(Item::new("Move pane", Action::Move).enabled(several));
            items.push(Item::new("Remove pane", Action::Remove).enabled(!path.is_empty()));
            items.push(Item::new("Reset layout", Action::Reset));
            if !monitor.copy_targets().is_empty() {
                items.push(
                    Item::new("Copy layout to", Action::Open(MenuKind::Copy)).mark(Mark::Open),
                );
            }
            items
        }
    };
    Shown {
        title: match menu.kind() {
            MenuKind::Panels => "Panel",
            MenuKind::Devices => "Device",
            MenuKind::Copy => "Copy layout to",
            _ => "Pane",
        }
        .to_string(),
        items,
        empty: Some("Nothing here."),
        footer: None,
    }
}

fn controls_menu(monitor: &Monitor, menu: &Menu, kind: MenuKind) -> Shown {
    let device = menu.device.as_deref().unwrap_or_default();
    let feed = monitor.feeds.get(device);
    let empty = serde_json::Map::new();
    let state = feed.map_or(&empty, |feed| &feed.state);
    let settings = feed.and_then(|feed| feed.settings.as_ref());
    let can = monitor.may_control(device);
    let free = |key: &str| can && !monitor.pending.contains(&format!("{device}:{key}"));
    let flag = |key: &str| controls::flag(state, key).unwrap_or_default();
    let toggle = |label: &str, key: &str, kind: &str| {
        let on = flag(key);
        Item::new(label, Action::Send(controls::toggle(kind, !on)))
            .mark(on_off(on))
            .enabled(free(kind))
    };
    let pick = |items: Vec<(&str, String)>, current: Option<&str>, kind: &str| -> Vec<Item> {
        items
            .into_iter()
            .map(|(id, label)| {
                let selected = current == Some(id);
                Item::new(
                    label,
                    if selected {
                        Action::Nothing
                    } else if kind == "setAutoSceneSwitcher" {
                        Action::Send(controls::auto_scene_switcher(
                            (!id.is_empty()).then_some(id),
                        ))
                    } else {
                        Action::Send(controls::select(kind, id))
                    },
                )
                .mark(if selected { Mark::Check } else { Mark::None })
                .enabled(free(kind))
            })
            .collect()
    };
    let current = |key: &str| state.get(key).and_then(Value::as_str);
    let name_of = |list: Option<&Value>, id: Option<&str>| {
        named(list)
            .into_iter()
            .find(|(item, _)| Some(*item) == id)
            .map(|(_, name)| name.to_string())
    };
    let loading = settings.is_none();
    let (title, items, empty, footer) = match kind {
        MenuKind::Scenes => (
            "Scene",
            pick(
                named(settings.map(|settings| &settings["scenes"]))
                    .into_iter()
                    .map(|(id, name)| (id, name.to_string()))
                    .collect(),
                current("scene"),
                "setScene",
            ),
            None,
            None,
        ),
        MenuKind::Zooms => (
            "Zoom preset",
            pick(
                named(state.get("zoomPresets"))
                    .into_iter()
                    .map(|(id, name)| (id, name.to_string()))
                    .collect(),
                current("zoomPreset"),
                "setZoomPreset",
            ),
            None,
            None,
        ),
        MenuKind::Mics => (
            "Mic",
            pick(
                named(settings.map(|settings| &settings["mics"]))
                    .into_iter()
                    .map(|(id, name)| (id, name.to_string()))
                    .collect(),
                current("mic"),
                "setMic",
            ),
            None,
            Some("Highest priority mic at the top of the list."),
        ),
        MenuKind::Bitrates => (
            "Bitrate",
            pick(
                settings
                    .and_then(|settings| settings["bitratePresets"].as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|preset| {
                        Some((
                            preset["id"].as_str()?,
                            format_bits(preset["bitrate"].as_f64()?),
                        ))
                    })
                    .collect(),
                current("bitrate"),
                "setBitratePreset",
            ),
            None,
            None,
        ),
        MenuKind::Switchers => (
            "Auto scene switcher",
            pick(
                std::iter::once(("", "Off".to_string()))
                    .chain(
                        named(settings.map(|settings| &settings["autoSceneSwitchers"]))
                            .into_iter()
                            .map(|(id, name)| (id, name.to_string())),
                    )
                    .collect(),
                Some(
                    state
                        .get("autoSceneSwitcher")
                        .and_then(|switcher| switcher["id"].as_str())
                        .unwrap_or_default(),
                ),
                "setAutoSceneSwitcher",
            ),
            None,
            None,
        ),
        MenuKind::Macros => (
            "Macros",
            state
                .get("macros")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|item| {
                    let (id, name) = (item["id"].as_str()?, item["name"].as_str()?);
                    let running = item["running"].as_bool().unwrap_or_default();
                    let control = controls::macro_run(id, running);
                    Some(
                        Item::new(name, Action::Send(control.clone()))
                            .detail(Some(if running { "Stop" } else { "Run" }.to_string()))
                            .mark(if running { Mark::Running } else { Mark::None })
                            .enabled(free(&control.key)),
                    )
                })
                .collect(),
            Some("No macros."),
            None,
        ),
        MenuKind::Filters => (
            "Filters",
            FILTERS
                .into_iter()
                .map(|(key, label)| {
                    let on = controls::filter_on(state, key);
                    Item::new(label, Action::Send(controls::filter(key, !on)))
                        .mark(on_off(on))
                        .enabled(free("setFilter"))
                })
                .collect(),
            None,
            None,
        ),
        MenuKind::Priorities => {
            let srt = settings.map(|settings| &settings["srt"]);
            let enabled = srt
                .and_then(|srt| srt["connectionPrioritiesEnabled"].as_bool())
                .unwrap_or_default();
            let mut items = vec![
                Item::new(
                    "Enabled",
                    Action::Send(controls::priorities_enabled(!enabled)),
                )
                .mark(on_off(enabled))
                .enabled(free("setSrtConnectionPrioritiesEnabled")),
            ];
            let connections: Vec<Item> = srt
                .and_then(|srt| srt["connectionPriorities"].as_array())
                .into_iter()
                .flatten()
                .filter_map(|connection| {
                    let id = connection["id"].as_str()?.to_string();
                    let enabled = connection["enabled"].as_bool().unwrap_or_default();
                    let priority = connection["priority"].as_i64().unwrap_or(1);
                    let key = format!("setSrtConnectionPriority:{id}");
                    Some(
                        Item::new(
                            connection["name"].as_str().unwrap_or_default(),
                            Action::Priority {
                                id,
                                priority,
                                enabled,
                            },
                        )
                        .detail(Some(format!("priority {priority}  ←→")))
                        .mark(on_off(enabled))
                        .enabled(free(&key)),
                    )
                })
                .collect();
            let none = connections.is_empty();
            items.extend(connections);
            (
                "Connection priorities",
                if loading { Vec::new() } else { items },
                None,
                Some(if none {
                    "The stream has no connection priorities."
                } else {
                    "Higher priority connections are used first."
                }),
            )
        }
        _ => {
            let streaming = controls::flag(state, "streaming");
            let go_live = monitor.page.as_ref().is_some_and(|page| page.go_live);
            let live = streaming == Some(true);
            let filters_on = FILTERS
                .iter()
                .filter(|(key, _)| controls::filter_on(state, key))
                .count();
            let mut items = vec![
                Item::new(
                    if live { "End" } else { "Go Live" },
                    Action::Confirm {
                        text: if live {
                            "End the stream on this device?"
                        } else {
                            "Go live on this device?"
                        },
                        yes: if live { "End stream" } else { "Go live" },
                        control: controls::toggle("setLive", !live),
                    },
                )
                .mark(on_off(live))
                .enabled(free("setLive") && !controls::go_live_disabled(state, go_live)),
            ];
            if !named(settings.map(|settings| &settings["scenes"])).is_empty() {
                items.push(
                    Item::new("Scene", Action::Open(MenuKind::Scenes))
                        .detail(name_of(
                            settings.map(|settings| &settings["scenes"]),
                            current("scene"),
                        ))
                        .mark(Mark::Open),
                );
            }
            if !named(state.get("zoomPresets")).is_empty() {
                items.push(
                    Item::new("Zoom preset", Action::Open(MenuKind::Zooms))
                        .detail(name_of(state.get("zoomPresets"), current("zoomPreset")))
                        .mark(Mark::Open),
                );
            }
            let recording = flag("recording");
            items.extend([
                toggle("Torch", "torchOn", "setTorch"),
                toggle("Mute", "muted", "setMute"),
                Item::new("Mic", Action::Open(MenuKind::Mics))
                    .detail(name_of(
                        settings.map(|settings| &settings["mics"]),
                        current("mic"),
                    ))
                    .mark(Mark::Open),
                Item::new(
                    "Record",
                    Action::Confirm {
                        text: if recording {
                            "Stop recording on this device?"
                        } else {
                            "Start recording on this device?"
                        },
                        yes: if recording {
                            "Stop recording"
                        } else {
                            "Start recording"
                        },
                        control: controls::toggle("setRecord", !recording),
                    },
                )
                .mark(on_off(recording))
                .enabled(free("setRecord")),
                toggle("Stealth mode", "stealthMode", "setStealthMode"),
                Item::new("Bitrate", Action::Open(MenuKind::Bitrates))
                    .detail(
                        settings
                            .and_then(|settings| settings["bitratePresets"].as_array())
                            .and_then(|presets| {
                                presets
                                    .iter()
                                    .find(|preset| preset["id"].as_str() == current("bitrate"))
                            })
                            .and_then(|preset| preset["bitrate"].as_f64())
                            .map(format_bits),
                    )
                    .mark(Mark::Open),
                Item::new("Connection priorities", Action::Open(MenuKind::Priorities))
                    .mark(Mark::Open),
                Item::new(
                    "Reload browser widgets",
                    Action::Send(controls::action("reloadBrowserWidgets")),
                )
                .enabled(free("reloadBrowserWidgets")),
                Item::new("Filters", Action::Open(MenuKind::Filters))
                    .detail((filters_on > 0).then(|| format!("{filters_on} on")))
                    .mark(Mark::Open),
                Item::new(
                    "Instant replay",
                    Action::Send(controls::action("instantReplay")),
                )
                .enabled(free("instantReplay")),
                Item::new("Auto scene switcher", Action::Open(MenuKind::Switchers))
                    .detail(Some(
                        name_of(
                            settings.map(|settings| &settings["autoSceneSwitchers"]),
                            state
                                .get("autoSceneSwitcher")
                                .and_then(|switcher| switcher["id"].as_str()),
                        )
                        .unwrap_or_else(|| "Off".to_string()),
                    ))
                    .mark(Mark::Open),
                Item::new("Macros", Action::Open(MenuKind::Macros)).mark(Mark::Open),
                toggle("Gimbal tracking", "gimbalTracking", "setGimbalTracking"),
                toggle("Preview stream", "previewStream", "setPreviewStream"),
            ]);
            ("Controls", items, None, None)
        }
    };
    let needs_settings = matches!(
        kind,
        MenuKind::Scenes | MenuKind::Mics | MenuKind::Bitrates | MenuKind::Priorities
    );
    let device_name = monitor
        .page
        .as_ref()
        .and_then(|page| page.devices.iter().find(|item| item.id == device))
        .map_or(device, |device| device.name.as_str());
    Shown {
        title: if kind == MenuKind::Controls {
            format!("{title} · {device_name}")
        } else {
            title.to_string()
        },
        items,
        empty: Some(if needs_settings && loading {
            "Loading…"
        } else {
            empty.unwrap_or("Nothing here.")
        }),
        footer,
    }
}
