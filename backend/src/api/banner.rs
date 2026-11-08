use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use log::info;

use crate::images;

use super::*;

pub(super) async fn handle_image(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SettingsResponse>, Failure> {
    let result = image(&api, &page_id, &headers, body).await;
    counted(&api, "imageUploads", result)
}

async fn image(
    api: &Api,
    page_id: &str,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<Json<SettingsResponse>, Failure> {
    let user = editor(api, headers, page_id)?;
    let premium = is_premium(api, &user.streamer);
    let response = save_image(
        api,
        &user.signed_in.account.id,
        &user.streamer,
        premium,
        body,
    )
    .await?;
    Ok(settings_for(&user, response))
}

async fn save_image(
    api: &Api,
    id: &str,
    account: &Account,
    premium: bool,
    body: Bytes,
) -> Result<Json<SettingsResponse>, Failure> {
    if !premium {
        return Err(Failure {
            reason: "not-premium",
            status: StatusCode::FORBIDDEN,
            text: "Banner images are only for premium accounts.".to_string(),
        });
    }
    let image = tokio::task::spawn_blocking(move || images::banner(&body))
        .await
        .ok()
        .flatten()
        .ok_or_else(|| Failure {
            reason: "invalid-image",
            status: StatusCode::BAD_REQUEST,
            text: "That image could not be read. Use a JPEG, PNG or WebP at least 600 pixels wide and at most 4000 pixels on each side."
                .to_string(),
        })?;
    let page_id = &account.page_id;
    let version = api.pages.set_image(page_id, &image).map_err(|_| Failure {
        reason: "not-saved",
        status: StatusCode::INTERNAL_SERVER_ERROR,
        text: "The image could not be saved. Try again.".to_string(),
    })?;
    info!(
        "{id} uploaded banner image {version} of {} bytes for page {page_id}",
        image.len()
    );
    api.remote_control.changes.notify(page_id, Change::Page);
    Ok(settings_response(api, account, api.pages.look(page_id)))
}

pub(super) async fn handle_image_remove(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, Failure> {
    let user = editor(&api, &headers, &page_id)?;
    let response = remove_image(&api, &user.signed_in.account.id, &user.streamer)?;
    Ok(settings_for(&user, response))
}

fn remove_image(
    api: &Api,
    id: &str,
    account: &Account,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let page_id = &account.page_id;
    api.pages.remove_image(page_id).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "the image could not be removed".to_string(),
        )
    })?;
    info!("{id} removed the banner image of page {page_id}");
    api.remote_control.changes.notify(page_id, Change::Page);
    Ok(settings_response(api, account, api.pages.look(page_id)))
}

#[cfg(test)]
#[path = "tests/banner.rs"]
mod tests;
