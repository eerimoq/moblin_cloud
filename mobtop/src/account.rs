use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub name: String,
    pub twitch: Option<Identity>,
    pub kick: Option<Identity>,
    pub devices: Vec<Device>,
    pub settings: Settings,
    #[serde(default)]
    pub assisting: Vec<Assisting>,
    pub remote_control: String,
    pub preview: bool,
    #[serde(default)]
    pub go_live: bool,
}

impl Account {
    pub fn address(&self) -> Option<String> {
        [("twitch", &self.twitch), ("kick", &self.kick)]
            .into_iter()
            .find_map(|(platform, identity)| {
                identity
                    .as_ref()
                    .map(|identity| format!("{platform}/{}", identity.login))
            })
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Identity {
    pub login: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub connected: bool,
    pub reconnecting: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Settings {
    pub id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Assisting {
    pub id: String,
    pub platform: String,
    pub name: String,
    pub login: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Tip,
    Spin,
    Effect,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Tip, Kind::Spin, Kind::Effect];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Tip => "tip",
            Kind::Spin => "spin",
            Kind::Effect => "effect",
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Recent {
    pub items: Vec<Item>,
    pub more: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub at: u64,
    pub user: String,
    pub amount: u32,
    pub currency: String,
    pub message: Option<String>,
    pub spins: Option<u32>,
    pub effect: Option<String>,
    pub tier: Option<u8>,
    #[serde(default)]
    pub kick_subscriber: bool,
    pub status: String,
    #[serde(default)]
    pub devices: Vec<Received>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Received {
    pub name: String,
    pub received: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connect {
    pub device_code: String,
    pub user_code: String,
    pub url: String,
    pub interval: u64,
}

pub fn price(amount: u32, currency: &str) -> String {
    format!("{}.{:02} {currency}", amount / 100, amount % 100)
}

pub fn effect_name(id: &str) -> &str {
    match id {
        "fart" => "Fart",
        "fart-2" => "Fart 2",
        "bad-chili-fart" => "Bad chili fart",
        "perfect-fart" => "Perfect fart",
        _ => id,
    }
}

impl Item {
    pub fn amount_label(&self) -> String {
        if self.kick_subscriber {
            "Kick subscriber".to_string()
        } else if let Some(tier) = self.tier {
            format!("Tier {tier} subscriber")
        } else {
            price(self.amount, &self.currency)
        }
    }
}

pub fn merge(page: Recent, old: &Recent) -> Recent {
    let last = page.items.last().map(|item| item.id.clone());
    let index = if page.more {
        old.items
            .iter()
            .position(|item| Some(&item.id) == last.as_ref())
    } else {
        None
    };
    match index {
        Some(index) if index + 1 < old.items.len() => Recent {
            items: page
                .items
                .into_iter()
                .chain(old.items[index + 1..].iter().cloned())
                .collect(),
            more: old.more,
        },
        _ => page,
    }
}

pub fn append(old: &mut Recent, page: Recent) {
    for item in page.items {
        if !old.items.iter().any(|old| old.id == item.id) {
            old.items.push(item);
        }
    }
    old.more = page.more;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> Item {
        Item {
            id: id.to_string(),
            at: 0,
            user: "Erik".to_string(),
            amount: 500,
            currency: "USD".to_string(),
            message: None,
            spins: None,
            effect: None,
            tier: None,
            kick_subscriber: false,
            status: "delivered".to_string(),
            devices: Vec::new(),
        }
    }

    fn ids(recent: &Recent) -> Vec<&str> {
        recent.items.iter().map(|item| item.id.as_str()).collect()
    }

    #[test]
    fn prices_are_hundredths() {
        assert_eq!(price(500, "USD"), "5.00 USD");
        assert_eq!(price(1234, "SEK"), "12.34 SEK");
        assert_eq!(price(7, "EUR"), "0.07 EUR");
        let mut tier = item("a");
        tier.tier = Some(2);
        assert_eq!(tier.amount_label(), "Tier 2 subscriber");
    }

    #[test]
    fn refreshed_items_keep_older_loaded_ones() {
        let old = Recent {
            items: vec![item("c"), item("b"), item("a")],
            more: false,
        };
        let page = Recent {
            items: vec![item("d"), item("c")],
            more: true,
        };
        let merged = merge(page, &old);
        assert_eq!(ids(&merged), ["d", "c", "b", "a"]);
        assert!(!merged.more);
        let page = Recent {
            items: vec![item("d")],
            more: false,
        };
        assert_eq!(ids(&merge(page, &old)), ["d"]);
        let mut old = old;
        append(
            &mut old,
            Recent {
                items: vec![item("a"), item("z")],
                more: true,
            },
        );
        assert_eq!(ids(&old), ["c", "b", "a", "z"]);
        assert!(old.more);
    }
}
