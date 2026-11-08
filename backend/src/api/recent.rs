use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use log::info;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::accounts::KnownDevice;
use crate::deliveries;
use crate::payments::{Item, Order, Payee, Stage};

use super::*;

#[derive(Deserialize)]
pub(super) struct RecentQuery {
    kind: Option<String>,
    before: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum RecentResponse {
    One(RecentList),
    All {
        tip: RecentList,
        spin: RecentList,
        effect: RecentList,
    },
}

#[derive(Serialize)]
pub(super) struct RecentList {
    items: Vec<Recent>,
    more: bool,
}

#[derive(Serialize)]
pub(super) struct Recent {
    id: String,
    at: u64,
    user: String,
    amount: u32,
    currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    spins: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effect: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tier: Option<u8>,
    #[serde(rename = "kickSubscriber", skip_serializing_if = "std::ops::Not::not")]
    kick_subscriber: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<&'static str>,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    devices: Option<Vec<RecentDevice>>,
}

#[derive(Serialize)]
pub(super) struct RecentDevice {
    name: String,
    received: bool,
}

pub(super) fn recent(
    api: &Api,
    page_id: &str,
    query: RecentQuery,
    owner: bool,
) -> Result<Json<RecentResponse>, (StatusCode, String)> {
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let known = api.accounts.known_devices(page_id);
    match query.kind.as_deref() {
        Some(kind @ ("tip" | "spin" | "effect")) => {
            let orders = api
                .orders
                .recent(page_id, kind, query.before.as_deref(), limit + 1);
            let mut deliveries = deliveries_of(api, &orders);
            Ok(Json(RecentResponse::One(recent_list(
                api,
                page_id,
                &known,
                orders,
                &mut deliveries,
                limit,
                owner,
            ))))
        }
        Some(_) => Err((StatusCode::BAD_REQUEST, "Unknown kind.".to_string())),
        None if query.before.is_some() => Err((
            StatusCode::BAD_REQUEST,
            "Give a kind to page through.".to_string(),
        )),
        None => {
            let mut by_kind = api.orders.recent_of_every_kind(page_id, limit + 1);
            let mut deliveries = deliveries_of(
                api,
                &by_kind.values().flatten().cloned().collect::<Vec<_>>(),
            );
            let mut list = |kind: &str| {
                recent_list(
                    api,
                    page_id,
                    &known,
                    by_kind.remove(kind).unwrap_or_default(),
                    &mut deliveries,
                    limit,
                    owner,
                )
            };
            Ok(Json(RecentResponse::All {
                tip: list("tip"),
                spin: list("spin"),
                effect: list("effect"),
            }))
        }
    }
}

fn deliveries_of(
    api: &Api,
    orders: &[(String, u64, Order)],
) -> HashMap<String, deliveries::Delivery> {
    let ids: Vec<String> = orders.iter().map(|(id, _, _)| id.clone()).collect();
    api.deliveries.get_many(&ids)
}

fn recent_list(
    api: &Api,
    page_id: &str,
    known: &[KnownDevice],
    mut orders: Vec<(String, u64, Order)>,
    deliveries: &mut HashMap<String, deliveries::Delivery>,
    limit: usize,
    owner: bool,
) -> RecentList {
    let more = orders.len() > limit;
    orders.truncate(limit);
    let items = orders
        .into_iter()
        .map(|(id, at, order)| {
            let delivery = deliveries.remove(&id);
            let devices = delivery
                .as_ref()
                .map(|delivery| recent_devices(api, page_id, known, &order.item, delivery));
            let tier = match order.payee {
                Payee::Twitch(tier) => Some(tier),
                _ => None,
            };
            let kick_subscriber = order.payee == Payee::Kick;
            let provider = match order.payee {
                Payee::PayPal(_) | Payee::Stripe(_) if owner => Some(order.payee.slug()),
                _ => None,
            };
            let (message, spins, effect) = match order.item {
                Item::Tip { message } => (message, None, None),
                Item::Spin { spins, message } => (message, Some(spins), None),
                Item::Effect { effect, message } => (message, None, Some(effect)),
            };
            Recent {
                id,
                at,
                user: order.user,
                amount: order.price,
                currency: order.currency,
                message,
                spins,
                effect,
                tier,
                kick_subscriber,
                provider,
                status: match &delivery {
                    None => "delivered",
                    Some(delivery) if delivery.finished => "delivered",
                    Some(_) => "waiting",
                },
                devices,
            }
        })
        .collect();
    RecentList { items, more }
}

fn recent_devices(
    api: &Api,
    page_id: &str,
    known: &[KnownDevice],
    item: &Item,
    delivery: &deliveries::Delivery,
) -> Vec<RecentDevice> {
    let mut devices: Vec<(String, bool)> = delivery
        .delivered
        .iter()
        .map(|device| (device.clone(), true))
        .collect();
    if !delivery.finished {
        let waiting: Vec<String> = match item {
            Item::Spin { .. } => api
                .remote_control
                .wheel_device(page_id)
                .into_iter()
                .collect(),
            Item::Tip { .. } | Item::Effect { .. } => api
                .remote_control
                .devices(page_id)
                .into_iter()
                .map(|device| device.id)
                .chain(known.iter().map(|device| device.id.clone()))
                .collect(),
        };
        for device in waiting {
            if !devices.iter().any(|(other, _)| *other == device) {
                devices.push((device, false));
            }
        }
    }
    devices
        .into_iter()
        .map(|(device, received)| RecentDevice {
            name: known
                .iter()
                .find(|other| other.id == device)
                .map(|other| other.name.clone())
                .unwrap_or_else(|| "Moblin".to_string()),
            received,
        })
        .collect()
}

pub(super) async fn handle_recent(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    headers: HeaderMap,
    Query(query): Query<RecentQuery>,
) -> Result<Json<RecentResponse>, Failure> {
    let user = assistant(&api, &headers, &page_id)?;
    recent(&api, &page_id, query, user.owner()).map_err(|(status, text)| Failure {
        reason: "invalid-kind",
        status,
        text,
    })
}

pub(super) async fn handle_resend(
    State(api): State<Arc<Api>>,
    Path((page_id, order_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<Value>), Failure> {
    assistant(&api, &headers, &page_id)?;
    resend(&api, &page_id, &order_id)
}

pub(super) fn resend(
    api: &Api,
    page_id: &str,
    order_id: &str,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let order = match api.orders.get(order_id) {
        Some(saved) if saved.stage == Stage::Captured && saved.order.streamer == page_id => {
            saved.order
        }
        _ => {
            return Err(failure(
                "not-found",
                StatusCode::NOT_FOUND,
                "This request does not exist.",
            ));
        }
    };
    api.deliveries
        .resend(&order.streamer, order_id, &delivery_item(&order))
        .map_err(|error| not_saved(error.into()))?;
    info!(
        "queued {:?} in order {order_id} again for remote control streamer {}",
        order.description(),
        order.streamer
    );
    api.remote_control.changes.notify(page_id, Change::Recent);
    Ok(accepted())
}

#[cfg(test)]
#[path = "tests/recent.rs"]
mod tests;
