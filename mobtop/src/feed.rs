use std::collections::VecDeque;

use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub at: u64,
    pub bitrate: Option<i64>,
    #[serde(default)]
    pub bonding: Vec<Bond>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bond {
    pub name: String,
    pub bytes_per_second: u64,
    pub rtt: Option<i64>,
}

#[derive(Default)]
pub struct Feed {
    pub connected: bool,
    pub general: Map<String, Value>,
    pub top_left: Map<String, Value>,
    pub top_right: Map<String, Value>,
    pub state: Map<String, Value>,
    pub settings: Option<Value>,
    pub samples: VecDeque<Sample>,
}

fn object(value: Option<&Value>) -> Option<&Map<String, Value>> {
    value.and_then(Value::as_object)
}

impl Feed {
    fn clear(&mut self) {
        self.general.clear();
        self.top_left.clear();
        self.top_right.clear();
        self.state.clear();
    }

    fn set_parts(&mut self, status: &Value, filled_only: bool) {
        for (key, part) in [
            ("general", &mut self.general),
            ("topLeft", &mut self.top_left),
            ("topRight", &mut self.top_right),
        ] {
            if let Some(value) = object(status.get(key))
                && !(filled_only && value.is_empty())
            {
                *part = value.clone();
            }
        }
    }

    pub fn apply(&mut self, event: &str, data: &Value) {
        match event {
            "snapshot" => {
                self.connected = data["connected"].as_bool().unwrap_or_default();
                if self.connected {
                    self.set_parts(&data["status"], true);
                    if let Some(state) = object(data.get("state")) {
                        self.state.extend(state.clone());
                    }
                } else {
                    self.clear();
                }
                if !data["settings"].is_null() {
                    self.settings = Some(data["settings"].clone());
                }
            }
            "status" => {
                self.set_parts(data, false);
                if let Ok(sample) = Sample::deserialize(&data["sample"]) {
                    if self.samples.len() == 60 {
                        self.samples.pop_front();
                    }
                    self.samples.push_back(sample);
                }
            }
            "state" => {
                if let Some(state) = object(data.get("data")) {
                    self.state.extend(state.clone());
                }
            }
            "settings" => self.settings = Some(data["data"].clone()),
            "connection" => {
                self.connected = data["connected"].as_bool().unwrap_or_default();
                if !self.connected {
                    self.clear();
                }
            }
            _ => {}
        }
    }

    pub fn setting_name(&self, list: &str, id: Option<&Value>) -> Option<&str> {
        let id = id?;
        self.settings.as_ref()?[list]
            .as_array()?
            .iter()
            .find(|item| item.get("id") == Some(id))?["name"]
            .as_str()
    }
}

#[cfg(test)]
mod tests {
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
}
