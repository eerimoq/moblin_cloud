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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct Direction {
    pub messages: u64,
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct Traffic {
    pub sent: Direction,
    pub received: Direction,
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
    pub traffic: Option<Traffic>,
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
                self.traffic = Traffic::deserialize(&data["traffic"]).ok();
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
                self.traffic = Traffic::deserialize(&data["traffic"]).ok();
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
                self.traffic = None;
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

    pub fn zoom_presets(&self) -> Option<Value> {
        let camera = self.state.get("zoomCamera")?;
        let x = |value: &Value, key: &str| value.get(key).and_then(Value::as_f64);
        let (min, max) = (x(camera, "minX")?, x(camera, "maxX")?);
        let presets = self
            .settings
            .as_ref()?
            .get("zoomPresets")?
            .get(camera.get("position")?.as_str()?)?
            .as_array()?
            .iter()
            .filter(|preset| x(preset, "x").is_some_and(|x| (min..=max).contains(&x)))
            .cloned()
            .collect();
        Some(Value::Array(presets))
    }
}

#[cfg(test)]
#[path = "tests/feed.rs"]
mod tests;
