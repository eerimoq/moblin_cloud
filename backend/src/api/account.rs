use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use log::info;
use serde::{Deserialize, Serialize};

use crate::accounts::{Access, Account, AccountError, Assistant, Platform};

use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AccountResponse {
    pub(super) name: String,
    pub(super) avatar: Option<String>,
    pub(super) twitch: Option<PublicIdentity>,
    pub(super) kick: Option<PublicIdentity>,
    #[serde(rename = "key")]
    pub(super) moblin_cloud_key: Option<String>,
    pub(super) devices: Vec<DeviceResponse>,
    pub(super) settings: SettingsResponse,
    pub(super) admin: bool,
    pub(super) premium: bool,
    pub(super) preview: bool,
    pub(super) go_live: bool,
    pub(super) edit_tips: bool,
    pub(super) remote_control: Access,
    pub(super) assisting: Vec<AssistingResponse>,
    pub(super) twitch_assistants: Vec<Assistant>,
    pub(super) kick_assistants: Vec<Assistant>,
}

#[derive(Serialize)]
pub(super) struct PublicIdentity {
    login: String,
    name: String,
    avatar: Option<String>,
}

pub(super) fn public_identity(account: &Account, platform: Platform) -> Option<PublicIdentity> {
    account.identity(platform).map(|identity| PublicIdentity {
        login: identity.login.clone(),
        name: identity.name.clone(),
        avatar: identity.avatar.clone(),
    })
}

#[derive(Serialize)]
pub(super) struct AssistingResponse {
    id: String,
    platform: Platform,
    name: String,
    login: String,
    avatar: Option<String>,
    twitch: Option<PublicIdentity>,
    kick: Option<PublicIdentity>,
}

pub(super) fn user_ids(account: &Account) -> [Option<&str>; 2] {
    [account.twitch_id(), account.kick_id()]
}

pub(super) fn assisting(api: &Api, account: &Account) -> Vec<AssistingResponse> {
    let mut assisting: Vec<AssistingResponse> = Vec::new();
    for (platform, streamer) in api.accounts.assisted_by(user_ids(account)) {
        if assisting.iter().any(|known| known.id == streamer.page_id) {
            continue;
        }
        let Some(channel) = streamer.identity(platform) else {
            continue;
        };
        assisting.push(AssistingResponse {
            id: streamer.page_id.clone(),
            platform,
            name: channel.name.clone(),
            login: channel.login.clone(),
            avatar: channel.avatar.clone(),
            twitch: public_identity(&streamer, Platform::Twitch),
            kick: public_identity(&streamer, Platform::Kick),
        });
    }
    assisting
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DeviceResponse {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) connected: bool,
    reconnecting: bool,
    wheel: bool,
    last_seen: Option<u64>,
    watchers: Vec<WatcherResponse>,
}

#[derive(Serialize)]
pub(super) struct WatcherResponse {
    name: String,
    avatar: Option<String>,
}

fn watchers(api: &Api, page_id: &str) -> HashMap<String, Vec<WatcherResponse>> {
    let watching = api.remote_control.watchers(page_id);
    let ids: Vec<&str> = watching
        .values()
        .flatten()
        .map(String::as_str)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let accounts = if ids.is_empty() {
        HashMap::new()
    } else {
        api.accounts.get_many(&ids)
    };
    watching
        .into_iter()
        .map(|(device, ids)| {
            let mut watchers: Vec<WatcherResponse> = ids
                .iter()
                .filter_map(|id| accounts.get(id))
                .filter(|account| account.page_id != page_id)
                .map(|account| WatcherResponse {
                    name: account.name().to_string(),
                    avatar: account.avatar().map(str::to_string),
                })
                .collect();
            watchers.sort_by_key(|watcher| watcher.name.to_lowercase());
            (device, watchers)
        })
        .collect()
}

pub(super) fn devices(api: &Api, page_id: &str, with_watchers: bool) -> Vec<DeviceResponse> {
    let known = api.accounts.known_devices(page_id);
    let connected = api.remote_control.devices(page_id);
    let mut watching = if with_watchers {
        watchers(api, page_id)
    } else {
        HashMap::new()
    };
    let mut devices: Vec<DeviceResponse> = connected
        .iter()
        .map(|device| DeviceResponse {
            id: device.id.clone(),
            name: known
                .iter()
                .find(|known| known.id == device.id)
                .map(|known| known.name.clone())
                .unwrap_or_else(|| "Moblin".to_string()),
            connected: !device.reconnecting,
            reconnecting: device.reconnecting,
            wheel: device.wheel,
            last_seen: None,
            watchers: watching.remove(&device.id).unwrap_or_default(),
        })
        .collect();
    devices.extend(
        known
            .into_iter()
            .filter(|known| !connected.iter().any(|device| device.id == known.id))
            .map(|known| DeviceResponse {
                watchers: watching.remove(&known.id).unwrap_or_default(),
                id: known.id,
                name: known.name,
                connected: false,
                reconnecting: false,
                wheel: false,
                last_seen: Some(known.last_seen),
            }),
    );
    devices
}

pub(super) fn account_response(api: &Api, account: Account) -> Json<AccountResponse> {
    let look = api.pages.look(&account.page_id);
    let (twitch_assistants, kick_assistants) = api.accounts.assistants_of(user_ids(&account));
    Json(AccountResponse {
        name: account.name().to_string(),
        avatar: account.avatar().map(str::to_string),
        twitch: public_identity(&account, Platform::Twitch),
        kick: public_identity(&account, Platform::Kick),
        moblin_cloud_key: Some(account.moblin_cloud_key.clone()),
        devices: devices(api, &account.page_id, true),
        settings: settings_response(api, &account, look).0,
        admin: is_admin(api, &account),
        premium: is_premium(api, &account),
        preview: true,
        go_live: true,
        edit_tips: true,
        remote_control: Access::Control,
        twitch_assistants,
        kick_assistants,
        assisting: assisting(api, &account),
    })
}

pub(super) fn account_error(error: AccountError) -> (StatusCode, String) {
    match error {
        AccountError::Unknown => not_signed_in(),
        AccountError::Full => (
            StatusCode::SERVICE_UNAVAILABLE,
            "too many accounts".to_string(),
        ),
        AccountError::UnknownAssistant => (StatusCode::NOT_FOUND, "no such assistant".to_string()),
        AccountError::UnknownDevice => (StatusCode::NOT_FOUND, "no such device".to_string()),
        AccountError::TooManyAssistants => {
            (StatusCode::CONFLICT, "at most 50 assistants".to_string())
        }
        AccountError::IdentityTaken(platform) => (
            StatusCode::CONFLICT,
            format!(
                "This {} account already has its own page. Delete it first.",
                platform.title()
            ),
        ),
        AccountError::UnknownIdentity => {
            (StatusCode::NOT_FOUND, "no such linked account".to_string())
        }
        AccountError::LastIdentity => (
            StatusCode::CONFLICT,
            "You can't unlink the only way to sign in. Link another platform first.".to_string(),
        ),
        AccountError::NotSaved => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "the account could not be saved".to_string(),
        ),
    }
}

pub(super) async fn handle_account(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let SignedIn { account, .. } = signed_in(&api, &headers)?;
    Ok(account_response(&api, account))
}

pub(super) async fn handle_account_key(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let id = signed_in(&api, &headers)?.account.id;
    let moblin_cloud_key = api.remote_control.new_moblin_cloud_key();
    let account = api
        .accounts
        .replace_moblin_cloud_key(&id, moblin_cloud_key)
        .map_err(account_error)?;
    api.remote_control.disconnect(&account.page_id);
    info!("{id} got a new Moblin Cloud key");
    Ok(account_response(&api, account))
}

#[derive(Deserialize)]
pub(super) struct DeviceNameRequest {
    name: String,
}

pub(super) async fn handle_account_device_name(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(device): Path<String>,
    Json(request): Json<DeviceNameRequest>,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let SignedIn { account, .. } = signed_in(&api, &headers)?;
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 30 {
        return Err((
            StatusCode::BAD_REQUEST,
            "the name must be 1 to 30 characters".to_string(),
        ));
    }
    api.accounts
        .rename_device(&account.page_id, &device, name)
        .map_err(account_error)?;
    info!("{} renamed Moblin device {device}", account.id);
    api.remote_control
        .changes
        .notify(&account.page_id, Change::Account);
    Ok(account_response(&api, account))
}

pub(super) async fn handle_account_device_remove(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(device): Path<String>,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let SignedIn { account, .. } = signed_in(&api, &headers)?;
    if api
        .remote_control
        .devices(&account.page_id)
        .iter()
        .any(|connected| connected.id == device)
    {
        return Err((
            StatusCode::CONFLICT,
            "Turn off Moblin Cloud on the device first.".to_string(),
        ));
    }
    api.accounts
        .remove_device(&account.page_id, &device)
        .map_err(account_error)?;
    api.deliveries.notify();
    info!("{} removed Moblin device {device}", account.id);
    api.remote_control
        .changes
        .notify(&account.page_id, Change::Account);
    Ok(account_response(&api, account))
}

pub(super) async fn handle_account_sign_out(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, String)> {
    let id = signed_in(&api, &headers)?.account.id;
    api.accounts.sign_out(&id).map_err(account_error)?;
    info!("{id} signed out everywhere");
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn handle_account_delete(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, String)> {
    let id = signed_in(&api, &headers)?.account.id;
    let account = api.accounts.delete(&id).map_err(account_error)?;
    api.remote_control.disconnect(&account.page_id);
    info!("{id} deleted their account");
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
#[path = "tests/account.rs"]
mod tests;
