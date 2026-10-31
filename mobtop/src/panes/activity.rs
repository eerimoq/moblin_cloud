use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState};

use crate::account::{Item, Kind, effect_name};
use crate::app::{Monitor, kinds_of};
use crate::layout::Leaf;
use crate::ui::{ACCENT, MUTED, ago, now};

use super::note;

fn kind_glyph(kind: Kind) -> Span<'static> {
    match kind {
        Kind::Tip => "♥ ".fg(ACCENT),
        Kind::Spin => "◎ ".cyan(),
        Kind::Effect => "✦ ".magenta(),
    }
}

fn activity_item<'a>(kind: Kind, item: &'a Item, now: u64, new: bool) -> ListItem<'a> {
    let mut first = vec![
        kind_glyph(kind),
        item.user.as_str().bold(),
        "  ".into(),
        item.amount_label().into(),
    ];
    if let Some(spins) = item.spins {
        first.push(format!(" · {spins} {}", if spins == 1 { "spin" } else { "spins" }).into());
    }
    if let Some(effect) = &item.effect {
        first.push(format!(" · {}", effect_name(effect)).into());
    }
    first.push(format!("  {}", ago(now.saturating_sub(item.at))).fg(MUTED));
    let mut lines = vec![Line::from(first)];
    if let Some(message) = item
        .message
        .as_deref()
        .filter(|message| !message.is_empty())
    {
        lines.push(Line::from(vec![
            "  ".into(),
            format!("“{message}”").italic(),
        ]));
    }
    let mut devices = vec![Span::raw("  ")];
    if item.devices.is_empty() {
        devices.push(match item.status.as_str() {
            "delivered" => "Delivered".fg(MUTED),
            _ => "Waiting for Moblin".yellow(),
        });
    } else {
        for device in &item.devices {
            devices.push(if device.received {
                format!("✓ {}  ", device.name).fg(MUTED)
            } else {
                format!("… {}  ", device.name).yellow()
            });
        }
    }
    lines.push(Line::from(devices));
    let item = ListItem::new(lines);
    if new {
        item.style(Style::new().bg(Color::Rgb(40, 70, 40)))
    } else {
        item
    }
}

pub fn draw(frame: &mut Frame, area: Rect, monitor: &Monitor, leaf: &Leaf) {
    let kinds = kinds_of(leaf);
    let [chips, list] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
    let mut title = vec![Span::raw(" ")];
    for (index, label) in ["Message", "Wheel of luck", "Effect"]
        .into_iter()
        .enumerate()
    {
        title.push(if kinds[index] {
            format!("{} {label}  ", index + 1).fg(Color::Reset)
        } else {
            format!("{} {label}  ", index + 1).fg(MUTED).crossed_out()
        });
    }
    frame.render_widget(Line::from(title), chips);
    if monitor.latest.iter().all(Option::is_none) {
        return note(frame, list, "Loading…");
    }
    let shown = monitor.activity_of(leaf);
    if shown.is_empty() {
        return note(frame, list, "Nothing yet.");
    }
    let now = now();
    let mut items: Vec<ListItem> = shown
        .iter()
        .map(|(kind, item)| activity_item(*kind, item, now, monitor.new_arrival(item)))
        .collect();
    let heights: Vec<usize> = items.iter().map(ListItem::height).collect();
    let more = Kind::ALL.into_iter().any(|kind| {
        kinds[kind as usize]
            && monitor.latest[kind as usize]
                .as_ref()
                .is_some_and(|recent| recent.more)
    });
    if more {
        items.push(ListItem::new(Line::from(" m shows more".fg(MUTED))));
    }
    let focused = monitor.focus == Some(leaf.id);
    let selected = monitor.selected_of(leaf.id).min(shown.len() - 1);
    let mut state = ListState::default().with_selected(Some(selected));
    frame.render_stateful_widget(
        List::new(items).highlight_style(if focused {
            Style::new().reversed()
        } else {
            Style::new().bold()
        }),
        list,
        &mut state,
    );
    let mut hits = monitor.hits.borrow_mut();
    let mut y = list.y;
    for (index, height) in heights.iter().enumerate().skip(state.offset()) {
        for _ in 0..*height {
            if y >= list.bottom() {
                return;
            }
            hits.rows.push((leaf.id, y, index));
            y += 1;
        }
    }
}
