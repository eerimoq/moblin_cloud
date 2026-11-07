use super::*;
use crate::database;

fn tip() -> Item {
    Item::Tip {
        user: "Viewer".to_string(),
        message: "Hi".to_string(),
        amount: 500,
        currency: "USD".to_string(),
    }
}

#[test]
fn effects_are_not_read_back_as_tips() {
    let effect = Item::Effect {
        user: "Viewer".to_string(),
        message: String::new(),
        effect: "fart-2".to_string(),
    };
    for item in [effect, tip(), spin()] {
        let json = serde_json::to_string(&item).unwrap();
        assert_eq!(serde_json::from_str::<Item>(&json).unwrap(), item);
    }
}

fn spin() -> Item {
    Item::Spin {
        user: "Viewer".to_string(),
        message: String::new(),
        spins: 2,
    }
}

fn request_ids(deliveries: &Deliveries) -> Vec<String> {
    deliveries
        .pending()
        .into_iter()
        .map(|delivery| delivery.request_id)
        .collect()
}

#[test]
fn deliveries_are_saved() {
    let path = database::tests::temporary_path("deliveries");
    let deliveries = Deliveries::new(Database::open(Some(&path)).unwrap());
    deliveries.add_test("S".to_string(), "A".to_string(), spin());
    deliveries.add_test("S".to_string(), "B".to_string(), tip());
    deliveries.add_test("S".to_string(), "C".to_string(), spin());
    deliveries.add_test("S".to_string(), "C".to_string(), tip());
    finish(&deliveries.database.lock(), "A");
    assert!(deliveries.get("A").unwrap().finished);
    let saved = deliveries.pending();
    let deliveries = Deliveries::new(Database::open(Some(&path)).unwrap());
    assert_eq!(deliveries.pending(), saved);
    assert_eq!(
        saved
            .iter()
            .map(|delivery| &delivery.item)
            .collect::<Vec<_>>(),
        vec![&tip(), &spin()]
    );
    database::tests::remove(&path);
}

#[test]
fn only_the_5_newest_tests_per_kind_are_kept() {
    let deliveries = Deliveries::new(Database::memory());
    for n in 0..7 {
        deliveries.add_test("S".to_string(), format!("T{n}"), tip());
    }
    deliveries.add_test("S".to_string(), "SPIN".to_string(), spin());
    deliveries.add_test("O".to_string(), "OTHER".to_string(), tip());
    insert(&deliveries.database.lock(), "S", "PAID", &tip()).unwrap();
    let ids = |streamer: &str| {
        query(
            &deliveries.database.lock(),
            "SELECT * FROM deliveries WHERE streamer = ?1 AND test IS NOT NULL ORDER BY rowid DESC",
            [streamer],
        )
        .into_iter()
        .map(|delivery| delivery.request_id)
        .collect::<Vec<_>>()
    };
    assert_eq!(ids("S"), vec!["SPIN", "T6", "T5", "T4", "T3", "T2"]);
    assert_eq!(ids("O"), vec!["OTHER"]);
    assert!(deliveries.get("PAID").is_some());
}

#[test]
fn resends_start_over_with_a_new_moblin_request_id() {
    let deliveries = Deliveries::new(Database::memory());
    insert(&deliveries.database.lock(), "S", "PAID", &tip()).unwrap();
    deliveries.delivered("PAID", None, "phone", &["phone".to_string()]);
    let delivery = deliveries.get("PAID").unwrap();
    assert!(delivery.finished);
    assert_eq!(delivery.moblin_request_id(), "PAID");
    deliveries.resend("S", "PAID", &tip()).unwrap();
    let delivery = deliveries.get("PAID").unwrap();
    assert!(!delivery.finished);
    assert!(delivery.delivered.is_empty());
    assert_eq!(delivery.resends, 1);
    assert_eq!(delivery.moblin_request_id(), "PAID-1");
    assert!(deliveries.failed(&delivery, "moblin-error"));
    deliveries.resend("S", "PAID", &tip()).unwrap();
    let delivery = deliveries.get("PAID").unwrap();
    assert_eq!(delivery.failure, None);
    assert_eq!(delivery.moblin_request_id(), "PAID-2");
    assert_eq!(request_ids(&deliveries), vec!["PAID"]);
    deliveries.resend("S", "GONE", &spin()).unwrap();
    assert_eq!(
        deliveries.get("GONE").unwrap().moblin_request_id(),
        "GONE-1"
    );
}

#[test]
fn deliveries_are_given_up_after_7_days() {
    let deliveries = Deliveries::new(Database::memory());
    for (request_id, item, age) in [
        ("C", tip(), 8),
        ("D", tip(), 6),
        ("E", spin(), 8),
        ("F", spin(), 6),
        ("G", tip(), 8),
    ] {
        deliveries.add_test("S".to_string(), request_id.to_string(), item);
        deliveries
            .database
            .lock()
            .execute(
                "UPDATE deliveries SET requested_at = ?1 WHERE request_id = ?2",
                params![now() - age * 24 * 60 * 60, request_id],
            )
            .unwrap();
    }
    deliveries.delivered(
        "G",
        None,
        "phone",
        &["phone".to_string(), "tablet".to_string()],
    );
    deliveries.expire();
    assert_eq!(request_ids(&deliveries), vec!["D", "F"]);
    let expiries: Vec<(String, u64)> = deliveries
        .database
        .lock()
        .prepare_cached("SELECT name, value FROM counters ORDER BY name")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        expiries,
        [
            ("spinTestExpiries".to_string(), 1),
            ("tipTestExpiries".to_string(), 1),
        ]
    );
}

#[test]
fn only_deliveries_no_device_got_are_counted() {
    let deliveries = Deliveries::new(Database::memory());
    deliveries.add_test("S".to_string(), "A".to_string(), tip());
    deliveries.add_test("S".to_string(), "B".to_string(), tip());
    assert_eq!(deliveries.count(), 2);
    deliveries.delivered(
        "A",
        None,
        "phone",
        &["phone".to_string(), "tablet".to_string()],
    );
    assert!(!deliveries.get("A").unwrap().finished);
    assert_eq!(deliveries.count(), 1);
}

#[test]
fn failures_are_counted_once() {
    let deliveries = Deliveries::new(Database::memory());
    deliveries.add_test("S".to_string(), "A".to_string(), spin());
    deliveries.add_test("S".to_string(), "B".to_string(), tip());
    let pending = deliveries.pending();
    assert!(deliveries.failed(&pending[0], "moblin-error"));
    assert!(!deliveries.failed(&pending[0], "moblin-error"));
    assert!(deliveries.failed(&pending[1], "moblin-error"));
    assert!(!deliveries.failed(&pending[1], "moblin-error"));
    assert_eq!(request_ids(&deliveries), vec!["A", "B"]);
    assert_eq!(
        deliveries.get("B").unwrap().failure.as_deref(),
        Some("moblin-error")
    );
    deliveries
        .database
        .lock()
        .execute(
            "UPDATE deliveries SET requested_at = ?1",
            [now() - 8 * 24 * 60 * 60],
        )
        .unwrap();
    deliveries.expire();
    assert!(deliveries.pending().is_empty());
    let counters: Vec<(String, u64)> = deliveries
        .database
        .lock()
        .prepare_cached("SELECT name, value FROM counters ORDER BY name")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        counters,
        [
            ("spinTestDeliveriesFailed:moblin-error".to_string(), 1),
            ("spinTestExpiries".to_string(), 1),
            ("tipTestDeliveriesFailed:moblin-error".to_string(), 1),
            ("tipTestExpiries".to_string(), 1),
        ]
    );
}

impl Deliveries {
    pub fn get(&self, request_id: &str) -> Option<Delivery> {
        query(
            &self.database.lock(),
            "SELECT * FROM deliveries WHERE request_id = ?1",
            [request_id],
        )
        .pop()
    }
}
