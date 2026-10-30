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
    let (platform, SignedIn { id, account, .. }, broadcaster) =
        broadcaster(&api, &platform, &headers)?;
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
                "looking up {} user {username} for {id} failed: {error:#}",
                platform.title()
            );
            return Err((
                StatusCode::BAD_GATEWAY,
                format!("Couldn't look the {} user up. Try again.", platform.title()),
            ));
        }
    };
    if assistant.id == broadcaster {
        return Err((
            StatusCode::CONFLICT,
            format!("That is your own {} account.", platform.title()),
        ));
    }
    api.accounts
        .add_assistant(platform, &broadcaster, &assistant)
        .map_err(account_error)?;
    info!(
        "{id} added {} assistant {} ({})",
        platform.title(),
        assistant.id,
        assistant.login
    );
    assistants_changed(&api, &account.page.id, platform, &assistant.id);
    Ok(account_response(&api, account))
}

pub(super) async fn handle_account_assistant_remove(
    State(api): State<Arc<Api>>,
    Path((platform, assistant)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let (platform, SignedIn { id, account, .. }, broadcaster) =
        broadcaster(&api, &platform, &headers)?;
    api.accounts
        .remove_assistant(platform, &broadcaster, &assistant)
        .map_err(account_error)?;
    info!("{id} removed {} assistant {assistant}", platform.title());
    assistants_changed(&api, &account.page.id, platform, &assistant);
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
    let (platform, SignedIn { id, account, .. }, broadcaster) =
        broadcaster(api, platform, headers)?;
    set(&api.accounts, platform, &broadcaster).map_err(account_error)?;
    info!("{id} {change} {} assistant {assistant}", platform.title());
    assistants_changed(api, &account.page.id, platform, assistant);
    Ok(account_response(api, account))
}

fn assistants_changed(api: &Api, page: &str, platform: Platform, assistant: &str) {
    api.remote_control.changes.notify(page, Change::Account);
    if let Some(id) = api.accounts.account_of_identity(platform, assistant) {
        account_changed(api, &id, Change::Account);
    }
}

pub(super) async fn handle_assistant_account(
    State(api): State<Arc<Api>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<AccountResponse>, Failure> {
    let signed_in = assistant(&api, &headers, &id)?;
    let account = api.accounts.of_page(&id).ok_or_else(no_such_page)?;
    let settings = api.pages.get(&id);
    Ok(Json(AccountResponse {
        twitch: public_identity(&account, Platform::Twitch),
        kick: public_identity(&account, Platform::Kick),
        name: account.name().to_string(),
        avatar: account.avatar().map(str::to_string),
        key: None,
        devices: devices(&api, &id),
        settings: without_payments(settings_response(&api, id.clone(), settings)).0,
        admin: false,
        premium: streamer_is_premium(&api, &id),
        preview: may(&api, &signed_in.account, &id, Permission::Preview),
        go_live: may(&api, &signed_in.account, &id, Permission::GoLive),
        edit_tips: may(&api, &signed_in.account, &id, Permission::EditTips),
        remote_control: access(&api, &signed_in.account, &id),
        twitch_assistants: Vec::new(),
        kick_assistants: Vec::new(),
        assisting: Vec::new(),
    }))
}

#[cfg(test)]
mod tests {

    use axum::http::StatusCode;
    use serde_json::{Value, json};

    use crate::accounts::Platform;
    use crate::api::test_utils::*;
    use crate::images;
    use crate::pages::{Feature, Settings, Spin, SpinOption, Tips};

    #[tokio::test]
    async fn kick_assistants_are_an_allow_list() {
        let (kick, fake) = start_kick().await;
        let (address, api) = start_api_with(None, None, None, Some(kick_app(&kick)), None).await;
        let flow = SignInFlow::new(&address, fake.clone());
        let client = reqwest::Client::new();
        let session = session_of(&flow.sign_in("kick", "kick-a").await);
        let id = account_of(&address, &session).await["settings"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let add = async |session: &str, username: &str| {
            client
                .post(format!("http://{address}/account/assistants/kick"))
                .bearer_auth(session)
                .json(&json!({"username": username}))
                .send()
                .await
                .unwrap()
        };
        let remove = async |session: &str, assistant: &str| {
            client
                .post(format!(
                    "http://{address}/account/assistants/kick/{assistant}/remove"
                ))
                .bearer_auth(session)
                .send()
                .await
                .unwrap()
        };
        let dashboard = async |session: &str| {
            client
                .get(format!("http://{address}/account/pages/{id}"))
                .bearer_auth(session)
                .send()
                .await
                .unwrap()
        };
        create_account(&api, "2", "Other", "other");
        let twitch_only = api.auth.session("2", 0);
        let response = add(&twitch_only, "bob").await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(response.text().await.unwrap(), "Link Kick first.");
        let response = remove(&twitch_only, "9").await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let response = add("", "bob").await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        for (username, status, text) in [
            ("", StatusCode::BAD_REQUEST, "Enter a Kick username."),
            (
                "bad name!",
                StatusCode::BAD_REQUEST,
                "Enter a Kick username.",
            ),
            (
                "nobody",
                StatusCode::NOT_FOUND,
                "There is no Kick user named nobody.",
            ),
            (
                "broken",
                StatusCode::BAD_GATEWAY,
                "Couldn't look the Kick user up. Try again.",
            ),
            (
                "KickErik",
                StatusCode::CONFLICT,
                "That is your own Kick account.",
            ),
        ] {
            let response = add(&session, username).await;
            assert_eq!(response.status(), status, "{username}");
            assert_eq!(response.text().await.unwrap(), text);
        }
        assert_eq!(
            account_of(&address, &session).await["kickAssistants"],
            json!([])
        );
        let response = add(&session, " @Bob ").await;
        assert_eq!(response.status(), StatusCode::OK);
        let account: Value = response.json().await.unwrap();
        assert_eq!(
            account["kickAssistants"],
            json!([{"id": "9", "login": "bob", "name": "bob", "avatar": null, "preview": false, "goLive": false, "editTips": false, "remoteControl": "none"}])
        );
        let token = fake.tokens.lock().unwrap().last().unwrap().clone();
        assert_eq!(token["grant_type"], "client_credentials");
        assert_eq!(token["client_id"], "kid");
        assert_eq!(token["client_secret"], "ksecret");
        let response = add(&session, "bob").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.json::<Value>().await.unwrap()["kickAssistants"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let other = session_of(&flow.sign_in("kick", "kick-b").await);
        assert_eq!(dashboard(&other).await.status(), StatusCode::FORBIDDEN);
        assert_eq!(account_of(&address, &other).await["assisting"], json!([]));
        let assistant = session_of(&flow.sign_in("kick", "kick-mod").await);
        let account = account_of(&address, &assistant).await;
        assert_eq!(account["kick"]["login"], "robert");
        assert_eq!(
            account["assisting"],
            json!([{
                "id": id,
                "platform": "kick",
                "name": "KickErik",
                "login": "kickerik",
                "avatar": "https://files.kick.com/erik.png",
                "twitch": null,
                "kick": {
                    "login": "kickerik",
                    "name": "KickErik",
                    "avatar": "https://files.kick.com/erik.png",
                },
            }])
        );
        assert_eq!(
            account_of(&address, &session).await["kickAssistants"],
            json!([{"id": "9", "login": "robert", "name": "Robert", "avatar": "https://files.kick.com/robert.png", "preview": false, "goLive": false, "editTips": false, "remoteControl": "none"}])
        );
        let response = dashboard(&assistant).await;
        assert_eq!(response.status(), StatusCode::OK);
        let streamer: Value = response.json().await.unwrap();
        assert_eq!(streamer["kick"]["login"], "kickerik");
        assert_eq!(streamer["twitch"], Value::Null);
        assert_eq!(streamer["kickAssistants"], json!([]));
        assert_eq!(streamer["twitchAssistants"], json!([]));
        let response = client
            .get(format!(
                "http://{address}/account/pages/{id}/recent?kind=tip"
            ))
            .bearer_auth(&assistant)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let save = async || {
            client
                .post(format!("http://{address}/account/pages/{id}/settings"))
                .bearer_auth(&assistant)
                .json(&json!({"message": "Hi from the mod"}))
                .send()
                .await
                .unwrap()
        };
        assert_eq!(save().await.status(), StatusCode::FORBIDDEN);
        let response = client
            .post(format!(
                "http://{address}/account/assistants/kick/9/edit-tips"
            ))
            .bearer_auth(&session)
            .json(&json!({"on": true}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(save().await.status(), StatusCode::OK);
        let (_, profile) = get_json(&address, &format!("/pages/{id}")).await;
        assert_eq!(profile["message"], "Hi from the mod");
        let response = remove(&session, "9").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.json::<Value>().await.unwrap()["kickAssistants"],
            json!([])
        );
        assert_eq!(dashboard(&assistant).await.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            account_of(&address, &assistant).await["assisting"],
            json!([])
        );
        let response = remove(&session, "9").await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.text().await.unwrap(), "no such assistant");
    }

    #[tokio::test]
    async fn twitch_assistants_are_added_by_username() {
        let twitch = start_twitch().await;
        let (address, api) =
            start_api_with(None, None, Some(twitch_app(&twitch)), None, None).await;
        let client = reqwest::Client::new();
        let (session, _, id) = streamer(&api);
        let add = async |username: &str| {
            client
                .post(format!("http://{address}/account/assistants/twitch"))
                .bearer_auth(&session)
                .json(&json!({"username": username}))
                .send()
                .await
                .unwrap()
        };
        for (username, status, text) in [
            ("", StatusCode::BAD_REQUEST, "Enter a Twitch username."),
            (
                "bad-name",
                StatusCode::BAD_REQUEST,
                "Enter a Twitch username.",
            ),
            (
                "nobody",
                StatusCode::NOT_FOUND,
                "There is no Twitch user named nobody.",
            ),
            (
                "broken",
                StatusCode::BAD_GATEWAY,
                "Couldn't look the Twitch user up. Try again.",
            ),
            (
                "Erik",
                StatusCode::CONFLICT,
                "That is your own Twitch account.",
            ),
        ] {
            let response = add(username).await;
            assert_eq!(response.status(), status, "{username}");
            assert_eq!(response.text().await.unwrap(), text);
        }
        let response = add(" @Bob ").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.json::<Value>().await.unwrap()["twitchAssistants"],
            json!([{"id": "9", "login": "bob", "name": "Bob", "avatar": null, "preview": false, "goLive": false, "editTips": false, "remoteControl": "none"}])
        );
        assert!(api.accounts.assists(Platform::Twitch, "9", "1"));
        create_account(&api, "9", "Robert", "robert");
        let assistant = api.auth.session("9", 0);
        let account = account_of(&address, &assistant).await;
        assert_eq!(account["assisting"][0]["id"], id);
        assert_eq!(account["assisting"][0]["platform"], "twitch");
        assert_eq!(
            account_of(&address, &session).await["twitchAssistants"],
            json!([{"id": "9", "login": "robert", "name": "Robert", "avatar": null, "preview": false, "goLive": false, "editTips": false, "remoteControl": "none"}])
        );
        let response = client
            .get(format!("http://{address}/account/pages/{id}"))
            .bearer_auth(&assistant)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        for n in 0..49 {
            api.accounts
                .add_assistant(
                    Platform::Twitch,
                    "1",
                    &twitch_assistant(&format!("{}", 100 + n), "m", "M"),
                )
                .unwrap();
        }
        let response = add("late").await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(response.text().await.unwrap(), "at most 50 assistants");
        let response = client
            .post(format!(
                "http://{address}/account/assistants/twitch/9/remove"
            ))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!api.accounts.assists(Platform::Twitch, "9", "1"));
        assert_eq!(
            account_of(&address, &assistant).await["assisting"],
            json!([])
        );
        let response = client
            .get(format!("http://{address}/account/pages/{id}"))
            .bearer_auth(&assistant)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn assistants_have_access_while_listed() {
        let twitch = start_twitch().await;
        let (address, api) =
            start_api_with(None, None, Some(twitch_app(&twitch)), None, None).await;
        let client = reqwest::Client::new();
        let (session, _, id) = streamer(&api);
        create_account(&api, "21", "Mod 21", "mod21");
        create_account(&api, "22", "Viewer 22", "viewer22");
        api.accounts
            .add_assistant(
                Platform::Twitch,
                "9",
                &twitch_assistant("22", "viewer22", "Viewer 22"),
            )
            .unwrap();
        let (mod1, viewer1) = (api.auth.session("21", 0), api.auth.session("22", 0));
        let assisting = async |session: &str| {
            client
                .get(format!("http://{address}/account"))
                .bearer_auth(session)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()["assisting"]
                .clone()
        };
        let dashboard = async |session: &str| {
            client
                .get(format!("http://{address}/account/pages/{id}"))
                .bearer_auth(session)
                .send()
                .await
                .unwrap()
                .status()
        };
        let toggle = async |assistant: &str, enabled: bool| {
            if enabled {
                client
                    .post(format!("http://{address}/account/assistants/twitch"))
                    .bearer_auth(&session)
                    .json(&json!({"username": assistant}))
            } else {
                client
                    .post(format!(
                        "http://{address}/account/assistants/twitch/{assistant}/remove"
                    ))
                    .bearer_auth(&session)
            }
            .send()
            .await
            .unwrap()
        };
        let settings = async |enabled: bool| {
            let response = toggle(if enabled { "mod21" } else { "21" }, enabled).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.json::<Value>().await.unwrap()["twitchAssistants"],
                if enabled {
                    json!([{"id": "21", "login": "mod21", "name": "Mod 21", "avatar": null, "preview": false, "goLive": false, "editTips": false, "remoteControl": "none"}])
                } else {
                    json!([])
                }
            );
        };
        assert_eq!(toggle("nobody", true).await.status(), StatusCode::NOT_FOUND);
        assert_eq!(toggle("22", false).await.status(), StatusCode::NOT_FOUND);
        assert_eq!(assisting(&mod1).await, json!([]));
        assert_eq!(dashboard(&mod1).await, StatusCode::FORBIDDEN);
        settings(true).await;
        assert_eq!(
            assisting(&mod1).await,
            json!([{
                "id": id,
                "platform": "twitch",
                "name": "Erik",
                "login": "erik",
                "avatar": null,
                "twitch": {"login": "erik", "name": "Erik", "avatar": null},
                "kick": null,
            }])
        );
        assert_eq!(assisting(&viewer1).await, json!([]));
        assert_eq!(dashboard(&viewer1).await, StatusCode::FORBIDDEN);
        assert_eq!(dashboard("garbage").await, StatusCode::UNAUTHORIZED);
        assert_eq!(dashboard(&mod1).await, StatusCode::OK);
        let recent = async |session: &str, kind: &str| {
            client
                .get(format!(
                    "http://{address}/account/pages/{id}/recent?kind={kind}"
                ))
                .bearer_auth(session)
                .send()
                .await
                .unwrap()
        };
        assert_eq!(
            recent(&viewer1, "tip").await.status(),
            StatusCode::FORBIDDEN
        );
        for kind in ["tip", "spin"] {
            let response = recent(&mod1, kind).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.json::<Value>().await.unwrap()["items"], json!([]));
        }
        assert_eq!(
            recent(&mod1, "other").await.status(),
            StatusCode::BAD_REQUEST
        );
        settings(false).await;
        assert_eq!(assisting(&mod1).await, json!([]));
        assert_eq!(dashboard(&mod1).await, StatusCode::FORBIDDEN);
        assert_eq!(dashboard(&session).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn assistants_use_the_streamers_dashboard() {
        let (address, api, _fake) = start_paid_fake().await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&api);
        connect_paypal(&api);
        api.pages
            .set(
                &id,
                Settings {
                    currency: Some("SEK".to_string()),
                    tips: Some(Feature::on(Tips { amounts: vec![500] })),
                    ..Settings::default()
                },
            )
            .unwrap();
        create_account(&api, "21", "Mod 21", "mod21");
        create_account(&api, "22", "Viewer 22", "viewer22");
        let (mod1, viewer1) = (api.auth.session("21", 0), api.auth.session("22", 0));
        let get = async |session: &str, path: &str| {
            client
                .get(format!("http://{address}/account/pages/{id}{path}"))
                .bearer_auth(session)
                .send()
                .await
                .unwrap()
        };
        let post = async |session: &str, path: &str, body: Value| {
            client
                .post(format!("http://{address}/account/pages/{id}{path}"))
                .bearer_auth(session)
                .json(&body)
                .send()
                .await
                .unwrap()
        };
        for (session, status) in [
            ("", StatusCode::UNAUTHORIZED),
            (mod1.as_str(), StatusCode::FORBIDDEN),
            (viewer1.as_str(), StatusCode::FORBIDDEN),
        ] {
            assert_eq!(get(session, "").await.status(), status);
            assert_eq!(
                post(session, "/settings", json!({"message": "Hi"}))
                    .await
                    .status(),
                status
            );
            assert_eq!(post(session, "/test/tip", json!({})).await.status(), status);
            assert_eq!(
                post(session, "/test/spin", json!({})).await.status(),
                status
            );
            assert_eq!(
                post(session, "/image/remove", json!({})).await.status(),
                status
            );
        }
        api.accounts
            .add_assistant(
                Platform::Twitch,
                "1",
                &twitch_assistant("21", "mod21", "Mod 21"),
            )
            .unwrap();
        let own: Value = client
            .get(format!("http://{address}/account"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(own["settings"]["merchantId"], "MERCHANT00001");
        let account: Value = get(&mod1, "").await.json().await.unwrap();
        assert_eq!(account["name"], "Erik");
        assert_eq!(account["twitch"]["login"], "erik");
        assert_eq!(account["kick"], Value::Null);
        assert_eq!(account["key"], Value::Null);
        assert_eq!(account["devices"], json!([]));
        assert_eq!(account["admin"], false);
        assert_eq!(account["premium"], false);
        assert_eq!(account["assisting"], json!([]));
        assert_eq!(account["twitchAssistants"], json!([]));
        assert_eq!(account["editTips"], false);
        let settings = &account["settings"];
        assert_eq!(settings["id"], id);
        assert_eq!(settings["currency"], "SEK");
        assert_eq!(settings["tips"], json!({"enabled": true, "amounts": [500]}));
        assert_eq!(settings["paypal"], false);
        assert_eq!(settings["merchantId"], Value::Null);
        assert_eq!(settings["paypalPending"], false);
        assert_eq!(settings["stripe"], false);
        assert_eq!(settings["stripeAccountId"], Value::Null);
        assert_eq!(settings["stripePending"], false);
        assert_eq!(settings["payable"], true);
        assert_eq!(get(&viewer1, "").await.status(), StatusCode::FORBIDDEN);
        for (path, body) in [
            ("/settings", json!({"message": "Hi"})),
            ("/image/remove", json!({})),
        ] {
            let response = post(&mod1, path, body).await;
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            assert_eq!(
                response.text().await.unwrap(),
                "The streamer has not let you edit tips."
            );
        }
        let response = client
            .post(format!("http://{address}/account/pages/{id}/image"))
            .bearer_auth(&mod1)
            .header("content-type", "image/png")
            .body(images::tests::png(1200, 400))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(api.pages.get(&id).message, None);
        let response = client
            .post(format!(
                "http://{address}/account/assistants/twitch/21/edit-tips"
            ))
            .bearer_auth(&session)
            .json(&json!({"on": true}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.json::<Value>().await.unwrap()["twitchAssistants"][0]["editTips"],
            true
        );
        assert_eq!(
            get(&mod1, "").await.json::<Value>().await.unwrap()["editTips"],
            true
        );
        let response = post(
            &mod1,
            "/settings",
            json!({
                "currency": "USD",
                "tips": {"amounts": [300, 700]},
                "spin": {"options": [{"spins": 1, "price": 100}]},
                "message": " Hi ",
                "subSpins": {"tier1": 1, "tier2": 2, "tier3": 3},
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let settings: Value = response.json().await.unwrap();
        assert_eq!(settings["message"], "Hi");
        assert_eq!(
            settings["subSpins"],
            json!({"enabled": true, "tier1": 1, "tier2": 2, "tier3": 3})
        );
        assert_eq!(settings["currency"], "SEK");
        assert_eq!(
            settings["tips"],
            json!({"enabled": true, "amounts": [300, 700]})
        );
        assert_eq!(
            settings["spin"],
            json!({"enabled": true, "options": [{"spins": 1, "price": 100}]})
        );
        assert_eq!(settings["merchantId"], Value::Null);
        assert_eq!(settings["paypal"], false);
        assert_eq!(settings["payable"], true);
        let saved = api.pages.get(&id);
        assert_eq!(saved.message.as_deref(), Some("Hi"));
        assert_eq!(saved.currency(), "SEK");
        assert_eq!(
            saved.tips(),
            Some(&Tips {
                amounts: vec![300, 700]
            })
        );
        assert_eq!(
            saved.spin(),
            Some(&Spin {
                options: vec![SpinOption {
                    spins: 1,
                    price: 100
                }]
            })
        );
        let response = post(&mod1, "/settings", json!({"tips": {"amounts": [100]}})).await;
        assert_eq!(response.status(), StatusCode::OK);
        let tips = |amounts: Vec<u32>| Tips { amounts };
        assert_eq!(api.pages.get(&id).tips(), Some(&tips(vec![100])));
        api.accounts.forget_paypal("1").unwrap();
        let account: Value = get(&mod1, "").await.json().await.unwrap();
        assert_eq!(account["settings"]["payable"], false);
        let response = post(&mod1, "/settings", json!({"tips": {"amounts": [150]}})).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.text().await.unwrap(),
            "connect PayPal or Stripe first"
        );
        assert_eq!(api.pages.get(&id).tips(), Some(&tips(vec![100])));
        let response = post(&mod1, "/settings", json!({"tips": {"amounts": [100]}})).await;
        assert_eq!(response.status(), StatusCode::OK);
        let off = json!({"tips": {"enabled": false, "amounts": [150]}});
        let response = post(&mod1, "/settings", off.clone()).await;
        assert_eq!(response.status(), StatusCode::OK);
        let settings: Value = response.json().await.unwrap();
        assert_eq!(settings["tips"], off["tips"]);
        assert_eq!(api.pages.get(&id).tips(), None);
        let response = post(&mod1, "/settings", json!({"tips": {"amounts": [150]}})).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let response = post(&mod1, "/settings", json!({"message": "Hi"})).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(api.pages.get(&id).tips, None);
        connect_paypal(&api);
        let response = post(&mod1, "/settings", json!({"font": "Poppins"})).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = client
            .post(format!("http://{address}/account/pages/{id}/image"))
            .bearer_auth(&mod1)
            .header("content-type", "image/png")
            .body(images::tests::png(1200, 400))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = post(&mod1, "/image/remove", json!({})).await;
        assert_eq!(response.status(), StatusCode::OK);
        let response = post(&mod1, "/test/spin", json!({})).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(response.text().await.unwrap(), "Connect Moblin first.");
        let response = post(&mod1, "/test/tip", json!({"message": " From a mod "})).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let mut socket = connect(&address, &key).await;
        let request = receive(&mut socket).await;
        let tip = &request["request"]["data"]["tip"];
        assert_eq!(tip["name"], "Mod 21");
        assert_eq!(tip["message"], "From a mod");
        assert_eq!(tip["kind"]["message"]["amount"], "5.00");
        assert_eq!(tip["kind"]["message"]["currency"], "SEK");
        answer(&mut socket, &request, Value::Null).await;
        wait_until_added(&api).await;
        assert_eq!(counter(&api, "tipTests"), 1);
        assert_eq!(counter(&api, "tipTestsFailed:not-assistant"), 2);
        assert_eq!(counter(&api, "spinTestsFailed:offline"), 1);
        assert_eq!(counter(&api, "imageUploadsFailed:not-premium"), 1);
        assert_eq!(counter(&api, "imageUploadsFailed:not-assistant"), 0);
    }
}
