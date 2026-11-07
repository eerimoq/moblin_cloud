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
