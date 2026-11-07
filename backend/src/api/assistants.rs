use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{MethodRouter, post};
use log::{info, warn};
use serde::Deserialize;

use crate::accounts::{Access, AccountError, Permission, Platform};

use super::*;

#[derive(Deserialize)]
pub(super) struct AssistantRequest {
    username: String,
}

#[derive(Deserialize)]
pub(super) struct PermissionRequest {
    on: bool,
}

fn broadcaster(
    api: &Api,
    platform: &str,
    headers: &HeaderMap,
) -> Result<(Platform, SignedIn, String), (StatusCode, String)> {
    let platform = platform_of(platform)?;
    let signed_in = signed_in(api, headers)?;
    let broadcaster = signed_in
        .account
        .identity(platform)
        .map(|identity| identity.user_id.clone())
        .ok_or_else(|| platform_needed(platform))?;
    Ok((platform, signed_in, broadcaster))
}

#[derive(Deserialize)]
pub(super) struct AccessRequest {
    access: Access,
}

pub(super) async fn handle_account_assistant_add(
    State(api): State<Arc<Api>>,
    Path(platform): Path<String>,
    headers: HeaderMap,
    Json(request): Json<AssistantRequest>,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let (platform, SignedIn { account, .. }, broadcaster) = broadcaster(&api, &platform, &headers)?;
    let username = request
        .username
        .trim()
        .trim_start_matches('@')
        .to_lowercase();
    let username = username.as_str();
    let (longest, allowed) = match platform {
        Platform::Twitch => (25, "_"),
        Platform::Kick => (50, "_-"),
    };
    if username.is_empty()
        || username.len() > longest
        || !username
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || allowed.contains(character))
    {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Enter a {} username.", platform.title()),
        ));
    }
    let assistant = match api.auth.user(platform, username).await {
        Ok(Some(assistant)) => assistant,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                format!("There is no {} user named {username}.", platform.title()),
            ));
        }
        Err(error) => {
            warn!(
                "looking up {} user {username} for {} failed: {error:#}",
                platform.title(),
                account.id
            );
            return Err((
                StatusCode::BAD_GATEWAY,
                format!("Couldn't look the {} user up. Try again.", platform.title()),
            ));
        }
    };
    if assistant.user_id == broadcaster {
        return Err((
            StatusCode::CONFLICT,
            format!("That is your own {} account.", platform.title()),
        ));
    }
    api.accounts
        .add_assistant(platform, &broadcaster, &assistant)
        .map_err(account_error)?;
    info!(
        "{} added {} assistant {} ({})",
        account.id,
        platform.title(),
        assistant.user_id,
        assistant.login
    );
    assistants_changed(&api, &account.page_id, platform, &assistant.user_id);
    Ok(account_response(&api, account))
}

pub(super) async fn handle_account_assistant_remove(
    State(api): State<Arc<Api>>,
    Path((platform, assistant)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let (platform, SignedIn { account, .. }, broadcaster) = broadcaster(&api, &platform, &headers)?;
    api.accounts
        .remove_assistant(platform, &broadcaster, &assistant)
        .map_err(account_error)?;
    info!(
        "{} removed {} assistant {assistant}",
        account.id,
        platform.title()
    );
    assistants_changed(&api, &account.page_id, platform, &assistant);
    Ok(account_response(&api, account))
}

pub(super) fn assistant_permission(permission: Permission) -> MethodRouter<Arc<Api>> {
    post(
        move |State(api): State<Arc<Api>>,
              Path((platform, assistant)): Path<(String, String)>,
              headers: HeaderMap,
              Json(request): Json<PermissionRequest>| async move {
            let on = request.on;
            set_assistant(
                &api,
                &platform,
                &assistant,
                &headers,
                |accounts, platform, broadcaster| {
                    accounts.set_assistant_permission(
                        platform,
                        broadcaster,
                        &assistant,
                        permission,
                        on,
                    )
                },
                format!(
                    "turned {permission:?} {} for",
                    if on { "on" } else { "off" }
                ),
            )
        },
    )
}

pub(super) async fn handle_account_assistant_remote_control(
    State(api): State<Arc<Api>>,
    Path((platform, assistant)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<AccessRequest>,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    set_assistant(
        &api,
        &platform,
        &assistant,
        &headers,
        |accounts, platform, broadcaster| {
            accounts.set_assistant_access(platform, broadcaster, &assistant, request.access)
        },
        format!("set the remote control access to {:?} of", request.access),
    )
}

fn set_assistant(
    api: &Api,
    platform: &str,
    assistant: &str,
    headers: &HeaderMap,
    set: impl FnOnce(&Accounts, Platform, &str) -> Result<(), AccountError>,
    change: String,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let (platform, SignedIn { account, .. }, broadcaster) = broadcaster(api, platform, headers)?;
    set(&api.accounts, platform, &broadcaster).map_err(account_error)?;
    info!(
        "{} {change} {} assistant {assistant}",
        account.id,
        platform.title()
    );
    assistants_changed(api, &account.page_id, platform, assistant);
    Ok(account_response(api, account))
}

fn assistants_changed(api: &Api, page_id: &str, platform: Platform, assistant: &str) {
    api.remote_control.changes.notify(page_id, Change::Account);
    if let Some(own) = api.accounts.page_of_identity(platform, assistant) {
        api.remote_control.changes.notify(&own, Change::Account);
    }
}

pub(super) async fn handle_assistant_account(
    State(api): State<Arc<Api>>,
    Path(page_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<AccountResponse>, Failure> {
    let user = assistant(&api, &headers, &page_id)?;
    let account = user.streamer;
    let look = api.pages.look(&page_id);
    Ok(Json(AccountResponse {
        twitch: public_identity(&account, Platform::Twitch),
        kick: public_identity(&account, Platform::Kick),
        name: account.name().to_string(),
        avatar: account.avatar().map(str::to_string),
        moblin_cloud_key: None,
        devices: devices(&api, &page_id, false),
        settings: without_payments(settings_response(&api, &account, look)).0,
        admin: false,
        premium: is_premium(&api, &account),
        preview: user.rights.preview,
        go_live: user.rights.go_live,
        edit_tips: user.rights.edit_tips,
        remote_control: user.rights.remote_control,
        twitch_assistants: Vec::new(),
        kick_assistants: Vec::new(),
        assisting: Vec::new(),
    }))
}

#[cfg(test)]
#[path = "tests/assistants.rs"]
mod tests;
