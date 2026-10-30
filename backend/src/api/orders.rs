use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::Json;
use axum::http::StatusCode;
use log::{info, warn};
use serde::Serialize;
use serde_json::Value;

use crate::counters;
use crate::deliveries;
use crate::pages::format_price;
use crate::payments::{CaptureError, Item, Order, Payee, PaymentStatus, RefundError, Stage};

use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OrderResponse {
    order_id: String,
    #[serde(flatten)]
    stripe: Option<StripePayment>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StripePayment {
    client_secret: String,
    price: u32,
    currency: String,
}

pub(super) fn payee(api: &Api, id: &str, provider: Option<&str>) -> Result<Payee, Failure> {
    match provider.unwrap_or("paypal") {
        "paypal" => merchant_of(api, id).map(Payee::PayPal),
        "stripe" => stripe_of(api, id).map(Payee::Stripe),
        _ => None,
    }
    .ok_or_else(payment_unavailable)
}

pub(super) async fn start_order(api: &Api, order: Order) -> Result<OrderResponse, Failure> {
    let name = format!("{}Orders", order.payee.slug());
    let result = create_order(api, order).await;
    counted(api, &name, result)
}

async fn create_order(api: &Api, order: Order) -> Result<OrderResponse, Failure> {
    let provider = order.payee.provider();
    let created = async {
        api.orders.make_room()?;
        match &order.payee {
            Payee::PayPal(merchant_id) => {
                let paypal = api.paypal.as_ref().context("no PayPal")?;
                let order_id = paypal.create_order(&order, merchant_id).await?;
                anyhow::Ok((order_id, None))
            }
            Payee::Stripe(account) => {
                let stripe = api.stripe.as_ref().context("no Stripe")?;
                let (order_id, client_secret) = stripe.create_payment(&order, account).await?;
                Ok((
                    order_id,
                    Some(StripePayment {
                        client_secret,
                        price: order.price,
                        currency: order.currency.clone(),
                    }),
                ))
            }
            Payee::Twitch(_) | Payee::Kick => {
                anyhow::bail!("free requests are not paid")
            }
        }
    };
    let (order_id, stripe) = created.await.map_err(|error| {
        warn!(
            "creating a {provider} {} order for remote control streamer {} failed: {error:#}",
            order.item.kind(),
            order.streamer
        );
        failure(
            "provider",
            StatusCode::BAD_GATEWAY,
            &format!("{provider} could not start the payment. Try again."),
        )
    })?;
    api.orders.insert(&order_id, &order).map_err(|error| {
        warn!("{error:#}");
        failure(
            "not-saved",
            StatusCode::SERVICE_UNAVAILABLE,
            "Tips are unavailable right now. Try again later.",
        )
    })?;
    Ok(OrderResponse { order_id, stripe })
}

pub(super) fn capture_crashed() -> Failure {
    failure(
        "crashed",
        StatusCode::INTERNAL_SERVER_ERROR,
        "Something went wrong with the tip.",
    )
}

fn payment_unavailable() -> Failure {
    failure(
        "payments-unavailable",
        StatusCode::SERVICE_UNAVAILABLE,
        "Tips are unavailable right now. Try again later.",
    )
}

pub(super) async fn capture_paid(
    api: Arc<Api>,
    id: String,
    order_id: String,
    kind: &'static str,
) -> Result<(StatusCode, Json<Value>), Failure> {
    if let Err(failure) = capture_order(&api, &id, &order_id, kind).await {
        api.counters
            .bump(&format!("{kind}PaymentsFailed:{}", failure.reason));
        return Err(failure);
    }
    Ok(accepted())
}

pub(super) fn expired() -> Failure {
    failure(
        "expired",
        StatusCode::GONE,
        "This tip is unknown or expired, so no money was taken.",
    )
}

pub(super) fn pending(provider: &str, kind: &str) -> Failure {
    let text = match kind {
        "tip" => format!(
            "{provider} is still reviewing the tip. If {provider} approves it within half an hour, the tip will be shown and the money reaches the streamer, otherwise the money is refunded."
        ),
        "effect" => format!(
            "{provider} is still reviewing the tip, so the effect was not played yet. It will be played if {provider} approves it within half an hour, otherwise the money is refunded."
        ),
        _ => format!(
            "{provider} is still reviewing the tip, so the wheel was not spun yet. It will be spun if {provider} approves it within half an hour, otherwise the money is refunded."
        ),
    };
    failure("pending", StatusCode::PAYMENT_REQUIRED, &text)
}

fn too_late() -> Failure {
    failure(
        "too-late",
        StatusCode::GONE,
        "The tip took too long, so no money was taken. Try again.",
    )
}

fn refunded() -> Failure {
    failure(
        "refunded",
        StatusCode::GONE,
        "The tip took too long, so the money was refunded.",
    )
}

async fn capture_order(api: &Api, id: &str, order_id: &str, kind: &str) -> Result<(), Failure> {
    let saved = api
        .orders
        .get(order_id)
        .filter(|saved| saved.order.streamer == id && saved.order.item.kind() == kind)
        .ok_or_else(expired)?;
    let order = &saved.order;
    match saved.stage {
        Stage::Captured => return Ok(()),
        Stage::Capturing => match settle_one(api, order_id, order, saved.late).await {
            Some(PaymentStatus::NotCaptured) if saved.late => return Err(too_late()),
            Some(PaymentStatus::NotCaptured) => {}
            status => return settled(order, status),
        },
        Stage::Created if saved.late => return Err(too_late()),
        Stage::Created => {}
    }
    if kind == "spin" && api.remote_control.wheel(id).is_none() {
        if let Payee::Stripe(account) = &order.payee
            && let Some(stripe) = &api.stripe
        {
            api.orders.remove(order_id);
            if let Err(error) = stripe.cancel(order_id, account).await {
                warn!("cancelling Stripe payment {order_id} failed: {error:#}");
            }
        }
        return Err(failure(
            "no-wheel",
            StatusCode::NOT_FOUND,
            "The streamer's wheel is gone, so your tip was not taken.",
        ));
    }
    if let Err(error) = api
        .orders
        .transition(order_id, Stage::Created, Stage::Capturing)
    {
        warn!("{error:#}");
        return Err(failure(
            "not-saved",
            StatusCode::SERVICE_UNAVAILABLE,
            "Tips are unavailable right now, so no money was taken. Try again later.",
        ));
    }
    capture_at_provider(api, order_id, order, kind).await
}

async fn capture_at_provider(
    api: &Api,
    order_id: &str,
    order: &Order,
    kind: &str,
) -> Result<(), Failure> {
    let provider = order.payee.provider();
    let captured = match &order.payee {
        Payee::PayPal(merchant_id) => {
            let paypal = api.paypal.as_ref().ok_or_else(payment_unavailable)?;
            paypal.capture(order_id, order, merchant_id).await
        }
        Payee::Stripe(account) => {
            let stripe = api.stripe.as_ref().ok_or_else(payment_unavailable)?;
            stripe.capture(order_id, order, account).await
        }
        Payee::Twitch(_) | Payee::Kick => return Err(payment_unavailable()),
    };
    let result = match captured {
        Ok(()) => {
            info!(
                "{:?} paid {} {} in {provider} order {order_id}",
                order.user,
                format_price(order.price),
                order.currency
            );
            record_capture(api, order_id, order);
            Ok(())
        }
        Err(CaptureError::Unknown) => settled(order, settle_one(api, order_id, order, false).await),
        Err(CaptureError::NotApproved) => Err(failure(
            "not-approved",
            StatusCode::PAYMENT_REQUIRED,
            "The tip was not completed.",
        )),
        Err(CaptureError::Pending) => Err(pending(provider, kind)),
        Err(CaptureError::Refused) => Err(failure(
            "declined",
            StatusCode::PAYMENT_REQUIRED,
            &format!("{provider} declined the payment."),
        )),
    };
    if let Err(failure) = &result {
        api.counters.bump(&format!(
            "{}PaymentsFailed:{}",
            order.payee.slug(),
            failure.reason
        ));
    }
    result
}

fn settled(order: &Order, status: Option<PaymentStatus>) -> Result<(), Failure> {
    let provider = order.payee.provider();
    match status {
        Some(PaymentStatus::Captured) => Ok(()),
        Some(PaymentStatus::Pending) => Err(pending(provider, order.item.kind())),
        Some(PaymentStatus::Gone) => Err(expired()),
        Some(PaymentStatus::Refunded) => Err(refunded()),
        Some(PaymentStatus::NotCaptured) | None => Err(failure(
            "unknown",
            StatusCode::BAD_GATEWAY,
            &format!(
                "{provider} did not answer. If the money was taken, {} or the money refunded.",
                match order.item.kind() {
                    "tip" => "the tip will still be shown",
                    "effect" => "the effect will still be played",
                    _ => "the wheel will still be spun",
                }
            ),
        )),
    }
}

async fn settle_one(api: &Api, order_id: &str, order: &Order, late: bool) -> Option<PaymentStatus> {
    let provider = order.payee.provider();
    let status = match &order.payee {
        Payee::PayPal(merchant_id) => {
            api.paypal
                .as_ref()?
                .status(order_id, order, merchant_id)
                .await
        }
        Payee::Stripe(account) => api.stripe.as_ref()?.status(order_id, order, account).await,
        Payee::Twitch(_) | Payee::Kick => return None,
    };
    let status = match status {
        Ok(status) => status,
        Err(error) => {
            warn!("settling {provider} order {order_id} failed: {error:#}");
            return None;
        }
    };
    match status {
        PaymentStatus::Captured if late => return refund_late(api, order_id, order).await,
        PaymentStatus::Captured => {
            info!(
                "{:?} paid {} {} in {provider} order {order_id}, found when settling it",
                order.user,
                format_price(order.price),
                order.currency
            );
            record_capture(api, order_id, order);
        }
        PaymentStatus::Pending => {
            info!("{provider} is still reviewing order {order_id}");
        }
        PaymentStatus::NotCaptured if !late => {
            info!("{provider} order {order_id} is not captured yet");
        }
        PaymentStatus::NotCaptured => {
            info!("{provider} order {order_id} was never captured, giving it up");
            api.orders.remove(order_id);
            cancel_expired(api, order_id, order).await;
        }
        PaymentStatus::Gone | PaymentStatus::Refunded => {
            info!("{provider} order {order_id} is gone");
            api.orders.remove(order_id);
        }
    }
    Some(status)
}

async fn refund_late(api: &Api, order_id: &str, order: &Order) -> Option<PaymentStatus> {
    let provider = order.payee.provider();
    let slug = order.payee.slug();
    let refunded = match &order.payee {
        Payee::PayPal(merchant_id) => api.paypal.as_ref()?.refund(order_id, merchant_id).await,
        Payee::Stripe(account) => api.stripe.as_ref()?.refund(order_id, account).await,
        Payee::Twitch(_) | Payee::Kick => return None,
    };
    match refunded {
        Ok(()) => {
            info!(
                "{:?} paid {} {} in {provider} order {order_id} too late, refunded it",
                order.user,
                format_price(order.price),
                order.currency
            );
            api.orders.remove(order_id);
            api.counters.bump(&format!("{}Refunds", order.item.kind()));
            api.counters.bump(&format!("{slug}Refunds"));
            Some(PaymentStatus::Refunded)
        }
        Err(RefundError::Refused) => {
            warn!("{provider} refused to refund order {order_id}, delivering it late");
            api.counters.bump(&format!("{slug}RefundsFailed:provider"));
            record_capture(api, order_id, order);
            Some(PaymentStatus::Captured)
        }
        Err(RefundError::Unknown) => {
            api.counters.bump(&format!("{slug}RefundsFailed:unknown"));
            None
        }
    }
}

pub async fn settle(api: &Api, min_age: Duration) {
    for saved in api.orders.unsettled(min_age) {
        settle_one(api, &saved.id, &saved.order, saved.late).await;
    }
}

pub(super) async fn expire_orders(api: &Api) {
    for (order_id, order) in api.orders.expire() {
        cancel_expired(api, &order_id, &order).await;
    }
}

async fn cancel_expired(api: &Api, order_id: &str, order: &Order) {
    if let Payee::Stripe(account) = &order.payee
        && let Some(stripe) = &api.stripe
    {
        match stripe.cancel(order_id, account).await {
            Ok(()) => info!("cancelled expired Stripe payment {order_id}"),
            Err(error) => {
                warn!("cancelling expired Stripe payment {order_id} failed: {error:#}")
            }
        }
    }
}

pub async fn settle_orders(api: Arc<Api>) {
    loop {
        settle(&api, Duration::from_secs(60)).await;
        expire_orders(&api).await;
        api.orders.prune();
        api.allowances.prune();
        api.deliveries.expire();
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

pub(super) fn delivery_item(order: &Order) -> deliveries::Item {
    match &order.item {
        Item::Tip { message } => deliveries::Item::Tip {
            user: order.user.clone(),
            message: message.clone().unwrap_or_default(),
            amount: order.price,
            currency: order.currency.clone(),
        },
        Item::Spin { spins, message } => deliveries::Item::Spin {
            user: order.user.clone(),
            message: message.clone().unwrap_or_default(),
            spins: *spins,
        },
        Item::Effect { effect, message } => deliveries::Item::Effect {
            user: order.user.clone(),
            message: message.clone().unwrap_or_default(),
            effect: effect.clone(),
        },
    }
}

fn record_capture(api: &Api, order_id: &str, order: &Order) {
    let item = delivery_item(order);
    let saved = api.orders.captured(order_id, |connection| {
        deliveries::insert(connection, &order.streamer, order_id, &item)?;
        counters::bump(connection, &format!("{}Payments", order.item.kind()));
        counters::bump(connection, &format!("{}Payments", order.payee.slug()));
        Ok(())
    });
    match saved {
        Ok(false) => {}
        Ok(true) => {
            api.deliveries.notify();
            api.remote_control
                .changes
                .notify(&order.streamer, Change::Recent);
            info!(
                "queued {:?} in order {order_id} for remote control streamer {}",
                order.description(),
                order.streamer
            );
        }
        Err(error) => warn!("{error:#}, settling it later"),
    }
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
        let (session, key, id) = streamer(&address, &api).await;
        connect_stripe(&api);
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"currency": "SEK", "spin": {"options": [{"spins": 1, "price": 250}]}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut socket = connect(&address, &key).await;
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
        let orders_url = format!("http://{address}/pages/{id}/spin/orders");
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
            if !api.remote_control.is_connected(&id) {
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
        paypal_tip_order_as(address, api).await.1
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
        let counters = api.counters.all();
        let count = |name: &str| {
            counters
                .iter()
                .find(|counter| counter.name == name)
                .map_or(0, |counter| counter.total)
        };
        assert_eq!(count("tipPayments"), 1);
        assert_eq!(count("paypalOrders"), 1);
        assert_eq!(count("paypalPaymentsFailed:unknown"), 1);
        assert_eq!(count("paypalPayments"), 1);
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
        let counters = api.counters.all();
        let count = |name: &str| {
            counters
                .iter()
                .find(|counter| counter.name == name)
                .map_or(0, |counter| counter.total)
        };
        assert_eq!(count("tipRefunds"), 1);
        assert_eq!(count("paypalRefunds"), 1);
        assert_eq!(count("tipPayments"), 0);
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
        let count = |name: &str| {
            api.counters
                .all()
                .iter()
                .find(|counter| counter.name == name)
                .map_or(0, |counter| counter.total)
        };
        assert_eq!(count("tipPaymentsFailed:declined"), 1);
        assert_eq!(count("paypalPaymentsFailed:declined"), 1);
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
        let (session, _, id) = streamer(&address, &api).await;
        connect_stripe(&api);
        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"currency": "eur", "tips": {"amounts": [500]}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let orders_url = format!("http://{address}/pages/{id}/tips/orders");
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
        let (session, _, id) = streamer(&address, &api).await;
        connect_stripe(&api);
        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"currency": "eur", "tips": {"amounts": [500]}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let orders_url = format!("http://{address}/pages/{id}/tips/orders");
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
        let (session, _, id) = streamer(&address, &api).await;
        connect_stripe(&api);
        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"currency": "eur", "tips": {"amounts": [500]}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let orders_url = format!("http://{address}/pages/{id}/tips/orders");
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
}
