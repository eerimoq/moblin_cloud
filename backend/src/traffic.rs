use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;
use serde_json::Value;

#[derive(Default)]
struct Kinds(Mutex<HashMap<String, (u64, u64)>>);

impl Kinds {
    fn record(&self, kind: &str, bytes: usize) {
        let add = |counts: &mut (u64, u64)| {
            counts.0 += 1;
            counts.1 += bytes as u64;
        };
        let mut kinds = self.0.lock().unwrap();
        match kinds.get_mut(kind) {
            Some(counts) => add(counts),
            None => {
                let kind = if kinds.len() < 100 { kind } else { "other" };
                add(kinds.entry(kind.to_string()).or_default());
            }
        }
    }

    fn stats(&self) -> Vec<MessageKind> {
        let mut kinds: Vec<MessageKind> = self
            .0
            .lock()
            .unwrap()
            .iter()
            .map(|(kind, (messages, bytes))| MessageKind {
                kind: kind.clone(),
                messages: *messages,
                bytes: *bytes,
            })
            .collect();
        kinds.sort_by(|a, b| {
            b.messages
                .cmp(&a.messages)
                .then_with(|| a.kind.cmp(&b.kind))
        });
        kinds
    }
}

#[derive(Default)]
pub struct Traffic {
    sent: Kinds,
    received: Kinds,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct TrafficStats {
    pub sent: Vec<MessageKind>,
    pub received: Vec<MessageKind>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct MessageKind {
    pub kind: String,
    pub messages: u64,
    pub bytes: u64,
}

fn name(value: &Value) -> Option<&str> {
    value
        .as_object()?
        .keys()
        .next()
        .map(String::as_str)
        .filter(|name| name.len() <= 40 && name.chars().all(|c| c.is_ascii_alphanumeric()))
}

fn kind(message: &Value) -> String {
    let Some(top) = name(message) else {
        return "invalid".to_string();
    };
    let body = &message[top];
    match top {
        "request" | "event" => name(&body["data"]).unwrap_or("invalid").to_string(),
        "response" => format!("response:{}", name(&body["result"]).unwrap_or("invalid")),
        _ => top.to_string(),
    }
}

impl Traffic {
    pub fn sent(&self, message: &Value, bytes: usize) {
        self.sent.record(&kind(message), bytes);
    }

    pub fn received(&self, text: &str) {
        let kind = serde_json::from_str(text)
            .map(|message| kind(&message))
            .unwrap_or_else(|_| "invalid".to_string());
        self.received.record(&kind, text.len());
    }

    pub fn stats(&self) -> TrafficStats {
        TrafficStats {
            sent: self.sent.stats(),
            received: self.received.stats(),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn messages_are_counted_per_kind() {
        let traffic = Traffic::default();
        traffic.received(r#"{"ping":{}}"#);
        traffic.received(r#"{"ping":{}}"#);
        traffic.received(r#"{"event":{"data":{"status":{"topRight":{}}}}}"#);
        traffic.received(r#"{"response":{"id":1,"result":{"ok":{}}}}"#);
        traffic.received(r#"{"response":{"id":2,"result":{"error":{}}}}"#);
        traffic.received("not json");
        traffic.sent(
            &json!({"request": {"id": 1, "data": {"getStatus": {}}}}),
            10,
        );
        traffic.sent(&json!({"pong": {}}), 11);
        assert_eq!(
            traffic.stats(),
            TrafficStats {
                sent: vec![
                    MessageKind {
                        kind: "getStatus".to_string(),
                        messages: 1,
                        bytes: 10,
                    },
                    MessageKind {
                        kind: "pong".to_string(),
                        messages: 1,
                        bytes: 11,
                    },
                ],
                received: vec![
                    MessageKind {
                        kind: "ping".to_string(),
                        messages: 2,
                        bytes: 22,
                    },
                    MessageKind {
                        kind: "invalid".to_string(),
                        messages: 1,
                        bytes: 8,
                    },
                    MessageKind {
                        kind: "response:error".to_string(),
                        messages: 1,
                        bytes: 43,
                    },
                    MessageKind {
                        kind: "response:ok".to_string(),
                        messages: 1,
                        bytes: 40,
                    },
                    MessageKind {
                        kind: "status".to_string(),
                        messages: 1,
                        bytes: 45,
                    },
                ],
            }
        );
    }

    #[test]
    fn kinds_are_capped() {
        let traffic = Traffic::default();
        for index in 0..120 {
            traffic.received(&format!(r#"{{"event":{{"data":{{"kind{index}":{{}}}}}}}}"#));
        }
        traffic.received(r#"{"event":{"data":{"kind5":{}}}}"#);
        traffic.received(r#"{"event":{"data":{"a-b":{}}}}"#);
        let received = traffic.stats().received;
        assert_eq!(received.len(), 101);
        assert_eq!(received[0].kind, "other");
        assert_eq!(received[0].messages, 21);
        assert_eq!(
            received
                .iter()
                .find(|kind| kind.kind == "kind5")
                .unwrap()
                .messages,
            2
        );
    }
}
