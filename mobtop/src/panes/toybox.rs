use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Stylize};
use ratatui::text::Line;

use crate::app::Monitor;
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

pub fn draw(frame: &mut Frame, area: Rect, monitor: &Monitor) {
    let squished = monitor
        .fart
        .is_some_and(|at| at.elapsed() < Duration::from_millis(700));
    let queued = monitor
        .queued
        .is_some_and(|at| at.elapsed() < Duration::from_secs(3));
    let mut lines: Vec<Line> = if squished { SQUISHED } else { CUSHION }
        .into_iter()
        .map(|line| Line::from(line.fg(Color::Magenta)))
        .collect();
    lines.push(Line::default());
    lines.push(if queued {
        Line::from("✓ Queued for Moblin".fg(ACCENT))
    } else {
        Line::from(vec!["Fart".bold(), "  enter".fg(MUTED)])
    });
    centered_text(frame, area, lines);
}
