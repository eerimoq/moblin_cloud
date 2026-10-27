use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::accounts::Platform;
use crate::paypal::Seller;
use crate::stripe;

use super::*;

#[derive(Serialize)]
pub(super) struct SignUpResponse {
    url: String,
}

pub(super) async fn handle_account_paypal_connect(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<SignUpResponse>, (StatusCode, String)> {
    let signed_in = signed_in(&api, &headers)?;
    if !signed_in.fresh {
        return Err((
            StatusCode::FORBIDDEN,
            "sign in again to connect PayPal".to_string(),
        ));
    }
    let paypal = api.paypal.as_ref().ok_or_else(payments_unavailable)?;
    let tracking_id = api.remote_control.new_request_id().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "PayPal could not be connected".to_string(),
    ))?;
    let url = paypal
        .referral(
            &tracking_id,
            &format!("{}/dashboard/", api.auth.frontend_url),
        )
        .await
        .map_err(|error| {
            warn!(
                "creating a PayPal referral for {} failed: {error:#}",
                signed_in.id
            );
            paypal_unreachable()
        })?;
    api.accounts
        .start_paypal(&signed_in.id, tracking_id.clone())
        .map_err(account_error)?;
    info!(
        "{} started connecting PayPal with tracking ID {tracking_id}",
        signed_in.id
    );
    Ok(Json(SignUpResponse { url }))
}

fn paypal_unreachable() -> (StatusCode, String) {
    (
        StatusCode::BAD_GATEWAY,
        "PayPal could not be reached".to_string(),
    )
}

pub(super) async fn handle_account_paypal_check(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let SignedIn { id, account, .. } = signed_in(&api, &headers)?;
    let page = account.page.id;
    let paypal = api.paypal.as_ref().ok_or_else(payments_unavailable)?;
    if let Some(tracking_id) = account.paypal_tracking_id {
        let seller = paypal.seller(&tracking_id).await.map_err(|error| {
            warn!("looking up PayPal tracking ID {tracking_id} of {id} failed: {error:#}");
            paypal_unreachable()
        })?;
        let seller = seller.ok_or((
            StatusCode::NOT_FOUND,
            "PayPal has not connected an account yet".to_string(),
        ))?;
        finish_onboarding(&api, &id, &tracking_id, seller)?;
    }
    let settings = api.pages.get(&page);
    Ok(settings_response(&api, page, settings))
}

pub(super) async fn handle_account_paypal_disconnect(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let SignedIn { id, account, .. } = signed_in(&api, &headers)?;
    let page = account.page.id;
    api.accounts.forget_paypal(&id).map_err(account_error)?;
    info!("{id} disconnected PayPal");
    let settings = api.pages.get(&page);
    Ok(settings_response(&api, page, settings))
}

fn finish_onboarding(
    api: &Api,
    id: &str,
    tracking_id: &str,
    seller: Seller,
) -> Result<(), (StatusCode, String)> {
    if !seller.permissions_granted {
        info!(
            "PayPal merchant {} of {id} did not grant the permissions",
            seller.merchant_id
        );
        return Err((
            StatusCode::CONFLICT,
            "the PayPal account did not grant the permissions".to_string(),
        ));
    }
    if !seller.payments_receivable || !seller.email_confirmed {
        info!(
            "PayPal merchant {} of {id} can't receive payments yet (payments receivable {}, email confirmed {})",
            seller.merchant_id, seller.payments_receivable, seller.email_confirmed
        );
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "the PayPal account can't receive payments yet".to_string(),
        ));
    }
    api.accounts
        .connect_paypal(id, tracking_id, seller.merchant_id.clone())
        .map_err(account_error)?;
    info!("{id} connected PayPal merchant {}", seller.merchant_id);
    Ok(())
}

pub(super) async fn handle_paypal_webhook(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    body: String,
) -> StatusCode {
    let Some(paypal) = api.paypal.as_ref().filter(|paypal| paypal.takes_webhooks()) else {
        return StatusCode::SERVICE_UNAVAILABLE;
    };
    let Ok(event) = serde_json::from_str::<Value>(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    match paypal.verify_webhook(&headers, &body).await {
        Ok(true) => {}
        Ok(false) => {
            warn!("PayPal webhook {} has a bad signature", event["id"]);
            return StatusCode::BAD_REQUEST;
        }
        Err(error) => {
            warn!("verifying PayPal webhook {} failed: {error:#}", event["id"]);
            return StatusCode::BAD_GATEWAY;
        }
    }
    let event_type = event["event_type"].as_str().unwrap_or_default();
    let merchant_id = event["resource"]["merchant_id"]
        .as_str()
        .unwrap_or_default();
    info!(
        "PayPal webhook {} {event_type} for merchant {merchant_id:?}",
        event["id"]
    );
    match event_type {
        "MERCHANT.ONBOARDING.COMPLETED" if !merchant_id.is_empty() => {
            let seller = match paypal.seller_status(merchant_id).await {
                Ok(seller) => seller,
                Err(error) => {
                    warn!("looking up PayPal merchant {merchant_id} failed: {error:#}");
                    return StatusCode::BAD_GATEWAY;
                }
            };
            let tracking_id = seller.tracking_id.clone().or_else(|| {
                event["resource"]["tracking_id"]
                    .as_str()
                    .map(str::to_string)
            });
            let Some((id, tracking_id)) = tracking_id.and_then(|tracking_id| {
                api.accounts
                    .of_paypal_tracking_id(&tracking_id)
                    .map(|id| (id, tracking_id))
            }) else {
                info!("no account is connecting PayPal merchant {merchant_id}");
                return StatusCode::OK;
            };
            let _ = finish_onboarding(&api, &id, &tracking_id, seller);
        }
        "MERCHANT.PARTNER-CONSENT.REVOKED" => {
            for id in api.accounts.disconnect_paypal(merchant_id) {
                info!("{id} was disconnected from PayPal merchant {merchant_id}");
            }
        }
        _ => {}
    }
    StatusCode::OK
}

#[derive(Deserialize)]
pub(super) struct StripeConnectRequest {
    country: Option<String>,
}

pub(super) async fn handle_account_stripe_connect(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    request: Option<Json<StripeConnectRequest>>,
) -> Result<Json<SignUpResponse>, (StatusCode, String)> {
    let SignedIn { id, account, fresh } = signed_in(&api, &headers)?;
    if !fresh {
        return Err((
            StatusCode::FORBIDDEN,
            "sign in again to connect Stripe".to_string(),
        ));
    }
    let stripe = api.stripe.as_ref().ok_or_else(payments_unavailable)?;
    let stripe_account_id = match account.stripe_account_id {
        Some(stripe_account_id) => stripe_account_id,
        None => {
            let country = request
                .and_then(|Json(request)| request.country)
                .map(|country| country.trim().to_uppercase())
                .filter(|country| stripe::COUNTRIES.contains(&country.as_str()))
                .ok_or((
                    StatusCode::BAD_REQUEST,
                    "pick one of the countries Stripe supports".to_string(),
                ))?;
            let user = account
                .twitch()
                .map(|twitch| ("twitch_user_id", twitch.user_id.as_str()))
                .or_else(|| {
                    account
                        .identity(Platform::Kick)
                        .map(|kick| ("kick_user_id", kick.user_id.as_str()))
                })
                .unwrap_or(("account", id.as_str()));
            let stripe_account_id = stripe
                .create_account(user, account.name(), &country)
                .await
                .map_err(|error| {
                    warn!("creating a Stripe account in {country} for {id} failed: {error:#}");
                    (
                        StatusCode::BAD_GATEWAY,
                        "Stripe could not create the account".to_string(),
                    )
                })?;
            api.accounts
                .start_stripe(&id, stripe_account_id.clone())
                .map_err(account_error)?;
            info!("{id} got Stripe account {stripe_account_id}");
            stripe_account_id
        }
    };
    let url = stripe
        .onboarding_link(
            &stripe_account_id,
            &format!("{}/dashboard/?stripe", api.auth.frontend_url),
        )
        .await
        .map_err(|error| {
            warn!("creating a Stripe account link for {id} failed: {error:#}");
            stripe_unreachable()
        })?;
    info!("{id} started connecting Stripe account {stripe_account_id}");
    Ok(Json(SignUpResponse { url }))
}

fn stripe_unreachable() -> (StatusCode, String) {
    (
        StatusCode::BAD_GATEWAY,
        "Stripe could not be reached".to_string(),
    )
}

pub(super) async fn handle_account_stripe_check(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let SignedIn { id, account, .. } = signed_in(&api, &headers)?;
    let page = account.page.id;
    let stripe = api.stripe.as_ref().ok_or_else(payments_unavailable)?;
    if let Some(stripe_account_id) = account.stripe_account_id {
        let ready = stripe
            .takes_payments(&stripe_account_id)
            .await
            .map_err(|error| {
                warn!("looking up Stripe account {stripe_account_id} of {id} failed: {error:#}");
                stripe_unreachable()
            })?;
        api.accounts
            .stripe_ready(&id, &stripe_account_id, ready)
            .map_err(account_error)?;
        if ready != account.stripe_ready {
            info!("Stripe account {stripe_account_id} of {id} takes payments: {ready}");
        }
        if !ready {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                "the Stripe account can't receive payments yet".to_string(),
            ));
        }
    }
    let settings = api.pages.get(&page);
    Ok(settings_response(&api, page, settings))
}

pub(super) async fn handle_account_stripe_disconnect(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let SignedIn { id, account, .. } = signed_in(&api, &headers)?;
    let page = account.page.id;
    api.accounts.forget_stripe(&id).map_err(account_error)?;
    info!("{id} disconnected Stripe");
    let settings = api.pages.get(&page);
    Ok(settings_response(&api, page, settings))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::{Value, json};

    use crate::api::test_utils::*;

    #[tokio::test]
    async fn stripe_connects_and_takes_tips() {
        let (address, api, fake) = start_stripe_api().await;
        let client = reqwest::Client::new();
        let (session, _, id) = streamer(&address, &api).await;
        let post = |path: &str| {
            client
                .post(format!("http://{address}{path}"))
                .bearer_auth(&session)
        };
        let settings: Value = post("/account/stripe/check")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["stripe"], true);
        assert_eq!(settings["stripeAccountId"], Value::Null);
        assert_eq!(settings["stripePending"], false);
        assert_eq!(settings["stripeTest"], true);
        assert_eq!(settings["paypal"], false);
        for request in [json!({}), json!({"country": "XX"})] {
            let response = post("/account/stripe/connect")
                .json(&request)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
        let response = post("/account/stripe/connect").send().await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        for request in [Some(json!({"country": " se "})), None] {
            let mut connect = post("/account/stripe/connect");
            if let Some(request) = request {
                connect = connect.json(&request);
            }
            let signed_up: Value = connect.send().await.unwrap().json().await.unwrap();
            assert_eq!(
                signed_up,
                json!({"url": "https://connect.stripe.com/setup/x"})
            );
        }
        let accounts = fake.accounts.lock().unwrap().clone();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0]["display_name"], "Erik");
        assert_eq!(accounts[0]["dashboard"], "full");
        assert_eq!(accounts[0]["identity"], json!({"country": "se"}));
        assert_eq!(accounts[0]["metadata"], json!({"twitch_user_id": "1"}));
        let link = fake.link.lock().unwrap().clone().unwrap();
        assert_eq!(link["account"], "acct_1");
        assert_eq!(
            link["use_case"]["account_onboarding"]["return_url"],
            "https://moblin.app/dashboard/?stripe"
        );
        let response = post("/account/stripe/check").send().await.unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let settings_request = json!({"currency": "eur", "tips": {"amounts": [500]}});
        let response = post("/account/settings")
            .json(&settings_request)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        *fake.active.lock().unwrap() = true;
        let settings: Value = post("/account/stripe/check")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["stripeAccountId"], "acct_1");
        assert_eq!(settings["stripePending"], false);
        let response = post("/account/settings")
            .json(&settings_request)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let tips_url = format!("http://{address}/pages/{id}/tips");
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
                "amounts": [500],
                "currency": "EUR",
                "clientId": null,
                "merchantId": null,
                "environment": null,
                "stripe": {"publishableKey": "pk_test_public", "accountId": "acct_1"},
            }})
        );
        let orders_url = format!("{tips_url}/orders");
        let response = client
            .post(&orders_url)
            .json(&json!({"amount": 700}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let order: Value = client
            .post(&orders_url)
            .json(&json!({"amount": 700, "user": "Viewer", "message": "Great stream!", "provider": "stripe"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            order,
            json!({
                "orderId": "pi_1",
                "clientSecret": "pi_1_secret_2",
                "price": 700,
                "currency": "EUR",
            })
        );
        let payment = fake.payment.lock().unwrap().clone().unwrap();
        assert_eq!(payment["account"], "acct_1");
        assert_eq!(payment["form"]["amount"], "700");
        assert_eq!(payment["form"]["currency"], "eur");
        assert_eq!(payment["form"]["capture_method"], "manual");
        assert_eq!(
            payment["form"]["description"],
            "Tip from Viewer: Great stream!"
        );
        let capture_url = format!("{orders_url}/pi_1/capture");
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
        *fake.state.lock().unwrap() = "requires_capture";
        let response = client.post(&capture_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
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
        assert_eq!(count("stripeOrders"), 1);
        assert_eq!(count("stripePaymentsFailed:not-approved"), 1);
        assert_eq!(count("stripePayments"), 1);
        assert_eq!(count("tipPayments"), 1);
        assert!(
            !counters
                .iter()
                .any(|counter| counter.name.starts_with("paypal"))
        );
        let settings: Value = post("/account/stripe/disconnect")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["stripeAccountId"], Value::Null);
        let public: Value = client
            .get(&tips_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(public, json!({"tips": null}));
    }

    #[tokio::test]
    async fn paypal_is_connected_through_onboarding() {
        let (paypal_url, fake) = start_paypal().await;
        let (address, api) = start_api(Some(new_paypal(paypal_url))).await;
        let (session, _, id) = streamer(&address, &api).await;
        let old = api.auth.session_at("1", 0, crate::deliveries::now() - 601);
        let client = reqwest::Client::new();
        let paypal_url = format!("http://{address}/account/paypal");
        let settings_url = format!("http://{address}/account/settings");
        let tips = json!({"currency": "SEK", "tips": {"amounts": [250]}});
        let response = client
            .post(&settings_url)
            .bearer_auth(&session)
            .json(&tips)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let response = client
            .post(format!("{paypal_url}/connect"))
            .bearer_auth(&old)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(fake.referral.lock().unwrap().is_none());
        let sign_up: Value = client
            .post(format!("{paypal_url}/connect"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            sign_up,
            json!({"url": "https://www.sandbox.paypal.com/bizsignup/x"})
        );
        let tracking_id = api.accounts.get("1").unwrap().paypal_tracking_id.unwrap();
        let referral = fake.referral.lock().unwrap().clone().unwrap();
        assert_eq!(referral["attribution"], "BN");
        assert_eq!(referral["body"]["tracking_id"], tracking_id.as_str());
        assert_eq!(
            referral["body"]["operations"][0]["api_integration_preference"]["rest_api_integration"]
                ["third_party_details"]["features"],
            json!(["PAYMENT", "REFUND"])
        );
        assert_eq!(
            referral["body"]["partner_config_override"]["return_url"],
            "https://moblin.app/dashboard/"
        );
        let account: Value = client
            .get(format!("http://{address}/account"))
            .bearer_auth(&old)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(account["settings"]["merchantId"], Value::Null);
        assert_eq!(account["settings"]["paypalPending"], true);
        let check = || {
            client
                .post(format!("{paypal_url}/check"))
                .bearer_auth(&old)
                .send()
        };
        assert_eq!(check().await.unwrap().status(), StatusCode::NOT_FOUND);
        *fake.seller.lock().unwrap() = Some(json!({
            "tracking_id": tracking_id,
            "merchant_id": "SELLER0000001",
            "payments_receivable": true,
            "primary_email_confirmed": true,
            "oauth_integrations": [{
                "integration_type": "OAUTH_THIRD_PARTY",
                "integration_method": "PAYPAL",
                "oauth_third_party": [{
                    "partner_client_id": "other",
                    "merchant_client_id": "m",
                    "scopes": ["https://uri.paypal.com/services/payments/realtimepayment"],
                }],
            }],
        }));
        assert_eq!(check().await.unwrap().status(), StatusCode::CONFLICT);
        {
            let mut seller = fake.seller.lock().unwrap();
            let seller = seller.as_mut().unwrap();
            seller["oauth_integrations"][0]["oauth_third_party"][0]["partner_client_id"] =
                json!("id");
            seller["primary_email_confirmed"] = json!(false);
        }
        assert_eq!(
            check().await.unwrap().status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        fake.seller.lock().unwrap().as_mut().unwrap()["primary_email_confirmed"] = json!(true);
        *fake.list_sellers.lock().unwrap() = true;
        let settings: Value = check().await.unwrap().json().await.unwrap();
        assert_eq!(settings["merchantId"], "SELLER0000001");
        assert_eq!(settings["paypalPending"], false);
        let response = client
            .post(&settings_url)
            .bearer_auth(&old)
            .json(&tips)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let public: Value = client
            .get(format!("http://{address}/pages/{id}/tips"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(public["tips"]["merchantId"], "SELLER0000001");
        assert_eq!(public["tips"]["clientId"], "id");
        let settings: Value = check().await.unwrap().json().await.unwrap();
        assert_eq!(settings["merchantId"], "SELLER0000001");
        let settings: Value = client
            .post(format!("{paypal_url}/disconnect"))
            .bearer_auth(&old)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["merchantId"], Value::Null);
        assert_eq!(settings["paypalPending"], false);
        let public: Value = client
            .get(format!("http://{address}/pages/{id}/tips"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(public["tips"], Value::Null);
        for (currency, status) in [("SEK", StatusCode::OK), ("EUR", StatusCode::CONFLICT)] {
            let response = client
                .post(&settings_url)
                .bearer_auth(&session)
                .json(&json!({"currency": currency, "tips": {"amounts": [250]}}))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), status);
        }
    }

    #[tokio::test]
    async fn paypal_webhooks_connect_and_disconnect_streamers() {
        let unconfigured = start().await;
        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://{unconfigured}/paypal/webhooks"))
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let (paypal_url, fake) = start_paypal().await;
        let (address, api) = start_api(Some(new_paypal(paypal_url))).await;
        let (session, _, id) = streamer(&address, &api).await;
        let response = client
            .post(format!("http://{address}/account/paypal/connect"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let tracking_id = api.accounts.get("1").unwrap().paypal_tracking_id.unwrap();
        *fake.seller.lock().unwrap() = Some(json!({
            "merchant_id": "SELLER0000001",
            "tracking_id": tracking_id,
            "payments_receivable": true,
            "primary_email_confirmed": true,
            "oauth_integrations": [{"oauth_third_party": [{
                "partner_client_id": "id",
                "merchant_client_id": "m",
                "scopes": ["https://uri.paypal.com/services/payments/realtimepayment"],
            }]}],
        }));
        let webhook = |signature: &str, event: &Value| {
            client
                .post(format!("http://{address}/paypal/webhooks"))
                .header("paypal-auth-algo", "SHA256withRSA")
                .header(
                    "paypal-cert-url",
                    "https://api.paypal.com/v1/notifications/certs/C",
                )
                .header("paypal-transmission-id", "T")
                .header("paypal-transmission-sig", signature)
                .header("paypal-transmission-time", "2026-10-16T05:19:23Z")
                .header("content-type", "application/json")
                .body(event.to_string())
                .send()
        };
        let completed = json!({
            "id": "WH-1",
            "event_type": "MERCHANT.ONBOARDING.COMPLETED",
            "resource": {"partner_client_id": "id", "merchant_id": "SELLER0000001", "links": []},
        });
        let response = webhook("bad", &completed).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(api.accounts.get("1").unwrap().merchant_id, None);
        let response = webhook("good", &completed).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let verification = fake.verification.lock().unwrap().clone().unwrap();
        assert_eq!(
            verification,
            json!({
                "auth_algo": "SHA256withRSA",
                "cert_url": "https://api.paypal.com/v1/notifications/certs/C",
                "transmission_id": "T",
                "transmission_sig": "good",
                "transmission_time": "2026-10-16T05:19:23Z",
                "webhook_id": "WH",
                "webhook_event": completed,
            })
        );
        let account = api.accounts.get("1").unwrap();
        assert_eq!(account.merchant_id.as_deref(), Some("SELLER0000001"));
        assert_eq!(account.paypal_tracking_id, None);
        let response = webhook("good", &completed).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let revoked = json!({
            "id": "WH-2",
            "event_type": "MERCHANT.PARTNER-CONSENT.REVOKED",
            "resource": {"merchant_id": "SELLER0000001", "tracking_id": tracking_id},
        });
        let response = webhook("good", &revoked).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(api.accounts.get("1").unwrap().merchant_id, None);
        let public: Value = client
            .get(format!("http://{address}/pages/{id}/tips"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(public, json!({"tips": null}));
    }
}
