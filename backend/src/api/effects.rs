use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::Value;

use crate::effects;
use crate::payments::{Item, Order};

use super::*;

#[derive(Deserialize)]
pub(super) struct EffectRequest {
    effect: String,
    #[serde(default)]
    price: Option<u32>,
    user: Option<String>,
    message: Option<String>,
    #[serde(default)]
    provider: Option<String>,
}

pub(super) async fn handle_effect_order(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    Json(request): Json<EffectRequest>,
) -> Result<Json<OrderResponse>, Failure> {
    let result = effect_order(&api, id, request).await;
    counted(&api, "effectOrders", result)
}

async fn effect_order(
    api: &Api,
    id: String,
    request: EffectRequest,
) -> Result<Json<OrderResponse>, Failure> {
    let settings = api.pages.get(&id);
    let currency = settings.currency().to_string();
    let effects = settings.effects().cloned().ok_or(failure(
        "feature-off",
        StatusCode::BAD_REQUEST,
        "The streamer turned effects off. Reload the page.",
    ))?;
    let payee = payee(api, &id, request.provider.as_deref())?;
    if request.price.is_some_and(|price| price != effects.price) {
        return Err(failure(
            "invalid-option",
            StatusCode::BAD_REQUEST,
            "The streamer changed the price. Reload the page.",
        ));
    }
    let effect = viewer_effect(&request.effect)?;
    let order = Order {
        streamer: id.clone(),
        item: Item::Effect {
            effect,
            message: viewer_message(request.message)?,
        },
        user: user_name(request.user, "Someone"),
        payee,
        price: effects.price,
        currency,
    };
    Ok(Json(start_order(api, order).await?))
}

pub(super) fn viewer_effect(effect: &str) -> Result<String, Failure> {
    effects::effect(effect)
        .map(|effect| effect.id.to_string())
        .ok_or(failure(
            "no-such-effect",
            StatusCode::BAD_REQUEST,
            "That effect is no longer available. Reload the page.",
        ))
}

pub(super) async fn handle_effect_capture(
    State(api): State<Arc<Api>>,
    Path((id, order_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    tokio::spawn(capture_paid(api, id, order_id, "effect"))
        .await
        .unwrap_or_else(|_| Err(capture_crashed()))
}

pub(super) async fn handle_effect_media(Path(id): Path<String>) -> Result<Response, StatusCode> {
    let effect = effects::effect(&id).ok_or(StatusCode::NOT_FOUND)?;
    Ok((
        [
            (CONTENT_TYPE, effect.content_type),
            (CACHE_CONTROL, "public, max-age=86400"),
            (X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        effect.data,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::{Value, json};

    use crate::accounts::Platform;
    use crate::api::test_utils::*;

    #[tokio::test]
    async fn effects_are_played_by_moblin() {
        let (address, api, paypal_order) = start_paid().await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        connect_paypal(&api);
        let effects_url = format!("http://{address}/pages/{id}/effects");
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
            .post(format!("http://{address}/account/settings"))
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
                "http://{address}/pages/{id}/tips/orders/ORDER1/capture"
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
        let mut socket = connect(&address, &key).await;
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
                .get(format!("http://{address}/account/recent?kind={kind}"))
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
        let (session, key, _) = streamer(&address, &api).await;
        let test = |request: Value| {
            client
                .post(format!("http://{address}/account/test/effect"))
                .bearer_auth(&session)
                .json(&request)
                .send()
        };
        let response = test(json!({"effect": "burp"})).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let response = test(json!({"effect": "perfect-fart"})).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let mut socket = connect(&address, &key).await;
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
            include_bytes!("../effects/perfect-fart.mp3").as_slice()
        );
        let response = reqwest::get(format!("http://{address}/effects/burp"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn subscribers_play_effects_every_month() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        let viewer = |viewer: &str, tier: Option<u8>| {
            api.auth.viewer_session(
                Platform::Twitch,
                viewer.to_string(),
                format!("Viewer {viewer}"),
                id.clone(),
                tier,
            )
        };
        let play = async |session: &str, effect: &str| {
            client
                .post(format!("http://{address}/pages/{id}/effects/subscriber"))
                .bearer_auth(session)
                .json(&json!({"effect": effect, "message": " Pfff "}))
                .send()
                .await
                .unwrap()
        };
        let left = async |session: &str| {
            client
                .get(format!("http://{address}/pages/{id}/effects/subscriber"))
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
            .post(format!("http://{address}/account/settings"))
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
            .get(format!("http://{address}/pages/{id}/effects"))
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
            .get(format!("http://{address}/pages/{id}/spin/subscriber"))
            .bearer_auth(&tier1)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(spins["left"], 5);
        let mut socket = connect(&address, &key).await;
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
            .get(format!("http://{address}/account/recent?kind=effect"))
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
}
