use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::mpsc::UnboundedSender;

use crate::app::Message;
use crate::backend::{Backend, BackendError};

#[derive(Debug, PartialEq)]
pub struct Event {
    pub name: String,
    pub data: String,
}

pub enum Delivery {
    Opened { again: bool },
    Event(Event),
}

#[derive(Default)]
pub struct Parser {
    buffer: Vec<u8>,
    name: Option<String>,
    data: Option<String>,
}

impl Parser {
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<Event> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        while let Some(end) = self.buffer.iter().position(|&byte| byte == b'\n') {
            let line: Vec<u8> = self.buffer.drain(..=end).collect();
            let line = String::from_utf8_lossy(&line);
            let line = line.trim_end_matches(['\n', '\r']);
            if line.is_empty() {
                let name = self.name.take();
                let data = self.data.take();
                if name.is_some() || data.is_some() {
                    events.push(Event {
                        name: name.unwrap_or_else(|| "message".to_string()),
                        data: data.unwrap_or_default(),
                    });
                }
            } else if let Some(name) = line.strip_prefix("event:") {
                self.name = Some(name.trim_start().to_string());
            } else if let Some(data) = line.strip_prefix("data:") {
                let data = data.strip_prefix(' ').unwrap_or(data);
                match &mut self.data {
                    Some(old) => {
                        old.push('\n');
                        old.push_str(data);
                    }
                    None => self.data = Some(data.to_string()),
                }
            }
        }
        events
    }
}

pub async fn subscribe<F>(
    backend: Backend,
    path: String,
    sender: UnboundedSender<Message>,
    mut handle: F,
) where
    F: FnMut(Delivery) -> Vec<Message>,
{
    let mut delay = Duration::from_secs(1);
    let mut opened = false;
    loop {
        match stream(&backend, &path, &sender, &mut handle, &mut opened).await {
            Ok(()) => delay = Duration::from_secs(1),
            Err(error) if error.signed_out() => {
                let _ = sender.send(Message::SignedOut);
                return;
            }
            Err(BackendError {
                status: Some(status),
                ..
            }) if status.is_client_error() => return,
            Err(_) => {}
        }
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(Duration::from_secs(10));
    }
}

async fn stream<F>(
    backend: &Backend,
    path: &str,
    sender: &UnboundedSender<Message>,
    handle: &mut F,
    opened: &mut bool,
) -> Result<(), BackendError>
where
    F: FnMut(Delivery) -> Vec<Message>,
{
    let request = backend.get_url(path).header("accept", "text/event-stream");
    let response = backend.send(request).await?;
    for message in handle(Delivery::Opened { again: *opened }) {
        let _ = sender.send(message);
    }
    *opened = true;
    let mut parser = Parser::default();
    let mut body = response.bytes_stream();
    while let Some(chunk) = body.next().await {
        for event in parser.feed(&chunk?) {
            for message in handle(Delivery::Event(event)) {
                let _ = sender.send(message);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(name: &str, data: &str) -> Event {
        Event {
            name: name.to_string(),
            data: data.to_string(),
        }
    }

    #[test]
    fn events_are_parsed_across_chunks() {
        let mut parser = Parser::default();
        assert_eq!(parser.feed(b":\n\nevent: acc"), []);
        assert_eq!(parser.feed(b"ount\ndata: {}\n"), []);
        assert_eq!(
            parser.feed(b"\nevent:recent\r\ndata:{\"a\":1}\r\n\r\n"),
            [event("account", "{}"), event("recent", "{\"a\":1}")]
        );
        assert_eq!(
            parser.feed(b"data: one\ndata: two\n\n"),
            [event("message", "one\ntwo")]
        );
        assert_eq!(parser.feed(b": keep alive\n\n"), []);
    }
}
