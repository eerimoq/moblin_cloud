use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::effects;
use crate::payments::{Item, Order};

use super::*;

#[derive(Deserialize)]
pub(super) struct EffectRequest {
    effect: String,
    #[serde(default)]
    price: Option<u32>,
    #[serde(flatten)]
    paid: PaidRequest,
}

pub(super) async fn handle_effect_order(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    Json(request): Json<EffectRequest>,
) -> Result<Json<OrderResponse>, Failure> {
    place_order(&api, "effect", effect_order(&api, page_id, request)).await
}

fn effect_order(api: &Api, id: String, request: EffectRequest) -> Result<Order, Failure> {
    let (account, settings) = page_settings(api, &id);
    let effects = settings.effects().cloned().ok_or(failure(
        "feature-off",
        StatusCode::BAD_REQUEST,
        "The streamer turned effects off. Reload the page.",
    ))?;
    let payee = request.paid.payee(api, account.as_ref())?;
    if request.price.is_some_and(|price| price != effects.price) {
        return Err(failure(
            "invalid-option",
            StatusCode::BAD_REQUEST,
            "The streamer changed the price. Reload the page.",
        ));
    }
    let effect = viewer_effect(&request.effect)?;
    request.paid.order(
        id,
        |message| Item::Effect { effect, message },
        payee,
        effects.price,
        settings.currency(),
    )
}

pub(super) fn viewer_effect(effect: &str) -> Result<String, Failure> {
    effects::effect(effect)
        .map(|effect| effect.id.to_string())
        .ok_or(failure(
            "no-such-effect",
            StatusCode::BAD_REQUEST,
            "That effect is no longer available. Reload the page.",
        ))
}

pub(super) async fn handle_effect_media(Path(id): Path<String>) -> Result<Response, StatusCode> {
    let effect = effects::effect(&id).ok_or(StatusCode::NOT_FOUND)?;
    Ok((
        [
            (CONTENT_TYPE, effect.content_type),
            (CACHE_CONTROL, "public, max-age=86400"),
            (X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        effect.data,
    )
        .into_response())
}

#[cfg(test)]
#[path = "tests/effects.rs"]
mod tests;
