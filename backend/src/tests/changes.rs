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
