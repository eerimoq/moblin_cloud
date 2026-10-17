use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Result, bail};
use tokio::time::Instant;

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Song { link: String, name: String },
    Tip { message: Option<String> },
    Spin { spins: u32, message: Option<String> },
}

impl Item {
    pub fn kind(&self) -> &'static str {
        match self {
            Item::Song { .. } => "song",
            Item::Tip { .. } => "tip",
            Item::Spin { .. } => "spin",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Payee {
    PayPal(String),
    Stripe(String),
}

impl Payee {
    pub fn provider(&self) -> &'static str {
        match self {
            Payee::PayPal(_) => "PayPal",
            Payee::Stripe(_) => "Stripe",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    pub streamer: String,
    pub item: Item,
    pub user: String,
    pub payee: Payee,
    pub price: u32,
    pub currency: String,
}

impl Order {
    pub fn description(&self) -> String {
        let (description, message) = match &self.item {
            Item::Song { name, .. } => (format!("Song request: {name}"), None),
            Item::Tip { message } => (format!("Tip from {}", self.user), message.as_ref()),
            Item::Spin { spins: 1, message } => {
                (format!("Wheel spin by {}", self.user), message.as_ref())
            }
            Item::Spin { spins, message } => (
                format!("{spins} wheel spins by {}", self.user),
                message.as_ref(),
            ),
        };
        let description = match message {
            Some(message) => format!("{description}: {message}"),
            None => description,
        };
        description.chars().take(127).collect()
    }
}

#[derive(Debug, PartialEq)]
pub enum CaptureError {
    NotApproved,
    AlreadyCaptured,
    Pending,
    Failed,
    Unexpected,
}

pub struct Orders {
    orders: Mutex<HashMap<String, (Instant, Order)>>,
}

impl Orders {
    pub fn new() -> Self {
        Self {
            orders: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, order_id: &str) -> Option<Order> {
        self.orders
            .lock()
            .unwrap()
            .get(order_id)
            .map(|(_, order)| order.clone())
    }

    pub fn make_room(&self) -> Result<()> {
        let mut orders = self.orders.lock().unwrap();
        orders.retain(|_, (created, _)| created.elapsed() < Duration::from_secs(3 * 60 * 60));
        if orders.len() >= 10000 {
            bail!("too many unfinished payments");
        }
        Ok(())
    }

    pub fn insert(&self, order_id: String, order: Order) {
        self.orders
            .lock()
            .unwrap()
            .insert(order_id, (Instant::now(), order));
    }

    pub fn remove(&self, order_id: &str) {
        self.orders.lock().unwrap().remove(order_id);
    }
}
