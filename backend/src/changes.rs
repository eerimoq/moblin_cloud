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
    pub fn subscribe(&self, page: &str) -> broadcast::Receiver<Change> {
        let mut pages = self.pages.lock().unwrap();
        pages.retain(|_, sender| sender.receiver_count() > 0);
        pages
            .entry(page.to_string())
            .or_insert_with(|| broadcast::channel(16).0)
            .subscribe()
    }

    pub fn notify(&self, page: &str, change: Change) {
        if let Some(sender) = self.pages.lock().unwrap().get(page) {
            let _ = sender.send(change);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribers_get_the_changes_of_their_page() {
        let changes = Changes::default();
        let mut first = changes.subscribe("A");
        let mut second = changes.subscribe("B");
        changes.notify("A", Change::Recent);
        changes.notify("C", Change::Page);
        assert_eq!(first.try_recv().unwrap(), Change::Recent);
        assert!(first.try_recv().is_err());
        assert!(second.try_recv().is_err());
        drop(first);
        drop(second);
        changes.subscribe("C");
        assert_eq!(changes.pages.lock().unwrap().len(), 1);
    }
}
