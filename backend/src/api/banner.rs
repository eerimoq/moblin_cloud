use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use log::info;

use crate::images;

use super::*;

pub(super) async fn handle_account_image(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SettingsResponse>, Failure> {
    let result = account_image(&api, &headers, body).await;
    counted(&api, "imageUploads", result)
}

async fn account_image(
    api: &Api,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<Json<SettingsResponse>, Failure> {
    let SignedIn { id, account, .. } = signed_in(api, headers)?;
    let premium = is_premium(api, &account);
    save_image(api, &id, account.page.id, premium, body).await
}

pub(super) async fn handle_moderator_image(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SettingsResponse>, Failure> {
    let result = moderator_image(&api, &id, &headers, body).await;
    counted(&api, "imageUploads", result)
}

async fn moderator_image(
    api: &Api,
    page: &str,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<Json<SettingsResponse>, Failure> {
    let user = moderator(api, headers, page)?.id;
    let premium = streamer_is_premium(api, page);
    save_image(api, &user, page.to_string(), premium, body)
        .await
        .map(without_payments)
}

pub(super) fn streamer_is_premium(api: &Api, page: &str) -> bool {
    api.accounts
        .of_page(page)
        .is_some_and(|account| is_premium(api, &account))
}

async fn save_image(
    api: &Api,
    id: &str,
    page: String,
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
    let version = api.pages.set_image(&page, &image).map_err(|_| Failure {
        reason: "not-saved",
        status: StatusCode::INTERNAL_SERVER_ERROR,
        text: "The image could not be saved. Try again.".to_string(),
    })?;
    info!(
        "{id} uploaded banner image {version} of {} bytes for page {page}",
        image.len()
    );
    let settings = api.pages.get(&page);
    Ok(settings_response(api, page, settings))
}

pub(super) async fn handle_account_image_remove(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let SignedIn { id, account, .. } = signed_in(&api, &headers)?;
    remove_image(&api, &id, account.page.id)
}

pub(super) async fn handle_moderator_image_remove(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, Failure> {
    let user = moderator(&api, &headers, &id)?.id;
    remove_image(&api, &user, id)
        .map(without_payments)
        .map_err(Failure::from)
}

fn remove_image(
    api: &Api,
    id: &str,
    page: String,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    api.pages.remove_image(&page).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "the image could not be removed".to_string(),
        )
    })?;
    info!("{id} removed the banner image of page {page}");
    let settings = api.pages.get(&page);
    Ok(settings_response(api, page, settings))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::Value;

    use crate::api::test_utils::*;
    use crate::images;

    #[tokio::test]
    async fn banner_images() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let upload = |session: String, body: Vec<u8>| {
            client
                .post(format!("http://{address}/account/image"))
                .bearer_auth(session)
                .header("content-type", "image/png")
                .body(body)
                .send()
        };
        let (session, _, id) = streamer(&address, &api).await;
        let response = upload(session, images::tests::png(1200, 400))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        create_account(&api, "premium", "Premium", "premium");
        let premium = api.auth.session("premium", 0);
        let account: Value = client
            .get(format!("http://{address}/account"))
            .bearer_auth(&premium)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(account["premium"], true);
        assert_eq!(account["settings"]["image"], Value::Null);
        let page = account["settings"]["id"].as_str().unwrap().to_string();
        let image_url = format!("http://{address}/pages/{page}/image");
        assert_eq!(
            client.get(&image_url).send().await.unwrap().status(),
            StatusCode::NOT_FOUND
        );
        let response = upload(premium.clone(), b"not an image".to_vec())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        match upload(premium.clone(), vec![0; 3 * 1024 * 1024]).await {
            Ok(response) => assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE),
            Err(error) => assert!(error.is_request(), "{error:?}"),
        }
        let settings: Value = upload(premium.clone(), images::tests::png(1200, 400))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let first = settings["image"].as_str().unwrap().to_string();
        let profile: Value = client
            .get(format!("http://{address}/pages/{page}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(profile["image"], first);
        let response = client.get(&image_url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "image/jpeg");
        assert!(!response.bytes().await.unwrap().is_empty());
        let settings: Value = upload(premium.clone(), images::tests::png(2400, 800))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_ne!(settings["image"], first);
        let other: Value = client
            .get(format!("http://{address}/pages/{id}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(other["image"], Value::Null);
        let settings: Value = client
            .post(format!("http://{address}/account/image/remove"))
            .bearer_auth(&premium)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["image"], Value::Null);
        assert_eq!(
            client.get(&image_url).send().await.unwrap().status(),
            StatusCode::NOT_FOUND
        );
        let counters = api.counters.all();
        let count = |name: &str| {
            counters
                .iter()
                .find(|counter| counter.name == name)
                .map(|counter| counter.total)
        };
        assert_eq!(count("imageUploads"), Some(2));
        assert_eq!(count("imageUploadsFailed:not-premium"), Some(1));
        assert_eq!(count("imageUploadsFailed:invalid-image"), Some(1));
    }
}
