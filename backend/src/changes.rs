use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::broadcast;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Change {
    Page,
    Account,
    Recent,
}

#[derive(Default)]
pub struct Changes {
    pages: Mutex<HashMap<String, broadcast::Sender<Change>>>,
}

impl Changes {
    pub fn subscribe(&self, page_id: &str) -> broadcast::Receiver<Change> {
        let mut pages = self.pages.lock().unwrap();
        pages.retain(|_, sender| sender.receiver_count() > 0);
        pages
            .entry(page_id.to_string())
            .or_insert_with(|| broadcast::channel(16).0)
            .subscribe()
    }

    pub fn notify(&self, page_id: &str, change: Change) {
        if let Some(sender) = self.pages.lock().unwrap().get(page_id) {
            let _ = sender.send(change);
        }
    }
}

#[cfg(test)]
#[path = "tests/changes.rs"]
mod tests;
