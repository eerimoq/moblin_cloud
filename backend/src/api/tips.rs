use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::Value;

use crate::pages::format_price;
use crate::payments::{Item, Order};

use super::*;

#[derive(Deserialize)]
pub(super) struct TipRequest {
    amount: u32,
    user: Option<String>,
    message: Option<String>,
    #[serde(default)]
    provider: Option<String>,
}

pub(super) async fn handle_tip_order(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    Json(request): Json<TipRequest>,
) -> Result<Json<OrderResponse>, Failure> {
    let result = tip_order(&api, id, request).await;
    counted(&api, "tipOrders", result)
}

async fn tip_order(
    api: &Api,
    id: String,
    request: TipRequest,
) -> Result<Json<OrderResponse>, Failure> {
    let settings = api.pages.get(&id);
    let currency = settings.currency().to_string();
    let tips = settings.tips().cloned().ok_or(failure(
        "feature-off",
        StatusCode::BAD_REQUEST,
        "The streamer no longer takes tips. Reload the page.",
    ))?;
    let payee = payee(api, &id, request.provider.as_deref())?;
    let minimum = tips.amounts.iter().copied().min().unwrap_or(50);
    if request.amount < minimum {
        return Err(failure(
            "amount-too-low",
            StatusCode::BAD_REQUEST,
            &format!("Tip at least {}.", format_price(minimum)),
        ));
    }
    let message = viewer_message(request.message)?;
    let order = Order {
        streamer: id.clone(),
        item: Item::Tip { message },
        user: user_name(request.user, "Someone"),
        payee,
        price: request.amount,
        currency,
    };
    Ok(Json(start_order(api, order).await?))
}

pub(super) async fn handle_tip_capture(
    State(api): State<Arc<Api>>,
    Path((id, order_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    tokio::spawn(capture_paid(api, id, order_id, "tip"))
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

    use super::*;
    use crate::api::test_utils::*;
    use crate::deliveries;
    use crate::payments::Stage;

    #[derive(Clone, Copy, Debug)]
    enum Fault {
        None,
        LostAnswer,
        LostAnswerAndLookups,
        ProviderDown,
        Refused,
        PendingReview,
        AlreadyCaptured,
        DatabaseReadOnly,
        CrashedAfterCapture,
    }

    async fn tip_delivered_once(
        mut socket: tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        address: &str,
        key: &str,
        api: &Api,
        currency: &str,
    ) {
        let tip = json!({"tip": {
            "name": "Viewer",
            "message": "",
            "kind": {"message": {"amount": "7.00", "currency": currency}},
            "requestId": "ORDER1",
        }});
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], tip);
        socket
            .send(Message::text(
                json!({"response": {"id": request["request"]["id"], "result": {"error": {}}}})
                    .to_string(),
            ))
            .await
            .unwrap();
        socket.close(None).await.unwrap();
        assert_eq!(api.deliveries.pending().len(), 1);
        let mut socket = connect(address, key).await;
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], tip);
        answer(&mut socket, &request, Value::Null).await;
        wait_until_added(api).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(200), receive(&mut socket))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn paid_tips_always_reach_the_queue_exactly_once() {
        for fault in [
            Fault::None,
            Fault::LostAnswer,
            Fault::LostAnswerAndLookups,
            Fault::ProviderDown,
            Fault::Refused,
            Fault::PendingReview,
            Fault::AlreadyCaptured,
            Fault::DatabaseReadOnly,
            Fault::CrashedAfterCapture,
        ] {
            let (address, api, fake) = start_paid_fake().await;
            let currency = match fault {
                Fault::PendingReview => "EUR",
                _ => "USD",
            };
            let (session, capture_url) = paypal_tip_order_in(&address, &api, currency).await;
            let client = reqwest::Client::new();
            let capture = || async { client.post(&capture_url).send().await.unwrap() };
            let not_paid = |api: &Api, fake: &FakePayPal| {
                assert!(!*fake.captured.lock().unwrap(), "{fault:?}");
                assert!(api.deliveries.pending().is_empty(), "{fault:?}");
            };
            match fault {
                Fault::None => {
                    assert_eq!(capture().await.status(), StatusCode::ACCEPTED, "{fault:?}");
                }
                Fault::LostAnswer => {
                    *fake.lose_capture.lock().unwrap() = true;
                    assert_eq!(capture().await.status(), StatusCode::ACCEPTED, "{fault:?}");
                }
                Fault::LostAnswerAndLookups => {
                    *fake.lose_capture.lock().unwrap() = true;
                    *fake.lookups_fail.lock().unwrap() = true;
                    assert_eq!(
                        capture().await.status(),
                        StatusCode::BAD_GATEWAY,
                        "{fault:?}"
                    );
                    assert!(*fake.captured.lock().unwrap());
                    assert!(api.deliveries.pending().is_empty());
                    *fake.lookups_fail.lock().unwrap() = false;
                    settle(&api, Duration::ZERO).await;
                }
                Fault::ProviderDown => {
                    *fake.fail_capture.lock().unwrap() = true;
                    assert_eq!(
                        capture().await.status(),
                        StatusCode::BAD_GATEWAY,
                        "{fault:?}"
                    );
                    not_paid(&api, &fake);
                    *fake.fail_capture.lock().unwrap() = false;
                    assert_eq!(capture().await.status(), StatusCode::ACCEPTED, "{fault:?}");
                }
                Fault::Refused => {
                    *fake.refuse_capture.lock().unwrap() = true;
                    assert_eq!(
                        capture().await.status(),
                        StatusCode::BAD_GATEWAY,
                        "{fault:?}"
                    );
                    not_paid(&api, &fake);
                    *fake.refuse_capture.lock().unwrap() = false;
                    assert_eq!(capture().await.status(), StatusCode::ACCEPTED, "{fault:?}");
                }
                Fault::PendingReview => {
                    assert_eq!(
                        capture().await.status(),
                        StatusCode::PAYMENT_REQUIRED,
                        "{fault:?}"
                    );
                    assert!(api.deliveries.pending().is_empty());
                    settle(&api, Duration::ZERO).await;
                    assert!(api.deliveries.pending().is_empty());
                    *fake.approved.lock().unwrap() = true;
                    settle(&api, Duration::ZERO).await;
                }
                Fault::AlreadyCaptured => {
                    *fake.captured.lock().unwrap() = true;
                    assert_eq!(capture().await.status(), StatusCode::ACCEPTED, "{fault:?}");
                }
                Fault::DatabaseReadOnly => {
                    api.orders.read_only(true);
                    assert_eq!(
                        capture().await.status(),
                        StatusCode::SERVICE_UNAVAILABLE,
                        "{fault:?}"
                    );
                    not_paid(&api, &fake);
                    api.orders.read_only(false);
                    assert_eq!(capture().await.status(), StatusCode::ACCEPTED, "{fault:?}");
                }
                Fault::CrashedAfterCapture => {
                    assert!(
                        api.orders
                            .transition("ORDER1", Stage::Created, Stage::Capturing)
                            .unwrap()
                    );
                    *fake.captured.lock().unwrap() = true;
                    assert!(api.deliveries.pending().is_empty());
                    settle(&api, Duration::ZERO).await;
                }
            }
            assert!(*fake.captured.lock().unwrap(), "{fault:?}");
            let pending = api.deliveries.pending();
            assert_eq!(pending.len(), 1, "{fault:?}");
            assert_eq!(pending[0].request_id, "ORDER1", "{fault:?}");
            assert_eq!(
                pending[0].item,
                deliveries::Item::Tip {
                    user: "Viewer".to_string(),
                    message: String::new(),
                    amount: 700,
                    currency: currency.to_string(),
                },
                "{fault:?}"
            );
            assert_eq!(
                api.orders.get("ORDER1").unwrap().0,
                Stage::Captured,
                "{fault:?}"
            );
            assert_eq!(capture().await.status(), StatusCode::ACCEPTED, "{fault:?}");
            settle(&api, Duration::ZERO).await;
            assert_eq!(api.deliveries.pending().len(), 1, "{fault:?}");
            let key = api.accounts.get("1").unwrap().page.key.clone();
            let socket = connect(&address, &key).await;
            tip_delivered_once(socket, &address, &key, &api, currency).await;
            let recent: Value = client
                .get(format!("http://{address}/account/recent?kind=tip"))
                .bearer_auth(&session)
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            assert_eq!(recent["items"][0]["status"], "delivered", "{fault:?}");
        }
    }

    #[tokio::test]
    async fn tips_reach_every_known_device_also_after_reconnecting() {
        let (address, api) = start_api(None).await;
        let (_, key, id) = streamer(&address, &api).await;
        let mut main = connect_as(&address, &key, "main").await;
        let backup = connect_as(&address, &key, "backup").await;
        for _ in 0..100 {
            if api.remote_control.devices(&id).len() == 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        drop(backup);
        for _ in 0..100 {
            if api.remote_control.devices(&id).len() == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        api.deliveries.add_test(
            id.clone(),
            "TIP1".to_string(),
            deliveries::Item::Tip {
                user: "Viewer".to_string(),
                message: String::new(),
                amount: 500,
                currency: "USD".to_string(),
            },
        );
        let request = receive(&mut main).await;
        assert_eq!(request["request"]["data"]["tip"]["requestId"], "TIP1");
        answer(&mut main, &request, Value::Null).await;
        for _ in 0..100 {
            if api.deliveries.get("TIP1").unwrap().delivered == ["main"] {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!api.deliveries.get("TIP1").unwrap().finished);
        let mut backup = connect_as(&address, &key, "backup").await;
        let request = receive(&mut backup).await;
        assert_eq!(request["request"]["data"]["tip"]["requestId"], "TIP1");
        answer(&mut backup, &request, Value::Null).await;
        for _ in 0..100 {
            if api.deliveries.get("TIP1").unwrap().finished {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(
            api.deliveries.get("TIP1").unwrap().delivered,
            ["main", "backup"]
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(200), receive(&mut main))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn tips_are_sent_to_the_streamer() {
        let (address, api, paypal_order) = start_paid().await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        connect_paypal(&api);
        let tips_url = format!("http://{address}/pages/{id}/tips");
        let public: Value = client
            .get(&tips_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(public, json!({"tips": null}));
        let orders_url = format!("{tips_url}/orders");
        let response = client
            .post(&orders_url)
            .json(&json!({"amount": 500}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let settings: Value = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"currency": "usd", "tips": {
                "amounts": [1000, 200, 500],
            }}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            settings["tips"],
            json!({"enabled": true, "amounts": [200, 500, 1000]})
        );
        let public: Value = client
            .get(&tips_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            public,
            json!({"tips": {
                "amounts": [200, 500, 1000],
                "currency": "USD",
                "clientId": "id",
                "merchantId": "MERCHANT00001",
                "environment": "sandbox",
                "stripe": null,
            }})
        );
        for request in [
            json!({"amount": 199}),
            json!({"amount": 700, "message": "a".repeat(201)}),
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
            .json(&json!({"amount": 700, "user": " Viewer ", "message": " Great stream! "}))
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
            json!({"currency_code": "USD", "value": "7.00"})
        );
        assert_eq!(
            sent["purchase_units"][0]["description"],
            "Tip from Viewer: Great stream!"
        );
        let response = client
            .post(format!(
                "http://{address}/pages/{id}/spin/orders/ORDER1/capture"
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
        let response = client
            .post(format!("{orders_url}/ORDER1/capture"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(api.deliveries.pending().len(), 1);
        let tip = json!({"tip": {
            "name": "Viewer",
            "message": "Great stream!",
            "kind": {"message": {"amount": "7.00", "currency": "USD"}},
            "requestId": "ORDER1",
        }});
        let mut socket = connect(&address, &key).await;
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], tip);
        socket
            .send(Message::text(
                json!({"response": {"id": request["request"]["id"], "result": {"error": {}}}})
                    .to_string(),
            ))
            .await
            .unwrap();
        socket.close(None).await.unwrap();
        let mut socket = connect(&address, &key).await;
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"], tip);
        answer(&mut socket, &request, Value::Null).await;
        wait_until_added(&api).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(200), receive(&mut socket))
                .await
                .is_err()
        );
    }
}
