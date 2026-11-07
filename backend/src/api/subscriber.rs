use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use log::info;
use serde::{Deserialize, Serialize};

use crate::accounts::Platform;
use crate::auth::Viewer;
use crate::deliveries;
use crate::pages::{Settings, Tiers};
use crate::payments::{Item, Order, Payee, insert_captured};

use super::*;

pub(super) fn viewer(
    api: &Api,
    headers: &HeaderMap,
    id: &str,
    account: Option<&Account>,
) -> Result<Viewer, Failure> {
    bearer(headers)
        .and_then(|session| api.auth.viewer_of(session))
        .filter(|viewer| viewer.page_id == id)
        .filter(|viewer| account.is_some_and(|account| account.identity(viewer.platform).is_some()))
        .ok_or(failure(
            "not-signed-in",
            StatusCode::UNAUTHORIZED,
            "Sign in again.",
        ))
}

#[derive(Serialize)]
pub(super) struct AllowanceResponse {
    platform: Platform,
    name: String,
    tier: Option<u8>,
    allowance: u32,
    left: u32,
}

#[derive(Clone, Copy)]
pub(super) enum Perk {
    Spin,
    Effect,
}

impl Perk {
    fn kind(self) -> &'static str {
        match self {
            Perk::Spin => "spin",
            Perk::Effect => "effect",
        }
    }

    fn doing(self) -> &'static str {
        match self {
            Perk::Spin => "spin the wheel",
            Perk::Effect => "play effects",
        }
    }

    fn used_up(self) -> Failure {
        match self {
            Perk::Spin => failure(
                "no-spins-left",
                StatusCode::CONFLICT,
                "You have used all your spins this month.",
            ),
            Perk::Effect => failure(
                "no-effects-left",
                StatusCode::CONFLICT,
                "You have used all your effects this month.",
            ),
        }
    }
}

fn allowance_response(
    api: &Api,
    headers: &HeaderMap,
    id: &str,
    perk: Perk,
    tiers: impl FnOnce(&Settings) -> Option<&Tiers>,
) -> Result<Json<AllowanceResponse>, Failure> {
    let (account, settings) = page_settings(api, id);
    let tiers = linked_tiers(api, account.as_ref(), tiers(&settings));
    let viewer = viewer(api, headers, id, account.as_ref())?;
    let allowance = tiers.map_or(0, |tiers| tiers.allowance(viewer.tier));
    Ok(Json(AllowanceResponse {
        left: api
            .allowances
            .left(id, &viewer.key(), perk.kind(), allowance)
            .map_err(not_saved)?,
        platform: viewer.platform,
        name: viewer.name,
        tier: viewer.tier,
        allowance,
    }))
}

pub(super) async fn handle_subscriber_spin_allowance(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<AllowanceResponse>, Failure> {
    allowance_response(&api, &headers, &page_id, Perk::Spin, Settings::sub_spins)
}

pub(super) async fn handle_subscriber_effect_allowance(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<AllowanceResponse>, Failure> {
    allowance_response(
        &api,
        &headers,
        &page_id,
        Perk::Effect,
        Settings::sub_effects,
    )
}

#[derive(Deserialize)]
pub(super) struct SubscriberSpinRequest {
    #[serde(default)]
    message: Option<String>,
}

#[derive(Serialize)]
pub(super) struct LeftResponse {
    left: u32,
}

pub(super) async fn handle_subscriber_spin(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<SubscriberSpinRequest>,
) -> Result<(StatusCode, Json<LeftResponse>), Failure> {
    let result = subscriber_spin(&api, page_id, &headers, request);
    counted(&api, "subSpins", result)
}

#[derive(Deserialize)]
pub(super) struct SubscriberEffectRequest {
    effect: String,
    #[serde(default)]
    message: Option<String>,
}

pub(super) async fn handle_subscriber_effect(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<SubscriberEffectRequest>,
) -> Result<(StatusCode, Json<LeftResponse>), Failure> {
    let result = subscriber_effect(&api, page_id, &headers, request);
    counted(&api, "subEffects", result)
}

pub(super) struct Subscriber {
    viewer: Viewer,
    tier: u8,
    allowance: u32,
    perk: Perk,
}

impl Subscriber {
    fn payee(&self) -> Payee {
        match self.viewer.platform {
            Platform::Twitch => Payee::Twitch(self.tier),
            Platform::Kick => Payee::Kick,
        }
    }
}

pub(super) fn subscriber(
    api: &Api,
    headers: &HeaderMap,
    id: &str,
    perk: Perk,
    account: Option<&Account>,
    tiers: Option<&Tiers>,
) -> Result<Subscriber, Failure> {
    let doing = perk.doing();
    let tiers = linked_tiers(api, account, tiers).ok_or(failure(
        "feature-off",
        StatusCode::BAD_REQUEST,
        &format!("The streamer no longer lets subscribers {doing}. Reload the page."),
    ))?;
    let viewer = viewer(api, headers, id, account)?;
    let tier = viewer.tier.ok_or(failure(
        "not-subscribed",
        StatusCode::FORBIDDEN,
        &format!(
            "Only {} subscribers of this streamer can {doing}.",
            viewer.platform.title()
        ),
    ))?;
    let allowance = tiers.allowance(Some(tier));
    if allowance == 0 {
        return Err(failure(
            "tier-excluded",
            StatusCode::FORBIDDEN,
            &match viewer.platform {
                Platform::Twitch => format!("Tier {tier} subscribers can't {doing} here."),
                Platform::Kick => format!("Kick subscribers can't {doing} here."),
            },
        ));
    }
    Ok(Subscriber {
        viewer,
        tier,
        allowance,
        perk,
    })
}

fn queue_for_subscriber(api: &Api, subscriber: &Subscriber, order: &Order) -> Result<u32, Failure> {
    let request_id = new_request_id(api)?;
    let item = delivery_item(order);
    let left = api
        .allowances
        .take(
            &order.streamer,
            &subscriber.viewer.key(),
            subscriber.perk.kind(),
            subscriber.allowance,
            |connection| {
                insert_captured(connection, &request_id, order)?;
                deliveries::insert(connection, &order.streamer, &request_id, &item)
            },
        )
        .map_err(not_saved)?
        .ok_or_else(|| subscriber.perk.used_up())?;
    api.deliveries.notify();
    api.remote_control
        .changes
        .notify(&order.streamer, Change::Recent);
    info!(
        "queued {:?} for {} tier {} subscriber {:?} in request {request_id} for remote control streamer {}",
        order.description(),
        subscriber.viewer.platform.title(),
        subscriber.tier,
        order.user,
        order.streamer
    );
    Ok(left)
}

fn subscriber_spin(
    api: &Api,
    id: String,
    headers: &HeaderMap,
    request: SubscriberSpinRequest,
) -> Result<(StatusCode, Json<LeftResponse>), Failure> {
    let (account, settings) = page_settings(api, &id);
    let subscriber = subscriber(
        api,
        headers,
        &id,
        Perk::Spin,
        account.as_ref(),
        settings.sub_spins(),
    )?;
    wheel_available(api, &id)?;
    let order = Order {
        streamer: id,
        item: Item::Spin {
            spins: 1,
            message: viewer_message(request.message)?,
        },
        user: user_name(Some(subscriber.viewer.name.clone())),
        payee: subscriber.payee(),
        price: 0,
        currency: settings.currency().to_string(),
    };
    let left = queue_for_subscriber(api, &subscriber, &order)?;
    Ok((StatusCode::ACCEPTED, Json(LeftResponse { left })))
}

fn subscriber_effect(
    api: &Api,
    id: String,
    headers: &HeaderMap,
    request: SubscriberEffectRequest,
) -> Result<(StatusCode, Json<LeftResponse>), Failure> {
    let (account, settings) = page_settings(api, &id);
    let subscriber = subscriber(
        api,
        headers,
        &id,
        Perk::Effect,
        account.as_ref(),
        settings.sub_effects(),
    )?;
    let order = Order {
        streamer: id,
        item: Item::Effect {
            effect: viewer_effect(&request.effect)?,
            message: viewer_message(request.message)?,
        },
        user: user_name(Some(subscriber.viewer.name.clone())),
        payee: subscriber.payee(),
        price: 0,
        currency: settings.currency().to_string(),
    };
    let left = queue_for_subscriber(api, &subscriber, &order)?;
    Ok((StatusCode::ACCEPTED, Json(LeftResponse { left })))
}

#[cfg(test)]
#[path = "tests/subscriber.rs"]
mod tests;
