use super::*;

#[test]
fn messages_are_counted_per_kind() {
    let traffic = Traffic::default();
    for text in [
        r#"{"ping":{}}"#,
        r#"{"ping":{}}"#,
        r#"{"event":{"data":{"status":{"topRight":{}}}}}"#,
        r#"{"response":{"id":1,"result":{"ok":{}}}}"#,
        r#"{"response":{"id":2,"result":{"error":{}}}}"#,
        "not json",
    ] {
        traffic.received(&unread(text), text.len());
    }
    traffic.sent("getStatus", 10);
    traffic.sent("pong", 11);
    assert_eq!(
        traffic.stats(),
        TrafficStats {
            sent: vec![
                MessageKind {
                    kind: "getStatus".to_string(),
                    messages: 1,
                    bytes: 10,
                },
                MessageKind {
                    kind: "pong".to_string(),
                    messages: 1,
                    bytes: 11,
                },
            ],
            received: vec![
                MessageKind {
                    kind: "ping".to_string(),
                    messages: 2,
                    bytes: 22,
                },
                MessageKind {
                    kind: "invalid".to_string(),
                    messages: 1,
                    bytes: 8,
                },
                MessageKind {
                    kind: "response:error".to_string(),
                    messages: 1,
                    bytes: 43,
                },
                MessageKind {
                    kind: "response:ok".to_string(),
                    messages: 1,
                    bytes: 40,
                },
                MessageKind {
                    kind: "status".to_string(),
                    messages: 1,
                    bytes: 45,
                },
            ],
        }
    );
}

#[test]
fn socket_traffic_is_totalled_per_direction() {
    let socket_traffic = SocketTraffic::default();
    socket_traffic.sent(10);
    socket_traffic.sent(5);
    socket_traffic.received(7);
    assert_eq!(
        socket_traffic.totals(),
        Totals {
            sent: Direction {
                messages: 2,
                bytes: 15,
            },
            received: Direction {
                messages: 1,
                bytes: 7,
            },
        }
    );
}

#[test]
fn kinds_are_capped() {
    let traffic = Traffic::default();
    for index in 0..120 {
        traffic.received(&format!("kind{index}"), 1);
    }
    traffic.received("kind5", 1);
    traffic.received(&unread(r#"{"event":{"data":{"a-b":{}}}}"#), 1);
    let received = traffic.stats().received;
    assert_eq!(received.len(), 101);
    assert_eq!(received[0].kind, "other");
    assert_eq!(received[0].messages, 21);
    assert_eq!(
        received
            .iter()
            .find(|kind| kind.kind == "kind5")
            .unwrap()
            .messages,
        2
    );
}
