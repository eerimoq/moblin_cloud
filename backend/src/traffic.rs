use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

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

pub fn unread(text: &str) -> String {
    serde_json::from_str(text)
        .map(|message| kind(&message))
        .unwrap_or_else(|_| "invalid".to_string())
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

#[derive(Default)]
struct Counter {
    messages: AtomicU64,
    bytes: AtomicU64,
}

impl Counter {
    fn record(&self, bytes: usize) {
        self.messages.fetch_add(1, Ordering::Relaxed);
        self.bytes.fetch_add(bytes as u64, Ordering::Relaxed);
    }

    fn totals(&self) -> Direction {
        Direction {
            messages: self.messages.load(Ordering::Relaxed),
            bytes: self.bytes.load(Ordering::Relaxed),
        }
    }
}

#[derive(Default)]
pub struct SocketTraffic {
    sent: Counter,
    received: Counter,
}

#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct Direction {
    pub messages: u64,
    pub bytes: u64,
}

#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct Totals {
    pub sent: Direction,
    pub received: Direction,
}

impl SocketTraffic {
    pub fn sent(&self, bytes: usize) {
        self.sent.record(bytes);
    }

    pub fn received(&self, bytes: usize) {
        self.received.record(bytes);
    }

    pub fn totals(&self) -> Totals {
        Totals {
            sent: self.sent.totals(),
            received: self.received.totals(),
        }
    }
}

impl Traffic {
    pub fn sent(&self, kind: &str, bytes: usize) {
        self.sent.record(kind, bytes);
    }

    pub fn received(&self, kind: &str, bytes: usize) {
        self.received.record(kind, bytes);
    }

    pub fn stats(&self) -> TrafficStats {
        TrafficStats {
            sent: self.sent.stats(),
            received: self.received.stats(),
        }
    }
}

#[cfg(test)]
#[path = "tests/traffic.rs"]
mod tests;
