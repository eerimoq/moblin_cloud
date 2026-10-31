use std::sync::atomic::{AtomicU64, Ordering};

use ratatui::layout::Rect;
use serde::Serialize;
use serde_json::{Value, json};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Panel {
    Phone,
    Activity,
    Graphs,
    Status,
    Message,
    Toybox,
    Web,
}

impl Panel {
    pub const ALL: [Panel; 7] = [
        Panel::Phone,
        Panel::Activity,
        Panel::Graphs,
        Panel::Status,
        Panel::Message,
        Panel::Toybox,
        Panel::Web,
    ];

    fn parse(name: &str) -> Option<Self> {
        Panel::ALL.into_iter().find(|panel| {
            serde_json::to_value(panel)
                .ok()
                .as_ref()
                .and_then(Value::as_str)
                == Some(name)
        })
    }

    pub fn title(self) -> &'static str {
        match self {
            Panel::Phone => "Phone",
            Panel::Activity => "Activity",
            Panel::Graphs => "Graphs",
            Panel::Status => "Status",
            Panel::Message => "Message to the streamer",
            Panel::Toybox => "Toybox",
            Panel::Web => "Web page",
        }
    }

    pub fn has_device(self) -> bool {
        matches!(
            self,
            Panel::Phone | Panel::Graphs | Panel::Status | Panel::Message
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Row,
    Column,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Left,
    Right,
    Top,
    Bottom,
    Center,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leaf {
    #[serde(skip)]
    pub id: u64,
    pub panel: Panel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kinds: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub twitch_chat: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub hide_preview: bool,
}

impl Leaf {
    pub fn new(panel: Panel) -> Self {
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            panel,
            device: None,
            kinds: None,
            url: None,
            twitch_chat: false,
            hide_preview: false,
        }
    }

    pub fn shows(&self, kind: &str) -> bool {
        self.kinds
            .as_ref()
            .is_none_or(|kinds| kinds.iter().any(|shown| shown == kind))
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Split {
    pub split: Direction,
    pub sizes: Vec<f64>,
    pub children: Vec<Pane>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum Pane {
    Split(Split),
    Leaf(Leaf),
}

fn normalized(sizes: Vec<f64>) -> Vec<f64> {
    let total: f64 = sizes.iter().sum();
    sizes.into_iter().map(|size| size / total).collect()
}

pub fn sanitize(value: &Value) -> Option<Pane> {
    sanitize_at(value, 0)
}

fn sanitize_at(value: &Value, depth: usize) -> Option<Pane> {
    let pane = value.as_object()?;
    if depth > 8 {
        return None;
    }
    let direction = match pane.get("split").and_then(Value::as_str) {
        Some("row") => Some(Direction::Row),
        Some("column") => Some(Direction::Column),
        _ => None,
    };
    if let Some(direction) = direction {
        let mut children: Vec<Pane> = pane
            .get("children")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|child| sanitize_at(child, depth + 1))
            .take(8)
            .collect();
        if children.len() <= 1 {
            return children.pop();
        }
        let sizes = pane.get("sizes").and_then(Value::as_array);
        let count = children.len() as f64;
        let sizes = (0..children.len())
            .map(|index| {
                sizes
                    .and_then(|sizes| sizes.get(index))
                    .and_then(Value::as_f64)
                    .filter(|size| *size > 0.05)
                    .unwrap_or(1.0 / count)
            })
            .collect();
        return Some(Pane::Split(Split {
            split: direction,
            sizes: normalized(sizes),
            children,
        }));
    }
    let mut leaf = Leaf::new(Panel::parse(pane.get("panel")?.as_str()?)?);
    leaf.device = pane
        .get("device")
        .and_then(Value::as_str)
        .map(str::to_string);
    leaf.kinds = pane.get("kinds").and_then(Value::as_array).map(|kinds| {
        kinds
            .iter()
            .filter_map(Value::as_str)
            .filter(|kind| ["tip", "spin", "effect"].contains(kind))
            .map(str::to_string)
            .collect()
    });
    leaf.url = pane
        .get("url")
        .and_then(Value::as_str)
        .filter(|url| url.starts_with("https://"))
        .map(str::to_string);
    leaf.twitch_chat = pane.get("twitchChat") == Some(&Value::Bool(true));
    leaf.hide_preview = pane.get("hidePreview") == Some(&Value::Bool(true));
    Some(Pane::Leaf(leaf))
}

pub fn default_layout() -> Pane {
    sanitize(&json!({
        "split": "row",
        "sizes": [0.6, 0.4],
        "children": [
            {"split": "column", "sizes": [0.65, 0.35], "children": [{"panel": "phone"}, {"panel": "graphs"}]},
            {"split": "column", "sizes": [0.7, 0.3], "children": [{"panel": "activity"}, {"panel": "status"}]},
        ],
    }))
    .unwrap()
}

impl Pane {
    pub fn leaves(&self) -> Vec<(Vec<usize>, &Leaf)> {
        let mut leaves = Vec::new();
        self.collect(&mut Vec::new(), &mut leaves);
        leaves
    }

    fn collect<'a>(&'a self, path: &mut Vec<usize>, leaves: &mut Vec<(Vec<usize>, &'a Leaf)>) {
        match self {
            Pane::Leaf(leaf) => leaves.push((path.clone(), leaf)),
            Pane::Split(split) => {
                for (index, child) in split.children.iter().enumerate() {
                    path.push(index);
                    child.collect(path, leaves);
                    path.pop();
                }
            }
        }
    }

    pub fn at(&self, path: &[usize]) -> Option<&Pane> {
        match path.split_first() {
            None => Some(self),
            Some((index, rest)) => match self {
                Pane::Split(split) => split.children.get(*index)?.at(rest),
                Pane::Leaf(_) => None,
            },
        }
    }

    pub fn at_mut(&mut self, path: &[usize]) -> Option<&mut Pane> {
        match path.split_first() {
            None => Some(self),
            Some((index, rest)) => match self {
                Pane::Split(split) => split.children.get_mut(*index)?.at_mut(rest),
                Pane::Leaf(_) => None,
            },
        }
    }

    pub fn leaf(&self, id: u64) -> Option<(Vec<usize>, &Leaf)> {
        self.leaves().into_iter().find(|(_, leaf)| leaf.id == id)
    }

    pub fn leaf_mut(&mut self, id: u64) -> Option<&mut Leaf> {
        let (path, _) = self.leaf(id)?;
        match self.at_mut(&path)? {
            Pane::Leaf(leaf) => Some(leaf),
            Pane::Split(_) => None,
        }
    }

    pub fn added(&self) -> Leaf {
        let used: Vec<Panel> = self.leaves().iter().map(|(_, leaf)| leaf.panel).collect();
        Leaf::new(
            Panel::ALL
                .into_iter()
                .find(|panel| !used.contains(panel))
                .unwrap_or(Panel::Web),
        )
    }

    fn update(&mut self, path: &[usize], update: impl FnOnce(Pane) -> Pane) {
        if let Some(pane) = self.at_mut(path) {
            let old = std::mem::replace(pane, Pane::Leaf(Leaf::new(Panel::Web)));
            *pane = update(old);
        }
    }

    pub fn split_at(&mut self, path: &[usize], direction: Direction, added: Leaf, before: bool) {
        let Some((index, parent_path)) = path.split_last() else {
            self.update(&[], |leaf| wrap(direction, leaf, added, before));
            return;
        };
        let index = *index;
        if let Some(Pane::Split(parent)) = self.at_mut(parent_path)
            && parent.split == direction
        {
            let half = parent.sizes[index] / 2.0;
            parent.sizes.splice(index..=index, [half, half]);
            parent
                .children
                .insert(if before { index } else { index + 1 }, Pane::Leaf(added));
            return;
        }
        self.update(path, |leaf| wrap(direction, leaf, added, before));
    }

    pub fn remove_at(&mut self, path: &[usize]) {
        let Some((index, parent_path)) = path.split_last() else {
            return;
        };
        let index = *index;
        self.update(parent_path, |parent| {
            let Pane::Split(mut parent) = parent else {
                return parent;
            };
            parent.children.remove(index);
            if parent.children.len() == 1 {
                return parent.children.pop().unwrap();
            }
            let removed = parent.sizes.remove(index);
            parent.sizes[index.saturating_sub(1)] += removed;
            Pane::Split(parent)
        });
    }

    pub fn move_to(&self, from: u64, to: u64, zone: Zone) -> Option<Pane> {
        let (from_path, source) = self.leaf(from)?;
        let (to_path, target) = self.leaf(to)?;
        if from == to {
            return None;
        }
        let (source, target) = (source.clone(), target.clone());
        let mut moved = self.clone();
        if zone == Zone::Center {
            *moved.at_mut(&from_path)? = Pane::Leaf(target);
            *moved.at_mut(&to_path)? = Pane::Leaf(source);
            return Some(moved);
        }
        moved.remove_at(&from_path);
        let (path, _) = moved.leaf(to)?;
        moved.split_at(
            &path,
            if matches!(zone, Zone::Left | Zone::Right) {
                Direction::Row
            } else {
                Direction::Column
            },
            source,
            matches!(zone, Zone::Left | Zone::Top),
        );
        Some(moved)
    }

    pub fn move_to_edge(&self, from: u64, edge: Zone) -> Option<Pane> {
        let (from_path, source) = self.leaf(from)?;
        if from_path.is_empty() || edge == Zone::Center {
            return None;
        }
        let source = Pane::Leaf(source.clone());
        let mut rest = self.clone();
        rest.remove_at(&from_path);
        let direction = if matches!(edge, Zone::Left | Zone::Right) {
            Direction::Row
        } else {
            Direction::Column
        };
        let first = matches!(edge, Zone::Left | Zone::Top);
        if let Pane::Split(split) = &mut rest
            && split.split == direction
        {
            for size in &mut split.sizes {
                *size *= 0.7;
            }
            if first {
                split.sizes.insert(0, 0.3);
                split.children.insert(0, source);
            } else {
                split.sizes.push(0.3);
                split.children.push(source);
            }
            return Some(rest);
        }
        Some(Pane::Split(Split {
            split: direction,
            sizes: if first {
                vec![0.3, 0.7]
            } else {
                vec![0.7, 0.3]
            },
            children: if first {
                vec![source, rest]
            } else {
                vec![rest, source]
            },
        }))
    }

    fn arrangement(&self) -> String {
        match self {
            Pane::Leaf(leaf) => leaf.id.to_string(),
            Pane::Split(split) => {
                let name = match split.split {
                    Direction::Row => "row",
                    Direction::Column => "column",
                };
                let parts: Vec<String> = split
                    .children
                    .iter()
                    .map(|child| match child {
                        Pane::Split(inner) if inner.split == split.split => {
                            let text = child.arrangement();
                            text[name.len() + 1..text.len() - 1].to_string()
                        }
                        _ => child.arrangement(),
                    })
                    .collect();
                format!("{name}({})", parts.join(","))
            }
        }
    }

    pub fn same_arrangement(&self, other: &Pane) -> bool {
        self.arrangement() == other.arrangement()
    }

    pub fn resize(&mut self, split_path: &[usize], index: usize, first: f64) {
        let Some(Pane::Split(split)) = self.at_mut(split_path) else {
            return;
        };
        if index == 0 || index >= split.sizes.len() {
            return;
        }
        let pair = split.sizes[index - 1] + split.sizes[index];
        let first = first.clamp(0.08, (pair - 0.08).max(0.08));
        split.sizes[index - 1] = first;
        split.sizes[index] = pair - first;
    }

    pub fn grow(&mut self, leaf: u64, step: f64) {
        let Some((path, _)) = self.leaf(leaf) else {
            return;
        };
        let Some((index, parent)) = path.split_last() else {
            return;
        };
        let Some(Pane::Split(split)) = self.at(parent) else {
            return;
        };
        let (index, size) = (*index, split.sizes[*index]);
        if index + 1 < split.sizes.len() {
            self.resize(parent, index + 1, size + step);
        } else if index > 0 {
            let before = split.sizes[index - 1];
            self.resize(parent, index, before - step);
        }
    }
}

fn wrap(direction: Direction, pane: Pane, added: Leaf, before: bool) -> Pane {
    Pane::Split(Split {
        split: direction,
        sizes: vec![0.5, 0.5],
        children: if before {
            vec![Pane::Leaf(added), pane]
        } else {
            vec![pane, Pane::Leaf(added)]
        },
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct Divider {
    pub split: Vec<usize>,
    pub index: usize,
    pub direction: Direction,
    pub area: Rect,
    pub at: u16,
}

pub fn areas(pane: &Pane, area: Rect) -> (Vec<(u64, Rect)>, Vec<Divider>) {
    let mut leaves = Vec::new();
    let mut dividers = Vec::new();
    lay_out(pane, area, &mut Vec::new(), &mut leaves, &mut dividers);
    (leaves, dividers)
}

fn lay_out(
    pane: &Pane,
    area: Rect,
    path: &mut Vec<usize>,
    leaves: &mut Vec<(u64, Rect)>,
    dividers: &mut Vec<Divider>,
) {
    match pane {
        Pane::Leaf(leaf) => leaves.push((leaf.id, area)),
        Pane::Split(split) => {
            let (start, length) = match split.split {
                Direction::Row => (area.x, area.width),
                Direction::Column => (area.y, area.height),
            };
            let mut total = 0.0;
            let mut from = start;
            for (index, child) in split.children.iter().enumerate() {
                total += split.sizes[index];
                let to = if index + 1 == split.children.len() {
                    start + length
                } else {
                    start + (f64::from(length) * total).round() as u16
                };
                let child_area = match split.split {
                    Direction::Row => Rect::new(from, area.y, to.saturating_sub(from), area.height),
                    Direction::Column => {
                        Rect::new(area.x, from, area.width, to.saturating_sub(from))
                    }
                };
                if index > 0 {
                    dividers.push(Divider {
                        split: path.clone(),
                        index,
                        direction: split.split,
                        area,
                        at: from,
                    });
                }
                path.push(index);
                lay_out(child, child_area, path, leaves, dividers);
                path.pop();
                from = to;
            }
        }
    }
}

pub fn zone_at(area: Rect, column: u16, row: u16) -> Zone {
    let x = (f64::from(column.saturating_sub(area.x)) + 0.5) / f64::from(area.width.max(1));
    let y = (f64::from(row.saturating_sub(area.y)) + 0.5) / f64::from(area.height.max(1));
    if x > 0.3 && x < 0.7 && y > 0.3 && y < 0.7 {
        return Zone::Center;
    }
    [
        (Zone::Left, x),
        (Zone::Right, 1.0 - x),
        (Zone::Top, y),
        (Zone::Bottom, 1.0 - y),
    ]
    .into_iter()
    .min_by(|a, b| a.1.total_cmp(&b.1))
    .map(|(zone, _)| zone)
    .unwrap_or(Zone::Center)
}

#[cfg(test)]
mod tests {
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
                {"panel": "activity", "kinds": ["tip", "bogus"], "device": "d", "hidePreview": true},
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
                {"panel": "activity", "device": "d", "kinds": ["tip"], "hidePreview": true},
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
}
