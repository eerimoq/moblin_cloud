use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use log::info;
use serde::Deserialize;
use serde_json::Value;

use crate::accounts::Account;
use crate::deliveries;
use crate::effects::EFFECTS;
use crate::payments::MediaPlatform;
use crate::youtube;

use super::*;

#[derive(Deserialize)]
pub(super) struct TestRequest {
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    effect: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

fn test_page(
    api: &Api,
    headers: &HeaderMap,
    page_id: &str,
) -> Result<(String, Account, String), Failure> {
    let account = assistant(api, headers, page_id)?.signed_in.account;
    let page_id = page_id.to_string();
    if !within_limit(&api.tests, &page_id, 10) {
        return Err(failure(
            "too-many-tests",
            StatusCode::TOO_MANY_REQUESTS,
            "Too many tests. Try again in a minute.",
        ));
    }
    let request_id = api.remote_control.new_request_id().ok_or(failure(
        "crashed",
        StatusCode::SERVICE_UNAVAILABLE,
        "The test could not be sent. Try again.",
    ))?;
    Ok((page_id, account, request_id))
}

fn moblin_not_connected() -> Failure {
    failure("offline", StatusCode::CONFLICT, "Connect Moblin first.")
}

pub(super) async fn handle_test_tip(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<TestRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let result = test_tip(&api, &headers, &page_id, request).await;
    counted(&api, "tipTests", result)
}

async fn test_tip(
    api: &Api,
    headers: &HeaderMap,
    page_id: &str,
    request: TestRequest,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let (page_id, account, request_id) = test_page(api, headers, page_id)?;
    let settings = api.pages.get(&page_id);
    let amount = settings
        .tips()
        .and_then(|tips| tips.amounts.first().copied())
        .unwrap_or(500);
    info!("queued a test tip in request {request_id} for remote control streamer {page_id}");
    api.deliveries.add_test(
        page_id,
        request_id,
        deliveries::Item::Tip {
            user: account.name().to_string(),
            message: viewer_message(request.message)?.unwrap_or_default(),
            amount,
            currency: settings.currency().to_string(),
        },
    );
    Ok(accepted())
}

pub(super) async fn handle_test_spin(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<TestRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let result = test_spin(&api, &headers, &page_id, request).await;
    counted(&api, "spinTests", result)
}

async fn test_spin(
    api: &Api,
    headers: &HeaderMap,
    page_id: &str,
    request: TestRequest,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let (page_id, account, request_id) = test_page(api, headers, page_id)?;
    if api.remote_control.wheel(&page_id).is_none() {
        return Err(if api.remote_control.is_connected(&page_id) {
            failure(
                "no-wheel",
                StatusCode::CONFLICT,
                "Pick a wheel of luck in Moblin's Moblin Cloud settings first.",
            )
        } else {
            moblin_not_connected()
        });
    }
    let spins = api
        .pages
        .get(&page_id)
        .spin()
        .and_then(|spin| spin.options.first().map(|option| option.spins))
        .unwrap_or(1);
    info!("queued a test spin in request {request_id} for remote control streamer {page_id}");
    api.deliveries.add_test(
        page_id,
        request_id,
        deliveries::Item::Spin {
            user: account.name().to_string(),
            message: viewer_message(request.message)?.unwrap_or_default(),
            spins,
        },
    );
    Ok(accepted())
}

pub(super) async fn handle_test_effect(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<TestRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let result = test_effect(&api, &headers, &page_id, request).await;
    counted(&api, "effectTests", result)
}

async fn test_effect(
    api: &Api,
    headers: &HeaderMap,
    page_id: &str,
    request: TestRequest,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let (page_id, account, request_id) = test_page(api, headers, page_id)?;
    let effect = viewer_effect(request.effect.as_deref().unwrap_or(EFFECTS[0].id))?;
    let message = viewer_message(request.message)?.unwrap_or_default();
    info!("queued a test effect in request {request_id} for remote control streamer {page_id}");
    api.deliveries.add_test(
        page_id,
        request_id,
        deliveries::Item::Effect {
            user: account.name().to_string(),
            message,
            effect,
        },
    );
    Ok(accepted())
}

pub(super) async fn handle_test_media(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<TestRequest>,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let result = test_media(&api, &headers, &page_id, request).await;
    counted(&api, "mediaTests", result)
}

async fn test_media(
    api: &Api,
    headers: &HeaderMap,
    page_id: &str,
    request: TestRequest,
) -> Result<(StatusCode, Json<Value>), Failure> {
    let (page_id, account, request_id) = test_page(api, headers, page_id)?;
    if api.remote_control.media_device(&page_id).is_none() {
        return Err(if api.remote_control.is_connected(&page_id) {
            failure(
                "no-media",
                StatusCode::CONFLICT,
                "Pick a browser widget for media share in Moblin's Moblin Cloud settings first.",
            )
        } else {
            moblin_not_connected()
        });
    }
    let message = viewer_message(request.message)?.unwrap_or_default();
    let link = request.url.as_deref().unwrap_or("dQw4w9WgXcQ");
    let user = account.name().to_string();
    let item = match youtube::video_id(link) {
        Some(video) => deliveries::Item::Media {
            user,
            message,
            video,
            start: youtube::start(link).unwrap_or(0),
            seconds: api.pages.get(&page_id).media_rules().seconds.min(15),
            title: "Test".to_string(),
            platform: MediaPlatform::YouTube,
            link: None,
            thumbnail: None,
        },
        None => video(api, 600, &VideoRequest::of(link.to_string()))
            .await?
            .delivery(user, message),
    };
    info!("queued a test video in request {request_id} for remote control streamer {page_id}");
    api.deliveries.add_test(page_id, request_id, item);
    Ok(accepted())
}

#[cfg(test)]
#[path = "tests/test_sends.rs"]
mod tests;
