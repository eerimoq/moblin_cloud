use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::accounts::Platform;
use crate::api::test_utils::*;

#[tokio::test]
async fn effects_are_played_by_moblin() {
    let (address, api, paypal_order) = start_paid().await;
    let client = reqwest::Client::new();
    let (session, moblin_cloud_key, page_id) = streamer(&api);
    connect_paypal(&api);
    let effects_url = format!("http://{address}/pages/{page_id}/effects");
    let orders_url = format!("{effects_url}/orders");
    let public = || async {
        client
            .get(&effects_url)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()
    };
    let order = |request: Value| client.post(&orders_url).json(&request).send();
    let options = json!([
        {"id": "fart", "kind": "sound"},
        {"id": "fart-2", "kind": "sound"},
        {"id": "bad-chili-fart", "kind": "sound"},
        {"id": "perfect-fart", "kind": "sound"},
    ]);
    assert_eq!(
        public().await,
        json!({"effects": null, "subEffects": null, "options": options})
    );
    let response = order(json!({"effect": "fart"})).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let settings: Value = client
        .post(format!("http://{address}/account/pages/{page_id}/settings"))
        .bearer_auth(&session)
        .json(&json!({"effects": {"price": 300}}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(settings["effects"], json!({"enabled": true, "price": 300}));
    assert_eq!(
        public().await,
        json!({
            "effects": {
                "price": 300,
                "currency": "USD",
                "clientId": "id",
                "merchantId": "MERCHANT00001",
                "environment": "sandbox",
                "stripe": null,
            },
            "subEffects": null,
            "options": options,
        })
    );
    for request in [
        json!({"effect": "burp"}),
        json!({"effect": "fart", "price": 200}),
        json!({"effect": "fart", "message": "a".repeat(201)}),
    ] {
        let response = order(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let response = order(json!({"effect": "bad-chili-fart", "price": 300, "user": "Viewer"}))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let sent = paypal_order.lock().unwrap().clone().unwrap();
    assert_eq!(
        sent["purchase_units"][0]["amount"],
        json!({"currency_code": "USD", "value": "3.00"})
    );
    assert_eq!(
        sent["purchase_units"][0]["description"],
        "Bad chili fart by Viewer"
    );
    let response = client
        .post(format!(
            "http://{address}/pages/{page_id}/tips/orders/ORDER1/capture"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::GONE);
    let response = client
        .post(format!("{orders_url}/ORDER1/capture"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let mut socket = connect(&address, &moblin_cloud_key).await;
    let request = receive(&mut socket).await;
    assert_eq!(
        request["request"]["data"],
        json!({"tip": {
            "name": "Viewer",
            "message": "",
            "kind": {"effect": {"sound": "Bad chili fart"}},
            "requestId": "ORDER1",
        }})
    );
    answer(&mut socket, &request, Value::Null).await;
    wait_until_added(&api).await;
    let recent = |kind: &str| {
        client
            .get(format!(
                "http://{address}/account/pages/{page_id}/recent?kind={kind}"
            ))
            .bearer_auth(&session)
            .send()
    };
    let effects: Value = recent("effect").await.unwrap().json().await.unwrap();
    assert_eq!(effects["items"][0]["effect"], "bad-chili-fart");
    assert_eq!(effects["items"][0]["amount"], 300);
    let tips: Value = recent("tip").await.unwrap().json().await.unwrap();
    assert_eq!(tips["items"], json!([]));
}

#[tokio::test]
async fn streamers_test_their_effects() {
    let (address, api) = start_api(None).await;
    let client = reqwest::Client::new();
    let (session, moblin_cloud_key, page_id) = streamer(&api);
    let test = |request: Value| {
        client
            .post(format!(
                "http://{address}/account/pages/{page_id}/test/effect"
            ))
            .bearer_auth(&session)
            .json(&request)
            .send()
    };
    let response = test(json!({"effect": "burp"})).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = test(json!({"effect": "perfect-fart"})).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let mut socket = connect(&address, &moblin_cloud_key).await;
    let request = receive(&mut socket).await;
    assert_eq!(
        request["request"]["data"]["tip"]["kind"]["effect"]["sound"],
        "Perfect fart"
    );
}

#[tokio::test]
async fn effects_can_be_listened_to() {
    let (address, _api) = start_api(None).await;
    let response = reqwest::get(format!("http://{address}/effects/perfect-fart"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "audio/mpeg");
    assert_eq!(
        response.bytes().await.unwrap(),
        include_bytes!("../../effects/perfect-fart.mp3").as_slice()
    );
    let response = reqwest::get(format!("http://{address}/effects/burp"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn subscribers_play_effects_every_month() {
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
    let play = async |session: &str, effect: &str| {
        client
            .post(format!(
                "http://{address}/pages/{page_id}/effects/subscriber"
            ))
            .bearer_auth(session)
            .json(&json!({"effect": effect, "message": " Pfff "}))
            .send()
            .await
            .unwrap()
    };
    let left = async |session: &str| {
        client
            .get(format!(
                "http://{address}/pages/{page_id}/effects/subscriber"
            ))
            .bearer_auth(session)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()
    };
    let tier1 = viewer("11", Some(1));
    assert_eq!(play(&tier1, "fart").await.status(), StatusCode::BAD_REQUEST);
    let response = client
        .post(format!("http://{address}/account/pages/{page_id}/settings"))
        .bearer_auth(&session)
        .json(&json!({
            "subSpins": {"tier1": 5, "tier2": 5, "tier3": 5},
            "subEffects": {"tier1": 1, "tier2": 0, "tier3": 3},
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.json::<Value>().await.unwrap()["subEffects"],
        json!({"enabled": true, "tier1": 1, "tier2": 0, "tier3": 3})
    );
    let public: Value = client
        .get(format!("http://{address}/pages/{page_id}/effects"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(public["effects"], Value::Null);
    assert_eq!(
        public["subEffects"],
        json!({"tier1": 1, "tier2": 0, "tier3": 3})
    );
    assert_eq!(left(&tier1).await["left"], 1);
    assert_eq!(
        play(&viewer("12", None), "fart").await.status(),
        StatusCode::FORBIDDEN
    );
    let response = play(&viewer("21", Some(2)), "fart").await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response.text().await.unwrap(),
        "Tier 2 subscribers can't play effects here."
    );
    assert_eq!(play(&tier1, "burp").await.status(), StatusCode::BAD_REQUEST);
    let response = play(&tier1, "fart-2").await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(response.json::<Value>().await.unwrap(), json!({"left": 0}));
    let response = play(&tier1, "fart").await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.text().await.unwrap(),
        "You have used all your effects this month."
    );
    let spins: Value = client
        .get(format!("http://{address}/pages/{page_id}/spin/subscriber"))
        .bearer_auth(&tier1)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(spins["left"], 5);
    let mut socket = connect(&address, &moblin_cloud_key).await;
    let request = receive(&mut socket).await;
    assert_eq!(
        request["request"]["data"],
        json!({"tip": {
            "name": "Viewer 11",
            "message": "Pfff",
            "kind": {"effect": {"sound": "Fart 2"}},
            "requestId": request["request"]["data"]["tip"]["requestId"],
        }})
    );
    answer(&mut socket, &request, Value::Null).await;
    let recent: Value = client
        .get(format!(
            "http://{address}/account/pages/{page_id}/recent?kind=effect"
        ))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(recent["items"][0]["effect"], "fart-2");
    assert_eq!(recent["items"][0]["tier"], 1);
}
