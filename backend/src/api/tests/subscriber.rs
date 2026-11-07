use std::collections::HashMap;
use std::time::Duration;

use axum::http::StatusCode;
use futures_util::SinkExt;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

use crate::accounts::Platform;
use crate::api::test_utils::*;

#[tokio::test]
async fn viewers_sign_in_with_their_subscription_tier() {
    let twitch = start_twitch().await;
    let (address, api) = start_api_with(None, None, Some(twitch_app(&twitch)), None, None).await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let (_, _, page_id) = streamer(&api);
    let response = client
        .get(format!("http://{address}/auth/twitch/login?page=unknown"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let sign_in = async |page_id: &str| {
        let response = client
            .get(format!("http://{address}/auth/twitch/login?page={page_id}"))
            .send()
            .await
            .unwrap();
        let login = reqwest::Url::parse(&location(&response)).unwrap();
        let query: HashMap<_, _> = login.query_pairs().into_owned().collect();
        assert_eq!(query["scope"], "user:read:subscriptions");
        location(
            &client
                .get(format!(
                    "http://{address}/auth/twitch/callback?code=c&state={}",
                    query["state"]
                ))
                .header("cookie", format!("moblin_sign_in_state={}", query["state"]))
                .send()
                .await
                .unwrap(),
        )
    };
    let page = sign_in(&page_id).await;
    let session = page
        .strip_prefix("https://cloud.moblin.app/twitch/erik#viewer=")
        .unwrap();
    let viewer = api.auth.viewer_of(session).unwrap();
    assert_eq!(
        (
            viewer.id.as_str(),
            viewer.name.as_str(),
            viewer.page_id.as_str(),
            viewer.tier
        ),
        ("42", "Erik", page_id.as_str(), Some(2))
    );
    assert_eq!(api.accounts.get("42"), None);
    assert_eq!(api.auth.session_of(session), None);
    let other = create_account(&api, "2", "Other", "other").page_id;
    let other = other.as_str();
    let page = sign_in(other).await;
    let session = page
        .strip_prefix("https://cloud.moblin.app/twitch/other#viewer=")
        .unwrap();
    assert_eq!(api.auth.viewer_of(session).unwrap().tier, None);
    let third = create_account(&api, "3", "Third", "third").page_id;
    let third = third.as_str();
    assert_eq!(
        sign_in(third).await,
        "https://cloud.moblin.app/twitch/third#viewerSignInFailed=twitch"
    );
    let response = client
        .get(format!("http://{address}/auth/twitch/login?page={page_id}"))
        .send()
        .await
        .unwrap();
    let state = reqwest::Url::parse(&location(&response))
        .unwrap()
        .query_pairs()
        .find(|(name, _)| name == "state")
        .unwrap()
        .1
        .into_owned();
    assert_eq!(
        location(
            &client
                .get(format!(
                    "http://{address}/auth/twitch/callback?error=access_denied&state={state}"
                ))
                .header("cookie", format!("moblin_sign_in_state={state}"))
                .send()
                .await
                .unwrap()
        ),
        "https://cloud.moblin.app/twitch/erik#viewerSignInFailed=twitch"
    );
    let response = client
        .get(format!("http://{address}/auth/twitch/login?page={page_id}"))
        .send()
        .await
        .unwrap();
    let state = reqwest::Url::parse(&location(&response))
        .unwrap()
        .query_pairs()
        .find(|(name, _)| name == "state")
        .unwrap()
        .1
        .into_owned();
    assert_eq!(
        location(
            &client
                .get(format!(
                    "http://{address}/auth/twitch/callback?code=c&state={state}"
                ))
                .header("cookie", "moblin_sign_in_state=other")
                .send()
                .await
                .unwrap()
        ),
        "https://cloud.moblin.app/twitch/erik#viewerSignInFailed=twitch"
    );
    assert_eq!(
        location(
            &client
                .get(format!(
                    "http://{address}/auth/twitch/callback?error=access_denied"
                ))
                .send()
                .await
                .unwrap()
        ),
        "https://cloud.moblin.app/dashboard/#signInFailed"
    );
}

#[tokio::test]
async fn twitch_subscribers_spin_the_wheel_every_month() {
    let twitch = start_twitch().await;
    let (address, api) = start_api_with(None, None, Some(twitch_app(&twitch)), None, None).await;
    let client = reqwest::Client::new();
    let (session, moblin_cloud_key, page_id) = streamer(&api);
    let viewer = |viewer: &str, tier: Option<u8>| {
        api.auth.viewer_session(
            Platform::Twitch,
            viewer.to_string(),
            format!("Viewer {viewer}"),
            page_id.clone(),
            tier,
        )
    };
    let spin = async |session: &str| {
        client
            .post(format!("http://{address}/pages/{page_id}/spin/subscriber"))
            .bearer_auth(session)
            .json(&json!({"message": " Big one "}))
            .send()
            .await
            .unwrap()
    };
    let settings = |body: Value| {
        client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&body)
            .send()
    };
    let tier1 = viewer("11", Some(1));
    assert_eq!(spin(&tier1).await.status(), StatusCode::BAD_REQUEST);
    for sub_spins in [
        json!({"tier1": 0, "tier2": 0, "tier3": 0}),
        json!({"tier1": 101, "tier2": 0, "tier3": 0}),
    ] {
        let response = settings(json!({"subSpins": sub_spins})).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let response = settings(json!({"subSpins": {"tier1": 1, "tier2": 0, "tier3": 2}}))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.json::<Value>().await.unwrap()["subSpins"],
        json!({"enabled": true, "tier1": 1, "tier2": 0, "tier3": 2})
    );
    let status = async |session: &str| {
        client
            .get(format!("http://{address}/pages/{page_id}/spin/subscriber"))
            .bearer_auth(session)
            .send()
            .await
            .unwrap()
            .status()
    };
    assert_eq!(status("garbage").await, StatusCode::UNAUTHORIZED);
    assert_eq!(
        status(&api.auth.session("1", 0)).await,
        StatusCode::UNAUTHORIZED
    );
    let public: Value = client
        .get(format!("http://{address}/pages/{page_id}/spin"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        public["subSpins"],
        json!({"tier1": 1, "tier2": 0, "tier3": 2})
    );
    assert_eq!(public["spin"], Value::Null);
    assert_eq!(spin(&tier1).await.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        spin(&viewer("12", None)).await.status(),
        StatusCode::FORBIDDEN
    );
    let response = spin(&viewer("21", Some(2))).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response.text().await.unwrap(),
        "Tier 2 subscribers can't spin the wheel here."
    );
    let mut socket = connect(&address, &moblin_cloud_key).await;
    socket
        .send(Message::text(
            json!({"event": {"data": {"wheelOfLuck": {"wheel": {"options": [{"text": "A", "weight": 1}]}}}}})
                .to_string(),
        ))
        .await
        .unwrap();
    socket
        .send(Message::text(json!({"ping": {}}).to_string()))
        .await
        .unwrap();
    assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
    let response = spin(&tier1).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(response.json::<Value>().await.unwrap(), json!({"left": 0}));
    let request = receive(&mut socket).await;
    let sent = &request["request"]["data"]["tip"];
    assert_eq!(sent["name"], "Viewer 11");
    assert_eq!(sent["message"], "Big one");
    assert_eq!(sent["kind"]["wheelOfLuck"]["count"], 1);
    answer(&mut socket, &request, Value::Null).await;
    wait_until_added(&api).await;
    let response = spin(&tier1).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.text().await.unwrap(),
        "You have used all your spins this month."
    );
    let upgraded: Value = client
        .get(format!("http://{address}/pages/{page_id}/spin/subscriber"))
        .bearer_auth(viewer("11", Some(3)))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        upgraded,
        json!({"platform": "twitch", "name": "Viewer 11", "tier": 3, "allowance": 2, "left": 1})
    );
    let status: Value = client
        .get(format!("http://{address}/pages/{page_id}/spin/subscriber"))
        .bearer_auth(viewer("31", Some(3)))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        status,
        json!({"platform": "twitch", "name": "Viewer 31", "tier": 3, "allowance": 2, "left": 2})
    );
    let recent: Value = client
        .get(format!("http://{address}/account/recent?kind=spin"))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(recent["items"][0]["user"], "Viewer 11");
    assert_eq!(recent["items"][0]["tier"], 1);
    assert_eq!(recent["items"][0]["spins"], 1);
    assert_eq!(recent["items"][0]["status"], "delivered");
    assert_eq!(counter(&api, "subSpins"), 1);
    assert_eq!(counter(&api, "subSpinsFailed:no-spins-left"), 1);
    assert_eq!(counter(&api, "subSpinsFailed:tier-excluded"), 1);
    assert_eq!(counter(&api, "subSpinsFailed:not-subscribed"), 1);
    assert_eq!(counter(&api, "subSpinsFailed:feature-off"), 1);
    assert_eq!(counter(&api, "subSpinsFailed:offline"), 1);
    assert_eq!(counter(&api, "spinDeliveries"), 1);
    let racer = viewer("13", Some(1));
    let (first, second) = tokio::join!(spin(&racer), spin(&racer));
    let mut statuses = [first.status(), second.status()];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::ACCEPTED, StatusCode::CONFLICT]);
    let request = receive(&mut socket).await;
    answer(&mut socket, &request, Value::Null).await;
    wait_until_added(&api).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(200), receive(&mut socket))
            .await
            .is_err()
    );
    let tier3 = viewer("31", Some(3));
    assert_eq!(spin(&tier3).await.status(), StatusCode::ACCEPTED);
    let request = receive(&mut socket).await;
    let request_id = request["request"]["data"]["tip"]["requestId"].clone();
    drop(socket);
    let mut socket = connect(&address, &moblin_cloud_key).await;
    let request = receive(&mut socket).await;
    assert_eq!(request["request"]["data"]["tip"]["requestId"], request_id);
    answer(&mut socket, &request, Value::Null).await;
    wait_until_added(&api).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(200), receive(&mut socket))
            .await
            .is_err()
    );
    let recent: Value = client
        .get(format!("http://{address}/account/recent?kind=spin"))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(recent["items"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn kick_subscribers_get_the_tier_1_allowance() {
    let (kick, fake) = start_kick().await;
    let (address, api) = start_api_with(None, None, None, Some(kick_app(&kick)), None).await;
    let flow = SignInFlow::new(&address, fake.clone());
    let client = reqwest::Client::new();
    let session = session_of(&flow.sign_in("kick", "kick-a").await);
    let account = account_of(&address, &session).await;
    let id = account["settings"]["id"].as_str().unwrap().to_string();
    let moblin_cloud_key = moblin_cloud_key_of(&account);
    let response = client
        .post(format!("http://{address}/account/settings"))
        .bearer_auth(&session)
        .json(&json!({"subSpins": {"tier1": 0, "tier2": 1, "tier3": 1}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let twitch_only = create_account(&api, "2", "Other", "other").page_id;
    for page_id in ["unknown", twitch_only.as_str()] {
        let response = client
            .get(format!("http://{address}/auth/kick/login?page={page_id}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    let response = client
        .get(format!("http://{address}/auth/twitch/login?page={id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let viewer_sign_in = async |code: &str| {
        let (query, cookie) = flow.start(&format!("/auth/kick/login?page={id}")).await;
        assert_eq!(query["scope"], "user:read channel:read");
        flow.finish("kick", code, &query["state"], &cookie).await
    };
    let viewer_of = |page_id: &str| {
        page_id
            .strip_prefix("https://cloud.moblin.app/kick/kickerik#viewer=")
            .unwrap_or_else(|| panic!("{page_id}"))
            .to_string()
    };
    let subscriber = viewer_of(&viewer_sign_in("kick-sub").await);
    let viewer = api.auth.viewer_of(&subscriber).unwrap();
    assert_eq!(
        (
            viewer.platform,
            viewer.id.as_str(),
            viewer.name.as_str(),
            viewer.page_id.as_str(),
            viewer.tier
        ),
        (Platform::Kick, "777", "Sub", id.as_str(), Some(1))
    );
    assert_eq!(
        api.accounts.account_of_identity(Platform::Kick, "777"),
        None
    );
    assert_eq!(api.auth.session_of(&subscriber), None);
    let allowance = async |session: &str| {
        let response = client
            .get(format!("http://{address}/pages/{id}/spin/subscriber"))
            .bearer_auth(session)
            .send()
            .await
            .unwrap();
        (
            response.status(),
            response.json::<Value>().await.unwrap_or_default(),
        )
    };
    assert_eq!(
        allowance(&subscriber).await,
        (
            StatusCode::OK,
            json!({"platform": "kick", "name": "Sub", "tier": 1, "allowance": 0, "left": 0})
        )
    );
    let not_subscribed = viewer_of(&viewer_sign_in("kick-b").await);
    assert_eq!(api.auth.viewer_of(&not_subscribed).unwrap().tier, None);
    let lapsed = viewer_of(&viewer_sign_in("kick-lapsed").await);
    assert_eq!(api.auth.viewer_of(&lapsed).unwrap().tier, None);
    assert_eq!(
        viewer_sign_in("kick-broken").await,
        "https://cloud.moblin.app/kick/kickerik#viewerCheckFailed"
    );
    assert_eq!(
        viewer_sign_in("unknown").await,
        "https://cloud.moblin.app/kick/kickerik#viewerSignInFailed=kick"
    );
    let (query, _) = flow.start(&format!("/auth/kick/login?page={id}")).await;
    assert_eq!(
        flow.finish(
            "kick",
            "kick-sub",
            &query["state"],
            "moblin_sign_in_state=other"
        )
        .await,
        "https://cloud.moblin.app/kick/kickerik#viewerSignInFailed=kick"
    );
    let spin = async |session: &str| {
        client
            .post(format!("http://{address}/pages/{id}/spin/subscriber"))
            .bearer_auth(session)
            .json(&json!({}))
            .send()
            .await
            .unwrap()
    };
    let response = spin(&not_subscribed).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response.text().await.unwrap(),
        "Only Kick subscribers of this streamer can spin the wheel."
    );
    let response = spin(&subscriber).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response.text().await.unwrap(),
        "Kick subscribers can't spin the wheel here."
    );
    let response = client
        .post(format!("http://{address}/account/settings"))
        .bearer_auth(&session)
        .json(&json!({"subSpins": {"tier1": 2, "tier2": 5, "tier3": 5}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let mut socket = connect(&address, &moblin_cloud_key).await;
    socket
        .send(Message::text(
            json!({"event": {"data": {"wheelOfLuck": {"wheel": {"options": [{"text": "A", "weight": 1}]}}}}})
                .to_string(),
        ))
        .await
        .unwrap();
    socket
        .send(Message::text(json!({"ping": {}}).to_string()))
        .await
        .unwrap();
    assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
    let response = spin(&subscriber).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(response.json::<Value>().await.unwrap(), json!({"left": 1}));
    let request = receive(&mut socket).await;
    assert_eq!(request["request"]["data"]["tip"]["name"], "Sub");
    answer(&mut socket, &request, Value::Null).await;
    wait_until_added(&api).await;
    assert_eq!(allowance(&subscriber).await.1["left"], 1);
    let recent: Value = client
        .get(format!("http://{address}/account/recent?kind=spin"))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(recent["items"][0]["user"], "Sub");
    assert_eq!(recent["items"][0]["kickSubscriber"], true);
    assert_eq!(recent["items"][0]["amount"], 0);
    assert!(recent["items"][0].get("tier").is_none());
    assert!(recent["items"][0].get("provider").is_none());
    assert_eq!(api.allowances.left(&id, "777", "spin", 2).unwrap(), 2);
    assert_eq!(api.allowances.left(&id, "kick:777", "spin", 2).unwrap(), 1);
    let twitch_viewer = api.auth.viewer_session(
        Platform::Twitch,
        "777".to_string(),
        "Sub".to_string(),
        id.clone(),
        Some(1),
    );
    assert_eq!(allowance(&twitch_viewer).await.0, StatusCode::UNAUTHORIZED);
    assert!(
        api.counters
            .all()
            .iter()
            .any(|counter| counter.name == "subSpins" && counter.total == 1)
    );
}
