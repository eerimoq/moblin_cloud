use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Block, Padding, Paragraph, Wrap};

use crate::app::Monitor;
use crate::layout::Leaf;
use crate::ui::{ACCENT, MUTED};

use super::problem;

pub fn draw(frame: &mut Frame, area: Rect, monitor: &Monitor, leaf: &Leaf) {
    let draft = monitor.drafts.get(&leaf.id).map_or("", String::as_str);
    let focused = monitor.focus == Some(leaf.id);
    let mut prompt = vec!["> ".fg(ACCENT), draft.to_string().into()];
    if focused {
        prompt.push("▏".fg(ACCENT));
    } else if draft.is_empty() {
        prompt.push("Message to the streamer".fg(MUTED));
    }
    let mut lines = vec![Line::from(prompt)];
    if !draft.is_empty() {
        lines.push(Line::from(
            format!("{} / 300", draft.chars().count()).fg(MUTED),
        ));
    }
    if let Some(problem) = problem(monitor, leaf) {
        lines.push(Line::from(
            format!("{problem} Messages to the streamer need a connected Moblin.").fg(MUTED),
        ));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(Block::new().padding(Padding::horizontal(1))),
        area,
    );
}
