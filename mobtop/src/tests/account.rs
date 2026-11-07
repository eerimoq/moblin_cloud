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

#[test]
fn only_configured_platforms_are_listed() {
    let platforms = Platforms {
        twitch: false,
        kick: true,
    };
    assert_eq!(platforms.configured(), vec![Platform::Kick]);
    let platforms = Platforms {
        twitch: true,
        kick: true,
    };
    assert_eq!(
        platforms.configured(),
        vec![Platform::Twitch, Platform::Kick]
    );
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
