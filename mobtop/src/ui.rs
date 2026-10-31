use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};

use crate::account::Connect;
use crate::app::{App, Screen};
use crate::panes;

pub const ACCENT: Color = Color::Green;
pub const MUTED: Color = Color::DarkGray;

pub fn draw(frame: &mut Frame, app: &App) {
    let [header, body, status, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    let mut hints = vec![];
    match &app.screen {
        Screen::SignedOut { error } => {
            signed_out(frame, body, app, error.as_deref());
            hints.extend([("t", "Twitch"), ("k", "Kick"), ("c", "another device")]);
        }
        Screen::SigningIn { platform, url } => {
            let mut lines = vec![
                Line::from(format!(
                    "Finish signing in with {} in your browser.",
                    platform.title()
                )),
                Line::default(),
            ];
            if let Some(url) = url {
                lines.push(Line::from(
                    "If no browser opened, open this address:".fg(MUTED),
                ));
                lines.push(Line::from(url.as_str()));
            }
            centered(frame, body, "Signing in", lines);
            hints.push(("esc", "cancel"));
        }
        Screen::Connecting { connect } => {
            connecting(frame, body, connect.as_ref());
            hints.push(("esc", "cancel"));
        }
        Screen::Loading { error } => {
            let mut lines = vec![Line::from("Loading…")];
            if let Some(error) = error {
                lines.push(Line::default());
                lines.push(Line::from(error.as_str().red()));
            }
            centered(frame, body, "Moblin Cloud", lines);
            hints.extend([("r", "retry"), ("s", "sign out")]);
        }
        Screen::SignedIn(monitor) => {
            frame.render_widget(panes::header(monitor), header);
            panes::draw(frame, body, monitor);
            hints.extend(panes::hints(monitor));
        }
    }
    if !matches!(&app.screen, Screen::SignedIn(monitor) if panes::typing(monitor)) {
        hints.push(("q", "quit"));
    }
    if !matches!(app.screen, Screen::SignedIn(_)) {
        frame.render_widget(Line::from(" Moblin Cloud".fg(ACCENT).bold()), header);
    }
    if let Some(shown) = &app.status {
        let style = if shown.error {
            Style::new().red()
        } else {
            Style::new().fg(ACCENT)
        };
        frame.render_widget(Line::styled(format!(" {}", shown.text), style), status);
    }
    let spans: Vec<Span> = hints
        .into_iter()
        .flat_map(|(key, label)| [format!(" {key} ").bold(), format!("{label}  ").fg(MUTED)])
        .collect();
    frame.render_widget(Line::from(spans), footer);
}

fn centered(frame: &mut Frame, area: Rect, title: &str, lines: Vec<Line>) {
    let width = lines
        .iter()
        .map(Line::width)
        .max()
        .unwrap_or_default()
        .max(40) as u16
        + 4;
    let height = lines.len() as u16 + 2;
    let [area] = Layout::horizontal([Constraint::Length(width.min(area.width))])
        .flex(ratatui::layout::Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([Constraint::Length(height.min(area.height))])
        .flex(ratatui::layout::Flex::Center)
        .areas(area);
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::bordered()
                .title(format!(" {title} "))
                .padding(ratatui::widgets::Padding::horizontal(1)),
        ),
        area,
    );
}

fn signed_out(frame: &mut Frame, area: Rect, app: &App, error: Option<&str>) {
    let mut lines = vec![
        Line::from("Sign in to watch your stream, its tips and your Moblins."),
        Line::default(),
        Line::from(vec![" t ".bold(), "Sign in with Twitch".into()]),
        Line::from(vec![" k ".bold(), "Sign in with Kick".into()]),
        Line::from(vec![" c ".bold(), "Sign in on another device".into()]),
        Line::default(),
        Line::from(app.backend.url.as_str().fg(MUTED)),
    ];
    if let Some(error) = error {
        lines.push(Line::default());
        lines.push(Line::from(error.red()));
    }
    centered(frame, area, "Moblin Cloud", lines);
}

fn connecting(frame: &mut Frame, area: Rect, connect: Option<&Connect>) {
    let Some(connect) = connect else {
        centered(
            frame,
            area,
            "Sign in on another device",
            vec![Line::from("Getting a code…")],
        );
        return;
    };
    let mut lines = vec![
        Line::from("On your phone or another computer, open"),
        Line::from(connect.url.as_str().underlined()),
        Line::from("and enter this code:"),
        Line::default(),
        Line::from(connect.user_code.as_str().bold().fg(ACCENT)),
    ];
    let qr = qr_lines(&connect.url);
    if lines.len() + qr.len() + 4 <= usize::from(area.height) {
        lines.push(Line::default());
        lines.push(Line::from("Or scan this:".fg(MUTED)));
        lines.extend(
            qr.into_iter()
                .map(|line| Line::from(line.fg(Color::Black).bg(Color::White))),
        );
    }
    centered(frame, area, "Sign in on another device", lines);
}

pub fn qr_lines(text: &str) -> Vec<String> {
    let Ok(code) = qrcode::QrCode::new(text) else {
        return Vec::new();
    };
    code.render::<qrcode::render::unicode::Dense1x2>()
        .dark_color(qrcode::render::unicode::Dense1x2::Dark)
        .light_color(qrcode::render::unicode::Dense1x2::Light)
        .quiet_zone(true)
        .build()
        .lines()
        .map(str::to_string)
        .collect()
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

pub fn ago(seconds: u64) -> String {
    match seconds {
        0..60 => "just now".to_string(),
        60..3600 => format!("{} min ago", seconds / 60),
        3600..86_400 => format!("{} h ago", seconds / 3600),
        _ => {
            let days = seconds / 86_400;
            format!("{days} {} ago", if days == 1 { "day" } else { "days" })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_codes_are_drawn_with_half_blocks() {
        let url = "https://cloud.moblin.app/connect/?code=KQMRXDBT";
        let lines = qr_lines(url);
        let size = qrcode::QrCode::new(url).unwrap().width() + 8;
        assert_eq!(lines.len(), size.div_ceil(2));
        assert!(lines.iter().all(|line| line.chars().count() == size));
    }

    #[test]
    fn times_are_relative() {
        assert_eq!(ago(5), "just now");
        assert_eq!(ago(125), "2 min ago");
        assert_eq!(ago(7200), "2 h ago");
        assert_eq!(ago(86_400), "1 day ago");
        assert_eq!(ago(3 * 86_400), "3 days ago");
    }
}
