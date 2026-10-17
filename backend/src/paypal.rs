use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use log::info;
use reqwest::header::{CONTENT_TYPE, HeaderMap};
use reqwest::{Client, RequestBuilder, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::time::Instant;

use crate::pages::format_price;
use crate::payments::{CaptureError, Order};

pub const LIVE_URL: &str = "https://api-m.paypal.com";
pub const SANDBOX_URL: &str = "https://api-m.sandbox.paypal.com";

fn order_body(order: &Order, merchant_id: &str) -> Value {
    json!({
        "intent": "CAPTURE",
        "purchase_units": [{
            "amount": {
                "currency_code": order.currency,
                "value": format_price(order.price),
            },
            "payee": {"merchant_id": merchant_id},
            "description": order.description(),
        }],
        "application_context": {"shipping_preference": "NO_SHIPPING"},
    })
}

trait Checked: Sized {
    async fn checked(self, what: &str) -> Result<Self>;
}

impl Checked for reqwest::Response {
    async fn checked(self, what: &str) -> Result<Self> {
        let status = self.status();
        if status.is_success() {
            return Ok(self);
        }
        let debug_id = self
            .headers()
            .get("paypal-debug-id")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let body = self.text().await.unwrap_or_default();
        bail!("PayPal refused to {what} ({status}, debug ID {debug_id:?}): {body}")
    }
}

#[derive(Debug, PartialEq)]
pub struct Seller {
    pub merchant_id: String,
    pub tracking_id: Option<String>,
    pub payments_receivable: bool,
    pub email_confirmed: bool,
    pub permissions_granted: bool,
}

pub struct PayPal {
    client: Client,
    url: String,
    client_id: String,
    client_secret: String,
    partner_id: String,
    attribution_id: Option<String>,
    webhook_id: Option<String>,
    token: tokio::sync::Mutex<Option<Token>>,
}

struct Token {
    access_token: Arc<str>,
    expires_at: Instant,
}

impl PayPal {
    pub fn new(
        client: Client,
        url: String,
        client_id: String,
        client_secret: String,
        partner_id: String,
        attribution_id: Option<String>,
        webhook_id: Option<String>,
    ) -> Self {
        Self {
            client,
            url,
            client_id,
            client_secret,
            partner_id,
            attribution_id,
            webhook_id,
            token: tokio::sync::Mutex::new(None),
        }
    }

    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    pub fn environment(&self) -> &'static str {
        if self.url == SANDBOX_URL {
            "sandbox"
        } else {
            "production"
        }
    }

    fn partner(&self, request: RequestBuilder) -> RequestBuilder {
        match &self.attribution_id {
            Some(id) => request.header("PayPal-Partner-Attribution-Id", id),
            None => request,
        }
    }

    pub async fn referral(&self, tracking_id: &str, return_url: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct Body {
            links: Vec<Link>,
        }
        #[derive(Deserialize)]
        struct Link {
            href: String,
            rel: String,
        }
        let token = self.token().await?;
        let body: Body = self
            .partner(
                self.client
                    .post(format!("{}/v2/customer/partner-referrals", self.url)),
            )
            .bearer_auth(&token)
            .json(&json!({
                "tracking_id": tracking_id,
                "operations": [{
                    "operation": "API_INTEGRATION",
                    "api_integration_preference": {"rest_api_integration": {
                        "integration_method": "PAYPAL",
                        "integration_type": "THIRD_PARTY",
                        "third_party_details": {"features": ["PAYMENT"]},
                    }},
                }],
                "products": ["EXPRESS_CHECKOUT"],
                "legal_consents": [{"type": "SHARE_DATA_CONSENT", "granted": true}],
                "partner_config_override": {
                    "return_url": return_url,
                    "return_url_description": "Return to Moblin",
                },
            }))
            .send()
            .await
            .context("failed to create a PayPal referral")?
            .checked("create a referral")
            .await?
            .json()
            .await
            .context("unexpected answer to a PayPal referral")?;
        body.links
            .into_iter()
            .find(|link| link.rel == "action_url")
            .map(|link| link.href)
            .context("no action URL in the PayPal referral")
    }

    fn integrations_url(&self) -> String {
        format!(
            "{}/v1/customer/partners/{}/merchant-integrations",
            self.url, self.partner_id
        )
    }

    pub async fn seller(&self, tracking_id: &str) -> Result<Option<Seller>> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Integrations {
            One(Integration),
            Many(Vec<Integration>),
        }
        #[derive(Deserialize)]
        struct Integration {
            merchant_id: Option<String>,
        }
        let token = self.token().await?;
        let response = self
            .partner(self.client.get(self.integrations_url()))
            .query(&[("tracking_id", tracking_id)])
            .bearer_auth(&token)
            .send()
            .await
            .context("failed to look a PayPal seller up")?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let integrations: Integrations = response
            .checked("look a seller up")
            .await?
            .json()
            .await
            .context("unexpected answer to a PayPal seller lookup")?;
        let merchant_id = match integrations {
            Integrations::One(integration) => integration.merchant_id,
            Integrations::Many(integrations) => integrations
                .into_iter()
                .find_map(|integration| integration.merchant_id),
        };
        match merchant_id {
            Some(merchant_id) => Ok(Some(self.seller_status(&merchant_id).await?)),
            None => Ok(None),
        }
    }

    pub async fn seller_status(&self, merchant_id: &str) -> Result<Seller> {
        #[derive(Deserialize)]
        struct Status {
            tracking_id: Option<String>,
            #[serde(default)]
            payments_receivable: bool,
            #[serde(default)]
            primary_email_confirmed: bool,
            #[serde(default)]
            oauth_integrations: Vec<OAuthIntegration>,
        }
        #[derive(Deserialize)]
        struct OAuthIntegration {
            #[serde(default)]
            oauth_third_party: Vec<Permission>,
        }
        #[derive(Deserialize)]
        struct Permission {
            partner_client_id: String,
            #[serde(default)]
            scopes: Vec<String>,
        }
        if merchant_id.is_empty() || !merchant_id.chars().all(|c| c.is_ascii_alphanumeric()) {
            bail!("bad PayPal merchant ID {merchant_id:?}");
        }
        let token = self.token().await?;
        let status: Status = self
            .partner(
                self.client
                    .get(format!("{}/{merchant_id}", self.integrations_url())),
            )
            .bearer_auth(&token)
            .send()
            .await
            .context("failed to get the status of a PayPal seller")?
            .checked("give the status of a seller")
            .await?
            .json()
            .await
            .context("unexpected answer to a PayPal seller status")?;
        let permissions_granted = status
            .oauth_integrations
            .iter()
            .flat_map(|integration| &integration.oauth_third_party)
            .any(|permission| {
                permission.partner_client_id == self.client_id && !permission.scopes.is_empty()
            });
        Ok(Seller {
            merchant_id: merchant_id.to_string(),
            tracking_id: status.tracking_id,
            payments_receivable: status.payments_receivable,
            email_confirmed: status.primary_email_confirmed,
            permissions_granted,
        })
    }

    pub fn takes_webhooks(&self) -> bool {
        self.webhook_id.is_some()
    }

    pub async fn verify_webhook(&self, headers: &HeaderMap, event: &str) -> Result<bool> {
        #[derive(Deserialize)]
        struct Body {
            verification_status: String,
        }
        let webhook_id = self.webhook_id.as_deref().context("no webhook ID")?;
        let header = |name: &str| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
        };
        let fields = json!({
            "auth_algo": header("paypal-auth-algo"),
            "cert_url": header("paypal-cert-url"),
            "transmission_id": header("paypal-transmission-id"),
            "transmission_sig": header("paypal-transmission-sig"),
            "transmission_time": header("paypal-transmission-time"),
            "webhook_id": webhook_id,
        })
        .to_string();
        let body = format!(
            "{},\"webhook_event\":{event}}}",
            fields.trim_end_matches('}')
        );
        let token = self.token().await?;
        let body: Body = self
            .client
            .post(format!(
                "{}/v1/notifications/verify-webhook-signature",
                self.url
            ))
            .bearer_auth(&token)
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .context("failed to verify a PayPal webhook")?
            .checked("verify a webhook")
            .await?
            .json()
            .await
            .context("unexpected answer to a PayPal webhook verification")?;
        Ok(body.verification_status == "SUCCESS")
    }

    fn auth_assertion(&self, merchant_id: &str) -> String {
        let header = BASE64_STANDARD.encode(json!({"alg": "none"}).to_string());
        let payload = BASE64_STANDARD
            .encode(json!({"iss": self.client_id, "payer_id": merchant_id}).to_string());
        format!("{header}.{payload}.")
    }

    async fn post_order(&self, order: &Order, merchant_id: &str) -> Result<reqwest::Response> {
        let token = self.token().await?;
        Ok(self
            .partner(self.client.post(format!("{}/v2/checkout/orders", self.url)))
            .bearer_auth(&token)
            .header("PayPal-Auth-Assertion", self.auth_assertion(merchant_id))
            .json(&order_body(order, merchant_id))
            .send()
            .await?)
    }

    pub async fn create_order(&self, order: &Order, merchant_id: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct Body {
            id: String,
        }
        let response = self
            .post_order(order, merchant_id)
            .await
            .context("failed to create a PayPal order")?;
        let status = response.status();
        if !status.is_success() {
            bail!(
                "PayPal refused to create an order ({status}): {}",
                response.text().await.unwrap_or_default()
            );
        }
        let body: Body = response
            .json()
            .await
            .context("unexpected answer to a PayPal order")?;
        Ok(body.id)
    }

    pub async fn capture(
        &self,
        order_id: &str,
        order: &Order,
        merchant_id: &str,
    ) -> Result<(), CaptureError> {
        let response = self
            .post_capture(order_id, merchant_id)
            .await
            .map_err(|error| {
                info!("capturing PayPal order {order_id} failed: {error:#}");
                CaptureError::Failed
            })?;
        let status = response.status();
        let body: Value = response.json().await.unwrap_or_default();
        if !status.is_success() {
            let issue = body["details"][0]["issue"].as_str().unwrap_or_default();
            info!("PayPal refused to capture order {order_id} ({status}): {issue}");
            return Err(match issue {
                "ORDER_NOT_APPROVED" | "PAYER_ACTION_REQUIRED" => CaptureError::NotApproved,
                "ORDER_ALREADY_CAPTURED" => CaptureError::AlreadyCaptured,
                _ => CaptureError::Failed,
            });
        }
        let capture = &body["purchase_units"][0]["payments"]["captures"][0];
        let amount_matches = capture["amount"]["currency_code"] == order.currency.as_str()
            && capture["amount"]["value"] == format_price(order.price).as_str();
        match capture["status"].as_str() {
            Some("COMPLETED") if body["status"] == "COMPLETED" && amount_matches => Ok(()),
            Some("PENDING") if amount_matches => {
                info!(
                    "capture of PayPal order {order_id} is pending: {}",
                    capture["status_details"]["reason"]
                );
                Err(CaptureError::Pending)
            }
            _ => {
                info!("unexpected capture of PayPal order {order_id}: {body}");
                Err(CaptureError::Unexpected)
            }
        }
    }

    async fn post_capture(&self, order_id: &str, merchant_id: &str) -> Result<reqwest::Response> {
        if !order_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            bail!("bad order id");
        }
        let token = self.token().await?;
        Ok(self
            .partner(self.client.post(format!(
                "{}/v2/checkout/orders/{order_id}/capture",
                self.url
            )))
            .bearer_auth(&token)
            .header("PayPal-Auth-Assertion", self.auth_assertion(merchant_id))
            .header(CONTENT_TYPE, "application/json")
            .send()
            .await?)
    }

    async fn token(&self) -> Result<Arc<str>> {
        #[derive(Deserialize)]
        struct Body {
            access_token: String,
            expires_in: u64,
        }
        let mut token = self.token.lock().await;
        if let Some(current) = token.as_ref()
            && current.expires_at > Instant::now() + Duration::from_secs(60)
        {
            return Ok(current.access_token.clone());
        }
        info!("getting a PayPal access token");
        let body: Body = self
            .client
            .post(format!("{}/v1/oauth2/token", self.url))
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .form(&[("grant_type", "client_credentials")])
            .send()
            .await
            .context("failed to get a PayPal access token")?
            .checked("issue an access token")
            .await?
            .json()
            .await
            .context("unexpected answer to the PayPal access token request")?;
        let access_token: Arc<str> = body.access_token.into();
        *token = Some(Token {
            access_token: access_token.clone(),
            expires_at: Instant::now() + Duration::from_secs(body.expires_in),
        });
        Ok(access_token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_assertions_name_the_seller() {
        let paypal = PayPal::new(
            Client::new(),
            SANDBOX_URL.to_string(),
            "client".to_string(),
            "secret".to_string(),
            "PARTNER000001".to_string(),
            None,
            None,
        );
        let assertion = paypal.auth_assertion("SELLER0000001");
        let parts: Vec<&str> = assertion.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "eyJhbGciOiJub25lIn0=");
        let payload: Value =
            serde_json::from_slice(&BASE64_STANDARD.decode(parts[1]).unwrap()).unwrap();
        assert_eq!(
            payload,
            json!({"iss": "client", "payer_id": "SELLER0000001"})
        );
        assert_eq!(parts[2], "");
    }
}
