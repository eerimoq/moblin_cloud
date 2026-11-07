use super::*;

fn panels(pane: &Pane) -> Vec<Panel> {
    pane.leaves().iter().map(|(_, leaf)| leaf.panel).collect()
}

fn id_of(pane: &Pane, panel: Panel) -> u64 {
    pane.leaves()
        .iter()
        .find(|(_, leaf)| leaf.panel == panel)
        .unwrap()
        .1
        .id
}

#[test]
fn a_pane_keeps_its_device_when_its_panel_changes() {
    let mut leaf = Leaf::new(Panel::Phone);
    leaf.device = Some("d".to_string());
    leaf.hide_preview = true;
    let activity = leaf.with_panel(Panel::Activity);
    assert_eq!(activity.id, leaf.id);
    assert_eq!(
        serde_json::to_value(&activity).unwrap(),
        json!({"panel": "activity", "device": "d"})
    );
    assert_eq!(
        serde_json::to_value(activity.with_panel(Panel::Status)).unwrap(),
        json!({"panel": "status", "device": "d"})
    );
}

#[test]
fn layouts_are_sanitized_like_the_web() {
    let layout = default_layout();
    assert_eq!(
        serde_json::to_value(&layout).unwrap(),
        json!({
            "split": "row",
            "sizes": [0.6, 0.4],
            "children": [
                {"split": "column", "sizes": [0.65, 0.35], "children": [{"panel": "phone"}, {"panel": "graphs"}]},
                {"split": "column", "sizes": [0.7, 0.3], "children": [{"panel": "activity"}, {"panel": "status"}]},
            ],
        })
    );
    let sanitized = sanitize(&json!({
        "split": "row",
        "sizes": [3, 0.01, "x"],
        "children": [
            {"panel": "web", "url": "http://example.com", "twitchChat": true, "extra": 1},
            {"panel": "activity", "kinds": ["tip", "bogus"], "device": "d", "hidePreview": true, "window": true},
            {"split": "column", "children": [{"panel": "nope"}, {"panel": "toybox"}]},
            {"panel": "unknown"},
        ],
    }))
    .unwrap();
    let value = serde_json::to_value(&sanitized).unwrap();
    assert_eq!(
        value["children"],
        json!([
            {"panel": "web", "twitchChat": true},
            {"panel": "activity", "device": "d", "kinds": ["tip"], "hidePreview": true, "window": true},
            {"panel": "toybox"},
        ])
    );
    let sizes: Vec<f64> = value["sizes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|size| size.as_f64().unwrap())
        .collect();
    let third = 1.0 / 3.0;
    let total = 3.0 + 2.0 * third;
    for (size, expected) in sizes
        .iter()
        .zip([3.0 / total, third / total, third / total])
    {
        assert!((size - expected).abs() < 1e-9);
    }
    let mut deep = json!({"panel": "phone"});
    for _ in 0..9 {
        deep = json!({"split": "row", "children": [deep, {"panel": "status"}]});
    }
    let deep = panels(&sanitize(&deep).unwrap());
    assert_eq!(deep, [Panel::Status; 8]);
    let many = json!({"split": "row", "children": vec![json!({"panel": "status"}); 10]});
    let Some(Pane::Split(split)) = sanitize(&many) else {
        panic!();
    };
    assert_eq!(split.children.len(), 8);
    assert!(sanitize(&json!({"split": "row", "children": []})).is_none());
    assert!(sanitize(&json!("phone")).is_none());
}

#[test]
fn panes_split_and_are_removed_like_the_web() {
    let mut layout = default_layout();
    let graphs = id_of(&layout, Panel::Graphs);
    let (path, _) = layout.leaf(graphs).unwrap();
    let added = layout.added();
    assert_eq!(added.panel, Panel::Message);
    layout.split_at(&path, Direction::Column, added, false);
    let value = serde_json::to_value(&layout).unwrap();
    assert_eq!(value["children"][0]["sizes"], json!([0.65, 0.175, 0.175]));
    assert_eq!(
        value["children"][0]["children"][2],
        json!({"panel": "message"})
    );
    let added = layout.added();
    layout.split_at(&path, Direction::Row, added, false);
    let value = serde_json::to_value(&layout).unwrap();
    assert_eq!(
        value["children"][0]["children"][1],
        json!({"split": "row", "sizes": [0.5, 0.5], "children": [{"panel": "graphs"}, {"panel": "toybox"}]})
    );
    layout.remove_at(&[0, 1, 1]);
    let value = serde_json::to_value(&layout).unwrap();
    assert_eq!(
        value["children"][0]["children"][1],
        json!({"panel": "graphs"})
    );
    layout.remove_at(&[0, 0]);
    let value = serde_json::to_value(&layout).unwrap();
    assert_eq!(value["children"][0]["sizes"], json!([0.825, 0.175]));
    layout.remove_at(&[]);
    assert_eq!(panels(&layout).len(), 4);
    let mut single = Pane::Leaf(Leaf::new(Panel::Phone));
    single.split_at(&[], Direction::Row, Leaf::new(Panel::Status), true);
    assert_eq!(panels(&single), [Panel::Status, Panel::Phone]);
    single.remove_at(&[0]);
    assert_eq!(
        serde_json::to_value(&single).unwrap(),
        json!({"panel": "phone"})
    );
}

#[test]
fn panes_move_like_the_web() {
    let layout = default_layout();
    let (phone, status, activity) = (
        id_of(&layout, Panel::Phone),
        id_of(&layout, Panel::Status),
        id_of(&layout, Panel::Activity),
    );
    let swapped = layout.move_to(phone, status, Zone::Center).unwrap();
    assert_eq!(
        panels(&swapped),
        [Panel::Status, Panel::Graphs, Panel::Activity, Panel::Phone]
    );
    let moved = layout.move_to(phone, activity, Zone::Left).unwrap();
    let value = serde_json::to_value(&moved).unwrap();
    assert_eq!(value["children"][0], json!({"panel": "graphs"}));
    assert_eq!(
        value["children"][1]["children"][0],
        json!({"split": "row", "sizes": [0.5, 0.5], "children": [{"panel": "phone"}, {"panel": "activity"}]})
    );
    assert!(layout.move_to(phone, phone, Zone::Left).is_none());
    let edge = layout.move_to_edge(status, Zone::Left).unwrap();
    let value = serde_json::to_value(&edge).unwrap();
    assert_eq!(value["children"][0], json!({"panel": "status"}));
    let sizes: Vec<f64> = value["sizes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|size| size.as_f64().unwrap())
        .collect();
    assert_eq!(sizes.len(), 3);
    assert!((sizes[0] - 0.3).abs() < 1e-9 && (sizes[1] - 0.42).abs() < 1e-9);
    let bottom = layout.move_to_edge(status, Zone::Bottom).unwrap();
    let value = serde_json::to_value(&bottom).unwrap();
    assert_eq!(value["split"], "column");
    assert_eq!(value["sizes"], json!([0.7, 0.3]));
    assert_eq!(value["children"][1], json!({"panel": "status"}));
    let graphs = id_of(&layout, Panel::Graphs);
    let same = layout.move_to(graphs, phone, Zone::Bottom).unwrap();
    assert!(same.same_arrangement(&layout));
    let different = layout.move_to(graphs, phone, Zone::Top).unwrap();
    assert!(!different.same_arrangement(&layout));
}

#[test]
fn panes_resize_within_their_split() {
    let mut layout = default_layout();
    layout.resize(&[], 1, 0.99);
    let Pane::Split(split) = &layout else {
        panic!();
    };
    assert!((split.sizes[0] - 0.92).abs() < 1e-9 && (split.sizes[1] - 0.08).abs() < 1e-9);
    let mut layout = default_layout();
    let status = id_of(&layout, Panel::Status);
    layout.grow(status, 0.02);
    let value = serde_json::to_value(&layout).unwrap();
    let sizes = value["children"][1]["sizes"].as_array().unwrap();
    assert!((sizes[1].as_f64().unwrap() - 0.32).abs() < 1e-9);
    let phone = id_of(&layout, Panel::Phone);
    layout.grow(phone, -0.02);
    let value = serde_json::to_value(&layout).unwrap();
    let sizes = value["children"][0]["sizes"].as_array().unwrap();
    assert!((sizes[0].as_f64().unwrap() - 0.63).abs() < 1e-9);
}

#[test]
fn areas_follow_the_sizes() {
    let layout = default_layout();
    let (leaves, dividers) = areas(&layout, Rect::new(0, 1, 140, 40));
    let rects: Vec<Rect> = leaves.iter().map(|(_, rect)| *rect).collect();
    assert_eq!(
        rects,
        [
            Rect::new(0, 1, 84, 26),
            Rect::new(0, 27, 84, 14),
            Rect::new(84, 1, 56, 28),
            Rect::new(84, 29, 56, 12),
        ]
    );
    assert_eq!(dividers.len(), 3);
    assert_eq!(dividers[0].at, 27);
    assert_eq!(dividers[1].split, Vec::<usize>::new());
    assert_eq!(dividers[1].at, 84);
    assert_eq!(zone_at(Rect::new(0, 0, 10, 10), 5, 5), Zone::Center);
    assert_eq!(zone_at(Rect::new(0, 0, 10, 10), 0, 5), Zone::Left);
    assert_eq!(zone_at(Rect::new(0, 0, 10, 10), 5, 9), Zone::Bottom);
}
