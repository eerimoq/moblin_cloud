use super::*;

fn item(id: &str, at: u64) -> Item {
    serde_json::from_value(serde_json::json!({
        "id": id, "at": at, "user": "Erik", "amount": 500, "currency": "USD", "status": "delivered",
    }))
    .unwrap()
}

fn recent(items: Vec<Item>, more: bool) -> Option<Recent> {
    Some(Recent { items, more })
}

#[test]
fn activity_merges_kinds_by_time_without_gaps() {
    let latest = [
        recent(vec![item("t3", 30), item("t1", 10)], false),
        recent(vec![item("s4", 40), item("s2", 20)], true),
        recent(vec![item("e3", 30)], false),
    ];
    let ids = |kinds| {
        activity(&latest, kinds)
            .into_iter()
            .map(|(_, item)| item.id.as_str())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids([true; 3]), ["s4", "t3", "e3", "s2"]);
    assert_eq!(ids([true, false, true]), ["t3", "e3", "t1"]);
    assert_eq!(activity(&latest, [false, true, false])[0].0, Kind::Spin);
    let mut leaf = Leaf::new(Panel::Activity);
    assert_eq!(kinds_of(&leaf), [true; 3]);
    leaf.kinds = Some(vec!["spin".to_string()]);
    assert_eq!(kinds_of(&leaf), [false, true, false]);
}

fn account(devices: serde_json::Value) -> Account {
    serde_json::from_value(serde_json::json!({
        "name": "Erik", "twitch": null, "kick": null, "devices": devices,
        "settings": {"id": "page"}, "remoteControl": "control", "preview": true,
    }))
    .unwrap()
}

#[test]
fn streams_follow_the_panes_devices() {
    let devices = serde_json::json!([
        {"id": "a", "name": "Moblin 1", "connected": false, "reconnecting": false},
        {"id": "b", "name": "Moblin 2", "connected": true, "reconnecting": false},
    ]);
    let mut monitor = Monitor::new(account(devices));
    monitor.layout = layout::sanitize(&serde_json::json!({
        "split": "row",
        "children": [
            {"panel": "phone"},
            {"panel": "status", "device": "a"},
            {"panel": "phone", "device": "a", "hidePreview": true},
            {"panel": "activity", "device": "a"},
        ],
    }));
    let (devices, previews) = wanted_streams(&monitor);
    assert_eq!(devices, HashSet::from(["a".to_string(), "b".to_string()]));
    assert_eq!(previews, HashSet::from(["b".to_string()]));
    monitor.layout = layout::sanitize(&serde_json::json!({"panel": "graphs", "device": "gone"}));
    let (devices, previews) = wanted_streams(&monitor);
    assert_eq!(devices, HashSet::from(["b".to_string()]));
    assert!(previews.is_empty());
    monitor.page.as_mut().unwrap().remote_control = "none".to_string();
    assert!(wanted_streams(&monitor).0.is_empty());
    let rects = [
        (1, Rect::new(0, 0, 10, 10)),
        (2, Rect::new(10, 0, 10, 5)),
        (3, Rect::new(10, 5, 10, 5)),
    ];
    assert_eq!(neighbor(&rects, 1, Zone::Right), Some(2));
    assert_eq!(neighbor(&rects, 2, Zone::Bottom), Some(3));
    assert_eq!(neighbor(&rects, 3, Zone::Left), Some(1));
    assert_eq!(neighbor(&rects, 1, Zone::Left), None);
}

#[test]
fn the_channel_picker_lines_up_its_addresses() {
    let own: Account = serde_json::from_value(serde_json::json!({
        "name": "Erik", "twitch": {"login": "eerimoq"}, "kick": null, "devices": [],
        "settings": {"id": "page"}, "remoteControl": "control", "preview": true,
        "assisting": [
            {"id": "a", "platform": "twitch", "name": "Streamer with a long name", "login": "streamer"},
            {"id": "b", "platform": "kick", "name": "Bo", "login": "bo"},
        ],
    }))
    .unwrap();
    let mut monitor = Monitor::new(own);
    monitor.modal = Some(Modal::Menu(Menu::new(0, None, MenuKind::Channels)));
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 20)).unwrap();
    terminal
        .draw(|frame| crate::panes::draw(frame, frame.area(), &monitor))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let rows: Vec<String> = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect()
        })
        .collect();
    let columns: Vec<usize> = ["twitch/eerimoq", "twitch/streamer", "kick/bo"]
        .iter()
        .map(|address| {
            let row = rows.iter().find(|row| row.contains(address)).unwrap();
            row[..row.find(address).unwrap()].chars().count()
        })
        .collect();
    assert_eq!(columns[0], columns[1]);
    assert_eq!(columns[1], columns[2]);
}
