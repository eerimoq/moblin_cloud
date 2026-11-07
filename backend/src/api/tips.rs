use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;

use crate::pages::format_price;
use crate::payments::{Item, Order};

use super::*;

#[derive(Deserialize)]
pub(super) struct TipRequest {
    amount: u32,
    #[serde(flatten)]
    paid: PaidRequest,
}

pub(super) async fn handle_tip_order(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    Json(request): Json<TipRequest>,
) -> Result<Json<OrderResponse>, Failure> {
    place_order(&api, "tip", tip_order(&api, page_id, request)).await
}

fn tip_order(api: &Api, id: String, request: TipRequest) -> Result<Order, Failure> {
    let (account, settings) = page_settings(api, &id);
    let tips = settings.tips().cloned().ok_or(failure(
        "feature-off",
        StatusCode::BAD_REQUEST,
        "The streamer no longer takes tips. Reload the page.",
    ))?;
    let payee = request.paid.payee(api, account.as_ref())?;
    let minimum = tips.amounts.iter().copied().min().unwrap_or(50);
    if request.amount < minimum {
        return Err(failure(
            "amount-too-low",
            StatusCode::BAD_REQUEST,
            &format!("Tip at least {}.", format_price(minimum)),
        ));
    }
    request.paid.order(
        id,
        |message| Item::Tip { message },
        payee,
        request.amount,
        settings.currency(),
    )
}

#[cfg(test)]
#[path = "tests/tips.rs"]
mod tests;
