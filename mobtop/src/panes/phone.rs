use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Padding, Paragraph, Wrap};
use serde_json::{Map, Value};

use crate::app::Monitor;
use crate::feed::Feed;
use crate::layout::Leaf;
use crate::preview;
use crate::ui::{ACCENT, MUTED};

use super::{bar, format_bits, note, problem};

fn row(label: &str, value: String) -> Line<'static> {
    Line::from(vec![format!("{label:<14}").fg(MUTED), value.into()])
}

fn status_line(feed: &Feed) -> Line<'static> {
    let state = &feed.state;
    let flag = |key: &str| state.get(key).and_then(Value::as_bool);
    let mut spans: Vec<Span> = vec![match flag("streaming") {
        Some(true) => "● Live".fg(ACCENT).bold(),
        Some(false) => "○ Not live".fg(MUTED),
        None => "".into(),
    }];
    if flag("recording") == Some(true) {
        spans.push("  ● Recording".red());
    }
    if flag("muted") == Some(true) {
        spans.push("  MUTED".red().bold());
    }
    if let Some(level) = feed.general.get("batteryLevel").and_then(Value::as_f64) {
        let charging = feed.general.get("batteryCharging").and_then(Value::as_bool) == Some(true);
        let text = format!(
            "  Battery {level:.0} %{}",
            if charging { " ⚡" } else { "" }
        );
        spans.push(if charging || level > 15.0 {
            text.into()
        } else {
            text.red()
        });
    }
    match feed.general.get("flame").and_then(Value::as_str) {
        Some("Red") => spans.push("  Very hot".red()),
        Some("Yellow") => spans.push("  Hot".yellow()),
        _ => {}
    }
    if let Some(audio) = audio_level(&feed.top_right) {
        spans.push("  ".into());
        spans.extend(audio);
    }
    Line::from(spans)
}

fn audio_level(top_right: &Map<String, Value>) -> Option<Vec<Span<'static>>> {
    let level = &top_right.get("audioInfo")?["audioLevel"];
    if level.get("muted").is_some() {
        return Some(vec!["Audio muted".red()]);
    }
    let db = level.get("value")?["_0"].as_f64()?;
    let color = if db > -8.5 {
        Color::Red
    } else if db > -20.0 {
        Color::Yellow
    } else {
        Color::Green
    };
    let mut spans = vec!["Audio ".fg(MUTED)];
    spans.extend(bar((db + 60.0) / 60.0, 12, color));
    Some(spans)
}

fn state_lines(feed: &Feed) -> Vec<Line<'static>> {
    let state = &feed.state;
    let flag = |key: &str| state.get(key).and_then(Value::as_bool);
    let mut lines = Vec::new();
    if let Some(scene) = feed.setting_name("scenes", state.get("scene")) {
        lines.push(row("Scene", scene.to_string()));
    }
    if let Some(zoom) = state.get("zoom").and_then(Value::as_f64) {
        let preset = state
            .get("zoomPresets")
            .and_then(Value::as_array)
            .and_then(|presets| {
                presets
                    .iter()
                    .find(|preset| Some(&preset["id"]) == state.get("zoomPreset"))
            })
            .and_then(|preset| preset["name"].as_str());
        lines.push(row(
            "Zoom",
            match preset {
                Some(preset) => format!("{zoom:.1}×  ({preset})"),
                None => format!("{zoom:.1}×"),
            },
        ));
    }
    if let Some(mic) = feed.setting_name("mics", state.get("mic")) {
        lines.push(row("Mic", mic.to_string()));
    }
    let bitrate = feed
        .settings
        .as_ref()
        .and_then(|settings| settings["bitratePresets"].as_array())
        .and_then(|presets| {
            presets
                .iter()
                .find(|preset| Some(&preset["id"]) == state.get("bitrate"))
        })
        .and_then(|preset| preset["bitrate"].as_f64());
    if let Some(bitrate) = bitrate {
        lines.push(row("Bitrate", format_bits(bitrate)));
    }
    for (key, label) in [
        ("torchOn", "Torch"),
        ("stealthMode", "Stealth mode"),
        ("gimbalTracking", "Gimbal tracking"),
        ("previewStream", "Preview stream"),
    ] {
        if flag(key) == Some(true) {
            lines.push(row(label, "on".to_string()));
        }
    }
    if let Some(switcher) = feed.setting_name(
        "autoSceneSwitchers",
        state
            .get("autoSceneSwitcher")
            .and_then(|switcher| switcher.get("id")),
    ) {
        lines.push(row("Scene switcher", switcher.to_string()));
    }
    let running: Vec<&str> = state
        .get("macros")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item["running"].as_bool() == Some(true))
        .filter_map(|item| item["name"].as_str())
        .collect();
    if !running.is_empty() {
        lines.push(row("Macros running", running.join(", ")));
    }
    let filters: Vec<&str> = crate::controls::FILTERS
        .iter()
        .filter(|(key, _)| crate::controls::filter_on(state, key))
        .map(|(_, label)| *label)
        .collect();
    if !filters.is_empty() {
        lines.push(row("Filters", filters.join(", ")));
    }
    lines
}

fn picture(frame: &mut Frame, area: Rect, monitor: &Monitor, device: &str) {
    let Some(cells) = monitor
        .frames
        .get(device)
        .and_then(|image| preview::cells(image, area.width, area.height))
    else {
        frame.render_widget(
            Paragraph::new("Waiting for the stream preview…".fg(MUTED))
                .alignment(Alignment::Center),
            area,
        );
        return;
    };
    let left = " ".repeat(usize::from(area.width.saturating_sub(cells.width) / 2));
    let lines: Vec<Line> = cells
        .rows
        .iter()
        .map(|row| {
            Line::from(
                std::iter::once(Span::raw(left.clone()))
                    .chain(row.iter().map(|(top, bottom)| {
                        Span::styled(
                            "▀",
                            Style::new()
                                .fg(Color::Rgb(top[0], top[1], top[2]))
                                .bg(Color::Rgb(bottom[0], bottom[1], bottom[2])),
                        )
                    }))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

pub fn draw(frame: &mut Frame, area: Rect, monitor: &Monitor, leaf: &Leaf) {
    if let Some(problem) = problem(monitor, leaf) {
        let offline = monitor.device_of(leaf).is_some() && monitor.may_watch();
        return note(
            frame,
            area,
            &if offline {
                format!("{problem} This page updates as soon as it connects again.")
            } else {
                problem
            },
        );
    }
    let (Some(page), Some(device)) = (&monitor.page, monitor.device_of(leaf)) else {
        return;
    };
    let Some(feed) = monitor.feeds.get(&device.id) else {
        return note(frame, area, "Loading…");
    };
    let mut notices = Vec::new();
    if !page.preview {
        notices.push("The streamer has not let you see the stream preview.");
    }
    if page.remote_control == "view" {
        notices.push("The streamer has not let you control Moblin.");
    } else if !page.go_live {
        notices.push("The streamer has not let you go live.");
    }
    let [top, notes, body, hint] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(notices.len() as u16),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    let padded = |rect: Rect| Rect {
        x: rect.x + 1,
        width: rect.width.saturating_sub(2),
        ..rect
    };
    frame.render_widget(status_line(feed), padded(top));
    frame.render_widget(
        Paragraph::new(
            notices
                .into_iter()
                .map(|notice| Line::from(notice.yellow()))
                .collect::<Vec<_>>(),
        )
        .wrap(Wrap { trim: true }),
        padded(notes),
    );
    let previewing = page.preview && !leaf.hide_preview;
    if previewing {
        picture(frame, body, monitor, &device.id);
    } else {
        frame.render_widget(
            Paragraph::new(state_lines(feed))
                .block(ratatui::widgets::Block::new().padding(Padding::new(1, 1, 1, 0))),
            body,
        );
    }
    let mut keys = vec![" enter ".bold(), "controls".fg(MUTED)];
    if page.preview {
        keys.extend([
            "   p ".bold(),
            if previewing {
                "hides the preview"
            } else {
                "shows the preview"
            }
            .fg(MUTED),
        ]);
    }
    frame.render_widget(Line::from(keys), hint);
}
