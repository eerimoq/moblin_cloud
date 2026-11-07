use super::*;

fn event(name: &str, data: &str) -> Event {
    Event {
        name: name.to_string(),
        data: data.to_string(),
    }
}

#[test]
fn events_are_parsed_across_chunks() {
    let mut parser = Parser::default();
    assert_eq!(parser.feed(b":\n\nevent: acc"), []);
    assert_eq!(parser.feed(b"ount\ndata: {}\n"), []);
    assert_eq!(
        parser.feed(b"\nevent:recent\r\ndata:{\"a\":1}\r\n\r\n"),
        [event("account", "{}"), event("recent", "{\"a\":1}")]
    );
    assert_eq!(
        parser.feed(b"data: one\ndata: two\n\n"),
        [event("message", "one\ntwo")]
    );
    assert_eq!(parser.feed(b": keep alive\n\n"), []);
}
