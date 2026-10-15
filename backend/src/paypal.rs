use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use log::info;
use reqwest::Client;
use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::time::Instant;

use crate::song_requests::format_price;

pub const LIVE_URL: &str = "https://api-m.paypal.com";
pub const SANDBOX_URL: &str = "https://api-m.sandbox.paypal.com";

#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    pub streamer: String,
    pub link: String,
    pub song: String,
    pub user: String,
    pub merchant_id: String,
    pub price: u32,
    pub currency: String,
}

impl Order {
    fn body(&self) -> Value {
        let description: String = format!("Song request: {}", self.song)
            .chars()
            .take(127)
            .collect();
        json!({
            "intent": "CAPTURE",
            "purchase_units": [{
                "amount": {
                    "currency_code": self.currency,
                    "value": format_price(self.price),
                },
                "payee": {"merchant_id": self.merchant_id},
                "description": description,
            }],
            "application_context": {"shipping_preference": "NO_SHIPPING"},
        })
    }
}

#[derive(Debug, PartialEq)]
pub enum PayeeError {
    Invalid,
    Failed,
}

#[derive(Debug, PartialEq)]
pub enum CaptureError {
    NotApproved,
    AlreadyCaptured,
    Pending,
    Failed,
}

pub struct PayPal {
    client: Client,
    url: String,
    client_id: String,
    client_secret: String,
    token: tokio::sync::Mutex<Option<Token>>,
    orders: Mutex<HashMap<String, (Instant, Order)>>,
}

struct Token {
    access_token: Arc<str>,
    expires_at: Instant,
}

impl PayPal {
    pub fn new(client: Client, url: String, client_id: String, client_secret: String) -> Self {
        Self {
            client,
            url,
            client_id,
            client_secret,
            token: tokio::sync::Mutex::new(None),
            orders: Mutex::new(HashMap::new()),
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

    pub async fn check_payee(
        &self,
        merchant_id: &str,
        price: u32,
        currency: &str,
    ) -> Result<(), PayeeError> {
        let order = Order {
            streamer: String::new(),
            link: String::new(),
            song: "Check".to_string(),
            user: String::new(),
            merchant_id: merchant_id.to_string(),
            price,
            currency: currency.to_string(),
        };
        let response = self.post_order(&order).await.map_err(|error| {
            info!("checking PayPal merchant {merchant_id} failed: {error:#}");
            PayeeError::Failed
        })?;
        let status = response.status();
        if status.is_client_error() && status != reqwest::StatusCode::UNAUTHORIZED {
            info!("PayPal refused merchant {merchant_id} as payee ({status})");
            return Err(PayeeError::Invalid);
        }
        if !status.is_success() {
            info!("checking PayPal merchant {merchant_id} failed ({status})");
            return Err(PayeeError::Failed);
        }
        Ok(())
    }

    async fn post_order(&self, order: &Order) -> Result<reqwest::Response> {
        let token = self.token().await?;
        Ok(self
            .client
            .post(format!("{}/v2/checkout/orders", self.url))
            .bearer_auth(&token)
            .json(&order.body())
            .send()
            .await?)
    }

    pub fn order(&self, order_id: &str) -> Option<Order> {
        self.orders
            .lock()
            .unwrap()
            .get(order_id)
            .map(|(_, order)| order.clone())
    }

    pub async fn create_order(&self, order: Order) -> Result<String> {
        #[derive(Deserialize)]
        struct Body {
            id: String,
        }
        {
            let mut orders = self.orders.lock().unwrap();
            orders.retain(|_, (created, _)| created.elapsed() < Duration::from_secs(3 * 60 * 60));
            if orders.len() >= 10000 {
                bail!("too many unfinished payments");
            }
        }
        let response = self
            .post_order(&order)
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
        self.orders
            .lock()
            .unwrap()
            .insert(body.id.clone(), (Instant::now(), order));
        Ok(body.id)
    }

    pub async fn capture(&self, order_id: &str, order: &Order) -> Result<(), CaptureError> {
        let response = self.post_capture(order_id).await.map_err(|error| {
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
                "ORDER_ALREADY_CAPTURED" => {
                    self.orders.lock().unwrap().remove(order_id);
                    CaptureError::AlreadyCaptured
                }
                _ => CaptureError::Failed,
            });
        }
        let capture = &body["purchase_units"][0]["payments"]["captures"][0];
        let amount_matches = capture["amount"]["currency_code"] == order.currency.as_str()
            && capture["amount"]["value"] == format_price(order.price).as_str();
        self.orders.lock().unwrap().remove(order_id);
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
                Err(CaptureError::Failed)
            }
        }
    }

    async fn post_capture(&self, order_id: &str) -> Result<reqwest::Response> {
        if !order_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            bail!("bad order id");
        }
        let token = self.token().await?;
        Ok(self
            .client
            .post(format!(
                "{}/v2/checkout/orders/{order_id}/capture",
                self.url
            ))
            .bearer_auth(&token)
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
            .error_for_status()
            .context("PayPal refused to issue an access token")?
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
