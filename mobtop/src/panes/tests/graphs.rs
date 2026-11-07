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
