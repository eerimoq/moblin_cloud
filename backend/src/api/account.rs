use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use log::info;
use serde::{Deserialize, Serialize};

use crate::accounts::{Account, AccountError, Moderator, Platform};

use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AccountResponse {
    pub(super) name: String,
    pub(super) avatar: Option<String>,
    pub(super) twitch: Option<PublicIdentity>,
    pub(super) kick: Option<PublicIdentity>,
    pub(super) key: Option<String>,
    pub(super) devices: Vec<DeviceResponse>,
    pub(super) settings: SettingsResponse,
    pub(super) admin: bool,
    pub(super) premium: bool,
    pub(super) moderating: Vec<ModeratingResponse>,
    pub(super) twitch_moderators: Vec<Moderator>,
    pub(super) kick_moderators: Vec<Moderator>,
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
pub(super) struct ModeratingResponse {
    id: String,
    platform: Platform,
    name: String,
    login: String,
    avatar: Option<String>,
    twitch: Option<PublicIdentity>,
    kick: Option<PublicIdentity>,
}

pub(super) fn moderating(api: &Api, account: &Account) -> Vec<ModeratingResponse> {
    let mut moderating: Vec<ModeratingResponse> = Vec::new();
    for (platform, streamer) in
        [Platform::Twitch, Platform::Kick]
            .into_iter()
            .flat_map(|platform| {
                account
                    .identity(platform)
                    .map(|identity| api.accounts.moderated(platform, &identity.user_id))
                    .unwrap_or_default()
                    .into_iter()
                    .map(move |streamer| (platform, streamer))
            })
    {
        if moderating.iter().any(|known| known.id == streamer.page.id) {
            continue;
        }
        let Some(channel) = streamer.identity(platform) else {
            continue;
        };
        moderating.push(ModeratingResponse {
            id: streamer.page.id.clone(),
            platform,
            name: channel.name.clone(),
            login: channel.login.clone(),
            avatar: channel.avatar.clone(),
            twitch: public_identity(&streamer, Platform::Twitch),
            kick: public_identity(&streamer, Platform::Kick),
        });
    }
    moderating
}

#[derive(Serialize)]
pub(super) struct DeviceResponse {
    id: String,
    name: String,
    music: bool,
    wheel: bool,
}

pub(super) fn device_name(names: &HashMap<String, String>, device: &str) -> String {
    names
        .get(device)
        .cloned()
        .unwrap_or_else(|| "Moblin".to_string())
}

pub(super) fn account_response(api: &Api, account: Account) -> Json<AccountResponse> {
    let page = &account.page;
    let settings = api.pages.get(&page.id);
    let twitch = account.twitch_id().map(str::to_string);
    Json(AccountResponse {
        name: account.name().to_string(),
        avatar: account.avatar().map(str::to_string),
        twitch: public_identity(&account, Platform::Twitch),
        kick: public_identity(&account, Platform::Kick),
        key: Some(page.key.clone()),
        devices: {
            let names = api.accounts.device_names(&page.id);
            api.remote_control
                .devices(&page.id)
                .into_iter()
                .map(|device| DeviceResponse {
                    name: device_name(&names, &device.id),
                    id: device.id,
                    music: device.music,
                    wheel: device.wheel,
                })
                .collect()
        },
        settings: settings_response(api, page.id.clone(), settings).0,
        admin: is_admin(api, &account),
        premium: is_premium(api, &account),
        twitch_moderators: twitch
            .as_deref()
            .map(|twitch| api.accounts.moderators(Platform::Twitch, twitch))
            .unwrap_or_default(),
        kick_moderators: account
            .kick_id()
            .map(|kick| api.accounts.moderators(Platform::Kick, kick))
            .unwrap_or_default(),
        moderating: moderating(api, &account),
    })
}

pub(super) fn account_error(error: AccountError) -> (StatusCode, String) {
    match error {
        AccountError::Unknown => not_signed_in(),
        AccountError::Full => (
            StatusCode::SERVICE_UNAVAILABLE,
            "too many accounts".to_string(),
        ),
        AccountError::UnknownModerator => (StatusCode::NOT_FOUND, "no such moderator".to_string()),
        AccountError::UnknownDevice => (StatusCode::NOT_FOUND, "no such device".to_string()),
        AccountError::TooManyModerators => {
            (StatusCode::CONFLICT, "at most 50 moderators".to_string())
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
    let id = signed_in(&api, &headers)?.id;
    let key = api.remote_control.new_key();
    let account = api.accounts.replace_key(&id, key).map_err(account_error)?;
    api.remote_control.disconnect(&account.page.id);
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
    let SignedIn { id, account, .. } = signed_in(&api, &headers)?;
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 30 {
        return Err((
            StatusCode::BAD_REQUEST,
            "the name must be 1 to 30 characters".to_string(),
        ));
    }
    api.accounts
        .rename_device(&account.page.id, &device, name)
        .map_err(account_error)?;
    info!("{id} renamed Moblin device {device}");
    Ok(account_response(&api, account))
}

pub(super) async fn handle_account_sign_out(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, String)> {
    let id = signed_in(&api, &headers)?.id;
    api.accounts.sign_out(&id).map_err(account_error)?;
    info!("{id} signed out everywhere");
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn handle_account_delete(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, String)> {
    let id = signed_in(&api, &headers)?.id;
    let account = api.accounts.delete(&id).map_err(account_error)?;
    api.remote_control.disconnect(&account.page.id);
    info!("{id} deleted their account");
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::http::{Method, StatusCode};
    use futures_util::StreamExt;
    use serde_json::{Value, json};
    use tokio_tungstenite::tungstenite::Message;

    use crate::accounts::Platform;
    use crate::api::test_utils::*;
    use crate::counters::Counter;

    #[tokio::test]
    async fn new_moblin_cloud_keys_keep_the_page() {
        let (address, api) = start_api(None).await;
        let (session, key, id) = streamer(&address, &api).await;
        let mut socket = connect(&address, &key).await;
        assert!(api.remote_control.is_connected(&id));
        let account: Value = reqwest::Client::new()
            .post(format!("http://{address}/account/key"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let new_key = key_of(&account);
        assert_ne!(new_key, key);
        assert_eq!(account["settings"]["id"], id);
        assert!(!api.remote_control.is_connected(&id));
        loop {
            match socket.next().await {
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            }
        }
        assert!(
            tokio_tungstenite::connect_async(format!(
                "ws://{address}/remote-control/streamer/{key}"
            ))
            .await
            .is_err()
        );
        let _socket = connect(&address, &new_key).await;
        assert!(api.remote_control.is_connected(&id));
        let name: Value = reqwest::Client::new()
            .get(format!("http://{address}/pages/twitch/erik"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(name["id"], id);
    }

    #[tokio::test]
    async fn signing_out_ends_every_session() {
        let (address, api) = start_api(None).await;
        let (session, _, _) = streamer(&address, &api).await;
        let other = api.auth.session("1", 0);
        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://{address}/account/sign-out"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        for token in [&session, &other] {
            let response = client
                .get(format!("http://{address}/account"))
                .bearer_auth(token)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
        let response = client
            .get(format!("http://{address}/account"))
            .bearer_auth(api.auth.session("1", 1))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn deleting_an_account_removes_everything_it_owns() {
        let (address, api) = start_api(None).await;
        let (session, key, id) = streamer(&address, &api).await;
        let moderator = create_account(&api, "21", "Mod 21", "mod21");
        api.accounts
            .add_moderator(
                Platform::Twitch,
                "1",
                &twitch_moderator("21", "mod21", "Mod 21"),
            )
            .unwrap();
        api.accounts
            .add_moderator(
                Platform::Twitch,
                "21",
                &twitch_moderator("1", "erik", "Erik"),
            )
            .unwrap();
        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"message": "Hi"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        api.deliveries.add_test(
            id.clone(),
            "r".to_string(),
            crate::deliveries::Item::Tip {
                user: "Erik".to_string(),
                message: String::new(),
                amount: 500,
                currency: "USD".to_string(),
            },
        );
        let socket = connect(&address, &key).await;
        assert!(api.remote_control.is_connected(&id));
        assert_eq!(api.deliveries.count(), 1);
        let response = client
            .post(format!("http://{address}/account/delete"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        drop(socket);
        assert!(!api.remote_control.is_connected(&id));
        assert_eq!(api.deliveries.count(), 0);
        assert!(api.accounts.get("1").is_none());
        assert!(
            api.accounts
                .page_of_login(Platform::Twitch, "erik")
                .is_none()
        );
        assert!(api.accounts.page_of_key(&key).is_none());
        assert!(api.accounts.moderators(Platform::Twitch, "21").is_empty());
        assert!(api.accounts.moderated(Platform::Twitch, "21").is_empty());
        assert_eq!(api.accounts.get("21").unwrap(), moderator);
        for (table, column, value) in [
            ("pages", "id", id.as_str()),
            ("twitch_moderators", "broadcaster", "1"),
            ("twitch_moderators", "moderator", "1"),
        ] {
            let count: u64 = api
                .database
                .lock()
                .query_row(
                    &format!("SELECT count(*) FROM {table} WHERE {column} = ?1"),
                    [value],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0, "{table}");
        }
        for path in ["/account", "/account/delete"] {
            let response = client
                .request(
                    if path == "/account" {
                        Method::GET
                    } else {
                        Method::POST
                    },
                    format!("http://{address}{path}"),
                )
                .bearer_auth(&session)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
        for path in ["/pages/twitch/erik".to_string(), format!("/pages/{id}")] {
            let response = client
                .get(format!("http://{address}{path}"))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
        assert!(
            tokio_tungstenite::connect_async(format!(
                "ws://{address}/remote-control/streamer/{key}"
            ))
            .await
            .is_err()
        );
        let again = create_account(&api, "1", "Erik", "erik");
        assert_ne!(again.page.id, id);
        assert_eq!(
            api.accounts.page_of_login(Platform::Twitch, "erik"),
            Some(again.page.id)
        );
    }

    #[tokio::test]
    async fn account_routes_need_a_session() {
        let (address, api) = start_api(None).await;
        let (session, _, _) = streamer(&address, &api).await;
        let unknown = api.auth.session("2", 0);
        let old = api.auth.session("1", 1);
        let (payload, _) = session.split_once('.').unwrap();
        let forged = format!("{payload}.AAAA");
        let client = reqwest::Client::new();
        for (method, path) in [
            ("GET", "/account"),
            ("POST", "/account/key"),
            ("POST", "/account/devices/d/name"),
            ("POST", "/account/settings"),
            ("POST", "/account/paypal/connect"),
            ("POST", "/account/paypal/check"),
            ("POST", "/account/sign-out"),
            ("POST", "/account/delete"),
        ] {
            for token in [
                None,
                Some("garbage"),
                Some(forged.as_str()),
                Some(unknown.as_str()),
                Some(old.as_str()),
            ] {
                let mut request = client
                    .request(method.parse().unwrap(), format!("http://{address}{path}"))
                    .json(&json!({"songs": null, "name": "Moblin", "music": "d", "wheel": "d", "enabled": false}));
                if let Some(token) = token {
                    request = request.bearer_auth(token);
                }
                let response = request.send().await.unwrap();
                assert_eq!(
                    response.status(),
                    StatusCode::UNAUTHORIZED,
                    "{method} {path} with {token:?}"
                );
            }
        }
        let unknown_key = api.remote_control.new_key();
        assert!(
            tokio_tungstenite::connect_async(format!(
                "ws://{address}/remote-control/streamer/{unknown_key}"
            ))
            .await
            .is_err()
        );
        assert_eq!(
            api.counters.all(),
            [Counter {
                name: "moblinConnectionsFailed:unknown-key".to_string(),
                today: 1,
                week: 1,
                total: 1,
            }]
        );
    }
}
