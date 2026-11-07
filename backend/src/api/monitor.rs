use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};

use super::*;

#[derive(Deserialize)]
pub(super) struct MonitorLayoutRequest {
    layout: Value,
}

pub(super) async fn handle_monitor_layout(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    let id = assistant(&api, &headers, &page_id)?.signed_in.account.id;
    let layout = api
        .accounts
        .monitor_layout(&id, &page_id)
        .and_then(|layout| serde_json::from_str::<Value>(&layout).ok())
        .unwrap_or(Value::Null);
    Ok(Json(json!({ "layout": layout })))
}

pub(super) async fn handle_monitor_layout_save(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<MonitorLayoutRequest>,
) -> Result<StatusCode, Failure> {
    let id = assistant(&api, &headers, &page_id)?.signed_in.account.id;
    if !request.layout.is_object() {
        return Err(failure(
            "invalid-layout",
            StatusCode::UNPROCESSABLE_ENTITY,
            "the layout must be an object",
        ));
    }
    api.accounts
        .set_monitor_layout(&id, &page_id, &request.layout.to_string())
        .map_err(account_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
#[path = "tests/monitor.rs"]
mod tests;
