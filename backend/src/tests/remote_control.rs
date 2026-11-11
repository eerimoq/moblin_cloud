use super::*;

fn encode(message: &MessageToStreamer) -> String {
    serde_json::to_string(message).unwrap()
}

fn decode(text: &str) -> MessageToAssistant {
    serde_json::from_str(text).unwrap()
}

#[test]
fn messages_to_streamer_match_moblin() {
    assert_eq!(encode(&MessageToStreamer::Pong {}), r#"{"pong":{}}"#);
    assert_eq!(
        encode(&MessageToStreamer::Identified {
            result: Outcome::Ok {}
        }),
        r#"{"identified":{"result":{"ok":{}}}}"#
    );
    assert_eq!(
        encode(&MessageToStreamer::Hello {
            api_version: "0.1".to_string(),
            authentication: Authentication {
                challenge: "c".to_string(),
                salt: "s".to_string(),
            },
        }),
        r#"{"hello":{"apiVersion":"0.1","authentication":{"challenge":"c","salt":"s"}}}"#
    );
    let tip = |id, kind| {
        encode(&MessageToStreamer::Request {
            id,
            data: Request::Tip {
                name: "Erik".to_string(),
                message: "Hi".to_string(),
                kind,
                request_id: "ORDER1".to_string(),
            },
        })
    };
    assert_eq!(
        tip(
            7,
            TipKind::Message {
                amount: "7.00".to_string(),
                currency: "USD".to_string(),
            }
        ),
        r#"{"request":{"id":7,"data":{"tip":{"name":"Erik","message":"Hi","kind":{"message":{"amount":"7.00","currency":"USD"}},"requestId":"ORDER1"}}}}"#
    );
    assert_eq!(
        tip(
            8,
            TipKind::Effect {
                sound: "Bad chili fart".to_string(),
            }
        ),
        r#"{"request":{"id":8,"data":{"tip":{"name":"Erik","message":"Hi","kind":{"effect":{"sound":"Bad chili fart"}},"requestId":"ORDER1"}}}}"#
    );
    assert_eq!(
        tip(9, TipKind::WheelOfLuck { count: 3 }),
        r#"{"request":{"id":9,"data":{"tip":{"name":"Erik","message":"Hi","kind":{"wheelOfLuck":{"count":3}},"requestId":"ORDER1"}}}}"#
    );
    let request = |id, data| encode(&MessageToStreamer::Request { id, data });
    assert_eq!(
        request(10, Request::GetSettings {}),
        r#"{"request":{"id":10,"data":{"getSettings":{}}}}"#
    );
    let id = Uuid::parse_str("3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B").unwrap();
    let editor = |control| request(20, Request::Control(control));
    assert_eq!(
        editor(Control::GetScenes {}),
        r#"{"request":{"id":20,"data":{"getScenes":{}}}}"#
    );
    assert_eq!(
        editor(Control::GetWidgets { secrets: true }),
        r#"{"request":{"id":20,"data":{"getWidgets":{"secrets":true}}}}"#
    );
    assert_eq!(
        editor(Control::GetVideoSources {}),
        r#"{"request":{"id":20,"data":{"getVideoSources":{}}}}"#
    );
    assert_eq!(
        editor(Control::GetAlertsMediaGallery {}),
        r#"{"request":{"id":20,"data":{"getAlertsMediaGallery":{}}}}"#
    );
    assert_eq!(
        editor(Control::GetEditorOptions {}),
        r#"{"request":{"id":20,"data":{"getEditorOptions":{}}}}"#
    );
    assert_eq!(
        editor(Control::CreateScene {
            scene: serde_json::json!({"id": "3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B", "name": "Remote"})
        }),
        r#"{"request":{"id":20,"data":{"createScene":{"scene":{"id":"3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B","name":"Remote"}}}}}"#
    );
    assert_eq!(
        editor(Control::UpdateScene {
            scene: serde_json::json!({"id": "3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B"})
        }),
        r#"{"request":{"id":20,"data":{"updateScene":{"scene":{"id":"3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B"}}}}}"#
    );
    assert_eq!(
        editor(Control::DeleteScene { id }),
        r#"{"request":{"id":20,"data":{"deleteScene":{"id":"3f2b8c1e-5a4d-4e7f-9b6a-1c2d3e4f5a6b"}}}}"#
    );
    assert_eq!(
        editor(Control::GetStream { id, secrets: false }),
        r#"{"request":{"id":20,"data":{"getStream":{"id":"3f2b8c1e-5a4d-4e7f-9b6a-1c2d3e4f5a6b","secrets":false}}}}"#
    );
    assert_eq!(
        editor(Control::CreateStream {
            stream: serde_json::json!({"id": "3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B", "name": "Kick"}),
            secrets: true
        }),
        r#"{"request":{"id":20,"data":{"createStream":{"stream":{"id":"3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B","name":"Kick"},"secrets":true}}}}"#
    );
    assert_eq!(
        editor(Control::UpdateStream {
            stream: serde_json::json!({"id": "3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B"}),
            secrets: false
        }),
        r#"{"request":{"id":20,"data":{"updateStream":{"stream":{"id":"3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B"},"secrets":false}}}}"#
    );
    assert_eq!(
        editor(Control::DeleteStream { id }),
        r#"{"request":{"id":20,"data":{"deleteStream":{"id":"3f2b8c1e-5a4d-4e7f-9b6a-1c2d3e4f5a6b"}}}}"#
    );
    assert_eq!(
        editor(Control::CreateWidget {
            widget: serde_json::json!({"id": "3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B", "type": "Text"}),
            scene_ids: vec![id],
            secrets: true,
        }),
        r#"{"request":{"id":20,"data":{"createWidget":{"widget":{"id":"3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B","type":"Text"},"sceneIds":["3f2b8c1e-5a4d-4e7f-9b6a-1c2d3e4f5a6b"],"secrets":true}}}}"#
    );
    assert_eq!(
        editor(Control::UpdateWidget {
            widget: serde_json::json!({"id": "3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B"}),
            secrets: false
        }),
        r#"{"request":{"id":20,"data":{"updateWidget":{"widget":{"id":"3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B"},"secrets":false}}}}"#
    );
    assert_eq!(
        editor(Control::DeleteWidget { id }),
        r#"{"request":{"id":20,"data":{"deleteWidget":{"id":"3f2b8c1e-5a4d-4e7f-9b6a-1c2d3e4f5a6b"}}}}"#
    );
    assert_eq!(
        editor(Control::SetScoreboardPlayers {
            players: serde_json::json!([{"id": "3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B", "name": "Erik"}])
        }),
        r#"{"request":{"id":20,"data":{"setScoreboardPlayers":{"players":[{"id":"3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B","name":"Erik"}]}}}}"#
    );
    assert_eq!(
        editor(Control::GetVideoSourceSnapshot { id }),
        r#"{"request":{"id":20,"data":{"getVideoSourceSnapshot":{"id":"3f2b8c1e-5a4d-4e7f-9b6a-1c2d3e4f5a6b"}}}}"#
    );
    assert_eq!(
        request(
            11,
            Request::StartStatus {
                interval: 1,
                filter: StatusFilter { top_right: true }
            }
        ),
        r#"{"request":{"id":11,"data":{"startStatus":{"interval":1,"filter":{"topRight":true}}}}}"#
    );
    assert_eq!(
        request(12, Request::StopStatus {}),
        r#"{"request":{"id":12,"data":{"stopStatus":{}}}}"#
    );
    assert_eq!(
        request(19, Request::GetStatus {}),
        r#"{"request":{"id":19,"data":{"getStatus":{}}}}"#
    );
    let control = |id, control| request(id, Request::Control(control));
    assert_eq!(
        control(13, Control::SetMute { on: true }),
        r#"{"request":{"id":13,"data":{"setMute":{"on":true}}}}"#
    );
    assert_eq!(
        control(14, Control::SetZoom { x: 2.5 }),
        r#"{"request":{"id":14,"data":{"setZoom":{"x":2.5}}}}"#
    );
    assert_eq!(
        control(
            15,
            Control::SetScene {
                id: "E621E1F8-C36C-495A-93FC-0C247A3E6E5F".parse().unwrap()
            }
        ),
        r#"{"request":{"id":15,"data":{"setScene":{"id":"e621e1f8-c36c-495a-93fc-0c247a3e6e5f"}}}}"#
    );
    assert_eq!(
        control(
            17,
            Control::SetSrtConnectionPriority {
                id: "0c4a1f9e-6b1d-4d8a-9f3e-2a7c5b8d1e60".parse().unwrap(),
                priority: 3,
                enabled: false
            }
        ),
        r#"{"request":{"id":17,"data":{"setSrtConnectionPriority":{"id":"0c4a1f9e-6b1d-4d8a-9f3e-2a7c5b8d1e60","priority":3,"enabled":false}}}}"#
    );
    assert_eq!(
        control(18, Control::ReloadBrowserWidgets {}),
        r#"{"request":{"id":18,"data":{"reloadBrowserWidgets":{}}}}"#
    );
    assert_eq!(
        control(
            19,
            serde_json::from_str(r#"{"setAutoSceneSwitcher":{}}"#).unwrap()
        ),
        r#"{"request":{"id":19,"data":{"setAutoSceneSwitcher":{}}}}"#
    );
    assert_eq!(
        control(
            20,
            Control::SetFilter {
                filter: Filter::GrayScale {},
                on: true
            }
        ),
        r#"{"request":{"id":20,"data":{"setFilter":{"filter":{"grayScale":{}},"on":true}}}}"#
    );
    assert_eq!(
        encode(&MessageToStreamer::Request {
            id: 24,
            data: Request::StartPreview {},
        }),
        r#"{"request":{"id":24,"data":{"startPreview":{}}}}"#
    );
    assert!(
        serde_json::from_str::<Control>(r#"{"setFilter":{"filter":"movie","on":true}}"#).is_err()
    );
    assert!(serde_json::from_str::<Control>(r#"{"setStream":{"id":"A"}}"#).is_err());
    assert!(serde_json::from_str::<Control>(r#"{"setScene":{"id":"C2"}}"#).is_err());
    assert!(serde_json::from_str::<Control>(r#"{"setMic":{"id":"bt"}}"#).is_ok());
    assert!(serde_json::from_str::<Control>(r#"{"getStatus":{}}"#).is_err());
    assert!(serde_json::from_str::<Control>(r#"{"tip":{}}"#).is_err());
    assert_eq!(
        control(
            21,
            Control::SendMessage {
                text: "Hi!".to_string(),
                name: None
            }
        ),
        r#"{"request":{"id":21,"data":{"sendMessage":{"text":"Hi!"}}}}"#
    );
    assert_eq!(
        control(
            22,
            Control::SendMessage {
                text: "Hi!".to_string(),
                name: Some("Erik".to_string())
            }
        ),
        r#"{"request":{"id":22,"data":{"sendMessage":{"text":"Hi!","name":"Erik"}}}}"#
    );
    assert_eq!(
        control(
            23,
            serde_json::from_str(r#"{"setMoblinCloudWheelOfLuckWidget":{}}"#).unwrap()
        ),
        r#"{"request":{"id":23,"data":{"setMoblinCloudWheelOfLuckWidget":{}}}}"#
    );
    assert_eq!(
        control(
            24,
            Control::SetMoblinCloudAlertsWidget {
                id: Some(Uuid::parse_str("0c4a1f9e-6b1d-4d8a-9f3e-2a7c5b8d1e60").unwrap())
            }
        ),
        r#"{"request":{"id":24,"data":{"setMoblinCloudAlertsWidget":{"id":"0c4a1f9e-6b1d-4d8a-9f3e-2a7c5b8d1e60"}}}}"#
    );
    assert!(
        serde_json::from_str::<Control>(r#"{"setMoblinCloudWheelOfLuckWidget":{"id":"W"}}"#)
            .is_err()
    );
    assert!(
        serde_json::from_str::<Control>(r#"{"setMoblinCloudWheelOfLuckWidget":{}}"#)
            .unwrap()
            .changes_settings()
    );
    assert_eq!(
        control(
            25,
            Control::SetStream {
                id: Uuid::parse_str("0c4a1f9e-6b1d-4d8a-9f3e-2a7c5b8d1e60").unwrap()
            }
        ),
        r#"{"request":{"id":25,"data":{"setStream":{"id":"0c4a1f9e-6b1d-4d8a-9f3e-2a7c5b8d1e60"}}}}"#
    );
    assert!(
        serde_json::from_str::<Control>(
            r#"{"setStream":{"id":"0c4a1f9e-6b1d-4d8a-9f3e-2a7c5b8d1e60"}}"#
        )
        .unwrap()
        .changes_settings()
    );
    let message = |text: &str| Control::SendMessage {
        text: text.to_string(),
        name: None,
    };
    assert!(message("Hi!").valid());
    assert!(!message(" ").valid());
    assert!(message(&"å".repeat(300)).valid());
    assert!(!message(&"å".repeat(301)).valid());
}

#[test]
fn messages_to_assistant_match_moblin() {
    assert_eq!(decode(r#"{"ping":{}}"#), MessageToAssistant::Ping {});
    assert_eq!(
        decode(r#"{"preview":{"preview":"/9j/"}}"#),
        MessageToAssistant::Preview {
            preview: "/9j/".to_string()
        }
    );
    assert_eq!(
        decode(r#"{"identify":{"streamerId":"ABC","authentication":"x"}}"#),
        MessageToAssistant::Identify {
            streamer_id: Some("ABC".to_string()),
            app_attest: None,
        }
    );
    assert_eq!(
        decode(r#"{"identify":{"authentication":"x"}}"#),
        MessageToAssistant::Identify {
            streamer_id: None,
            app_attest: None,
        }
    );
    assert_eq!(
        decode(
            r#"{"identify":{"streamerId":"ABC","authentication":"x","appAttest":{"keyId":"k","attestation":"a","attestationChallenge":"c","assertion":"s"}}}"#
        ),
        MessageToAssistant::Identify {
            streamer_id: Some("ABC".to_string()),
            app_attest: Some(Attestation {
                key_id: Some("k".to_string()),
                attestation: Some("a".to_string()),
                attestation_challenge: Some("c".to_string()),
                assertion: Some("s".to_string()),
            }),
        }
    );
    for malformed in [
        r#"{"identify":{"streamerId":"ABC","authentication":"x","appAttest":{}}}"#,
        r#"{"identify":{"streamerId":"ABC","authentication":"x","appAttest":{"keyId":1}}}"#,
        r#"{"identify":{"streamerId":"ABC","authentication":"x","appAttest":"x"}}"#,
    ] {
        assert_eq!(
            decode(malformed),
            MessageToAssistant::Identify {
                streamer_id: Some("ABC".to_string()),
                app_attest: Some(Attestation::default()),
            }
        );
    }
    assert_eq!(
        decode(r#"{"identify":{"streamerId":"ABC","authentication":"x","appAttest":null}}"#),
        MessageToAssistant::Identify {
            streamer_id: Some("ABC".to_string()),
            app_attest: None,
        }
    );
    assert_eq!(
        decode(r#"{"response":{"id":1,"result":{"ok":{}}}}"#),
        MessageToAssistant::Response {
            id: 1,
            result: Outcome::Ok {},
            data: None,
        }
    );
    assert_eq!(
        decode(
            r#"{"response":{"id":2,"result":{"ok":{}},"data":{"getSettings":{"data":{"streams":[{"id":"A","name":"Twitch"}],"scenes":[],"bitratePresets":[],"mics":[],"srt":{"connectionPrioritiesEnabled":false,"connectionPriorities":[]}}}}}}"#
        ),
        MessageToAssistant::Response {
            id: 2,
            result: Outcome::Ok {},
            data: Some(serde_json::json!({
                "getSettings": {
                    "data": {
                        "streams": [{"id": "A", "name": "Twitch"}],
                        "scenes": [],
                        "bitratePresets": [],
                        "mics": [],
                        "srt": {"connectionPrioritiesEnabled": false, "connectionPriorities": []},
                    }
                }
            })),
        }
    );
}

#[test]
fn events_match_moblin() {
    assert_eq!(
        decode(
            r#"{"event":{"data":{"wheelOfLuck":{"wheel":{"options":[{"text":"Yes","weight":1},{"text":" No ","weight":3}]}}}}}"#
        ),
        MessageToAssistant::Event {
            data: Event::WheelOfLuck {
                wheel: Some(Wheel {
                    options: vec![
                        WheelOption {
                            text: "Yes".to_string(),
                            weight: 1,
                        },
                        WheelOption {
                            text: " No ".to_string(),
                            weight: 3,
                        },
                    ],
                }),
            },
        }
    );
    assert_eq!(
        decode(r#"{"event":{"data":{"wheelOfLuck":{}}}}"#),
        MessageToAssistant::Event {
            data: Event::WheelOfLuck { wheel: None },
        }
    );
    assert_eq!(
        decode(r#"{"event":{"data":{"icon":{"id":"AppIconKing"}}}}"#),
        MessageToAssistant::Event {
            data: Event::Icon {
                id: "AppIconKing".to_string()
            },
        }
    );
    assert_eq!(
        decode(
            r#"{"event":{"data":{"status":{"topRight":{"bitrate":{"message":"5.2 Mbps (1.2 GB)","ok":true},"bitrateInfo":{"bitsPerSecond":5200000,"totalBytes":1200000000},"bondingInfo":[{"name":"Cellular","bytesPerSecond":400000,"rtt":45},{"name":"WiFi","bytesPerSecond":250000}]}}}}}"#
        ),
        MessageToAssistant::Event {
            data: Event::Status {
                general: None,
                top_left: None,
                top_right: Some(serde_json::json!({
                    "bitrate": {"message": "5.2 Mbps (1.2 GB)", "ok": true},
                    "bitrateInfo": {"bitsPerSecond": 5200000, "totalBytes": 1200000000u64},
                    "bondingInfo": [
                        {"name": "Cellular", "bytesPerSecond": 400000, "rtt": 45},
                        {"name": "WiFi", "bytesPerSecond": 250000},
                    ],
                })),
            },
        }
    );
    assert_eq!(
        decode(r#"{"event":{"data":{"state":{"data":{"muted":true,"autoSceneSwitcher":{}}}}}}"#),
        MessageToAssistant::Event {
            data: Event::State {
                data: serde_json::json!({"muted": true, "autoSceneSwitcher": {}}),
            },
        }
    );
    assert!(
        serde_json::from_str::<MessageToAssistant>(r#"{"event":{"data":{"stats":{"data":{}}}}}"#)
            .is_err()
    );
}

#[test]
fn editor_controls_are_checked() {
    let valid = |text: &str| serde_json::from_str::<Control>(text).unwrap().valid();
    assert!(valid(
        r#"{"createScene":{"scene":{"id":"3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B"}}}"#
    ));
    assert!(!valid(r#"{"createScene":{"scene":{"name":"No id"}}}"#));
    assert!(!valid(r#"{"updateWidget":{"widget":{"id":"W1"}}}"#));
    assert!(valid(
        r#"{"updateStream":{"stream":{"id":"3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B"}}}"#
    ));
    assert!(!valid(r#"{"createStream":{"stream":{"name":"No id"}}}"#));
    for upload in [
        r#"{"setWidgetImage":{"id":"3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B","image":"iVBORw0KGgo="}}"#,
        r#"{"setVTuberModel":{"id":"3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B","type":"VRM","model":"AAAA"}}"#,
        r#"{"setPngTuberModel":{"id":"3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B","model":"AAAA"}}"#,
        r#"{"setAlertImage":{"media":{"id":"3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B","name":"A"},"image":"AAAA"}}"#,
        r#"{"setAlertSound":{"media":{"id":"3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B","name":"A"},"sound":"AAAA"}}"#,
        r#"{"setAlertVideo":{"alertId":"3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B","videoName":"clip.mp4","video":"AAAA"}}"#,
    ] {
        assert!(serde_json::from_str::<Control>(upload).is_err());
    }
    assert!(valid(
        r#"{"setScoreboardPlayers":{"players":[{"id":"3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B","name":"Erik"}]}}"#
    ));
    assert!(!valid(
        r#"{"setScoreboardPlayers":{"players":{"id":"3F2B8C1E-5A4D-4E7F-9B6A-1C2D3E4F5A6B"}}}"#
    ));
}

#[test]
fn sent_messages_are_counted_like_unread_ones() {
    let id = Uuid::nil();
    let requests = [
        Request::Tip {
            name: "Erik".to_string(),
            message: "Hi".to_string(),
            kind: TipKind::WheelOfLuck { count: 1 },
            request_id: "ORDER1".to_string(),
        },
        Request::GetSettings {},
        Request::GetStatus {},
        Request::StartStatus {
            interval: 1,
            filter: StatusFilter { top_right: true },
        },
        Request::StopStatus {},
        Request::StartPreview {},
        Request::StopPreview {},
    ];
    let controls = [
        Control::SetRecord { on: true },
        Control::SetLive { on: true },
        Control::SetMute { on: true },
        Control::SetStealthMode { on: true },
        Control::SetTorch { on: true },
        Control::SetZoom { x: 1.0 },
        Control::SetZoomPreset { id },
        Control::SetStream { id },
        Control::SetScene { id },
        Control::SetBitratePreset { id },
        Control::SetMic {
            id: "mic".to_string(),
        },
        Control::SetSrtConnectionPriority {
            id,
            priority: 1,
            enabled: true,
        },
        Control::SetSrtConnectionPrioritiesEnabled { enabled: true },
        Control::ReloadBrowserWidgets {},
        Control::InstantReplay {},
        Control::SetAutoSceneSwitcher { id: None },
        Control::SetPreviewStream { on: true },
        Control::SetFilter {
            filter: Filter::Crt {},
            on: true,
        },
        Control::StartMacro { id },
        Control::StopMacro { id },
        Control::SetGimbalTracking { on: true },
        Control::SendMessage {
            text: "Hi".to_string(),
            name: None,
        },
        Control::SetMoblinCloudAlertsWidget { id: Some(id) },
        Control::SetMoblinCloudWheelOfLuckWidget { id: None },
        Control::GetStream { id, secrets: true },
        Control::CreateStream {
            stream: serde_json::json!({}),
            secrets: false,
        },
        Control::UpdateStream {
            stream: serde_json::json!({}),
            secrets: true,
        },
        Control::DeleteStream { id },
    ];
    let messages = [
        MessageToStreamer::Hello {
            api_version: "0.1".to_string(),
            authentication: Authentication {
                challenge: "c".to_string(),
                salt: "s".to_string(),
            },
        },
        MessageToStreamer::Identified {
            result: Outcome::Ok {},
        },
        MessageToStreamer::Pong {},
    ];
    for message in messages.into_iter().chain(
        requests
            .into_iter()
            .chain(controls.into_iter().map(Request::Control))
            .map(|data| MessageToStreamer::Request { id: 1, data }),
    ) {
        let text = encode(&message);
        assert_eq!(message.kind(), traffic::unread(&text), "{text}");
    }
}

#[test]
fn received_messages_are_counted_like_unread_ones() {
    for text in [
        r#"{"ping":{}}"#,
        r#"{"preview":{"preview":"/9j/"}}"#,
        r#"{"identify":{"streamerId":"ABC"}}"#,
        r#"{"response":{"id":1,"result":{"ok":{}}}}"#,
        r#"{"response":{"id":2,"result":{"wrongPassword":{}}}}"#,
        r#"{"response":{"id":3,"result":{"unknownRequest":{}}}}"#,
        r#"{"response":{"id":4,"result":{"notIdentified":{}}}}"#,
        r#"{"response":{"id":5,"result":{"alreadyIdentified":{}}}}"#,
        r#"{"response":{"id":6,"result":{"error":{}}}}"#,
        r#"{"event":{"data":{"wheelOfLuck":{}}}}"#,
        r#"{"event":{"data":{"icon":{"id":"AppIcon"}}}}"#,
        r#"{"event":{"data":{"status":{"topRight":{}}}}}"#,
        r#"{"event":{"data":{"state":{"data":{}}}}}"#,
    ] {
        assert_eq!(decode(text).kind(), traffic::unread(text), "{text}");
    }
    let streamers = Streamers::new(Duration::ZERO);
    assert_eq!(
        streamers.receive(
            r#"{"event":{"data":{"chat":{}}}}"#,
            &SocketTraffic::default()
        ),
        None
    );
    assert_eq!(
        streamers.receive("not json", &SocketTraffic::default()),
        None
    );
    let received = streamers.traffic.stats().received;
    assert_eq!(received.len(), 2);
    assert_eq!(received[0].kind, "chat");
    assert_eq!(received[1].kind, "invalid");
}

#[test]
fn samples_are_taken_from_the_top_right_status() {
    let sample = Sample::of(&serde_json::json!({
        "bitrateInfo": {"bitsPerSecond": 5200000, "totalBytes": 1200000000u64},
        "bondingInfo": [
            {"name": "Cellular", "bytesPerSecond": 400000, "rtt": 45},
            {"name": "WiFi", "bytesPerSecond": 250000},
            {"bytesPerSecond": 1},
        ],
    }));
    assert_eq!(sample.bitrate, Some(5_200_000));
    assert_eq!(sample.total_bytes, Some(1_200_000_000));
    assert_eq!(
        sample.bonding,
        vec![
            BondingSample {
                name: "Cellular".to_string(),
                bytes_per_second: 400_000,
                rtt: Some(45),
            },
            BondingSample {
                name: "WiFi".to_string(),
                bytes_per_second: 250_000,
                rtt: None,
            },
        ]
    );
    let empty = Sample::of(&serde_json::json!({"uptime": {"message": "1:00:00"}}));
    assert_eq!(empty.bitrate, None);
    assert!(empty.bonding.is_empty());
}

#[test]
fn wheels_are_normalized() {
    let option = |text: &str, weight| WheelOption {
        text: text.to_string(),
        weight,
    };
    assert_eq!(
        Wheel {
            options: vec![
                option(" Yes ", 1),
                option("Never", 0),
                option(&"a".repeat(101), 5000),
            ],
        }
        .normalize(),
        Some(Wheel {
            options: vec![option("Yes", 1), option(&"a".repeat(100), 1000)],
        })
    );
    assert_eq!(
        Wheel {
            options: vec![option("Never", 0)],
        }
        .normalize(),
        None
    );
    assert_eq!(
        Wheel {
            options: (0..150).map(|_| option("Yes", 1)).collect(),
        }
        .normalize()
        .unwrap()
        .options
        .len(),
        100
    );
}

#[tokio::test]
async fn watches_outlive_sessions() {
    let streamers = Streamers::new(Duration::ZERO);
    let offline = streamers.watch(("page", "device"), "viewer");
    assert!(!offline.snapshot.connected);
    let old = streamers.connect("page", "device", Arc::default()).unwrap();
    let mut watch = streamers.watch(("page", "device"), "viewer");
    assert!(watch.snapshot.connected);
    streamers.remove("page", "device", old.number);
    assert_eq!(
        *watch.events.recv().await.unwrap(),
        Update::Connection { connected: false }
    );
    drop(offline);
    let mut new = streamers.connect("page", "device", Arc::default()).unwrap();
    let watchers = |streamers: &Streamers| {
        streamers
            .sessions
            .lock()
            .unwrap()
            .monitor_mut("page", "device")
            .map(|monitor| monitor.watchers.values().sum::<usize>())
    };
    assert_eq!(watchers(&streamers), Some(1));
    assert!(new.receiver.try_recv().is_err());
    drop(watch);
    assert_eq!(watchers(&streamers), None);
    assert!(new.receiver.try_recv().is_err());
}

#[tokio::test]
async fn a_disconnect_forgets_the_status_and_state() {
    let streamers = Streamers::new(Duration::ZERO);
    let old = streamers.connect("page", "device", Arc::default()).unwrap();
    let watch = streamers.watch(("page", "device"), "viewer");
    let on_icon = |_: &'static Icon| {};
    streamers.handle(
        "page",
        "device",
        old.number,
        decode(r#"{"event":{"data":{"status":{"general":{"batteryLevel":50},"topRight":{}}}}}"#),
        &on_icon,
    );
    streamers.handle(
        "page",
        "device",
        old.number,
        decode(r#"{"event":{"data":{"state":{"data":{"streaming":true}}}}}"#),
        &on_icon,
    );
    let snapshot = streamers.watch(("page", "device"), "viewer").snapshot;
    assert_eq!(snapshot.status["general"]["batteryLevel"], 50);
    assert_eq!(snapshot.state["streaming"], true);
    streamers.remove("page", "device", old.number);
    let snapshot = streamers.watch(("page", "device"), "viewer").snapshot;
    assert_eq!(snapshot.status, empty_status());
    assert_eq!(snapshot.state, serde_json::json!({}));
    drop(watch);
}

#[test]
fn keys_and_page_ids_are_random() {
    let streamers = Streamers::new(Duration::ZERO);
    let moblin_cloud_key = streamers.new_moblin_cloud_key();
    assert_eq!(moblin_cloud_key.len(), 36);
    assert_eq!(
        moblin_cloud_key
            .split('-')
            .map(str::len)
            .collect::<Vec<_>>(),
        [8, 4, 4, 4, 12]
    );
    assert_eq!(&moblin_cloud_key[14..15], "4");
    assert!("89ab".contains(&moblin_cloud_key[19..20]));
    assert!(
        moblin_cloud_key
            .chars()
            .all(|c| c == '-' || c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    );
    assert_ne!(streamers.new_moblin_cloud_key(), moblin_cloud_key);
    let page_id = streamers.new_page_id().unwrap();
    assert_eq!(page_id.len(), 16);
    assert_ne!(streamers.new_page_id().unwrap(), page_id);
}

#[tokio::test]
async fn filter_changes_are_merged_into_the_state() {
    let streamers = Streamers::new(Duration::ZERO);
    let session = streamers.connect("page", "device", Arc::default()).unwrap();
    let mut watch = streamers.watch(("page", "device"), "viewer");
    let on_icon = |_: &'static Icon| {};
    for filters in [
        r#"[{"movie":{}},true,{"sepia":{}},false]"#,
        r#"[{"sepia":{}},true,{"crt":{}},true]"#,
    ] {
        streamers.handle(
            "page",
            "device",
            session.number,
            decode(&format!(
                r#"{{"event":{{"data":{{"state":{{"data":{{"filters":{filters}}}}}}}}}}}"#
            )),
            &on_icon,
        );
    }
    let merged = serde_json::json!([{"movie": {}}, true, {"sepia": {}}, true, {"crt": {}}, true]);
    watch.events.recv().await.unwrap();
    assert_eq!(
        *watch.events.recv().await.unwrap(),
        Update::State {
            data: serde_json::json!({"filters": merged}),
        }
    );
    let snapshot = streamers.watch(("page", "device"), "viewer").snapshot;
    assert_eq!(snapshot.state["filters"], merged);
}
