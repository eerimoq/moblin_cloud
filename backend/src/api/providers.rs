use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::accounts::{Account, AccountError, Platform};
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
    freshly_signed_in(&signed_in, "PayPal")?;
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
                signed_in.account.id
            );
            unreachable("PayPal")
        })?;
    api.accounts
        .start_paypal(&signed_in.account.id, tracking_id.clone())
        .map_err(account_error)?;
    info!(
        "{} started connecting PayPal with tracking ID {tracking_id}",
        signed_in.account.id
    );
    api.remote_control
        .changes
        .notify(&signed_in.account.page_id, Change::Account);
    Ok(Json(SignUpResponse { url }))
}

fn freshly_signed_in(signed_in: &SignedIn, provider: &str) -> Result<(), (StatusCode, String)> {
    if !signed_in.fresh {
        return Err((
            StatusCode::FORBIDDEN,
            format!("sign in again to connect {provider}"),
        ));
    }
    Ok(())
}

fn unreachable(provider: &str) -> (StatusCode, String) {
    (
        StatusCode::BAD_GATEWAY,
        format!("{provider} could not be reached"),
    )
}

fn disconnect(
    api: &Api,
    headers: &HeaderMap,
    provider: &str,
    forget: impl FnOnce(&Accounts, &str) -> Result<Account, AccountError>,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let signed_in = signed_in(api, headers)?;
    let account = forget(&api.accounts, &signed_in.account.id).map_err(account_error)?;
    let page_id = &account.page_id;
    info!("{} disconnected {provider}", account.id);
    api.remote_control.changes.notify(page_id, Change::Page);
    Ok(settings_response(api, &account, api.pages.look(page_id)))
}

pub(super) async fn handle_account_paypal_check(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let SignedIn { mut account, .. } = signed_in(&api, &headers)?;
    let id = account.id.clone();
    let paypal = api.paypal.as_ref().ok_or_else(payments_unavailable)?;
    if let Some(tracking_id) = account.paypal_tracking_id.clone() {
        let seller = paypal.seller(&tracking_id).await.map_err(|error| {
            warn!("looking up PayPal tracking ID {tracking_id} of {id} failed: {error:#}");
            unreachable("PayPal")
        })?;
        let seller = seller.ok_or((
            StatusCode::NOT_FOUND,
            "PayPal has not connected an account yet".to_string(),
        ))?;
        account = finish_onboarding(&api, &id, &tracking_id, seller)?;
    }
    let look = api.pages.look(&account.page_id);
    Ok(settings_response(&api, &account, look))
}

pub(super) async fn handle_account_paypal_disconnect(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    disconnect(&api, &headers, "PayPal", Accounts::forget_paypal)
}

fn finish_onboarding(
    api: &Api,
    id: &str,
    tracking_id: &str,
    seller: Seller,
) -> Result<Account, (StatusCode, String)> {
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
    let account = api
        .accounts
        .connect_paypal(id, tracking_id, seller.merchant_id.clone())
        .map_err(account_error)?;
    info!("{id} connected PayPal merchant {}", seller.merchant_id);
    api.remote_control
        .changes
        .notify(&account.page_id, Change::Page);
    Ok(account)
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
                account_changed(&api, &id, Change::Page);
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
    let signed_in = signed_in(&api, &headers)?;
    freshly_signed_in(&signed_in, "Stripe")?;
    let SignedIn { account, .. } = signed_in;
    let id = account.id.as_str();
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
                .unwrap_or(("account", id));
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
                .start_stripe(id, stripe_account_id.clone())
                .map_err(account_error)?;
            info!("{id} got Stripe account {stripe_account_id}");
            api.remote_control
                .changes
                .notify(&account.page_id, Change::Account);
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
            unreachable("Stripe")
        })?;
    info!("{id} started connecting Stripe account {stripe_account_id}");
    Ok(Json(SignUpResponse { url }))
}

pub(super) async fn handle_account_stripe_check(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let SignedIn { mut account, .. } = signed_in(&api, &headers)?;
    let id = account.id.clone();
    let stripe = api.stripe.as_ref().ok_or_else(payments_unavailable)?;
    if let Some(stripe_account_id) = account.stripe_account_id.clone() {
        let ready = stripe
            .takes_payments(&stripe_account_id)
            .await
            .map_err(|error| {
                warn!("looking up Stripe account {stripe_account_id} of {id} failed: {error:#}");
                unreachable("Stripe")
            })?;
        let was_ready = account.stripe_ready;
        account = api
            .accounts
            .stripe_ready(&id, &stripe_account_id, ready)
            .map_err(account_error)?;
        if ready != was_ready {
            info!("Stripe account {stripe_account_id} of {id} takes payments: {ready}");
            api.remote_control
                .changes
                .notify(&account.page_id, Change::Page);
        }
        if !ready {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                "the Stripe account can't receive payments yet".to_string(),
            ));
        }
    }
    let look = api.pages.look(&account.page_id);
    Ok(settings_response(&api, &account, look))
}

pub(super) async fn handle_account_stripe_disconnect(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    disconnect(&api, &headers, "Stripe", Accounts::forget_stripe)
}

#[cfg(test)]
#[path = "tests/providers.rs"]
mod tests;
