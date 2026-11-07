use super::*;

#[test]
fn qr_codes_are_drawn_with_half_blocks() {
    let url = "https://cloud.moblin.app/connect/?code=KQMRXDBT";
    let lines = qr_lines(url);
    let size = qrcode::QrCode::new(url).unwrap().width() + 8;
    assert_eq!(lines.len(), size.div_ceil(2));
    assert!(lines.iter().all(|line| line.chars().count() == size));
}

#[test]
fn times_are_relative() {
    assert_eq!(ago(5), "just now");
    assert_eq!(ago(125), "2 min ago");
    assert_eq!(ago(7200), "2 h ago");
    assert_eq!(ago(86_400), "1 day ago");
    assert_eq!(ago(3 * 86_400), "3 days ago");
}
