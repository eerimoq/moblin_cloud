use super::*;

#[test]
fn kinds_are_counted_apart_and_old_months_pruned() {
    let allowances = Allowances::new(Database::memory());
    let take = |kind, allowance| {
        allowances
            .take("p", "v", kind, allowance, |_| Ok(()))
            .unwrap()
    };
    assert_eq!(take("effect", 2), Some(1));
    assert_eq!(take("effect", 2), Some(0));
    assert_eq!(take("effect", 2), None);
    assert_eq!(take("spin", 1), Some(0));
    assert_eq!(take("spin", 1), None);
    assert_eq!(allowances.left("p", "v", "effect", 3).unwrap(), 1);
    assert_eq!(allowances.left("p", "other", "effect", 3).unwrap(), 3);
    assert_eq!(allowances.left("other", "v", "effect", 3).unwrap(), 3);
    allowances.last_month();
    assert_eq!(take("effect", 3), Some(2));
    allowances.prune();
    let rows: u32 = allowances
        .database
        .lock()
        .query_row("SELECT count(*) FROM subscriber_allowances", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(rows, 1);
    assert_eq!(allowances.left("p", "v", "effect", 3).unwrap(), 2);
    assert_eq!(allowances.left("p", "v", "spin", 1).unwrap(), 1);
}

#[test]
fn nothing_is_counted_when_saving_the_order_fails() {
    let allowances = Allowances::new(Database::memory());
    assert!(
        allowances
            .take("p", "v", "effect", 2, |_| Err(
                rusqlite::Error::InvalidQuery
            ))
            .is_err()
    );
    assert_eq!(allowances.left("p", "v", "effect", 2).unwrap(), 2);
}

impl Allowances {
    pub fn last_month(&self) {
        self.database
            .lock()
            .execute(
                "UPDATE subscriber_allowances SET month = strftime('%Y-%m', 'now', 'start of month', '-1 month')",
                [],
            )
            .unwrap();
    }
}
