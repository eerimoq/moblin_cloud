use super::*;

#[test]
fn bump_and_read() {
    let database = Database::memory();
    let counters = Counters::new(database.clone());
    assert!(counters.all().is_empty());
    counters.bump("signIns");
    counters.bump("signIns");
    bump(&database.lock(), "livePosts");
    database
        .lock()
        .execute(
            "INSERT INTO counters (name, day, value) VALUES ('signIns', date('now', '-3 days'), 5), ('signIns', date('now', '-10 days'), 7)",
            [],
        )
        .unwrap();
    assert_eq!(
        counters.all(),
        [
            Counter {
                name: "livePosts".to_string(),
                today: 1,
                week: 1,
                total: 1,
            },
            Counter {
                name: "signIns".to_string(),
                today: 2,
                week: 7,
                total: 14,
            },
        ]
    );
}
