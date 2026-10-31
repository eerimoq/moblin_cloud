use serde_json::{Map, Value, json};

pub const FILTERS: [(&str, &str); 16] = [
    ("poll", "Poll"),
    ("pinch", "Pinch"),
    ("whirlpool", "Whirlpool"),
    ("blurFaces", "Blur faces"),
    ("privacy", "Privacy"),
    ("movie", "Movie"),
    ("fourThree", "4:3"),
    ("crt", "CRT"),
    ("pixellate", "Pixellate"),
    ("grayScale", "Gray scale"),
    ("sepia", "Sepia"),
    ("triple", "Triple"),
    ("twin", "Twin"),
    ("moblinInMouth", "Moblin in mouth"),
    ("cameraMan", "Camera man"),
    ("beauty", "Beauty"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct Control {
    pub key: String,
    pub body: Value,
}

impl Control {
    fn new(kind: &str, data: Value) -> Self {
        Self {
            key: kind.to_string(),
            body: json!({ kind: data }),
        }
    }

    fn keyed(kind: &str, id: &str, data: Value) -> Self {
        Self {
            key: format!("{kind}:{id}"),
            body: json!({ kind: data }),
        }
    }
}

pub fn toggle(kind: &str, on: bool) -> Control {
    Control::new(kind, json!({ "on": on }))
}

pub fn select(kind: &str, id: &str) -> Control {
    Control::new(kind, json!({ "id": id }))
}

pub fn action(kind: &str) -> Control {
    Control::new(kind, json!({}))
}

pub fn filter(key: &str, on: bool) -> Control {
    Control::new("setFilter", json!({ "filter": { key: {} }, "on": on }))
}

pub fn auto_scene_switcher(id: Option<&str>) -> Control {
    Control::new(
        "setAutoSceneSwitcher",
        match id {
            Some(id) => json!({ "id": id }),
            None => json!({}),
        },
    )
}

pub fn priorities_enabled(enabled: bool) -> Control {
    Control::new(
        "setSrtConnectionPrioritiesEnabled",
        json!({ "enabled": enabled }),
    )
}

pub fn priority(id: &str, priority: i64, enabled: bool) -> Control {
    Control::keyed(
        "setSrtConnectionPriority",
        id,
        json!({ "id": id, "priority": priority, "enabled": enabled }),
    )
}

pub fn macro_run(id: &str, running: bool) -> Control {
    let kind = if running { "stopMacro" } else { "startMacro" };
    Control::keyed(kind, id, json!({ "id": id }))
}

pub fn message(text: &str) -> Control {
    Control::new("sendMessage", json!({ "text": text }))
}

pub fn flag(state: &Map<String, Value>, key: &str) -> Option<bool> {
    state.get(key).and_then(Value::as_bool)
}

pub fn filter_on(state: &Map<String, Value>, key: &str) -> bool {
    let Some(filters) = state.get("filters").and_then(Value::as_array) else {
        return false;
    };
    filters.chunks(2).any(|pair| {
        pair[0]
            .as_object()
            .and_then(|filter| filter.keys().next())
            .is_some_and(|name| name == key)
            && pair.get(1) == Some(&Value::Bool(true))
    })
}

pub fn go_live_disabled(state: &Map<String, Value>, go_live: bool) -> bool {
    match flag(state, "streaming") {
        None => true,
        Some(streaming) => !go_live && !streaming,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_match_the_backend_shapes() {
        assert_eq!(
            toggle("setTorch", true).body,
            json!({"setTorch": {"on": true}})
        );
        assert_eq!(select("setScene", "a").key, "setScene");
        assert_eq!(
            action("reloadBrowserWidgets").body,
            json!({"reloadBrowserWidgets": {}})
        );
        assert_eq!(
            filter("grayScale", false).body,
            json!({"setFilter": {"filter": {"grayScale": {}}, "on": false}})
        );
        assert_eq!(
            auto_scene_switcher(None).body,
            json!({"setAutoSceneSwitcher": {}})
        );
        assert_eq!(
            auto_scene_switcher(Some("s")).body,
            json!({"setAutoSceneSwitcher": {"id": "s"}})
        );
        let set = priority("c", 3, true);
        assert_eq!(set.key, "setSrtConnectionPriority:c");
        assert_eq!(
            set.body,
            json!({"setSrtConnectionPriority": {"id": "c", "priority": 3, "enabled": true}})
        );
        assert_eq!(macro_run("m", true).key, "stopMacro:m");
        assert_eq!(
            macro_run("m", false).body,
            json!({"startMacro": {"id": "m"}})
        );
        assert_eq!(message("hi").body, json!({"sendMessage": {"text": "hi"}}));
    }

    #[test]
    fn states_are_read_like_the_web() {
        let state = json!({
            "filters": [{"grayScale": {}}, true, {"sepia": {}}, false, {"crt": {}}],
            "streaming": false,
        });
        let state = state.as_object().unwrap();
        assert!(filter_on(state, "grayScale"));
        assert!(!filter_on(state, "sepia"));
        assert!(!filter_on(state, "crt"));
        assert!(!filter_on(state, "movie"));
        assert!(go_live_disabled(state, false));
        assert!(!go_live_disabled(state, true));
        let live = json!({"streaming": true});
        assert!(!go_live_disabled(live.as_object().unwrap(), false));
        assert!(go_live_disabled(&Map::new(), true));
    }
}
