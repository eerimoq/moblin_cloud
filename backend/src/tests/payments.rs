use super::*;

fn order() -> Order {
    Order {
        streamer: "p".to_string(),
        item: Item::Tip {
            message: Some("Hi".to_string()),
        },
        user: "Viewer".to_string(),
        payee: Payee::PayPal("MERCHANT00001".to_string()),
        price: 700,
        currency: "USD".to_string(),
    }
}

#[test]
fn orders_survive_restarts() {
    let database = Database::memory();
    let orders = Orders::new(database.clone(), DEADLINE);
    orders.make_room().unwrap();
    orders.insert("ORDER1", &order()).unwrap();
    assert!(
        orders
            .transition("ORDER1", Stage::Created, Stage::Capturing)
            .unwrap()
    );
    let orders = Orders::new(database, DEADLINE);
    let saved = Saved {
        id: "ORDER1".to_string(),
        stage: Stage::Capturing,
        order: order(),
        late: false,
    };
    assert_eq!(orders.get("ORDER1"), Some(saved.clone()));
    assert_eq!(orders.get("ORDER2"), None);
    assert_eq!(orders.unfinished(), 1);
    assert_eq!(orders.unsettled(Duration::ZERO), vec![saved]);
    assert!(orders.unsettled(Duration::from_secs(60)).is_empty());
}

#[test]
fn transitions_only_from_the_expected_state() {
    let orders = Orders::new(Database::memory(), DEADLINE);
    orders.insert("ORDER1", &order()).unwrap();
    assert!(
        !orders
            .transition("ORDER1", Stage::Capturing, Stage::Created)
            .unwrap()
    );
    assert!(
        orders
            .transition("ORDER1", Stage::Created, Stage::Capturing)
            .unwrap()
    );
    assert!(
        !orders
            .transition("ORDER1", Stage::Created, Stage::Capturing)
            .unwrap()
    );
    assert!(
        !orders
            .transition("ORDER2", Stage::Created, Stage::Capturing)
            .unwrap()
    );
    orders
        .captured("ORDER1", |connection| {
            connection.execute(
                "INSERT INTO counters (name, day, value) VALUES ('x', 'd', 1)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    assert!(
        !orders
            .captured("ORDER1", |_| panic!("captured twice"))
            .unwrap()
    );
    assert_eq!(orders.get("ORDER1").unwrap().stage, Stage::Captured);
    assert_eq!(orders.unfinished(), 0);
    assert!(orders.unsettled(Duration::ZERO).is_empty());
    orders.remove("ORDER1");
    assert_eq!(orders.get("ORDER1"), None);
}

#[test]
fn a_failed_capture_record_keeps_the_order_capturing() {
    let orders = Orders::new(Database::memory(), DEADLINE);
    orders.insert("ORDER1", &order()).unwrap();
    orders
        .transition("ORDER1", Stage::Created, Stage::Capturing)
        .unwrap();
    assert!(
        orders
            .captured("ORDER1", |connection| {
                connection.execute("INSERT INTO nowhere VALUES (1)", [])?;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(orders.get("ORDER1").unwrap().stage, Stage::Capturing);
}

#[test]
fn prune_gives_old_orders_up_and_forgets_old_captures() {
    let database = Database::memory();
    let orders = Orders::new(database.clone(), DEADLINE);
    for id in [
        "OLD_CAPTURING",
        "OLD_CAPTURED",
        "NEW_CAPTURING",
        "NEW_CAPTURED",
    ] {
        orders.insert(id, &order()).unwrap();
    }
    for id in ["OLD_CAPTURING", "NEW_CAPTURING"] {
        assert!(
            orders
                .transition(id, Stage::Created, Stage::Capturing)
                .unwrap()
        );
    }
    for id in ["OLD_CAPTURED", "NEW_CAPTURED"] {
        orders.captured(id, |_| Ok(())).unwrap();
    }
    database
        .lock()
        .execute(
            "UPDATE orders SET created_at = created_at - 31 * 24 * 60 * 60, updated_at = updated_at - 31 * 24 * 60 * 60 WHERE id LIKE 'OLD%'",
            [],
        )
        .unwrap();
    orders.prune();
    for id in ["OLD_CAPTURING", "OLD_CAPTURED"] {
        assert_eq!(orders.get(id), None, "{id}");
    }
    assert_eq!(orders.get("NEW_CAPTURING").unwrap().stage, Stage::Capturing);
    assert_eq!(orders.get("NEW_CAPTURED").unwrap().stage, Stage::Captured);
    assert_eq!(orders.unsettled(Duration::ZERO).len(), 1);
}

#[test]
fn created_orders_expire_after_30_minutes() {
    let database = Database::memory();
    let orders = Orders::new(database.clone(), DEADLINE);
    orders.insert("OLD", &order()).unwrap();
    orders.insert("NEW", &order()).unwrap();
    database
        .lock()
        .execute(
            "UPDATE orders SET created_at = created_at - 31 * 60 WHERE id = 'OLD'",
            [],
        )
        .unwrap();
    assert!(orders.get("OLD").unwrap().late);
    assert!(!orders.get("NEW").unwrap().late);
    assert_eq!(orders.expire(), vec![("OLD".to_string(), order())]);
    assert_eq!(orders.get("OLD"), None);
    assert!(orders.get("NEW").is_some());
}

impl Orders {
    pub fn age(&self, id: &str, seconds: u64) {
        self.database
            .lock()
            .execute(
                "UPDATE orders SET created_at = created_at - ?2, updated_at = updated_at - ?2 WHERE id = ?1",
                params![id, seconds],
            )
            .unwrap();
    }

    pub fn read_only(&self, on: bool) {
        self.database
            .lock()
            .pragma_update(None, "query_only", on)
            .unwrap();
    }
}
