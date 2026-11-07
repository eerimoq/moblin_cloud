use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;

use crate::payments::{Item, Order};

use super::*;

#[derive(Deserialize)]
pub(super) struct SpinRequest {
    spins: u32,
    #[serde(default)]
    price: Option<u32>,
    #[serde(flatten)]
    paid: PaidRequest,
}

pub(super) async fn handle_spin_order(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    Json(request): Json<SpinRequest>,
) -> Result<Json<OrderResponse>, Failure> {
    place_order(&api, "spin", spin_order(&api, page_id, request)).await
}

fn spin_order(api: &Api, id: String, request: SpinRequest) -> Result<Order, Failure> {
    let (account, settings) = page_settings(api, &id);
    let spin = settings.spin().cloned().ok_or(failure(
        "feature-off",
        StatusCode::BAD_REQUEST,
        "The streamer turned the wheel off. Reload the page.",
    ))?;
    wheel_available(api, &id)?;
    let payee = request.paid.payee(api, account.as_ref())?;
    let option = spin
        .options
        .iter()
        .find(|option| {
            option.spins == request.spins && request.price.is_none_or(|price| price == option.price)
        })
        .ok_or(failure(
            "invalid-option",
            StatusCode::BAD_REQUEST,
            "The streamer changed the prices. Reload the page.",
        ))?;
    request.paid.order(
        id,
        |message| Item::Spin {
            spins: option.spins,
            message,
        },
        payee,
        option.price,
        settings.currency(),
    )
}

pub(super) fn wheel_available(api: &Api, id: &str) -> Result<(), Failure> {
    if api.remote_control.wheel(id).is_none() {
        return Err(if api.remote_control.is_connected(id) {
            failure(
                "no-wheel",
                StatusCode::CONFLICT,
                "The streamer is not taking wheel spins right now.",
            )
        } else {
            failure(
                "offline",
                StatusCode::NOT_FOUND,
                "The streamer's wheel is not available right now. Try again when they are live.",
            )
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/spins.rs"]
mod tests;
