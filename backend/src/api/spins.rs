use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::Value;

use crate::payments::{Item, Order};

use super::*;

#[derive(Deserialize)]
pub(super) struct SpinRequest {
    spins: u32,
    #[serde(default)]
    price: Option<u32>,
    user: Option<String>,
    message: Option<String>,
    #[serde(default)]
    provider: Option<String>,
}

pub(super) async fn handle_spin_order(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    Json(request): Json<SpinRequest>,
) -> Result<Json<OrderResponse>, Failure> {
    let result = spin_order(&api, id, request).await;
    counted(&api, "spinOrders", result)
}

async fn spin_order(
    api: &Api,
    id: String,
    request: SpinRequest,
) -> Result<Json<OrderResponse>, Failure> {
    let settings = api.pages.get(&id);
    let currency = settings.currency().to_string();
    let spin = settings.spin().cloned().ok_or(failure(
        "feature-off",
        StatusCode::BAD_REQUEST,
        "The streamer turned the wheel off. Reload the page.",
    ))?;
    wheel_available(api, &id)?;
    let payee = payee(api, &id, request.provider.as_deref())?;
    let option = spin
        .options
        .iter()
        .find(|option| {
            option.spins == request.spins && request.price.is_none_or(|price| price == option.price)
        })
        .ok_or(failure(
            "invalid-option",
            StatusCode::BAD_REQUEST,
            "The streamer changed the prices. Reload the page.",
        ))?;
    let order = Order {
        streamer: id.clone(),
        item: Item::Spin {
            spins: option.spins,
            message: viewer_message(request.message)?,
        },
        user: user_name(request.user, "Someone"),
        payee,
        price: option.price,
        currency,
    };
    Ok(Json(start_order(api, order).await?))
}

pub(super) fn wheel_available(api: &Api, id: &str) -> Result<(), Failure> {
    if api.remote_control.wheel(id).is_none() {
        return Err(if api.remote_control.is_connected(id) {
            failure(
                "no-wheel",
                StatusCode::CONFLICT,
                "The streamer is not taking wheel spins right now.",
            )
        } else {
            failure(
                "offline",
                StatusCode::NOT_FOUND,
                "The streamer's wheel is not available right now. Try again when they are live.",
            )
        });
    }
    Ok(())
}

pub(super) async fn handle_spin_capture(
    State(api): State<Arc<Api>>,
    Path((id, order_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    tokio::spawn(capture_paid(api, id, order_id, "spin"))
        .await
        .unwrap_or_else(|_| Err(capture_crashed()))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::http::StatusCode;
    use futures_util::SinkExt;
    use serde_json::{Value, json};
    use tokio_tungstenite::tungstenite::Message;

    use crate::api::test_utils::*;
    use crate::payments::Stage;

    #[tokio::test]
    async fn spin_captures_need_the_wheel() {
        let (address, api, _) = start_paid().await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        connect_paypal(&api);
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"currency": "usd", "songs": null, "spin": {
                "options": [{"spins": 2, "price": 800}],
            }}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let wheel = json!({"event": {"data": {"wheelOfLuck": {"wheel": {"options": [
            {"text": "Yes", "weight": 1},
        ]}}}}});
        let mut socket = connect(&address, &key).await;
        socket.send(Message::text(wheel.to_string())).await.unwrap();
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        let orders_url = format!("http://{address}/pages/{id}/spin/orders");
        let order: Value = client
            .post(&orders_url)
            .json(&json!({"user": "Viewer", "spins": 2}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(order, json!({"orderId": "ORDER1"}));
        socket.close(None).await.unwrap();
        for _ in 0..100 {
            if !api.remote_control.is_connected(&id) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let capture_url = format!("{orders_url}/ORDER1/capture");
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response.text().await.unwrap(),
            "The streamer's wheel is gone, so your payment was not taken."
        );
        assert_eq!(api.orders.get("ORDER1").unwrap().0, Stage::Created);
        assert!(api.deliveries.pending().is_empty());
        let mut socket = connect(&address, &key).await;
        socket.send(Message::text(wheel.to_string())).await.unwrap();
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let request = receive(&mut socket).await;
        assert_eq!(
            request["request"]["data"]["spinWheelOfLuck"]["requestId"],
            "ORDER1"
        );
    }

    #[tokio::test]
    async fn wheels_are_pushed_by_the_streamer() {
        let (address, api) = start_api(None).await;
        let (_, key, id) = streamer(&address, &api).await;
        let spin_url = format!("http://{address}/pages/{id}/spin");
        let wheel = || async {
            reqwest::get(&spin_url)
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()["wheel"]
                .clone()
        };
        let mut socket = connect(&address, &key).await;
        assert_eq!(wheel().await, Value::Null);
        let options = json!([{"text": "Yes", "weight": 1}, {"text": "No", "weight": 3}]);
        socket
            .send(Message::text(
                json!({"event": {"data": {"wheelOfLuck": {"wheel": {"options": options}}}}})
                    .to_string(),
            ))
            .await
            .unwrap();
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        assert_eq!(wheel().await, json!({"options": options}));
        socket
            .send(Message::text(
                json!({"event": {"data": {"wheelOfLuck": {}}}}).to_string(),
            ))
            .await
            .unwrap();
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        assert_eq!(wheel().await, Value::Null);
        socket
            .send(Message::text(
                json!({"event": {"data": {"wheelOfLuck": {"wheel": {"options": options}}}}})
                    .to_string(),
            ))
            .await
            .unwrap();
        drop(socket);
        for _ in 0..100 {
            if !api.remote_control.is_connected(&id) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(wheel().await, Value::Null);
    }

    #[tokio::test]
    async fn spins_are_sent_to_the_streamer() {
        let (address, api, paypal_order) = start_paid().await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        connect_paypal(&api);
        let spin_url = format!("http://{address}/pages/{id}/spin");
        let public: Value = client
            .get(&spin_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            public,
            json!({"spin": null, "subSpins": null, "wheel": null, "connected": false})
        );
        let settings_url = format!("http://{address}/account/settings");
        let response = client
            .post(&settings_url)
            .bearer_auth(&session)
            .json(&json!({"currency": "jpy", "songs": null, "spin": {
                "options": [{"spins": 1, "price": 500}],
            }}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let settings: Value = client
            .post(&settings_url)
            .bearer_auth(&session)
            .json(&json!({"currency": "usd", "songs": null, "spin": {
                "options": [{"spins": 2, "price": 800}, {"spins": 1, "price": 500}],
            }}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            settings["spin"],
            json!({"enabled": true, "options": [{"spins": 1, "price": 500}, {"spins": 2, "price": 800}]})
        );
        let public: Value = client
            .get(&spin_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            public,
            json!({"spin": {
                "options": [{"spins": 1, "price": 500}, {"spins": 2, "price": 800}],
                "currency": "USD",
                "clientId": "id",
                "merchantId": "MERCHANT00001",
                "environment": "production",
                "stripe": null,
            }, "subSpins": null, "wheel": null, "connected": false})
        );
        let orders_url = format!("{spin_url}/orders");
        let response = client
            .post(&orders_url)
            .json(&json!({"user": "Viewer", "spins": 2}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let mut socket = connect(&address, &key).await;
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        let public: Value = client
            .get(&spin_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(public["wheel"], Value::Null);
        assert_eq!(public["connected"], true);
        let response = client
            .post(&orders_url)
            .json(&json!({"user": "Viewer", "spins": 2}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.text().await.unwrap(),
            "The streamer is not taking wheel spins right now."
        );
        socket
            .send(Message::text(
                json!({"event": {"data": {"wheelOfLuck": {"wheel": {"options": [
                    {"text": "Yes", "weight": 1},
                ]}}}}})
                .to_string(),
            ))
            .await
            .unwrap();
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        for request in [
            json!({"user": "Viewer", "spins": 3}),
            json!({"user": "Viewer", "spins": 2, "message": "a".repeat(201)}),
        ] {
            let response = client
                .post(&orders_url)
                .json(&request)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
        let order: Value = client
            .post(&orders_url)
            .json(&json!({"user": "Viewer", "spins": 2, "message": " Go big! "}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(order, json!({"orderId": "ORDER1"}));
        let sent = paypal_order.lock().unwrap().clone().unwrap();
        assert_eq!(
            sent["purchase_units"][0]["amount"],
            json!({"currency_code": "USD", "value": "8.00"})
        );
        assert_eq!(
            sent["purchase_units"][0]["description"],
            "2 wheel spins by Viewer: Go big!"
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
        let request = receive(&mut socket).await;
        let spin = json!({"spinWheelOfLuck": {
            "name": "Viewer",
            "message": "Go big!",
            "count": 2,
            "requestId": "ORDER1",
        }});
        assert_eq!(request["request"]["data"], spin);
        socket.close(None).await.unwrap();
        let mut socket = connect(&address, &key).await;
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], spin);
        answer(&mut socket, &request, Value::Null).await;
        wait_until_added(&api).await;
    }
}
