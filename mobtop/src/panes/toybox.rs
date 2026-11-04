use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Stylize};
use ratatui::text::Line;

use crate::account::{EFFECTS, effect_name};
use crate::app::Monitor;
use crate::layout::Leaf;
use crate::ui::{ACCENT, MUTED};

use super::centered_text;

const CUSHION: [&str; 4] = [
    "   .-\"\"\"\"\"\"\"-.   ",
    "  /  .-\"\"\"-.  \\  ",
    " |  (       )  |~",
    "  '-._______.-'  ",
];

const SQUISHED: [&str; 4] = [
    "                  ",
    "   .-\"\"\"\"\"\"\"\"-.  ",
    " (  (       )  )~~",
    "  '-.________.-'  ",
];

pub fn draw(frame: &mut Frame, area: Rect, monitor: &Monitor, leaf: &Leaf) {
    let squished = monitor
        .fart
        .is_some_and(|at| at.elapsed() < Duration::from_millis(700));
    let queued = monitor
        .queued
        .is_some_and(|at| at.elapsed() < Duration::from_secs(3));
    let focused = monitor.focus == Some(leaf.id);
    let selected = monitor.selected_of(leaf.id).min(EFFECTS.len() - 1);
    let mut lines: Vec<Line> = if squished { SQUISHED } else { CUSHION }
        .into_iter()
        .map(|line| Line::from(line.fg(Color::Magenta)))
        .collect();
    lines.push(Line::default());
    let first = lines.len();
    for (index, effect) in EFFECTS.into_iter().enumerate() {
        let name = format!(" {} {} ", index + 1, effect_name(effect));
        lines.push(Line::from(if index != selected {
            name.into()
        } else if focused {
            name.bold().reversed()
        } else {
            name.bold()
        }));
    }
    lines.push(Line::default());
    lines.push(if queued {
        Line::from("✓ Sent".fg(ACCENT))
    } else {
        Line::from("enter or 1-4".fg(MUTED))
    });
    let rect = centered_text(frame, area, lines);
    let mut hits = monitor.hits.borrow_mut();
    for index in 0..EFFECTS.len() {
        let y = rect.y + (first + index) as u16;
        if y < rect.bottom() {
            hits.rows.push((leaf.id, y, index));
        }
    }
}
