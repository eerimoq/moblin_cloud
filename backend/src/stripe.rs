use anyhow::{Context, Result, bail};
use log::{info, warn};
use reqwest::{Client, RequestBuilder};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::payments::{CaptureError, Order, PaymentStatus};

pub const URL: &str = "https://api.stripe.com";

pub const COUNTRIES: [&str; 44] = [
    "AE", "AT", "AU", "BE", "BG", "BR", "CA", "CH", "CY", "CZ", "DE", "DK", "EE", "ES", "FI", "FR",
    "GB", "GI", "GR", "HK", "HR", "HU", "IE", "IT", "JP", "LI", "LT", "LU", "LV", "MT", "MX", "MY",
    "NL", "NO", "NZ", "PL", "PT", "RO", "SE", "SG", "SI", "SK", "TH", "US",
];

pub struct Stripe {
    client: Client,
    url: String,
    secret_key: String,
    publishable_key: String,
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
        let body = self.text().await.unwrap_or_default();
        bail!("Stripe refused to {what} ({status}): {body}")
    }
}

fn check_id(id: &str, prefix: &str) -> Result<()> {
    if !id.starts_with(prefix) || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        bail!("bad Stripe ID {id:?}");
    }
    Ok(())
}

impl Stripe {
    pub fn new(client: Client, url: String, secret_key: String, publishable_key: String) -> Self {
        Self {
            client,
            url,
            secret_key,
            publishable_key,
        }
    }

    pub fn publishable_key(&self) -> &str {
        &self.publishable_key
    }

    pub fn is_test(&self) -> bool {
        self.secret_key.starts_with("sk_test_") || self.secret_key.starts_with("rk_test_")
    }

    fn v2(&self, request: RequestBuilder) -> RequestBuilder {
        request
            .bearer_auth(&self.secret_key)
            .header("Stripe-Version", "2026-09-30.endive")
    }

    fn connected(&self, request: RequestBuilder, account: &str) -> RequestBuilder {
        request
            .bearer_auth(&self.secret_key)
            .header("Stripe-Account", account)
    }

    pub async fn create_account(
        &self,
        user: (&str, &str),
        name: &str,
        country: &str,
    ) -> Result<String> {
        #[derive(Deserialize)]
        struct Body {
            id: String,
        }
        let body: Body = self
            .v2(self.client.post(format!("{}/v2/core/accounts", self.url)))
            .json(&json!({
                "display_name": name,
                "dashboard": "full",
                "identity": {"country": country.to_lowercase()},
                "defaults": {"responsibilities": {
                    "fees_collector": "stripe",
                    "losses_collector": "stripe",
                }},
                "configuration": {"merchant": {"capabilities": {
                    "card_payments": {"requested": true},
                }}},
                "metadata": {user.0: user.1},
            }))
            .send()
            .await
            .context("failed to create a Stripe account")?
            .checked("create an account")
            .await?
            .json()
            .await
            .context("unexpected answer to a Stripe account")?;
        check_id(&body.id, "acct_")?;
        Ok(body.id)
    }

    pub async fn onboarding_link(&self, account: &str, return_url: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct Body {
            url: String,
        }
        let body: Body = self
            .v2(self
                .client
                .post(format!("{}/v2/core/account_links", self.url)))
            .json(&json!({
                "account": account,
                "use_case": {
                    "type": "account_onboarding",
                    "account_onboarding": {
                        "return_url": return_url,
                        "refresh_url": return_url,
                    },
                },
            }))
            .send()
            .await
            .context("failed to create a Stripe account link")?
            .checked("create an account link")
            .await?
            .json()
            .await
            .context("unexpected answer to a Stripe account link")?;
        Ok(body.url)
    }

    pub async fn takes_payments(&self, account: &str) -> Result<bool> {
        check_id(account, "acct_")?;
        let body: Value = self
            .v2(self
                .client
                .get(format!("{}/v2/core/accounts/{account}", self.url)))
            .query(&[("include", "configuration.merchant")])
            .send()
            .await
            .context("failed to look a Stripe account up")?
            .checked("look an account up")
            .await?
            .json()
            .await
            .context("unexpected answer to a Stripe account lookup")?;
        Ok(
            body["configuration"]["merchant"]["capabilities"]["card_payments"]["status"]
                == "active",
        )
    }

    pub async fn create_payment(&self, order: &Order, account: &str) -> Result<(String, String)> {
        #[derive(Deserialize)]
        struct Body {
            id: String,
            client_secret: String,
        }
        check_id(account, "acct_")?;
        let body: Body = self
            .connected(
                self.client.post(format!("{}/v1/payment_intents", self.url)),
                account,
            )
            .form(&[
                ("amount", order.price.to_string()),
                ("currency", order.currency.to_lowercase()),
                ("capture_method", "manual".to_string()),
                ("automatic_payment_methods[enabled]", "true".to_string()),
                (
                    "automatic_payment_methods[allow_redirects]",
                    "never".to_string(),
                ),
                ("description", order.description()),
                ("metadata[streamer]", order.streamer.clone()),
                ("metadata[kind]", order.item.kind().to_string()),
            ])
            .send()
            .await
            .context("failed to create a Stripe payment")?
            .checked("create a payment")
            .await?
            .json()
            .await
            .context("unexpected answer to a Stripe payment")?;
        check_id(&body.id, "pi_")?;
        Ok((body.id, body.client_secret))
    }

    pub async fn capture(
        &self,
        payment_id: &str,
        order: &Order,
        account: &str,
    ) -> Result<(), CaptureError> {
        let response = self
            .post_capture(payment_id, account)
            .await
            .map_err(|error| {
                warn!("capturing Stripe payment {payment_id} failed: {error:#}");
                CaptureError::Unknown
            })?;
        let status = response.status();
        let body: Value = response.json().await.unwrap_or_default();
        if status.is_server_error() {
            warn!("Stripe failed to capture payment {payment_id} ({status}): {body}");
            return Err(CaptureError::Unknown);
        }
        if !status.is_success() {
            let state = body["error"]["payment_intent"]["status"]
                .as_str()
                .unwrap_or_default();
            info!(
                "Stripe refused to capture payment {payment_id} ({status}): {} in state {state:?}",
                body["error"]["code"]
            );
            return Err(match state {
                "requires_payment_method"
                | "requires_confirmation"
                | "requires_action"
                | "processing" => CaptureError::NotApproved,
                "succeeded" => CaptureError::AlreadyCaptured,
                _ => CaptureError::Refused,
            });
        }
        let amount_matches = body["amount_received"] == order.price
            && body["currency"] == order.currency.to_lowercase().as_str();
        match body["status"].as_str() {
            Some("succeeded") if amount_matches => Ok(()),
            Some("processing") => {
                info!("capture of Stripe payment {payment_id} is processing");
                Err(CaptureError::Pending)
            }
            _ => {
                warn!("unexpected capture of Stripe payment {payment_id}: {body}");
                Err(CaptureError::Unknown)
            }
        }
    }

    pub async fn status(
        &self,
        payment_id: &str,
        order: &Order,
        account: &str,
    ) -> Result<PaymentStatus> {
        check_id(payment_id, "pi_")?;
        check_id(account, "acct_")?;
        let response = self
            .connected(
                self.client
                    .get(format!("{}/v1/payment_intents/{payment_id}", self.url)),
                account,
            )
            .send()
            .await
            .context("failed to look a Stripe payment up")?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(PaymentStatus::Gone);
        }
        let body: Value = response
            .checked("look a payment up")
            .await?
            .json()
            .await
            .context("unexpected answer to a Stripe payment lookup")?;
        Ok(match body["status"].as_str() {
            Some("succeeded") => {
                if body["amount_received"] != order.price
                    || body["currency"] != order.currency.to_lowercase().as_str()
                {
                    warn!(
                        "Stripe payment {payment_id} received {} {} instead of {} {}",
                        body["amount_received"], body["currency"], order.price, order.currency
                    );
                }
                PaymentStatus::Captured
            }
            Some("processing") => PaymentStatus::Pending,
            Some("canceled") => PaymentStatus::Gone,
            _ => PaymentStatus::NotCaptured,
        })
    }

    async fn post_capture(&self, payment_id: &str, account: &str) -> Result<reqwest::Response> {
        check_id(payment_id, "pi_")?;
        check_id(account, "acct_")?;
        Ok(self
            .connected(
                self.client.post(format!(
                    "{}/v1/payment_intents/{payment_id}/capture",
                    self.url
                )),
                account,
            )
            .send()
            .await?)
    }

    pub async fn cancel(&self, payment_id: &str, account: &str) -> Result<()> {
        check_id(payment_id, "pi_")?;
        check_id(account, "acct_")?;
        self.connected(
            self.client.post(format!(
                "{}/v1/payment_intents/{payment_id}/cancel",
                self.url
            )),
            account,
        )
        .send()
        .await
        .context("failed to cancel a Stripe payment")?
        .checked("cancel a payment")
        .await?;
        Ok(())
    }
}
