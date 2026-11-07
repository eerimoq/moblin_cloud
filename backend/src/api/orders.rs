use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{MethodRouter, post};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::counters;
use crate::deliveries;
use crate::pages::format_price;
use crate::payments::{CaptureError, Item, Order, Payee, PaymentStatus, RefundError, Stage};
use crate::paypal::PayPal;
use crate::stripe::Stripe;

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

pub(super) fn payee(
    api: &Api,
    account: Option<&Account>,
    provider: Option<&str>,
) -> Result<Payee, Failure> {
    let (merchant_id, stripe_account_id) =
        account.map_or((None, None), |account| payment_targets(api, account));
    match provider.unwrap_or("paypal") {
        "paypal" => merchant_id.map(Payee::PayPal),
        "stripe" => stripe_account_id.map(Payee::Stripe),
        _ => None,
    }
    .ok_or_else(payment_unavailable)
}

enum Provider<'a> {
    PayPal(&'a PayPal, &'a str),
    Stripe(&'a Stripe, &'a str),
}

impl<'a> Provider<'a> {
    fn of(api: &'a Api, payee: &'a Payee) -> Option<Self> {
        match payee {
            Payee::PayPal(merchant_id) => Some(Provider::PayPal(api.paypal.as_ref()?, merchant_id)),
            Payee::Stripe(account) => Some(Provider::Stripe(api.stripe.as_ref()?, account)),
            Payee::Twitch(_) | Payee::Kick => None,
        }
    }

    async fn create(&self, order: &Order) -> Result<(String, Option<String>)> {
        match self {
            Provider::PayPal(paypal, merchant_id) => {
                Ok((paypal.create_order(order, merchant_id).await?, None))
            }
            Provider::Stripe(stripe, account) => {
                let (order_id, client_secret) = stripe.create_payment(order, account).await?;
                Ok((order_id, Some(client_secret)))
            }
        }
    }

    async fn capture(&self, order_id: &str, order: &Order) -> Result<(), CaptureError> {
        match self {
            Provider::PayPal(paypal, merchant_id) => {
                paypal.capture(order_id, order, merchant_id).await
            }
            Provider::Stripe(stripe, account) => stripe.capture(order_id, order, account).await,
        }
    }

    async fn status(&self, order_id: &str, order: &Order) -> Result<PaymentStatus> {
        match self {
            Provider::PayPal(paypal, merchant_id) => {
                paypal.status(order_id, order, merchant_id).await
            }
            Provider::Stripe(stripe, account) => stripe.status(order_id, order, account).await,
        }
    }

    async fn refund(&self, order_id: &str) -> Result<(), RefundError> {
        match self {
            Provider::PayPal(paypal, merchant_id) => paypal.refund(order_id, merchant_id).await,
            Provider::Stripe(stripe, account) => stripe.refund(order_id, account).await,
        }
    }

    async fn cancel(&self, order_id: &str) -> Result<bool> {
        match self {
            Provider::PayPal(..) => Ok(false),
            Provider::Stripe(stripe, account) => {
                stripe.cancel(order_id, account).await.map(|_| true)
            }
        }
    }
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
        let (order_id, client_secret) = Provider::of(api, &order.payee)
            .context("no such provider")?
            .create(&order)
            .await?;
        anyhow::Ok((
            order_id,
            client_secret.map(|client_secret| StripePayment {
                client_secret,
                price: order.price,
                currency: order.currency.clone(),
            }),
        ))
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

#[derive(Deserialize)]
pub(super) struct PaidRequest {
    user: Option<String>,
    message: Option<String>,
    #[serde(default)]
    provider: Option<String>,
}

impl PaidRequest {
    pub(super) fn payee(&self, api: &Api, account: Option<&Account>) -> Result<Payee, Failure> {
        payee(api, account, self.provider.as_deref())
    }

    pub(super) fn order(
        self,
        id: String,
        item: impl FnOnce(Option<String>) -> Item,
        payee: Payee,
        price: u32,
        currency: &str,
    ) -> Result<Order, Failure> {
        Ok(Order {
            streamer: id,
            item: item(viewer_message(self.message)?),
            user: user_name(self.user),
            payee,
            price,
            currency: currency.to_string(),
        })
    }
}

pub(super) async fn place_order(
    api: &Api,
    kind: &str,
    order: Result<Order, Failure>,
) -> Result<Json<OrderResponse>, Failure> {
    let result = match order {
        Ok(order) => start_order(api, order).await.map(Json),
        Err(failure) => Err(failure),
    };
    counted(api, &format!("{kind}Orders"), result)
}

pub(super) fn capture(kind: &'static str) -> MethodRouter<Arc<Api>> {
    post(
        move |State(api): State<Arc<Api>>, Path((page_id, order_id)): Path<(String, String)>| async move {
            tokio::spawn(capture_paid(api, page_id, order_id, kind))
                .await
                .unwrap_or_else(|_| {
                    Err(failure(
                        "crashed",
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Something went wrong with the tip.",
                    ))
                })
        },
    )
}

fn payment_unavailable() -> Failure {
    failure(
        "payments-unavailable",
        StatusCode::SERVICE_UNAVAILABLE,
        "Tips are unavailable right now. Try again later.",
    )
}

async fn capture_paid(
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
        if let Payee::Stripe(_) = &order.payee
            && let Some(provider) = Provider::of(api, &order.payee)
        {
            api.orders.remove(order_id);
            if let Err(error) = provider.cancel(order_id).await {
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
    let captured = Provider::of(api, &order.payee)
        .ok_or_else(payment_unavailable)?
        .capture(order_id, order)
        .await;
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
    let status = match Provider::of(api, &order.payee)?
        .status(order_id, order)
        .await
    {
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
    match Provider::of(api, &order.payee)?.refund(order_id).await {
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
    if let Some(provider) = Provider::of(api, &order.payee) {
        match provider.cancel(order_id).await {
            Ok(false) => {}
            Ok(true) => info!("cancelled expired Stripe payment {order_id}"),
            Err(error) => {
                warn!("cancelling expired Stripe payment {order_id} failed: {error:#}")
            }
        }
    }
}

pub async fn settle_orders(api: Arc<Api>) {
    loop {
        settle(&api, api.timing.settle).await;
        expire_orders(&api).await;
        api.orders.prune();
        api.allowances.prune();
        api.deliveries.expire();
        tokio::time::sleep(api.timing.settle).await;
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
#[path = "tests/orders.rs"]
mod tests;
