use super::*;
use crate::database;

#[test]
fn settings_are_saved() {
    let path = database::tests::temporary_path("pages");
    let pages = Pages::new(Database::open(Some(&path)).unwrap());
    let tips = Tips { amounts: vec![250] };
    let settings = Settings {
        currency: Some("SEK".to_string()),
        tips: Some(Feature::on(tips.clone())),
        message: Some("Hi".to_string()),
        ..Settings::default()
    };
    pages.set("a", settings.clone()).unwrap();
    pages.set("b", settings).unwrap();
    pages.set("b", Settings::default()).unwrap();
    let pages = Pages::new(Database::open(Some(&path)).unwrap());
    assert_eq!(pages.get("a").tips(), Some(&tips));
    assert_eq!(pages.get("a").currency(), "SEK");
    assert_eq!(pages.get("a").message.as_deref(), Some("Hi"));
    assert_eq!(pages.get("b"), Settings::default());
    let count: u32 = pages
        .database
        .lock()
        .query_row("SELECT count(*) FROM pages", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
    database::tests::remove(&path);
}

#[test]
fn turned_off_features_keep_their_values() {
    let pages = Pages::new(Database::memory());
    let off = |amount| Settings {
        tips: Some(Feature {
            enabled: false,
            value: Tips {
                amounts: vec![amount],
            },
        }),
        sub_spins: Some(Feature {
            enabled: false,
            value: Tiers {
                tier1: 1,
                tier2: 2,
                tier3: 3,
            },
        }),
        ..Settings::default()
    };
    assert_eq!(pages.set("a", off(250)), Ok(off(250)));
    let settings = pages.get("a");
    assert_eq!(settings, off(250));
    assert_eq!(settings.tips(), None);
    assert_eq!(settings.sub_spins(), None);
    assert_eq!(pages.set("a", off(49)), Err(SettingsError::InvalidTips));
}

#[test]
fn effects_need_a_valid_price() {
    let pages = Pages::new(Database::memory());
    let effects = |price| Settings {
        effects: Some(Feature::on(Effects { price })),
        ..Settings::default()
    };
    assert_eq!(pages.set("a", effects(300)), Ok(effects(300)));
    assert_eq!(pages.get("a").effects(), Some(&Effects { price: 300 }));
    assert_eq!(
        pages.set("a", effects(49)),
        Err(SettingsError::InvalidEffects)
    );
}

#[test]
fn images_are_saved() {
    let path = database::tests::temporary_path("page-images");
    let pages = Pages::new(Database::open(Some(&path)).unwrap());
    assert_eq!(pages.look("a").image, None);
    let first = pages.set_image("a", b"first").unwrap();
    assert_eq!(first.len(), 16);
    let second = pages.set_image("a", b"second").unwrap();
    assert_ne!(first, second);
    pages.set_image("b", b"first").unwrap();
    pages.remove_image("b").unwrap();
    pages.set("a", Settings::default()).unwrap();
    let pages = Pages::new(Database::open(Some(&path)).unwrap());
    assert_eq!(pages.look("a").image, Some(second.clone()));
    assert_eq!(pages.image("a"), Some((second, b"second".to_vec())));
    assert_eq!(pages.image("b"), None);
    database::tests::remove(&path);
}

#[test]
fn currencies_are_validated() {
    let pages = Pages::new(Database::memory());
    let settings = |currency: &str| Settings {
        currency: Some(currency.to_string()),
        ..Settings::default()
    };
    assert_eq!(pages.set("a", settings(" sek ")), Ok(settings("SEK")));
    assert_eq!(pages.get("a").currency(), "SEK");
    assert_eq!(pages.set("a", settings("usd")), Ok(Settings::default()));
    assert_eq!(pages.get("a").currency(), "USD");
    assert_eq!(
        pages.set("a", settings("JPY")),
        Err(SettingsError::InvalidCurrency)
    );
}

#[test]
fn tips_are_validated() {
    let pages = Pages::new(Database::memory());
    let tips = |amounts: &[u32]| Settings {
        tips: Some(Feature::on(Tips {
            amounts: amounts.to_vec(),
        })),
        ..Settings::default()
    };
    assert_eq!(
        pages.set("a", tips(&[1000, 200, 500, 200])).unwrap(),
        tips(&[200, 500, 1000])
    );
    assert_eq!(pages.get("a").tips().unwrap().amounts, [200, 500, 1000]);
    for amounts in [
        &[][..],
        &[100, 200, 300, 400, 500, 600],
        &[49, 200],
        &[10_000_001],
    ] {
        assert_eq!(
            pages.set("a", tips(amounts)),
            Err(SettingsError::InvalidTips)
        );
    }
}

#[test]
fn spins_are_validated() {
    let pages = Pages::new(Database::memory());
    let spin = |options: &[(u32, u32)]| Settings {
        spin: Some(Feature::on(Spin {
            options: options
                .iter()
                .map(|(spins, price)| SpinOption {
                    spins: *spins,
                    price: *price,
                })
                .collect(),
        })),
        ..Settings::default()
    };
    assert_eq!(
        pages.set("a", spin(&[(2, 800), (1, 500)])),
        Ok(spin(&[(1, 500), (2, 800)]))
    );
    assert_eq!(
        pages.set("a", spin(&[(1, 800), (1, 500), (1, 500)])),
        Ok(spin(&[(1, 500), (1, 500), (1, 800)]))
    );
    for options in [
        &[(1, 49)][..],
        &[(0, 500)],
        &[(11, 500)],
        &[],
        &[(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)],
    ] {
        assert_eq!(
            pages.set("a", spin(options)),
            Err(SettingsError::InvalidSpin)
        );
    }
}

#[test]
fn sub_spins_are_validated() {
    let pages = Pages::new(Database::memory());
    let sub_spins = |tier1, tier2, tier3| Settings {
        sub_spins: Some(Feature::on(Tiers {
            tier1,
            tier2,
            tier3,
        })),
        ..Settings::default()
    };
    assert_eq!(pages.set("a", sub_spins(1, 5, 10)), Ok(sub_spins(1, 5, 10)));
    assert_eq!(
        pages.set("a", sub_spins(0, 0, 100)),
        Ok(sub_spins(0, 0, 100))
    );
    for (tier1, tier2, tier3) in [(0, 0, 0), (101, 5, 10), (1, 5, 101)] {
        assert_eq!(
            pages.set("a", sub_spins(tier1, tier2, tier3)),
            Err(SettingsError::InvalidTiers)
        );
    }
    let sub_spins = sub_spins(1, 0, 10).sub_spins.unwrap().value;
    assert_eq!(
        [None, Some(1), Some(2), Some(3)].map(|tier| sub_spins.allowance(tier)),
        [0, 1, 0, 10]
    );
}

#[test]
fn pages_are_validated() {
    let pages = Pages::new(Database::memory());
    let page = |message: &str, accent: &str, font: &str| Settings {
        message: Some(message.to_string()),
        accent: Some(accent.to_string()),
        font: Some(font.to_string()),
        ..Settings::default()
    };
    assert_eq!(
        pages.set("a", page(" Hi! ", "purple", "serif")),
        Ok(Settings {
            message: Some("Hi!".to_string()),
            accent: Some("purple".to_string()),
            font: Some("serif".to_string()),
            ..Settings::default()
        })
    );
    assert_eq!(
        pages.set("a", page("", "#A1B2c3", "rounded")),
        Ok(Settings {
            accent: Some("#a1b2c3".to_string()),
            ..Settings::default()
        })
    );
    assert_eq!(
        pages.set("a", page("  ", "green", "rounded")),
        Ok(Settings::default())
    );
    assert_eq!(pages.get("a"), Settings::default());
    assert!(
        pages
            .set("a", page(&"å".repeat(300), "red", "pixel"))
            .is_ok()
    );
    for (message, accent, font) in [
        ("a".repeat(301), "red", "mono"),
        (String::new(), "teal", "mono"),
        (String::new(), "#12345", "mono"),
        (String::new(), "#12345g", "mono"),
        (String::new(), "123456", "mono"),
        (String::new(), "red", "comic"),
    ] {
        assert_eq!(
            pages.set("a", page(&message, accent, font)),
            Err(SettingsError::InvalidPage)
        );
    }
}

#[test]
fn prices_are_formatted() {
    assert_eq!(format_price(50), "0.50");
    assert_eq!(format_price(205), "2.05");
    assert_eq!(format_price(10_000_000), "100000.00");
}

impl<T> Feature<T> {
    pub fn on(value: T) -> Self {
        Self {
            enabled: true,
            value,
        }
    }
}
