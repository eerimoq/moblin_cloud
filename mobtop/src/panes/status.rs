use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Padding, Paragraph};
use serde_json::Value;

use crate::feed::{Direction, Feed};
use crate::ui::MUTED;

const TOP_LEFT: [(&str, &str); 8] = [
    ("stream", "Stream"),
    ("camera", "Camera"),
    ("mic", "Mic"),
    ("zoom", "Zoom"),
    ("obs", "OBS"),
    ("events", "Events"),
    ("chat", "Chat"),
    ("viewers", "Viewers"),
];

const TOP_RIGHT: [(&str, &str); 14] = [
    ("rtmpServer", "Ingests"),
    ("moblink", "Moblink"),
    ("remoteControl", "Remote control"),
    ("djiDevices", "DJI"),
    ("gameController", "Controller"),
    ("bitrate", "Bitrate"),
    ("srtla", "Bonding"),
    ("srtlaRtts", "Bonding RTT"),
    ("replay", "Replay"),
    ("uptime", "Uptime"),
    ("location", "Location"),
    ("recording", "Recording"),
    ("browserWidgets", "Widgets"),
    ("systemMonitor", "System"),
];

#[derive(Debug, PartialEq)]
pub struct StatusRow {
    pub label: &'static str,
    pub text: String,
    pub ok: bool,
}

fn format_bytes(bytes: u64) -> String {
    let bytes = bytes as f64;
    if bytes >= 1e9 {
        format!("{:.2} GB", bytes / 1e9)
    } else if bytes >= 1e6 {
        format!("{:.1} MB", bytes / 1e6)
    } else if bytes >= 1e3 {
        format!("{:.0} kB", bytes / 1e3)
    } else {
        format!("{bytes} B")
    }
}

fn traffic_amount(direction: Direction) -> String {
    format!("{} ({})", format_bytes(direction.bytes), direction.messages)
}

pub fn status_rows(feed: &Feed) -> Vec<StatusRow> {
    let mut rows = Vec::new();
    let mut push = |label: &'static str, text: String, ok: bool| {
        rows.push(StatusRow { label, text, ok });
    };
    let flag = |key: &str| feed.state.get(key).and_then(Value::as_bool);
    if let Some(live) = flag("streaming") {
        push("", if live { "Live" } else { "Not live" }.to_string(), true);
    }
    if flag("recording") == Some(true) {
        push("", "Recording".to_string(), true);
    }
    if flag("muted") == Some(true) {
        push("", "Muted".to_string(), false);
    }
    if let Some(level) = feed.general.get("batteryLevel").and_then(Value::as_f64) {
        let charging = feed
            .general
            .get("batteryCharging")
            .and_then(Value::as_bool)
            .unwrap_or_default();
        push(
            "Battery",
            format!("{level:.0} %{}", if charging { ", charging" } else { "" }),
            charging || level > 15.0,
        );
    }
    if let Some(flame) = feed.general.get("flame").and_then(Value::as_str) {
        let (text, ok) = match flame {
            "Red" => ("Very hot", false),
            "Yellow" => ("Hot", true),
            _ => ("Normal", true),
        };
        push("Thermal state", text.to_string(), ok);
    }
    if let Some(ssid) = feed.general.get("wiFiSsid").and_then(Value::as_str) {
        push("Wi-Fi", ssid.to_string(), true);
    }
    for (part, keys) in [
        (&feed.top_left, &TOP_LEFT[..]),
        (&feed.top_right, &TOP_RIGHT[..]),
    ] {
        for (key, label) in keys {
            let Some(item) = part.get(*key) else {
                continue;
            };
            let Some(message) = item["message"].as_str() else {
                continue;
            };
            push(
                label,
                message.to_string(),
                item.get("ok").and_then(Value::as_bool).unwrap_or(true),
            );
        }
    }
    if let Some(traffic) = feed.traffic {
        push(
            "Traffic",
            format!(
                "↑ {} ↓ {}",
                traffic_amount(traffic.received),
                traffic_amount(traffic.sent)
            ),
            true,
        );
    }
    rows
}

pub fn draw(frame: &mut Frame, area: Rect, feed: &Feed) {
    let lines: Vec<Line> = status_rows(feed)
        .into_iter()
        .map(|row| {
            let style = if row.ok {
                Style::new()
            } else {
                Style::new().red()
            };
            Line::from(vec![
                format!("{:<15}", row.label).fg(MUTED),
                Span::styled(row.text, style),
            ])
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines).block(ratatui::widgets::Block::new().padding(Padding::horizontal(1))),
        area,
    );
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn status_rows_follow_the_web_order() {
        let mut feed = Feed::default();
        feed.apply(
            "snapshot",
            &json!({
                "connected": true,
                "status": {
                    "general": {"batteryLevel": 12.0, "flame": "Red", "wiFiSsid": "Home"},
                    "topLeft": {"viewers": {"message": "42"}, "stream": {"message": "Main"}},
                    "topRight": {"uptime": {"message": "1 h"}, "bitrate": {"message": "5 Mbps", "ok": false}},
                },
                "state": {"streaming": true, "muted": true},
                "settings": null,
                "traffic": {
                    "sent": {"messages": 12, "bytes": 1500},
                    "received": {"messages": 3, "bytes": 2_500_000},
                },
            }),
        );
        let rows = status_rows(&feed);
        assert_eq!(
            rows.iter()
                .map(|row| (row.label, row.text.as_str(), row.ok))
                .collect::<Vec<_>>(),
            [
                ("", "Live", true),
                ("", "Muted", false),
                ("Battery", "12 %", false),
                ("Thermal state", "Very hot", false),
                ("Wi-Fi", "Home", true),
                ("Stream", "Main", true),
                ("Viewers", "42", true),
                ("Bitrate", "5 Mbps", false),
                ("Uptime", "1 h", true),
                ("Traffic", "↑ 2.5 MB (3) ↓ 2 kB (12)", true),
            ]
        );
    }
}
