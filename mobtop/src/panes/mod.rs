mod activity;
mod graphs;
mod message;
mod phone;
mod status;
mod toybox;
mod web;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Padding, Paragraph, Wrap};

pub use graphs::format_bits;

use crate::app::{Drag, Hits, Modal, Monitor, Target};
use crate::layout::{self, Leaf, Panel, Zone};
use crate::menu::{self, Mark, MenuKind};
use crate::ui::{ACCENT, MUTED};

pub fn header(monitor: &Monitor) -> Line<'_> {
    let (name, address) = match monitor.assisted() {
        Some(channel) => (
            channel.name.clone(),
            Some(format!("{}/{}", channel.platform, channel.login)),
        ),
        None => (monitor.own.name.clone(), monitor.own.address()),
    };
    let mut spans = vec![" Moblin Cloud".fg(ACCENT).bold(), "  ".into(), name.bold()];
    if let Some(address) = address {
        spans.push(format!(" ({address})").fg(MUTED));
    }
    if let Some(page) = &monitor.page {
        let connected = page
            .devices
            .iter()
            .filter(|device| device.connected)
            .count();
        spans.push("  ·  ".fg(MUTED));
        spans.push(if connected > 0 {
            format!("● {connected} of {} Moblins connected", page.devices.len()).fg(ACCENT)
        } else {
            "○ no Moblin connected".fg(MUTED)
        });
    }
    Line::from(spans)
}

pub fn typing(monitor: &Monitor) -> bool {
    matches!(monitor.modal, Some(Modal::Input(_)))
        || (monitor.modal.is_none()
            && monitor.moving.is_none()
            && monitor
                .focused()
                .is_some_and(|leaf| leaf.panel == Panel::Message))
}

pub fn hints(monitor: &Monitor) -> Vec<(&'static str, &'static str)> {
    match &monitor.modal {
        Some(Modal::Menu(menu)) => {
            let mut hints = vec![("↑↓", "select"), ("enter", "choose"), ("esc", "back")];
            if menu.kind() == MenuKind::Priorities {
                hints.push(("←→", "priority"));
            }
            return hints;
        }
        Some(Modal::Confirm(confirm)) => return vec![("y", confirm.yes), ("n", "cancel")],
        Some(Modal::Input(_)) => return vec![("enter", "show"), ("esc", "cancel")],
        None => {}
    }
    if monitor.moving.is_some() {
        return vec![
            ("arrows", "target"),
            ("enter", "swap"),
            ("hjkl", "beside it"),
            ("HJKL", "window edge"),
            ("esc", "cancel"),
        ];
    }
    if monitor
        .focused()
        .is_some_and(|leaf| leaf.panel == Panel::Message)
    {
        return vec![
            ("enter", "send"),
            ("esc", "clear"),
            ("tab", "pane"),
            ("click title", "pane menu"),
        ];
    }
    let mut hints = match monitor.focused().map(|leaf| leaf.panel) {
        Some(Panel::Phone) => vec![("enter", "controls"), ("p", "preview")],
        Some(Panel::Activity) => vec![
            ("↑↓", "select"),
            ("r", "send again"),
            ("m", "more"),
            ("1-3", "filter"),
        ],
        Some(Panel::Toybox) => vec![("enter", "fart")],
        Some(Panel::Web) => vec![("enter", "address"), ("t", "Twitch chat"), ("o", "open")],
        _ => Vec::new(),
    };
    hints.extend([("tab", "pane"), ("space", "pane menu")]);
    if monitor.can_maximize() {
        hints.push((
            "z",
            if monitor.maximized.is_some() {
                "restore"
            } else {
                "maximize"
            },
        ));
    }
    hints.extend([("<>", "resize"), ("M", "move")]);
    if !monitor.own.assisting.is_empty() {
        hints.push(("c", "channel"));
    }
    hints.push(("s", "sign out"));
    hints
}

pub fn note(frame: &mut Frame, area: Rect, text: &str) {
    frame.render_widget(
        Paragraph::new(text.to_string().fg(MUTED))
            .wrap(Wrap { trim: true })
            .block(Block::new().padding(Padding::horizontal(1))),
        area,
    );
}

pub fn problem(monitor: &Monitor, leaf: &Leaf) -> Option<String> {
    let Some(page) = &monitor.page else {
        return Some("Loading…".to_string());
    };
    if page.remote_control == "none" {
        return Some("The streamer has not let you use the remote control.".to_string());
    }
    let Some(device) = monitor.device_of(leaf) else {
        return Some(if monitor.channel.is_some() {
            "The streamer has no Moblin devices yet.".to_string()
        } else {
            "No Moblin has connected yet. Paste your Moblin Cloud key into Moblin's Moblin Cloud settings.".to_string()
        });
    };
    if !monitor.connected(&device.id) {
        return Some(if device.reconnecting {
            format!("{} is reconnecting.", device.name)
        } else {
            format!("{} is not connected.", device.name)
        });
    }
    None
}

pub fn draw(frame: &mut Frame, area: Rect, monitor: &Monitor) {
    monitor.narrow.set(area.width < 80);
    for picture in monitor.images.borrow_mut().values_mut() {
        picture.wanted = None;
    }
    *monitor.hits.borrow_mut() = Hits {
        panes: area,
        ..Hits::default()
    };
    let Some(layout) = &monitor.layout else {
        return note(frame, area, "Loading…");
    };
    let single = if monitor.narrow.get() {
        monitor.focus
    } else {
        monitor.maximized
    };
    let (leaves, dividers) = match single {
        Some(leaf) => (vec![(leaf, area)], Vec::new()),
        None => layout::areas(layout, area),
    };
    {
        let mut hits = monitor.hits.borrow_mut();
        hits.leaves = leaves.clone();
        hits.dividers = dividers;
    }
    for (id, rect) in &leaves {
        if let Some((_, leaf)) = layout.leaf(*id) {
            draw_leaf(frame, *rect, monitor, leaf);
        }
    }
    if let Some(Drag::Pane {
        target: Some(target),
        ..
    }) = &monitor.drag
    {
        let zone_rect = match *target {
            Target::Leaf(leaf, zone) => leaves
                .iter()
                .find(|(id, _)| *id == leaf)
                .map(|(_, rect)| part(*rect, zone, 0.5)),
            Target::Edge(edge) => Some(part(area, edge, 0.3)),
        };
        if let Some(zone_rect) = zone_rect {
            frame.render_widget(
                Block::bordered()
                    .border_type(BorderType::Thick)
                    .border_style(Style::new().yellow()),
                zone_rect,
            );
        }
    }
    match &monitor.modal {
        Some(Modal::Menu(menu)) => draw_menu(frame, area, monitor, menu),
        Some(Modal::Confirm(confirm)) => {
            let lines = vec![
                Line::from(confirm.text),
                Line::default(),
                Line::from(vec![
                    " y ".bold(),
                    confirm.yes.into(),
                    "     ".into(),
                    " n ".bold(),
                    "Cancel".fg(MUTED),
                ]),
            ];
            popup(frame, area, "Confirm", lines, 40);
        }
        _ => {}
    }
}

fn part(rect: Rect, zone: Zone, share: f64) -> Rect {
    let width = ((f64::from(rect.width) * share).round() as u16).max(3);
    let height = ((f64::from(rect.height) * share).round() as u16).max(3);
    match zone {
        Zone::Left => Rect { width, ..rect },
        Zone::Right => Rect {
            x: rect.right().saturating_sub(width),
            width,
            ..rect
        },
        Zone::Top => Rect { height, ..rect },
        Zone::Bottom => Rect {
            y: rect.bottom().saturating_sub(height),
            height,
            ..rect
        },
        Zone::Center => rect,
    }
}

fn draw_leaf(frame: &mut Frame, rect: Rect, monitor: &Monitor, leaf: &Leaf) {
    let targeted = monitor
        .moving
        .as_ref()
        .is_some_and(|moving| moving.target == leaf.id && moving.leaf != leaf.id);
    let border = if targeted {
        Style::new().yellow()
    } else if monitor.focus == Some(leaf.id) {
        Style::new().fg(ACCENT)
    } else {
        Style::new().fg(MUTED)
    };
    let mut title = vec![format!(" {}", leaf.panel.title()).bold()];
    let devices = monitor.page.as_ref().map_or(0, |page| page.devices.len());
    if leaf.panel.has_device()
        && devices > 1
        && let Some(device) = monitor.device_of(leaf)
    {
        title.push(
            if leaf.device.is_some() {
                format!(" · {}", device.name)
            } else {
                format!(" · Automatic ({})", device.name)
            }
            .into(),
        );
    }
    if leaf.panel == Panel::Phone
        && leaf.hide_preview
        && monitor.page.as_ref().is_some_and(|page| page.preview)
    {
        title.push(" · preview off".fg(MUTED));
    }
    if monitor
        .moving
        .as_ref()
        .is_some_and(|moving| moving.leaf == leaf.id)
    {
        title.push(" · moving".yellow());
    }
    if monitor.maximized == Some(leaf.id) {
        title.push(" · maximized".fg(MUTED));
    }
    title.push(" ".into());
    let block = Block::bordered()
        .title(Line::from(title))
        .border_style(border);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    if inner.width < 8 || inner.height < 1 {
        return;
    }
    match leaf.panel {
        Panel::Phone => phone::draw(frame, inner, monitor, leaf),
        Panel::Activity => activity::draw(frame, inner, monitor, leaf),
        Panel::Graphs | Panel::Status => {
            if let Some(problem) = problem(monitor, leaf) {
                return note(frame, inner, &problem);
            }
            let Some(feed) = monitor
                .device_of(leaf)
                .and_then(|device| monitor.feeds.get(&device.id))
            else {
                return note(frame, inner, "Loading…");
            };
            if leaf.panel == Panel::Graphs {
                graphs::draw(frame, inner, feed);
            } else {
                status::draw(frame, inner, feed);
            }
        }
        Panel::Message => message::draw(frame, inner, monitor, leaf),
        Panel::Toybox => toybox::draw(frame, inner, monitor),
        Panel::Web => web::draw(frame, inner, monitor, leaf),
    }
}

fn popup(frame: &mut Frame, area: Rect, title: &str, lines: Vec<Line>, min_width: u16) -> Rect {
    let width = lines
        .iter()
        .map(Line::width)
        .max()
        .unwrap_or_default()
        .max(usize::from(min_width)) as u16
        + 4;
    let height = lines.len() as u16 + 2;
    let [rect] = Layout::horizontal([Constraint::Length(width.min(area.width))])
        .flex(Flex::Center)
        .areas(area);
    let [rect] = Layout::vertical([Constraint::Length(height.min(area.height))])
        .flex(Flex::Center)
        .areas(rect);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .title(format!(" {title} ").bold())
                .border_style(Style::new().fg(ACCENT))
                .padding(Padding::horizontal(1)),
        ),
        rect,
    );
    rect
}

fn draw_menu(frame: &mut Frame, area: Rect, monitor: &Monitor, menu: &menu::Menu) {
    let shown = menu::shown(monitor, menu);
    let selected = menu.selected().min(shown.items.len().saturating_sub(1));
    let label_width = shown
        .items
        .iter()
        .map(|item| item.label.chars().count())
        .max()
        .unwrap_or_default();
    let detail_width = shown
        .items
        .iter()
        .filter_map(|item| item.detail.as_ref())
        .map(|detail| detail.chars().count())
        .max()
        .unwrap_or_default();
    let extra = usize::from(shown.items.is_empty()) + usize::from(shown.footer.is_some()) * 2;
    let visible = usize::from(area.height.saturating_sub(2))
        .saturating_sub(extra)
        .max(1);
    let offset = selected.saturating_sub(visible - 1);
    let mut lines: Vec<Line> = shown
        .items
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible)
        .map(|(index, item)| {
            let mark = match item.mark {
                Mark::On => "● ".fg(ACCENT),
                Mark::Off => "○ ".fg(MUTED),
                Mark::Check => "✓ ".fg(ACCENT),
                Mark::Running => "▶ ".fg(ACCENT),
                Mark::None | Mark::Open => "  ".into(),
            };
            let label = format!("{:<label_width$}", item.label);
            let detail = format!(
                "  {:>detail_width$}{}",
                item.detail.clone().unwrap_or_default(),
                if item.mark == Mark::Open {
                    " ▸"
                } else {
                    "  "
                }
            );
            let mut style = Style::new();
            if !item.enabled {
                style = style.fg(MUTED);
            }
            if index == selected {
                style = style.reversed();
            }
            Line::from(vec![
                mark,
                Span::styled(label, style),
                Span::styled(detail, style.fg(MUTED)),
            ])
        })
        .collect();
    if shown.items.is_empty()
        && let Some(empty) = shown.empty
    {
        lines.push(Line::from(empty.fg(MUTED)));
    }
    if let Some(footer) = shown.footer {
        lines.push(Line::default());
        lines.push(Line::from(footer.fg(MUTED)));
    }
    let rect = popup(frame, area, &shown.title, lines, 34);
    let mut hits = monitor.hits.borrow_mut();
    hits.menu = Some(rect);
    hits.menu_items = (offset..shown.items.len().min(offset + visible))
        .enumerate()
        .map(|(row, index)| {
            (
                Rect::new(
                    rect.x + 1,
                    rect.y + 1 + row as u16,
                    rect.width.saturating_sub(2),
                    1,
                ),
                index,
            )
        })
        .collect();
}

pub fn centered_text(frame: &mut Frame, area: Rect, lines: Vec<Line>) {
    let height = (lines.len() as u16).min(area.height);
    let [rect] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), rect);
}

pub fn bar(fraction: f64, width: usize, color: Color) -> Vec<Span<'static>> {
    let filled = ((fraction.clamp(0.0, 1.0) * width as f64).round() as usize).min(width);
    vec![
        "█".repeat(filled).fg(color),
        "░".repeat(width - filled).fg(MUTED),
    ]
}
