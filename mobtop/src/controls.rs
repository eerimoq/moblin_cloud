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

pub const OPTIONAL_PICKS: [&str; 3] = [
    "setAutoSceneSwitcher",
    "setMoblinCloudAlertsWidget",
    "setMoblinCloudWheelOfLuckWidget",
];

pub fn optional(kind: &str, id: Option<&str>) -> Control {
    Control::new(
        kind,
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
#[path = "tests/controls.rs"]
mod tests;
