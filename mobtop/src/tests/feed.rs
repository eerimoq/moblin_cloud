use serde_json::json;

use super::*;

#[test]
fn events_are_merged_like_the_web_monitor() {
    let mut feed = Feed::default();
    feed.apply(
        "snapshot",
        &json!({
            "name": "Moblin 1",
            "connected": true,
            "status": {"general": {"batteryLevel": 80}, "topLeft": {}, "topRight": {"bitrate": {"message": "5 Mbps"}}},
            "state": {"streaming": true, "scene": "a"},
            "settings": {"scenes": [{"id": "a", "name": "Main"}]},
        }),
    );
    assert!(feed.connected);
    assert_eq!(feed.general["batteryLevel"], 80);
    assert_eq!(
        feed.setting_name("scenes", feed.state.get("scene")),
        Some("Main")
    );
    feed.apply(
        "status",
        &json!({"general": null, "topLeft": {"mic": {"message": "Built-in"}}, "topRight": {"uptime": {"message": "1 m"}}, "sample": null}),
    );
    assert_eq!(feed.general["batteryLevel"], 80);
    assert_eq!(feed.top_left["mic"]["message"], "Built-in");
    assert!(!feed.top_right.contains_key("bitrate"));
    assert!(feed.samples.is_empty());
    feed.apply("state", &json!({"data": {"muted": true}}));
    assert_eq!(feed.state["muted"], true);
    assert_eq!(feed.state["streaming"], true);
    for at in 0..61 {
        feed.apply(
            "status",
            &json!({"general": null, "topLeft": null, "topRight": null, "sample": {"at": at, "bitrate": 1000, "totalBytes": 5, "bonding": [{"name": "Wi-Fi", "bytesPerSecond": 10, "rtt": null}]}}),
        );
    }
    assert_eq!(feed.samples.len(), 60);
    assert_eq!(feed.samples[0].at, 1);
    assert_eq!(feed.samples[59].bonding[0].name, "Wi-Fi");
    feed.apply("connection", &json!({"connected": false}));
    assert!(!feed.connected);
    assert!(feed.state.is_empty() && feed.top_left.is_empty());
    assert!(feed.settings.is_some());
    assert_eq!(feed.samples.len(), 60);
}

#[test]
fn zoom_presets_are_the_active_cameras() {
    let mut feed = Feed::default();
    assert_eq!(feed.zoom_presets(), None);
    feed.apply(
        "snapshot",
        &json!({
            "connected": true,
            "status": {},
            "state": {"zoomCamera": "back"},
            "settings": {"zoomPresets": {"front": [{"id": "f", "name": "1x"}], "back": [{"id": "b", "name": "2x"}]}},
        }),
    );
    assert_eq!(
        feed.zoom_presets(),
        Some(&json!([{"id": "b", "name": "2x"}]))
    );
    feed.apply("state", &json!({"data": {"zoomCamera": "other"}}));
    assert_eq!(feed.zoom_presets(), None);
}
