use std::sync::Arc;

use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};

use super::*;
use crate::stripe::Stripe;

#[derive(Clone, Default)]
pub(crate) struct FakeStripe {
    pub(crate) accounts: Arc<std::sync::Mutex<Vec<Value>>>,
    pub(crate) link: Arc<std::sync::Mutex<Option<Value>>>,
    pub(crate) active: Arc<std::sync::Mutex<bool>>,
    pub(crate) payment: Arc<std::sync::Mutex<Option<Value>>>,
    pub(crate) state: Arc<std::sync::Mutex<&'static str>>,
    pub(crate) cancelled: Arc<std::sync::Mutex<Vec<String>>>,
    pub(crate) lose_capture: Arc<std::sync::Mutex<bool>>,
    pub(crate) processing: Arc<std::sync::Mutex<bool>>,
    pub(crate) refunds: Arc<std::sync::Mutex<Vec<(String, String)>>>,
}

pub(crate) fn new_stripe(url: String) -> Stripe {
    Stripe::new(
        reqwest::Client::new(),
        url,
        "sk_test_secret".to_string(),
        "pk_test_public".to_string(),
    )
}

pub(crate) async fn start_stripe() -> (String, FakeStripe) {
    let fake = FakeStripe::default();
    *fake.state.lock().unwrap() = "requires_payment_method";
    let router = Router::new()
        .route(
            "/v2/core/accounts",
            post({
                let accounts = fake.accounts.clone();
                move |headers: HeaderMap, Json(body): Json<Value>| async move {
                    assert!(headers.contains_key("stripe-version"));
                    accounts.lock().unwrap().push(body);
                    Json(json!({"id": "acct_1", "object": "v2.core.account"}))
                }
            }),
        )
        .route(
            "/v2/core/account_links",
            post({
                let link = fake.link.clone();
                move |Json(body): Json<Value>| async move {
                    *link.lock().unwrap() = Some(body);
                    Json(json!({"url": "https://connect.stripe.com/setup/x"}))
                }
            }),
        )
        .route(
            "/v2/core/accounts/{account}",
            get({
                let active = fake.active.clone();
                move |Path(account): Path<String>| async move {
                    assert_eq!(account, "acct_1");
                    let status = if *active.lock().unwrap() {
                        "active"
                    } else {
                        "pending"
                    };
                    Json(json!({"id": account, "configuration": {"merchant": {
                        "capabilities": {"card_payments": {"status": status}},
                    }}}))
                }
            }),
        )
        .route(
            "/v1/payment_intents",
            post({
                let payment = fake.payment.clone();
                move |headers: HeaderMap,
                      axum::Form(form): axum::Form<
                    std::collections::HashMap<String, String>,
                >| async move {
                    *payment.lock().unwrap() = Some(json!({
                        "account": headers["stripe-account"].to_str().unwrap(),
                        "form": form,
                    }));
                    Json(json!({
                        "id": "pi_1",
                        "client_secret": "pi_1_secret_2",
                        "status": "requires_payment_method",
                    }))
                }
            }),
        )
        .route(
            "/v1/payment_intents/{id}/capture",
            post({
                let payment = fake.payment.clone();
                let state = fake.state.clone();
                let lose_capture = fake.lose_capture.clone();
                let processing = fake.processing.clone();
                move |headers: HeaderMap, Path(id): Path<String>| async move {
                    let payment = payment.lock().unwrap().clone().unwrap();
                    assert_eq!(headers["stripe-account"], "acct_1");
                    let mut state = state.lock().unwrap();
                    if id != "pi_1" || *state != "requires_capture" {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({"error": {
                                "code": "payment_intent_unexpected_state",
                                "payment_intent": {"id": id, "status": *state},
                            }})),
                        );
                    }
                    if *processing.lock().unwrap() {
                        *state = "processing";
                        return (
                            StatusCode::OK,
                            Json(json!({
                                "id": id,
                                "status": "processing",
                                "amount_received": 0,
                                "currency": payment["form"]["currency"],
                            })),
                        );
                    }
                    *state = "succeeded";
                    if *lose_capture.lock().unwrap() {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": {"type": "api_error"}})),
                        );
                    }
                    (
                        StatusCode::OK,
                        Json(json!({
                            "id": id,
                            "status": "succeeded",
                            "amount_received": payment["form"]["amount"]
                                .as_str()
                                .unwrap()
                                .parse::<u32>()
                                .unwrap(),
                            "currency": payment["form"]["currency"],
                        })),
                    )
                }
            }),
        )
        .route(
            "/v1/payment_intents/{id}",
            get({
                let payment = fake.payment.clone();
                let state = fake.state.clone();
                move |headers: HeaderMap, Path(id): Path<String>| async move {
                    assert_eq!(headers["stripe-account"], "acct_1");
                    if id != "pi_1" {
                        return (
                            StatusCode::NOT_FOUND,
                            Json(json!({"error": {"code": "resource_missing"}})),
                        );
                    }
                    let payment = payment.lock().unwrap().clone().unwrap();
                    let state = *state.lock().unwrap();
                    let amount = payment["form"]["amount"]
                        .as_str()
                        .unwrap()
                        .parse::<u32>()
                        .unwrap();
                    (
                        StatusCode::OK,
                        Json(json!({
                            "id": id,
                            "status": state,
                            "amount_received": if state == "succeeded" { amount } else { 0 },
                            "currency": payment["form"]["currency"],
                        })),
                    )
                }
            }),
        )
        .route(
            "/v1/refunds",
            post({
                let refunds = fake.refunds.clone();
                move |headers: HeaderMap, body: String| async move {
                    assert_eq!(headers["stripe-account"], "acct_1");
                    refunds.lock().unwrap().push((
                        body,
                        headers["idempotency-key"].to_str().unwrap().to_string(),
                    ));
                    Json(json!({"id": "re_1", "status": "succeeded"}))
                }
            }),
        )
        .route(
            "/v1/payment_intents/{id}/cancel",
            post({
                let cancelled = fake.cancelled.clone();
                move |Path(id): Path<String>| async move {
                    cancelled.lock().unwrap().push(id);
                    Json(json!({"status": "canceled"}))
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await });
    (format!("http://{address}"), fake)
}

pub(crate) async fn start_stripe_api() -> (String, Arc<Api>, FakeStripe) {
    let (stripe_url, fake) = start_stripe().await;
    let (address, api) = start_api_with(None, Some(new_stripe(stripe_url)), None, None, None).await;
    (address, api, fake)
}
