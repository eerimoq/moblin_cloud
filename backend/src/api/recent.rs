use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use log::info;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::accounts::Page;
use crate::deliveries;
use crate::payments::{Item, Payee, Stage};

use super::*;

#[derive(Deserialize)]
pub(super) struct RecentQuery {
    kind: String,
    before: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize)]
pub(super) struct RecentResponse {
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

pub(super) async fn handle_account_recent(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Query(query): Query<RecentQuery>,
) -> Result<Json<RecentResponse>, (StatusCode, String)> {
    let SignedIn { account, .. } = signed_in(&api, &headers)?;
    recent(&api, account.page, query, true)
}

pub(super) fn recent(
    api: &Api,
    page: Page,
    query: RecentQuery,
    owner: bool,
) -> Result<Json<RecentResponse>, (StatusCode, String)> {
    if !["tip", "spin", "effect"].contains(&query.kind.as_str()) {
        return Err((StatusCode::BAD_REQUEST, "Unknown kind.".to_string()));
    }
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let mut orders = api
        .orders
        .recent(&page.id, &query.kind, query.before.as_deref(), limit + 1);
    let more = orders.len() > limit;
    orders.truncate(limit);
    let names = api.accounts.device_names(&page.id);
    let items = orders
        .into_iter()
        .map(|(id, at, order)| {
            let delivery = api.deliveries.get(&id);
            let devices = delivery
                .as_ref()
                .map(|delivery| recent_devices(api, &page, &names, &order.item, delivery));
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
    Ok(Json(RecentResponse { items, more }))
}

fn recent_devices(
    api: &Api,
    page: &Page,
    names: &HashMap<String, String>,
    item: &Item,
    delivery: &deliveries::Delivery,
) -> Vec<RecentDevice> {
    let mut devices: Vec<(String, bool)> = delivery
        .delivered
        .iter()
        .map(|device| (device.clone(), true))
        .collect();
    if !delivery.finished {
        let waiting = match item {
            Item::Spin { .. } => api
                .remote_control
                .wheel_device(&page.id)
                .into_iter()
                .collect(),
            Item::Tip { .. } | Item::Effect { .. } => {
                api.deliveries.tip_devices(&api.remote_control, &page.id)
            }
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
            name: device_name(names, &device),
            received,
        })
        .collect()
}

pub(super) async fn handle_assistant_recent(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Query(query): Query<RecentQuery>,
) -> Result<Json<RecentResponse>, Failure> {
    assistant(&api, &headers, &id)?;
    let page = page_of(&api, &id).ok_or_else(no_such_page)?;
    recent(&api, page, query, false).map_err(|(status, text)| Failure {
        reason: "invalid-kind",
        status,
        text,
    })
}

pub(super) async fn handle_account_resend(
    State(api): State<Arc<Api>>,
    Path(order_id): Path<String>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let SignedIn { account, .. } = signed_in(&api, &headers)?;
    resend(&api, &account.page.id, &order_id)
}

pub(super) async fn handle_assistant_resend(
    State(api): State<Arc<Api>>,
    Path((id, order_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<Value>), Failure> {
    assistant(&api, &headers, &id)?;
    resend(&api, &id, &order_id)
}

pub(super) fn resend(
    api: &Api,
    page: &str,
    order_id: &str,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let order = match api.orders.get(order_id) {
        Some(saved) if saved.stage == Stage::Captured && saved.order.streamer == page => {
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
    api.remote_control.changes.notify(page, Change::Recent);
    Ok(accepted())
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use futures_util::SinkExt;
    use serde_json::{Value, json};
    use tokio_tungstenite::tungstenite::Message;

    use crate::api::test_utils::*;
    use crate::deliveries;
    use crate::payments::{Item, Order, Payee};

    #[tokio::test]
    async fn recent_payments_are_listed_to_the_streamer() {
        let (address, api, _) = start_paid_fake().await;
        let (session, capture_url) = paypal_tip_order_in(&address, &api, "USD").await;
        let client = reqwest::Client::new();
        let recent = || async {
            client
                .get(format!("http://{address}/account/recent?kind=tip"))
                .bearer_auth(&session)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        };
        assert_eq!(recent().await, json!({"items": [], "more": false}));
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let listed = recent().await;
        assert_eq!(listed["more"], false);
        assert_eq!(listed["items"].as_array().unwrap().len(), 1);
        let tip = &listed["items"][0];
        assert_eq!(tip["id"], "ORDER1");
        assert_eq!(tip["user"], "Viewer");
        assert_eq!(tip["amount"], 700);
        assert_eq!(tip["currency"], "USD");
        assert_eq!(tip["provider"], "paypal");
        assert_eq!(tip["status"], "waiting");
        assert!(tip["at"].as_u64().unwrap() > 0);
        assert!(tip.get("message").is_none());
        let id = api.accounts.get("1").unwrap().page.id;
        let assisted: Value = client
            .get(format!(
                "http://{address}/account/pages/{id}/recent?kind=tip"
            ))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(assisted["items"][0]["id"], "ORDER1");
        assert!(assisted["items"][0].get("provider").is_none());
        assert_eq!(recent().await["items"][0]["devices"], json!([]));
        let key = api.accounts.get("1").unwrap().page.key.clone();
        let mut socket = connect(&address, &key).await;
        let request = receive(&mut socket).await;
        assert_eq!(request["request"]["data"]["tip"]["requestId"], "ORDER1");
        answer(&mut socket, &request, Value::Null).await;
        wait_until_added(&api).await;
        let tip = &recent().await["items"][0];
        assert_eq!(tip["status"], "delivered");
        assert_eq!(
            tip["devices"],
            json!([{"name": "Moblin 1", "received": true}])
        );
        let response = client
            .get(format!("http://{address}/account/recent?kind=tip"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn recent_payments_can_be_sent_again() {
        let (address, api, _) = start_paid_fake().await;
        let (session, capture_url) = paypal_tip_order_in(&address, &api, "USD").await;
        let client = reqwest::Client::new();
        let status = || async {
            client
                .get(format!("http://{address}/account/recent?kind=tip"))
                .bearer_auth(&session)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()["items"][0]["status"]
                .clone()
        };
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let key = api.accounts.get("1").unwrap().page.key.clone();
        let mut socket = connect(&address, &key).await;
        let acknowledge = async |socket: &mut Socket, request_id: &str| {
            let request = receive(socket).await;
            assert_eq!(request["request"]["data"]["tip"]["requestId"], request_id);
            socket
                .send(Message::text(
                    json!({"response": {"id": request["request"]["id"], "result": {"ok": {}}}})
                        .to_string(),
                ))
                .await
                .unwrap();
            wait_until_added(&api).await;
        };
        acknowledge(&mut socket, "ORDER1").await;
        assert_eq!(status().await, "delivered");
        let id = api.accounts.get("1").unwrap().page.id;
        for (n, url) in [
            format!("http://{address}/account/recent/ORDER1/resend"),
            format!("http://{address}/account/pages/{id}/recent/ORDER1/resend"),
        ]
        .into_iter()
        .enumerate()
        {
            let response = client
                .post(&url)
                .bearer_auth(&session)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::ACCEPTED);
            acknowledge(&mut socket, &format!("ORDER1-{}", n + 1)).await;
            assert_eq!(status().await, "delivered");
        }
        for url in [
            format!("http://{address}/account/recent/ORDER2/resend"),
            format!("http://{address}/account/pages/OTHER/recent/ORDER1/resend"),
        ] {
            let response = client
                .post(&url)
                .bearer_auth(&session)
                .send()
                .await
                .unwrap();
            assert!(
                [StatusCode::NOT_FOUND, StatusCode::FORBIDDEN].contains(&response.status()),
                "{url}"
            );
        }
        let response = client
            .post(format!("http://{address}/account/recent/ORDER1/resend"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(status().await, "delivered");
    }

    #[tokio::test]
    async fn recent_lists_are_paged_per_kind() {
        let (address, api) = start_api(None).await;
        let (session, _, id) = streamer(&api);
        let order = |item: Item, price: u32| Order {
            streamer: id.clone(),
            item,
            user: "Viewer".to_string(),
            payee: Payee::PayPal("MERCHANT00001".to_string()),
            price,
            currency: "USD".to_string(),
        };
        for n in 0..45 {
            let order_id = format!("TIP{n}");
            api.orders
                .insert(
                    &order_id,
                    &order(
                        Item::Tip {
                            message: Some(format!("m{n}")),
                        },
                        100 + n,
                    ),
                )
                .unwrap();
            api.orders.captured(&order_id, |_| Ok(())).unwrap();
        }
        api.orders
            .insert(
                "SPIN1",
                &order(
                    Item::Spin {
                        spins: 2,
                        message: Some("Go!".to_string()),
                    },
                    800,
                ),
            )
            .unwrap();
        api.orders
            .captured("SPIN1", |connection| {
                deliveries::insert(
                    connection,
                    &id,
                    "SPIN1",
                    &deliveries::Item::Spin {
                        user: "Viewer".to_string(),
                        message: "Go!".to_string(),
                        spins: 2,
                    },
                )
            })
            .unwrap();
        let client = reqwest::Client::new();
        let recent = |query: String| {
            let request = client
                .get(format!("http://{address}/account/recent?{query}"))
                .bearer_auth(&session);
            async move { request.send().await.unwrap() }
        };
        for kind in ["gift", "other"] {
            let response = recent(format!("kind={kind}")).await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
        let mut ids = Vec::new();
        let mut before = String::new();
        for (expected, more) in [(20, true), (20, true), (5, false)] {
            let listed: Value = recent(format!("kind=tip{before}"))
                .await
                .json()
                .await
                .unwrap();
            let items = listed["items"].as_array().unwrap();
            assert_eq!(items.len(), expected);
            assert_eq!(listed["more"], more);
            ids.extend(
                items
                    .iter()
                    .map(|item| item["id"].as_str().unwrap().to_string()),
            );
            before = format!("&before={}", ids.last().unwrap());
        }
        assert_eq!(
            ids,
            (0..45).rev().map(|n| format!("TIP{n}")).collect::<Vec<_>>()
        );
        let listed: Value = recent("kind=tip&limit=5".to_string())
            .await
            .json()
            .await
            .unwrap();
        let tips = listed["items"].as_array().unwrap();
        assert_eq!(tips.len(), 5);
        assert_eq!(listed["more"], true);
        assert_eq!(tips[0]["id"], "TIP44");
        assert_eq!(tips[0]["message"], "m44");
        assert_eq!(tips[0]["amount"], 144);
        assert_eq!(tips[0]["status"], "delivered");
        let listed: Value = recent("kind=spin".to_string()).await.json().await.unwrap();
        assert_eq!(listed["items"].as_array().unwrap().len(), 1);
        let spin = &listed["items"][0];
        assert_eq!(spin["spins"], 2);
        assert_eq!(spin["message"], "Go!");
        assert_eq!(spin["status"], "waiting");
        assert_eq!(spin["devices"], json!([]));
        api.orders
            .insert(
                "OTHER",
                &Order {
                    streamer: "other".to_string(),
                    ..order(Item::Tip { message: None }, 100)
                },
            )
            .unwrap();
        api.orders.captured("OTHER", |_| Ok(())).unwrap();
        let listed: Value = recent("kind=tip&before=OTHER".to_string())
            .await
            .json()
            .await
            .unwrap();
        assert_eq!(listed, json!({"items": [], "more": false}));
        let listed: Value = recent("kind=tip".to_string()).await.json().await.unwrap();
        assert_eq!(listed["items"][0]["id"], "TIP44");
    }
}
