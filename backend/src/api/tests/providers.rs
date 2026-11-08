use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::api::test_utils::*;

#[tokio::test]
async fn only_configured_payment_providers_are_offered() {
    let (address, _, _) = start_stripe_api().await;
    let config: Value = reqwest::get(format!("http://{address}/config"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(config["payments"], json!({"paypal": false, "stripe": true}));
}

#[tokio::test]
async fn stripe_connects_and_takes_tips() {
    let (address, api, fake) = start_stripe_api().await;
    let client = reqwest::Client::new();
    let (session, _, page_id) = streamer(&api);
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
        "https://cloud.moblin.app/dashboard/?stripe"
    );
    let response = post("/account/stripe/check").send().await.unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let settings_request = json!({"currency": "eur", "tips": {"amounts": [500]}});
    let response = post(&format!("/account/pages/{page_id}/settings"))
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
    let response = post(&format!("/account/pages/{page_id}/settings"))
        .json(&settings_request)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let tips_url = format!("http://{address}/pages/{page_id}/tips");
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
    assert_eq!(counter(&api, "stripeOrders"), 1);
    assert_eq!(counter(&api, "stripePaymentsFailed:not-approved"), 1);
    assert_eq!(counter(&api, "stripePayments"), 1);
    assert_eq!(counter(&api, "tipPayments"), 1);
    assert!(
        !api.counters
            .all()
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
    let (session, _, page_id) = streamer(&api);
    let old = api.auth.session_at("1", 0, crate::deliveries::now() - 601);
    let client = reqwest::Client::new();
    let paypal_url = format!("http://{address}/account/paypal");
    let settings_url = format!("http://{address}/account/pages/{page_id}/settings");
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
        referral["body"]["operations"][0]["api_integration_preference"]["rest_api_integration"]["third_party_details"]
            ["features"],
        json!(["PAYMENT", "REFUND"])
    );
    assert_eq!(
        referral["body"]["partner_config_override"]["return_url"],
        "https://cloud.moblin.app/dashboard/"
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
        seller["oauth_integrations"][0]["oauth_third_party"][0]["partner_client_id"] = json!("id");
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
        .get(format!("http://{address}/pages/{page_id}/tips"))
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
        .get(format!("http://{address}/pages/{page_id}/tips"))
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
    let (session, _, page_id) = streamer(&api);
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
        .get(format!("http://{address}/pages/{page_id}/tips"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(public, json!({"tips": null}));
}
