use super::*;

#[test]
fn controls_match_the_backend_shapes() {
    assert_eq!(
        toggle("setTorch", true).body,
        json!({"setTorch": {"on": true}})
    );
    assert_eq!(select("setScene", "a").key, "setScene");
    assert_eq!(
        action("reloadBrowserWidgets").body,
        json!({"reloadBrowserWidgets": {}})
    );
    assert_eq!(
        filter("grayScale", false).body,
        json!({"setFilter": {"filter": {"grayScale": {}}, "on": false}})
    );
    assert_eq!(
        optional("setAutoSceneSwitcher", None).body,
        json!({"setAutoSceneSwitcher": {}})
    );
    assert_eq!(
        optional("setMoblinCloudWheelOfLuckWidget", Some("w")).body,
        json!({"setMoblinCloudWheelOfLuckWidget": {"id": "w"}})
    );
    let set = priority("c", 3, true);
    assert_eq!(set.key, "setSrtConnectionPriority:c");
    assert_eq!(
        set.body,
        json!({"setSrtConnectionPriority": {"id": "c", "priority": 3, "enabled": true}})
    );
    assert_eq!(macro_run("m", true).key, "stopMacro:m");
    assert_eq!(
        macro_run("m", false).body,
        json!({"startMacro": {"id": "m"}})
    );
    assert_eq!(message("hi").body, json!({"sendMessage": {"text": "hi"}}));
}

#[test]
fn states_are_read_like_the_web() {
    let state = json!({
        "filters": [{"grayScale": {}}, true, {"sepia": {}}, false, {"crt": {}}],
        "streaming": false,
    });
    let state = state.as_object().unwrap();
    assert!(filter_on(state, "grayScale"));
    assert!(!filter_on(state, "sepia"));
    assert!(!filter_on(state, "crt"));
    assert!(!filter_on(state, "movie"));
    assert!(go_live_disabled(state, false));
    assert!(!go_live_disabled(state, true));
    let live = json!({"streaming": true});
    assert!(!go_live_disabled(live.as_object().unwrap(), false));
    assert!(go_live_disabled(&Map::new(), true));
}
