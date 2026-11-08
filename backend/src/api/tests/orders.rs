use std::time::Duration;

use axum::http::StatusCode;
use futures_util::SinkExt;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

use super::*;
use crate::api::test_utils::*;
use crate::payments::{Item, Order, Payee, Stage};

fn connect_stripe(api: &Api) {
    api.accounts
        .start_stripe("1", "acct_1".to_string())
        .unwrap();
    api.accounts.stripe_ready("1", "acct_1", true).unwrap();
}

#[tokio::test]
async fn stripe_holds_of_expired_orders_are_released() {
    let (_, api, fake) = start_stripe_api().await;
    let order = |payee: Payee| Order {
        streamer: "p".to_string(),
        item: Item::Tip { message: None },
        user: "Viewer".to_string(),
        payee,
        price: 700,
        currency: "USD".to_string(),
    };
    for (id, payee) in [
        ("pi_1", Payee::Stripe("acct_1".to_string())),
        ("pi_2", Payee::Stripe("acct_1".to_string())),
        ("ORDER1", Payee::PayPal("MERCHANT00001".to_string())),
    ] {
        api.orders.insert(id, &order(payee)).unwrap();
    }
    api.orders.age("pi_1", 4 * 60 * 60);
    api.orders.age("ORDER1", 4 * 60 * 60);
    expire_orders(&api).await;
    assert_eq!(*fake.cancelled.lock().unwrap(), ["pi_1"]);
    assert_eq!(api.orders.get("pi_1"), None);
    assert_eq!(api.orders.get("ORDER1"), None);
    assert_eq!(api.orders.get("pi_2").unwrap().stage, Stage::Created);
}

#[tokio::test]
async fn stripe_holds_are_released_when_the_wheel_is_gone() {
    let (address, api, fake) = start_stripe_api().await;
    let client = reqwest::Client::new();
    let (session, moblin_cloud_key, page_id) = streamer(&api);
    connect_stripe(&api);
    let response = client
        .post(format!("http://{address}/account/pages/{page_id}/settings"))
        .bearer_auth(&session)
        .json(&json!({"currency": "SEK", "spin": {"options": [{"spins": 1, "price": 250}]}}))
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
    let orders_url = format!("http://{address}/pages/{page_id}/spin/orders");
    let created: Value = client
        .post(&orders_url)
        .json(&json!({"user": "Viewer", "spins": 1, "provider": "stripe"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(created["orderId"], "pi_1");
    let payment = fake.payment.lock().unwrap().clone().unwrap();
    assert_eq!(payment["form"]["amount"], "250");
    assert_eq!(payment["form"]["currency"], "sek");
    socket.close(None).await.unwrap();
    for _ in 0..100 {
        if !api.remote_control.is_connected(&page_id) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    *fake.state.lock().unwrap() = "requires_capture";
    let capture_url = format!("{orders_url}/pi_1/capture");
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(*fake.cancelled.lock().unwrap(), ["pi_1"]);
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::GONE);
    assert!(api.deliveries.pending().is_empty());
}

async fn paypal_tip_order(address: &str, api: &Api) -> String {
    paypal_tip_order_in(address, api, "USD").await.1
}

#[tokio::test]
async fn lost_capture_answers_are_settled_right_away() {
    let (address, api, fake) = start_paid_fake().await;
    let capture_url = paypal_tip_order(&address, &api).await;
    *fake.lose_capture.lock().unwrap() = true;
    let client = reqwest::Client::new();
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(api.deliveries.pending().len(), 1);
    assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Captured);
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(api.deliveries.pending().len(), 1);
}

#[tokio::test]
async fn lost_capture_answers_are_settled_later() {
    let (address, api, fake) = start_paid_fake().await;
    let capture_url = paypal_tip_order(&address, &api).await;
    *fake.lose_capture.lock().unwrap() = true;
    *fake.lookups_fail.lock().unwrap() = true;
    let client = reqwest::Client::new();
    let unknown = "PayPal did not answer. If the money was taken, the tip will still be shown or the money refunded.";
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(response.text().await.unwrap(), unknown);
    assert!(api.deliveries.pending().is_empty());
    assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Capturing);
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(response.text().await.unwrap(), unknown);
    settle(&api, Duration::ZERO).await;
    assert!(api.deliveries.pending().is_empty());
    *fake.lookups_fail.lock().unwrap() = false;
    settle(&api, Duration::from_secs(60)).await;
    assert!(api.deliveries.pending().is_empty());
    settle(&api, Duration::ZERO).await;
    assert_eq!(api.deliveries.pending().len(), 1);
    assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Captured);
    assert!(
        api.counters
            .all()
            .iter()
            .any(|counter| counter.name == "tipPayments" && counter.total == 1)
    );
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(api.deliveries.pending().len(), 1);
    assert_eq!(counter(&api, "tipPayments"), 1);
    assert_eq!(counter(&api, "paypalOrders"), 1);
    assert_eq!(counter(&api, "paypalPaymentsFailed:unknown"), 1);
    assert_eq!(counter(&api, "paypalPayments"), 1);
}

#[tokio::test]
async fn lost_captures_completed_later_are_delivered() {
    let (address, api, fake) = start_paid_fake().await;
    let capture_url = paypal_tip_order(&address, &api).await;
    *fake.capture_later.lock().unwrap() = true;
    let client = reqwest::Client::new();
    let unknown = "PayPal did not answer. If the money was taken, the tip will still be shown or the money refunded.";
    for _ in 0..2 {
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(response.text().await.unwrap(), unknown);
        assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Capturing);
    }
    *fake.captured.lock().unwrap() = true;
    settle(&api, Duration::ZERO).await;
    assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Captured);
    assert_eq!(api.deliveries.pending().len(), 1);
}

#[tokio::test]
async fn lost_captures_never_completed_can_be_captured_again() {
    let (address, api, fake) = start_paid_fake().await;
    let capture_url = paypal_tip_order(&address, &api).await;
    *fake.capture_later.lock().unwrap() = true;
    let client = reqwest::Client::new();
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    settle(&api, Duration::ZERO).await;
    assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Capturing);
    *fake.capture_later.lock().unwrap() = false;
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(api.deliveries.pending().len(), 1);
}

#[tokio::test]
async fn captures_after_the_deadline_are_refused() {
    let (address, api, fake) = start_paid_fake().await;
    let capture_url = paypal_tip_order(&address, &api).await;
    api.orders.age("ORDER1", 31 * 60);
    let response = reqwest::Client::new()
        .post(&capture_url)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::GONE);
    assert_eq!(
        response.text().await.unwrap(),
        "The tip took too long, so no money was taken. Try again."
    );
    assert!(!*fake.captured.lock().unwrap());
    expire_orders(&api).await;
    assert_eq!(api.orders.get("ORDER1"), None);
}

#[tokio::test]
async fn late_payments_are_refunded() {
    let (address, api, fake) = start_paid_fake().await;
    let capture_url = paypal_tip_order(&address, &api).await;
    *fake.capture_later.lock().unwrap() = true;
    let client = reqwest::Client::new();
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    api.orders.age("ORDER1", 31 * 60);
    expire_orders(&api).await;
    assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Capturing);
    *fake.captured.lock().unwrap() = true;
    settle(&api, Duration::ZERO).await;
    assert_eq!(api.orders.get("ORDER1"), None);
    assert!(api.deliveries.pending().is_empty());
    assert_eq!(*fake.refunds.lock().unwrap(), ["refund-CAPTURE1"]);
    assert_eq!(counter(&api, "tipRefunds"), 1);
    assert_eq!(counter(&api, "paypalRefunds"), 1);
    assert_eq!(counter(&api, "tipPayments"), 0);
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::GONE);
}

#[tokio::test]
async fn late_payments_are_refunded_when_the_viewer_retries() {
    let (address, api, fake) = start_paid_fake().await;
    let capture_url = paypal_tip_order(&address, &api).await;
    *fake.capture_later.lock().unwrap() = true;
    let client = reqwest::Client::new();
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    api.orders.age("ORDER1", 31 * 60);
    *fake.captured.lock().unwrap() = true;
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::GONE);
    assert_eq!(
        response.text().await.unwrap(),
        "The tip took too long, so the money was refunded."
    );
    assert_eq!(*fake.refunds.lock().unwrap(), ["refund-CAPTURE1"]);
    assert!(api.deliveries.pending().is_empty());
}

#[tokio::test]
async fn late_payments_are_delivered_when_they_cannot_be_refunded() {
    let (address, api, fake) = start_paid_fake().await;
    let capture_url = paypal_tip_order(&address, &api).await;
    *fake.capture_later.lock().unwrap() = true;
    *fake.refuse_refund.lock().unwrap() = true;
    let response = reqwest::Client::new()
        .post(&capture_url)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    api.orders.age("ORDER1", 31 * 60);
    *fake.captured.lock().unwrap() = true;
    settle(&api, Duration::ZERO).await;
    assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Captured);
    assert_eq!(api.deliveries.pending().len(), 1);
}

#[tokio::test]
async fn tried_captures_never_completed_are_given_up_after_30_minutes() {
    let (address, api, fake) = start_paid_fake().await;
    let capture_url = paypal_tip_order(&address, &api).await;
    *fake.capture_later.lock().unwrap() = true;
    let client = reqwest::Client::new();
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    api.orders.age("ORDER1", 20 * 60);
    settle(&api, Duration::ZERO).await;
    assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Capturing);
    api.orders.age("ORDER1", 20 * 60);
    settle(&api, Duration::ZERO).await;
    assert_eq!(api.orders.get("ORDER1"), None);
    assert!(api.deliveries.pending().is_empty());
    assert!(fake.refunds.lock().unwrap().is_empty());
}

#[tokio::test]
async fn declined_captures_are_refused() {
    let (address, api, fake) = start_paid_fake().await;
    let capture_url = paypal_tip_order(&address, &api).await;
    *fake.decline_capture.lock().unwrap() = true;
    let response = reqwest::Client::new()
        .post(&capture_url)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
    assert_eq!(
        response.text().await.unwrap(),
        "PayPal declined the payment."
    );
    assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Capturing);
    assert!(api.deliveries.pending().is_empty());
    assert_eq!(counter(&api, "tipPaymentsFailed:declined"), 1);
    assert_eq!(counter(&api, "paypalPaymentsFailed:declined"), 1);
}

#[tokio::test]
async fn failed_captures_can_be_retried() {
    let (address, api, fake) = start_paid_fake().await;
    let capture_url = paypal_tip_order(&address, &api).await;
    *fake.fail_capture.lock().unwrap() = true;
    let client = reqwest::Client::new();
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(
        response.text().await.unwrap(),
        "PayPal did not answer. If the money was taken, the tip will still be shown or the money refunded."
    );
    assert!(api.deliveries.pending().is_empty());
    assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Capturing);
    *fake.fail_capture.lock().unwrap() = false;
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(api.deliveries.pending().len(), 1);
}

#[tokio::test]
async fn captures_need_a_writable_database_before_taking_money() {
    let (address, api, fake) = start_paid_fake().await;
    let capture_url = paypal_tip_order(&address, &api).await;
    api.orders.read_only(true);
    let client = reqwest::Client::new();
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response.text().await.unwrap(),
        "Tips are unavailable right now, so no money was taken. Try again later."
    );
    assert!(!*fake.captured.lock().unwrap());
    assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Created);
    api.orders.read_only(false);
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert!(*fake.captured.lock().unwrap());
    assert_eq!(api.deliveries.pending().len(), 1);
}

#[tokio::test]
async fn refused_captures_can_be_retried() {
    let (address, api, fake) = start_paid_fake().await;
    let capture_url = paypal_tip_order(&address, &api).await;
    *fake.refuse_capture.lock().unwrap() = true;
    let client = reqwest::Client::new();
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
    assert_eq!(
        response.text().await.unwrap(),
        "PayPal declined the payment."
    );
    assert!(api.deliveries.pending().is_empty());
    assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Capturing);
    assert!(!*fake.captured.lock().unwrap());
    settle(&api, Duration::ZERO).await;
    assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Capturing);
    *fake.refuse_capture.lock().unwrap() = false;
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(api.deliveries.pending().len(), 1);
}

#[tokio::test]
async fn gone_orders_are_forgotten_when_settling() {
    let (address, api, fake) = start_paid_fake().await;
    let capture_url = paypal_tip_order(&address, &api).await;
    *fake.lose_capture.lock().unwrap() = true;
    *fake.lookups_fail.lock().unwrap() = true;
    let client = reqwest::Client::new();
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Capturing);
    *fake.lookups_fail.lock().unwrap() = false;
    *fake.gone.lock().unwrap() = true;
    settle(&api, Duration::ZERO).await;
    assert_eq!(api.orders.get("ORDER1"), None);
    assert!(api.deliveries.pending().is_empty());
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::GONE);
}

#[tokio::test]
async fn processing_stripe_captures_are_settled_once_succeeded() {
    let (address, api, fake) = start_stripe_api().await;
    let (session, _, page_id) = streamer(&api);
    connect_stripe(&api);
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{address}/account/pages/{page_id}/settings"))
        .bearer_auth(&session)
        .json(&json!({"currency": "eur", "tips": {"amounts": [500]}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let orders_url = format!("http://{address}/pages/{page_id}/tips/orders");
    let order: Value = client
        .post(&orders_url)
        .json(&json!({"amount": 700, "user": "Viewer", "provider": "stripe"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(order["orderId"], "pi_1");
    *fake.state.lock().unwrap() = "requires_capture";
    *fake.processing.lock().unwrap() = true;
    let capture_url = format!("{orders_url}/pi_1/capture");
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
    assert_eq!(
        response.text().await.unwrap(),
        "Stripe is still reviewing the tip. If Stripe approves it within half an hour, the tip will be shown and the money reaches the streamer, otherwise the money is refunded."
    );
    assert_eq!(api.orders.get("pi_1").unwrap().stage, Stage::Capturing);
    settle(&api, Duration::ZERO).await;
    assert!(api.deliveries.pending().is_empty());
    *fake.state.lock().unwrap() = "succeeded";
    settle(&api, Duration::ZERO).await;
    assert_eq!(api.deliveries.pending().len(), 1);
    assert_eq!(api.orders.get("pi_1").unwrap().stage, Stage::Captured);
}

#[tokio::test]
async fn late_stripe_payments_are_refunded() {
    let (address, api, fake) = start_stripe_api().await;
    let (session, _, page_id) = streamer(&api);
    connect_stripe(&api);
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{address}/account/pages/{page_id}/settings"))
        .bearer_auth(&session)
        .json(&json!({"currency": "eur", "tips": {"amounts": [500]}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let orders_url = format!("http://{address}/pages/{page_id}/tips/orders");
    let response = client
        .post(&orders_url)
        .json(&json!({"amount": 700, "user": "Viewer", "provider": "stripe"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    *fake.state.lock().unwrap() = "requires_capture";
    *fake.processing.lock().unwrap() = true;
    let response = client
        .post(format!("{orders_url}/pi_1/capture"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
    api.orders.age("pi_1", 31 * 60);
    *fake.state.lock().unwrap() = "succeeded";
    settle(&api, Duration::ZERO).await;
    assert_eq!(api.orders.get("pi_1"), None);
    assert!(api.deliveries.pending().is_empty());
    assert_eq!(
        *fake.refunds.lock().unwrap(),
        [("payment_intent=pi_1".to_string(), "refund-pi_1".to_string())]
    );
}

#[tokio::test]
async fn already_captured_orders_are_delivered() {
    let (address, api, fake) = start_paid_fake().await;
    let capture_url = paypal_tip_order(&address, &api).await;
    *fake.captured.lock().unwrap() = true;
    let response = reqwest::Client::new()
        .post(&capture_url)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(api.deliveries.pending().len(), 1);
    assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Captured);
}

#[tokio::test]
async fn lost_stripe_capture_answers_are_settled() {
    let (address, api, fake) = start_stripe_api().await;
    let (session, _, page_id) = streamer(&api);
    connect_stripe(&api);
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{address}/account/pages/{page_id}/settings"))
        .bearer_auth(&session)
        .json(&json!({"currency": "eur", "tips": {"amounts": [500]}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let orders_url = format!("http://{address}/pages/{page_id}/tips/orders");
    let order: Value = client
        .post(&orders_url)
        .json(&json!({"amount": 700, "user": "Viewer", "provider": "stripe"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(order["orderId"], "pi_1");
    let capture_url = format!("{orders_url}/pi_1/capture");
    *fake.lose_capture.lock().unwrap() = true;
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
    assert_eq!(api.orders.get("pi_1").unwrap().stage, Stage::Capturing);
    *fake.state.lock().unwrap() = "requires_capture";
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(api.deliveries.pending().len(), 1);
    assert_eq!(api.orders.get("pi_1").unwrap().stage, Stage::Captured);
}

#[tokio::test]
async fn pending_captures_show_the_tip_once_approved() {
    let (address, api, fake) = start_paid_fake().await;
    let client = reqwest::Client::new();
    let (_, capture_url) = paypal_tip_order_in(&address, &api, "EUR").await;
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
    assert_eq!(
        response.text().await.unwrap(),
        "PayPal is still reviewing the tip. If PayPal approves it within half an hour, the tip will be shown and the money reaches the streamer, otherwise the money is refunded."
    );
    assert!(api.deliveries.pending().is_empty());
    assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Capturing);
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
    settle(&api, Duration::ZERO).await;
    assert!(api.deliveries.pending().is_empty());
    *fake.approved.lock().unwrap() = true;
    settle(&api, Duration::ZERO).await;
    let pending = api.deliveries.pending();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].request_id, "ORDER1");
    assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Captured);
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}
