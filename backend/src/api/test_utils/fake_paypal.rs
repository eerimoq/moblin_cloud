use std::sync::Arc;

use axum::extract::{Path, Query};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use serde_json::{Value, json};

use crate::paypal::PayPal;

#[derive(Clone, Default)]
pub(crate) struct FakePayPal {
    pub(crate) order: Arc<std::sync::Mutex<Option<Value>>>,
    pub(crate) referral: Arc<std::sync::Mutex<Option<Value>>>,
    pub(crate) seller: Arc<std::sync::Mutex<Option<Value>>>,
    pub(crate) list_sellers: Arc<std::sync::Mutex<bool>>,
    pub(crate) verification: Arc<std::sync::Mutex<Option<Value>>>,
    pub(crate) captured: Arc<std::sync::Mutex<bool>>,
    pub(crate) lose_capture: Arc<std::sync::Mutex<bool>>,
    pub(crate) capture_later: Arc<std::sync::Mutex<bool>>,
    pub(crate) fail_capture: Arc<std::sync::Mutex<bool>>,
    pub(crate) lookups_fail: Arc<std::sync::Mutex<bool>>,
    pub(crate) approved: Arc<std::sync::Mutex<bool>>,
    pub(crate) refuse_capture: Arc<std::sync::Mutex<bool>>,
    pub(crate) gone: Arc<std::sync::Mutex<bool>>,
    pub(crate) refunds: Arc<std::sync::Mutex<Vec<String>>>,
    pub(crate) refuse_refund: Arc<std::sync::Mutex<bool>>,
    pub(crate) decline_capture: Arc<std::sync::Mutex<bool>>,
}

pub(crate) fn new_paypal(url: String) -> PayPal {
    PayPal::new(
        reqwest::Client::new(),
        url,
        "id".to_string(),
        "secret".to_string(),
        "PARTNER000001".to_string(),
        Some("BN".to_string()),
        Some("WH".to_string()),
    )
}

pub(crate) fn payer_of(assertion: &str) -> Value {
    let payload = assertion.split('.').nth(1).unwrap();
    serde_json::from_slice::<Value>(&BASE64_STANDARD.decode(payload).unwrap()).unwrap()["payer_id"]
        .clone()
}

pub(crate) async fn start_paypal() -> (String, FakePayPal) {
    let fake = FakePayPal::default();
    let order = fake.order.clone();
    let router = Router::new()
        .route(
            "/v1/notifications/verify-webhook-signature",
            post({
                let verification = fake.verification.clone();
                move |Json(body): Json<Value>| async move {
                    let good = body["transmission_sig"] == "good" && body["webhook_id"] == "WH";
                    *verification.lock().unwrap() = Some(body);
                    Json(json!({
                        "verification_status": if good { "SUCCESS" } else { "FAILURE" },
                    }))
                }
            }),
        )
        .route(
            "/v2/customer/partner-referrals",
            post({
                let referral = fake.referral.clone();
                move |headers: HeaderMap, Json(body): Json<Value>| async move {
                    *referral.lock().unwrap() = Some(json!({
                        "attribution": headers["paypal-partner-attribution-id"].to_str().unwrap(),
                        "body": body,
                    }));
                    (
                        StatusCode::CREATED,
                        Json(json!({"links": [
                            {"href": "https://api-m.sandbox.paypal.com/v2/customer/partner-referrals/x", "rel": "self", "method": "GET"},
                            {"href": "https://www.sandbox.paypal.com/bizsignup/x", "rel": "action_url", "method": "GET"},
                        ]})),
                    )
                }
            }),
        )
        .route(
            "/v1/customer/partners/PARTNER000001/merchant-integrations",
            get({
                let seller = fake.seller.clone();
                let list_sellers = fake.list_sellers.clone();
                move |Query(query): Query<std::collections::HashMap<String, String>>| async move {
                    match seller.lock().unwrap().clone() {
                        Some(seller) if seller["tracking_id"] == query["tracking_id"].as_str() => {
                            let integration = json!({
                                "merchant_id": seller["merchant_id"],
                                "tracking_id": seller["tracking_id"],
                                "links": [],
                            });
                            if *list_sellers.lock().unwrap() {
                                (StatusCode::OK, Json(json!([integration])))
                            } else {
                                (StatusCode::OK, Json(integration))
                            }
                        }
                        _ => (
                            StatusCode::NOT_FOUND,
                            Json(json!({"name": "RESOURCE_NOT_FOUND"})),
                        ),
                    }
                }
            }),
        )
        .route(
            "/v1/customer/partners/PARTNER000001/merchant-integrations/{merchant}",
            get({
                let seller = fake.seller.clone();
                move |Path(merchant): Path<String>| async move {
                    let seller = seller.lock().unwrap().clone().unwrap();
                    assert_eq!(seller["merchant_id"], merchant.as_str());
                    Json(seller)
                }
            }),
        )
        .route(
            "/v1/oauth2/token",
            post(|| async {
                Json(json!({"access_token": "t", "token_type": "Bearer", "expires_in": 3600}))
            }),
        )
        .route(
            "/v2/checkout/orders",
            post({
                let order = order.clone();
                let captured = fake.captured.clone();
                move |headers: HeaderMap, Json(mut request): Json<Value>| async move {
                    request["assertion"] =
                        json!(headers["paypal-auth-assertion"].to_str().unwrap());
                    *order.lock().unwrap() = Some(request);
                    *captured.lock().unwrap() = false;
                    (
                        StatusCode::CREATED,
                        Json(json!({
                            "id": "ORDER1",
                            "status": "CREATED",
                        })),
                    )
                }
            }),
        )
        .route(
            "/v2/checkout/orders/{id}/capture",
            post({
                let order = order.clone();
                let captured = fake.captured.clone();
                let lose_capture = fake.lose_capture.clone();
                let capture_later = fake.capture_later.clone();
                let decline_capture = fake.decline_capture.clone();
                let fail_capture = fake.fail_capture.clone();
                let refuse_capture = fake.refuse_capture.clone();
                move |headers: HeaderMap, Path(id): Path<String>| async move {
                    assert_eq!(
                        payer_of(headers["paypal-auth-assertion"].to_str().unwrap()),
                        payer_of(
                            order.lock().unwrap().as_ref().unwrap()["assertion"]
                                .as_str()
                                .unwrap()
                        )
                    );
                    let amount =
                        order.lock().unwrap().as_ref().unwrap()["purchase_units"][0]["amount"]
                            .clone();
                    if id != "ORDER1" {
                        return (
                            StatusCode::UNPROCESSABLE_ENTITY,
                            Json(json!({"details": [{"issue": "ORDER_NOT_APPROVED"}]})),
                        );
                    }
                    if *fail_capture.lock().unwrap() {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"name": "INTERNAL_SERVER_ERROR"})),
                        );
                    }
                    if *refuse_capture.lock().unwrap() {
                        return (
                            StatusCode::UNPROCESSABLE_ENTITY,
                            Json(json!({"details": [{"issue": "INSTRUMENT_DECLINED"}]})),
                        );
                    }
                    if *captured.lock().unwrap() {
                        return (
                            StatusCode::UNPROCESSABLE_ENTITY,
                            Json(json!({"details": [{"issue": "ORDER_ALREADY_CAPTURED"}]})),
                        );
                    }
                    if *decline_capture.lock().unwrap() {
                        return (
                            StatusCode::CREATED,
                            Json(json!({
                                "id": id,
                                "status": "COMPLETED",
                                "purchase_units": [{"payments": {"captures": [
                                    {"id": "CAPTURE1", "status": "DECLINED", "amount": amount},
                                ]}}],
                            })),
                        );
                    }
                    if *capture_later.lock().unwrap() {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"name": "INTERNAL_SERVER_ERROR"})),
                        );
                    }
                    *captured.lock().unwrap() = true;
                    if *lose_capture.lock().unwrap() {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"name": "INTERNAL_SERVER_ERROR"})),
                        );
                    }
                    let status = if amount["currency_code"] == "EUR" {
                        "PENDING"
                    } else {
                        "COMPLETED"
                    };
                    (
                        StatusCode::CREATED,
                        Json(json!({
                            "id": id,
                            "status": "COMPLETED",
                            "purchase_units": [{"payments": {"captures": [
                                {"id": "CAPTURE1", "status": status, "amount": amount},
                            ]}}],
                        })),
                    )
                }
            }),
        )
        .route(
            "/v2/checkout/orders/{id}",
            get({
                let order = order.clone();
                let captured = fake.captured.clone();
                let lookups_fail = fake.lookups_fail.clone();
                let approved = fake.approved.clone();
                let gone = fake.gone.clone();
                move |Path(id): Path<String>| async move {
                    if *lookups_fail.lock().unwrap() {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"name": "INTERNAL_SERVER_ERROR"})),
                        );
                    }
                    if id != "ORDER1" || *gone.lock().unwrap() {
                        return (
                            StatusCode::NOT_FOUND,
                            Json(json!({"name": "RESOURCE_NOT_FOUND"})),
                        );
                    }
                    let amount =
                        order.lock().unwrap().as_ref().unwrap()["purchase_units"][0]["amount"]
                            .clone();
                    if !*captured.lock().unwrap() {
                        return (StatusCode::OK, Json(json!({"id": id, "status": "APPROVED"})));
                    }
                    let status = if amount["currency_code"] == "EUR" && !*approved.lock().unwrap() {
                        "PENDING"
                    } else {
                        "COMPLETED"
                    };
                    (
                        StatusCode::OK,
                        Json(json!({
                            "id": id,
                            "status": "COMPLETED",
                            "purchase_units": [{"payments": {"captures": [
                                {"id": "CAPTURE1", "status": status, "amount": amount},
                            ]}}],
                        })),
                    )
                }
            }),
        )
        .route(
            "/v2/payments/captures/{id}/refund",
            post({
                let refunds = fake.refunds.clone();
                let refuse_refund = fake.refuse_refund.clone();
                move |headers: HeaderMap, Path(id): Path<String>| async move {
                    assert_eq!(id, "CAPTURE1");
                    if *refuse_refund.lock().unwrap() {
                        return (
                            StatusCode::FORBIDDEN,
                            Json(json!({"name": "NOT_AUTHORIZED"})),
                        );
                    }
                    refunds
                        .lock()
                        .unwrap()
                        .push(headers["paypal-request-id"].to_str().unwrap().to_string());
                    (
                        StatusCode::CREATED,
                        Json(json!({"id": "REFUND1", "status": "COMPLETED"})),
                    )
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await });
    (format!("http://{address}"), fake)
}
