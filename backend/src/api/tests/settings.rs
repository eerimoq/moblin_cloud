use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::api::test_utils::*;
use crate::pages::{Feature, Settings, Tips};

#[tokio::test]
async fn custom_colors_and_fonts_are_for_pro_streamers() {
    let (address, api) = start_api(None).await;
    let client = reqwest::Client::new();
    let save = |session: String, accent: &'static str| {
        client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(session)
            .json(&json!({"accent": accent}))
            .send()
    };
    let save_font = |session: String, font: &'static str| {
        client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(session)
            .json(&json!({"font": font}))
            .send()
    };
    let (session, _, _) = streamer(&api);
    let response = save(session.clone(), "#123abc").await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = save_font(session.clone(), "handwritten").await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = save_font(session.clone(), "rounded").await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = save(session, "pink").await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    create_account(&api, "premium", "Premium", "premium");
    let premium = api.auth.session("premium", 0);
    let account: Value = client
        .get(format!("http://{address}/account"))
        .bearer_auth(&premium)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let page_id = account["settings"]["id"].as_str().unwrap().to_string();
    let settings: Value = save(premium.clone(), "#123ABC")
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(settings["accent"], "#123abc");
    let profile: Value = client
        .get(format!("http://{address}/pages/{page_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(profile["accent"], "#123abc");
    let response = save(premium.clone(), "#12345").await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let settings: Value = save_font(premium.clone(), "handwritten")
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(settings["font"], "handwritten");
    let response = save_font(premium, "comic").await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn the_currency_only_changes_while_paid_features_are_off() {
    let (address, api, _) = start_paid().await;
    let (session, _, _) = streamer(&api);
    connect_paypal(&api);
    let client = reqwest::Client::new();
    for (request, status) in [
        (
            json!({"currency": "SEK", "tips": {"amounts": [500]}}),
            StatusCode::OK,
        ),
        (
            json!({"currency": "EUR", "tips": {"amounts": [500]}}),
            StatusCode::CONFLICT,
        ),
        (
            json!({"currency": "EUR", "tips": null}),
            StatusCode::CONFLICT,
        ),
        (
            json!({"currency": "SEK", "tips": {"amounts": [700]}}),
            StatusCode::OK,
        ),
        (
            json!({"currency": "SEK", "tips": {"enabled": false, "amounts": [700]}}),
            StatusCode::OK,
        ),
        (
            json!({"currency": "EUR", "spin": {"options": [{"spins": 1, "price": 100}]}}),
            StatusCode::OK,
        ),
        (
            json!({"currency": "USD", "spin": {"enabled": false, "options": [{"spins": 1, "price": 100}]}}),
            StatusCode::CONFLICT,
        ),
        (json!({"currency": "EUR"}), StatusCode::OK),
        (json!({"currency": "USD"}), StatusCode::OK),
    ] {
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&request)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{request}");
    }
}

#[tokio::test]
async fn a_paypal_merchant_is_not_payable_without_paypal() {
    let (address, api, _) = start_stripe_api().await;
    let (session, _, page_id) = streamer(&api);
    connect_paypal(&api);
    api.pages
        .set(
            &page_id,
            Settings {
                tips: Some(Feature::on(Tips { amounts: vec![500] })),
                ..Default::default()
            },
        )
        .unwrap();
    let account = account_of(&address, &session).await;
    assert_eq!(account["settings"]["payable"], false);
    assert_eq!(
        get_json(&address, &format!("/pages/{page_id}/tips"))
            .await
            .1["tips"],
        Value::Null
    );
    let response = reqwest::Client::new()
        .post(format!("http://{address}/account/settings"))
        .bearer_auth(&session)
        .json(&json!({"tips": {"amounts": [700]}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
}
