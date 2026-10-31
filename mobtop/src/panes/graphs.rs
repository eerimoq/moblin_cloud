use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::symbols::Marker;
use ratatui::text::Line;
use ratatui::widgets::{Axis, Block, Chart, Dataset, GraphType};

use crate::feed::{Feed, Sample};
use crate::ui::MUTED;

use super::note;

const SERIES_COLORS: [Color; 6] = [
    Color::Green,
    Color::Cyan,
    Color::Magenta,
    Color::Yellow,
    Color::Blue,
    Color::Red,
];

pub fn format_bits(bits: f64) -> String {
    if bits >= 1e6 {
        format!("{:.1} Mbps", bits / 1e6)
    } else {
        format!("{} kbps", (bits / 1e3).round())
    }
}

#[derive(Debug, PartialEq)]
pub struct Series {
    pub name: String,
    pub points: Vec<(f64, f64)>,
}

fn seconds(sample: &Sample, last: u64) -> f64 {
    -((last.saturating_sub(sample.at)) as f64 / 1000.0)
}

fn bonding_names(samples: &[Sample]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for bond in samples.iter().flat_map(|sample| &sample.bonding) {
        if !names.contains(&bond.name) {
            names.push(bond.name.clone());
        }
    }
    names
}

pub fn bitrate_series(samples: &[Sample]) -> Vec<Series> {
    let Some(last) = samples.last().map(|sample| sample.at) else {
        return Vec::new();
    };
    let total = Series {
        name: "Total".to_string(),
        points: samples
            .iter()
            .filter_map(|sample| Some((seconds(sample, last), sample.bitrate? as f64)))
            .collect(),
    };
    std::iter::once(total)
        .chain(bonding_names(samples).into_iter().map(|name| {
            Series {
                points: samples
                    .iter()
                    .filter_map(|sample| {
                        let bond = sample.bonding.iter().find(|bond| bond.name == name)?;
                        Some((seconds(sample, last), bond.bytes_per_second as f64 * 8.0))
                    })
                    .collect(),
                name,
            }
        }))
        .filter(|series| !series.points.is_empty())
        .collect()
}

pub fn rtt_series(samples: &[Sample]) -> Vec<Series> {
    let Some(last) = samples.last().map(|sample| sample.at) else {
        return Vec::new();
    };
    bonding_names(samples)
        .into_iter()
        .map(|name| Series {
            points: samples
                .iter()
                .filter_map(|sample| {
                    let bond = sample.bonding.iter().find(|bond| bond.name == name)?;
                    Some((seconds(sample, last), bond.rtt? as f64))
                })
                .collect(),
            name,
        })
        .filter(|series| !series.points.is_empty())
        .collect()
}

fn chart<'a>(
    title: &'a str,
    series: &'a [Series],
    format: fn(f64) -> String,
    first_color: Color,
) -> Chart<'a> {
    let max = series
        .iter()
        .flat_map(|series| series.points.iter().map(|(_, value)| *value))
        .fold(0.0, f64::max)
        .max(1.0)
        * 1.1;
    let color = |index: usize| {
        if index == 0 {
            first_color
        } else {
            SERIES_COLORS[(index - 1) % SERIES_COLORS.len()]
        }
    };
    let mut legend = vec![format!(" {title} ").bold()];
    for (index, series) in series.iter().enumerate() {
        if let Some((_, value)) = series.points.last() {
            legend.push(format!(" {} {} ", series.name, format(*value)).fg(color(index)));
        }
    }
    let datasets = series
        .iter()
        .enumerate()
        .map(|(index, series)| {
            Dataset::default()
                .marker(Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::new().fg(color(index)))
                .data(&series.points)
        })
        .collect();
    Chart::new(datasets)
        .block(Block::new().title(Line::from(legend)))
        .x_axis(
            Axis::default()
                .bounds([-60.0, 0.0])
                .labels(["-60 s".fg(MUTED), "now".fg(MUTED)]),
        )
        .y_axis(
            Axis::default()
                .bounds([0.0, max])
                .labels(["0".fg(MUTED), format(max).fg(MUTED)]),
        )
        .legend_position(None)
}

pub fn draw(frame: &mut Frame, area: Rect, feed: &Feed) {
    let samples: Vec<Sample> = feed.samples.iter().cloned().collect();
    let bitrate = bitrate_series(&samples);
    if bitrate.is_empty() {
        return note(
            frame,
            area,
            "No bitrate yet. It is shown while the stream is live.",
        );
    }
    let rtt = rtt_series(&samples);
    let format_ms = |ms: f64| format!("{} ms", ms.round());
    if rtt.is_empty() || area.height < 10 {
        frame.render_widget(chart("Bitrate", &bitrate, format_bits, Color::White), area);
        return;
    }
    let [top, bottom] =
        Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)]).areas(area);
    frame.render_widget(chart("Bitrate", &bitrate, format_bits, Color::White), top);
    frame.render_widget(
        chart("Round trip time", &rtt, format_ms, SERIES_COLORS[0]),
        bottom,
    );
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn sample(at: u64, bitrate: Option<i64>, bonding: Value) -> Sample {
        serde_json::from_value(json!({"at": at, "bitrate": bitrate, "bonding": bonding})).unwrap()
    }

    #[test]
    fn series_are_built_from_samples() {
        let samples = [
            sample(
                1000,
                Some(4_000_000),
                json!([{"name": "LTE", "bytesPerSecond": 250000, "rtt": 40}]),
            ),
            sample(
                2000,
                None,
                json!([{"name": "Wi-Fi", "bytesPerSecond": 125000, "rtt": null}, {"name": "LTE", "bytesPerSecond": 0, "rtt": 60}]),
            ),
        ];
        let bitrate = bitrate_series(&samples);
        let names: Vec<&str> = bitrate.iter().map(|series| series.name.as_str()).collect();
        assert_eq!(names, ["Total", "LTE", "Wi-Fi"]);
        assert_eq!(bitrate[0].points, [(-1.0, 4_000_000.0)]);
        assert_eq!(bitrate[1].points, [(-1.0, 2_000_000.0), (0.0, 0.0)]);
        assert_eq!(bitrate[2].points, [(0.0, 1_000_000.0)]);
        let rtt = rtt_series(&samples);
        assert_eq!(rtt.len(), 1);
        assert_eq!(rtt[0].points, [(-1.0, 40.0), (0.0, 60.0)]);
        assert_eq!(format_bits(4_260_000.0), "4.3 Mbps");
        assert_eq!(format_bits(512_400.0), "512 kbps");
    }
}
