use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderName, StatusCode};
use axum::response::Redirect;
use log::{info, warn};
use serde::Deserialize;

use crate::accounts::{Page, Platform};
use crate::auth::{STATE_COOKIE, SignIn, StateError};

use super::*;

#[derive(Deserialize)]
pub(super) struct LoginQuery {
    page: Option<String>,
    link: Option<String>,
}

pub(super) async fn handle_twitch_login(
    State(api): State<Arc<Api>>,
    Query(query): Query<LoginQuery>,
) -> Result<([(HeaderName, String); 1], Redirect), StatusCode> {
    if let Some(page) = &query.page
        && api
            .accounts
            .of_page(page)
            .is_none_or(|account| account.twitch().is_none())
    {
        return Err(StatusCode::NOT_FOUND);
    }
    let (url, state) = api
        .auth
        .twitch_login_url(query.page.as_deref(), query.link.is_some())
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    Ok((
        [(SET_COOKIE, api.auth.state_cookie(&state))],
        Redirect::to(&url),
    ))
}

pub(super) async fn handle_kick_login(
    State(api): State<Arc<Api>>,
    Query(query): Query<LoginQuery>,
) -> Result<([(HeaderName, String); 1], Redirect), StatusCode> {
    if let Some(page) = &query.page
        && api
            .accounts
            .of_page(page)
            .is_none_or(|account| account.kick().is_none())
    {
        return Err(StatusCode::NOT_FOUND);
    }
    let (url, state) = api
        .auth
        .kick_login_url(query.page.as_deref(), query.link.is_some())
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    Ok((
        [(SET_COOKIE, api.auth.state_cookie(&state))],
        Redirect::to(&url),
    ))
}

pub(super) fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value)
}

#[derive(Deserialize)]
pub(super) struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

pub(super) async fn handle_twitch_callback(
    State(api): State<Arc<Api>>,
    Query(query): Query<CallbackQuery>,
    headers: HeaderMap,
) -> ([(HeaderName, String); 1], Redirect) {
    (
        [(SET_COOKIE, api.auth.cleared_state_cookie())],
        signed_in_redirect(
            &api,
            Platform::Twitch,
            query,
            cookie(&headers, STATE_COOKIE),
        )
        .await,
    )
}

pub(super) async fn handle_kick_callback(
    State(api): State<Arc<Api>>,
    Query(query): Query<CallbackQuery>,
    headers: HeaderMap,
) -> ([(HeaderName, String); 1], Redirect) {
    (
        [(SET_COOKIE, api.auth.cleared_state_cookie())],
        signed_in_redirect(&api, Platform::Kick, query, cookie(&headers, STATE_COOKIE)).await,
    )
}

async fn signed_in_redirect(
    api: &Api,
    platform: Platform,
    query: CallbackQuery,
    browser_state: Option<&str>,
) -> Redirect {
    let dashboard = format!("{}/dashboard/", api.auth.frontend_url);
    let account = format!("{}/account/", api.auth.frontend_url);
    let failed = |link: bool| {
        Redirect::to(&if link {
            format!("{account}#linkFailed")
        } else {
            format!("{dashboard}#signInFailed")
        })
    };
    let Some(state) = query.state else {
        return failed(false);
    };
    let state = match api.auth.consume_state(&state, browser_state) {
        Ok(state) => state,
        Err(StateError {
            page: Some(page),
            error,
        }) => return viewer_redirect(api, platform, &page, Err(error)).await,
        Err(StateError { error, .. }) => {
            warn!("signing in with {} failed: {error:#}", platform.title());
            return failed(false);
        }
    };
    let sign_in = match query.code {
        Some(code) => match platform {
            Platform::Twitch => api.auth.twitch_callback(&code).await,
            Platform::Kick => {
                api.auth
                    .kick_callback(&code, state.verifier.as_deref().unwrap_or_default())
                    .await
            }
        },
        None => Err(anyhow::anyhow!(
            "{} answered {}",
            platform.title(),
            query.error.as_deref().unwrap_or("no code")
        )),
    };
    if let Some(page) = state.page {
        return viewer_redirect(api, platform, &page, sign_in).await;
    }
    let sign_in = match sign_in {
        Ok(sign_in) => sign_in,
        Err(error) => {
            warn!(
                "{} with {} failed: {error:#}",
                if state.link { "linking" } else { "signing in" },
                platform.title()
            );
            return failed(state.link);
        }
    };
    let identity = &sign_in.identity;
    if state.link {
        info!(
            "{} user {} ({}) is being linked",
            platform.title(),
            identity.user_id,
            identity.login
        );
        return Redirect::to(&format!(
            "{account}#link={}",
            api.auth.link_token(sign_in.identity)
        ));
    }
    let key = api.remote_control.new_key();
    let (Some(page), Some(new_id)) = (
        api.remote_control.new_page_id(),
        api.remote_control.new_account_id(),
    ) else {
        warn!(
            "no page could be created for {} user {}",
            platform.title(),
            identity.user_id
        );
        return failed(false);
    };
    let (id, account) = match api
        .accounts
        .signed_in(identity, (new_id, Page { id: page, key }))
    {
        Ok(signed_in) => signed_in,
        Err(error) => {
            warn!(
                "saving the account of {} user {} failed: {error:?}",
                platform.title(),
                identity.user_id
            );
            return failed(false);
        }
    };
    info!(
        "{} user {} ({}) signed in to account {id}",
        platform.title(),
        identity.user_id,
        identity.login
    );
    Redirect::to(&format!(
        "{dashboard}#session={}",
        api.auth.session(&id, account.sessions)
    ))
}

pub(super) enum ViewerSignInError {
    Failed(anyhow::Error),
    Unchecked(anyhow::Error),
}

async fn viewer_redirect(
    api: &Api,
    platform: Platform,
    page: &str,
    sign_in: Result<SignIn>,
) -> Redirect {
    let account = api.accounts.of_page(page);
    let streamer = account
        .as_ref()
        .and_then(|account| account.identity(platform).cloned());
    let address = streamer
        .iter()
        .map(|identity| (platform, identity))
        .chain(account.iter().flat_map(|account| {
            account
                .identities
                .iter()
                .map(|identity| (identity.platform, identity))
        }))
        .find(|(_, identity)| !identity.login.is_empty());
    let url = match address {
        Some((platform, identity)) => format!(
            "{}/{}/{}",
            api.auth.frontend_url,
            platform.name(),
            identity.login
        ),
        None => format!("{}/page/?streamer={page}", api.auth.frontend_url),
    };
    let tier = async {
        let sign_in = sign_in.map_err(ViewerSignInError::Failed)?;
        let streamer = streamer
            .with_context(|| format!("the page has no {} streamer", platform.title()))
            .map_err(ViewerSignInError::Failed)?;
        let tier = match platform {
            Platform::Twitch => api
                .auth
                .twitch_subscription_tier(&sign_in, &streamer.user_id)
                .await
                .map_err(ViewerSignInError::Failed)?,
            Platform::Kick => api
                .auth
                .kick_subscriber(&streamer.login, &sign_in.identity.login)
                .await
                .map_err(ViewerSignInError::Unchecked)?
                .then_some(1),
        };
        Ok((sign_in.identity, tier))
    };
    match tier.await {
        Ok((user, tier)) => {
            info!(
                "{} user {} ({}) signed in as a viewer of page {page} with subscription tier {tier:?}",
                platform.title(),
                user.user_id,
                user.login
            );
            Redirect::to(&format!(
                "{url}#viewer={}",
                api.auth
                    .viewer_session(platform, user.user_id, user.name, page.to_string(), tier)
            ))
        }
        Err(ViewerSignInError::Unchecked(error)) => {
            warn!("checking the Kick subscription of a viewer of page {page} failed: {error:#}");
            Redirect::to(&format!("{url}#viewerCheckFailed"))
        }
        Err(ViewerSignInError::Failed(error)) => {
            warn!(
                "signing in with {} as a viewer of page {page} failed: {error:#}",
                platform.title()
            );
            Redirect::to(&format!("{url}#viewerSignInFailed={}", platform.name()))
        }
    }
}

#[derive(Deserialize)]
pub(super) struct LinkRequest {
    token: String,
}

pub(super) async fn handle_account_link(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<LinkRequest>,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let SignedIn { id, .. } = signed_in(&api, &headers)?;
    let identity = api.auth.link_of(&request.token).ok_or((
        StatusCode::BAD_REQUEST,
        "Linking took too long. Try again.".to_string(),
    ))?;
    let account = api.accounts.link(&id, &identity).map_err(account_error)?;
    api.remote_control
        .changes
        .notify(&account.page.id, Change::Page);
    info!(
        "{id} linked {} user {} ({})",
        identity.platform.title(),
        identity.user_id,
        identity.login
    );
    Ok(account_response(&api, account))
}

pub(super) async fn handle_account_unlink(
    State(api): State<Arc<Api>>,
    Path(platform): Path<String>,
    headers: HeaderMap,
) -> Result<Json<AccountResponse>, (StatusCode, String)> {
    let SignedIn { id, .. } = signed_in(&api, &headers)?;
    let platform = platform_of(&platform)?;
    let account = api.accounts.unlink(&id, platform).map_err(account_error)?;
    api.remote_control
        .changes
        .notify(&account.page.id, Change::Page);
    info!("{id} unlinked {}", platform.title());
    Ok(account_response(&api, account))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::http::StatusCode;
    use serde_json::{Value, json};

    use super::*;
    use crate::accounts::{Accounts, Platform};
    use crate::api::test_utils::*;
    use crate::auth::Auth;
    use crate::counters::Counters;
    use crate::database::Database;
    use crate::deliveries::Deliveries;
    use crate::pages::Pages;
    use crate::payments::Orders;
    use crate::remote_control::Streamers;
    use crate::requests::Requests;
    use crate::subscribers::Allowances;

    #[tokio::test]
    async fn streamers_sign_in_and_manage_their_page() {
        let twitch = start_twitch().await;
        let (address, api) =
            start_api_with(None, None, Some(twitch_app(&twitch)), None, None).await;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let unconfigured = start().await;
        let response = client
            .get(format!("http://{unconfigured}/auth/twitch/login"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let response = client
            .get(format!("http://{address}/auth/twitch/login"))
            .send()
            .await
            .unwrap();
        let login = location(&response);
        let cookie = response.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        assert!(login.starts_with(&format!("{twitch}/authorize?")));
        let state = reqwest::Url::parse(&login)
            .unwrap()
            .query_pairs()
            .find(|(name, _)| name == "state")
            .unwrap()
            .1
            .into_owned();
        assert_eq!(cookie, format!("moblin_sign_in_state={state}"));
        let callback = format!("http://{address}/auth/twitch/callback?code=c&state={state}");
        let page = location(
            &client
                .get(&callback)
                .header("cookie", &cookie)
                .send()
                .await
                .unwrap(),
        );
        let session = page
            .strip_prefix("https://cloud.moblin.app/dashboard/#session=")
            .unwrap()
            .to_string();
        create_account(&api, "1", "One", "one");
        api.accounts
            .add_moderator(
                Platform::Twitch,
                "1",
                &twitch_moderator("42", "fortytwo", "42"),
            )
            .unwrap();
        assert!(api.accounts.moderates(Platform::Twitch, "42", "1"));
        assert!(!api.accounts.moderates(Platform::Twitch, "42", "9"));
        let response = client
            .get(format!("http://{address}/auth/twitch/login"))
            .send()
            .await
            .unwrap();
        let again = reqwest::Url::parse(&location(&response)).unwrap();
        let again_cookie = response.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        let again_state = again
            .query_pairs()
            .find(|(name, _)| name == "state")
            .unwrap()
            .1
            .into_owned();
        let again = location(
            &client
                .get(format!(
                    "http://{address}/auth/twitch/callback?code=c&state={again_state}"
                ))
                .header("cookie", &again_cookie)
                .send()
                .await
                .unwrap(),
        );
        assert!(
            again.starts_with("https://cloud.moblin.app/dashboard/#session="),
            "{again}"
        );
        assert!(api.accounts.moderates(Platform::Twitch, "42", "1"));
        assert_eq!(
            location(
                &client
                    .get(&callback)
                    .header("cookie", &cookie)
                    .send()
                    .await
                    .unwrap()
            ),
            "https://cloud.moblin.app/dashboard/#signInFailed"
        );
        let response = client
            .get(format!("http://{address}/auth/twitch/login"))
            .send()
            .await
            .unwrap();
        let state = reqwest::Url::parse(&location(&response))
            .unwrap()
            .query_pairs()
            .find(|(name, _)| name == "state")
            .unwrap()
            .1
            .into_owned();
        assert_eq!(
            location(
                &client
                    .get(format!(
                        "http://{address}/auth/twitch/callback?code=c&state={state}"
                    ))
                    .header("cookie", "moblin_sign_in_state=other")
                    .send()
                    .await
                    .unwrap()
            ),
            "https://cloud.moblin.app/dashboard/#signInFailed"
        );
        let response = client
            .get(format!("http://{address}/account"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let account: Value = client
            .get(format!("http://{address}/account"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let key = key_of(&account);
        assert_eq!(account["settings"]["id"].as_str().unwrap().len(), 16);
        assert_eq!(account["name"], "Erik");
        assert_eq!(account["avatar"], "https://static-cdn.jtvnw.net/erik.png");
        assert_eq!(
            account["twitch"],
            json!({"login": "erik", "name": "Erik", "avatar": "https://static-cdn.jtvnw.net/erik.png"})
        );
        assert_eq!(account["kick"], Value::Null);
        assert_eq!(account["devices"], json!([]));
        assert_eq!(account["admin"], false);
        assert_eq!(account["premium"], false);
        let settings: Value = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings, account["settings"]);
        let settings: Value = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({
                "message": "Thanks for hanging out!",
                "accent": "pink",
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(settings["message"], "Thanks for hanging out!");
        assert_eq!(settings["accent"], "pink");
        assert_eq!(settings["font"], Value::Null);
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"accent": "teal"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let name: Value = client
            .get(format!("http://{address}/pages/twitch/Erik"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(name["id"], account["settings"]["id"]);
        let profile: Value = client
            .get(format!(
                "http://{address}/pages/{}",
                name["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            profile,
            json!({
                "twitch": {
                    "name": "Erik",
                    "login": "erik",
                    "avatar": "https://static-cdn.jtvnw.net/erik.png",
                },
                "kick": null,
                "message": "Thanks for hanging out!",
                "accent": "pink",
                "font": null,
                "image": null,
                "icon": null,
            })
        );
        let response = client
            .get(format!("http://{address}/pages/unknown"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let response = client
            .get(format!("http://{address}/pages/twitch/other"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let again: Value = client
            .get(format!("http://{address}/account"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(key_of(&again), key);
    }

    fn link_of(account: &str) -> String {
        account
            .strip_prefix("https://cloud.moblin.app/account/#link=")
            .unwrap_or_else(|| panic!("{account}"))
            .to_string()
    }

    #[tokio::test]
    async fn kick_streamers_sign_in_with_pkce() {
        let (kick, fake) = start_kick().await;
        let (address, api) = start_api_with(None, None, None, Some(kick_app(&kick)), None).await;
        let flow = SignInFlow::new(&address, fake.clone());
        let unconfigured = start().await;
        let response = reqwest::Client::new()
            .get(format!("http://{unconfigured}/auth/kick/login"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let (query, cookie) = flow.start("/auth/kick/login").await;
        assert_eq!(query["client_id"], "kid");
        assert_eq!(query["redirect_uri"], "http://localhost/auth/kick/callback");
        assert_eq!(query["scope"], "user:read channel:read");
        assert_eq!(query["code_challenge_method"], "S256");
        assert_eq!(cookie, format!("moblin_sign_in_state={}", query["state"]));
        assert_eq!(
            flow.finish(
                "kick",
                "kick-a",
                &query["state"],
                "moblin_sign_in_state=other"
            )
            .await,
            "https://cloud.moblin.app/dashboard/#signInFailed"
        );
        let session = session_of(&flow.sign_in("kick", "kick-a").await);
        let token = fake.tokens.lock().unwrap().last().unwrap().clone();
        assert_eq!(token["client_id"], "kid");
        assert_eq!(token["client_secret"], "ksecret");
        assert_eq!(token["grant_type"], "authorization_code");
        assert_eq!(token["redirect_uri"], "http://localhost/auth/kick/callback");
        let account = account_of(&address, &session).await;
        assert_eq!(account["name"], "KickErik");
        assert_eq!(account["avatar"], "https://files.kick.com/erik.png");
        assert_eq!(account["twitch"], Value::Null);
        assert_eq!(
            account["kick"],
            json!({"login": "kickerik", "name": "KickErik", "avatar": "https://files.kick.com/erik.png"})
        );
        assert_eq!(account["twitchModerators"], json!([]));
        assert_eq!(account["moderating"], json!([]));
        let id = account["settings"]["id"].as_str().unwrap().to_string();
        let session_account = api.auth.session_of(&session).unwrap().account;
        assert_ne!(session_account, "123");
        assert_eq!(session_account.len(), 32);
        assert!(session_account.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(api.accounts.get(&session_account).unwrap().page.id, id);
        assert_eq!(api.accounts.get("123"), None);
        let (status, name) = get_json(&address, "/pages/kick/KickErik").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(name["id"], id);
        let (status, _) = get_json(&address, "/pages/twitch/kickerik").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (_, profile) = get_json(&address, &format!("/pages/{id}")).await;
        assert_eq!(profile["twitch"], Value::Null);
        assert_eq!(profile["kick"]["login"], "kickerik");
        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"subSpins": {"tier1": 1, "tier2": 2, "tier3": 3}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let (_, spin) = get_json(&address, &format!("/pages/{id}/spin")).await;
        assert_eq!(
            spin["subSpins"],
            json!({"tier1": 1, "tier2": 2, "tier3": 3})
        );
        let response = client
            .get(format!("http://{address}/auth/twitch/login?page={id}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let response = client
            .post(format!("http://{address}/account/moderators/twitch"))
            .bearer_auth(&session)
            .json(&json!({"username": "bob"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(response.text().await.unwrap(), "Link Twitch first.");
        let response = client
            .post(format!("http://{address}/account/moderators/youtube"))
            .bearer_auth(&session)
            .json(&json!({"username": "bob"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let again = session_of(&flow.sign_in("kick", "kick-a").await);
        assert_eq!(
            api.auth.session_of(&again).unwrap().account,
            session_account
        );
        assert_eq!(account_of(&address, &again).await["settings"]["id"], id);
        assert_eq!(
            flow.sign_in("kick", "unknown").await,
            "https://cloud.moblin.app/dashboard/#signInFailed"
        );
        let other = session_of(&flow.sign_in("kick", "kick-b").await);
        let other = account_of(&address, &other).await;
        assert_eq!(
            other["kick"],
            json!({"login": "other-kick", "name": "other-kick", "avatar": null})
        );
        assert_ne!(other["settings"]["id"], id);
        let account = api.accounts.get(&session_account).unwrap();
        assert_eq!(account.identities[0].admin_key(), "kick:123");
        assert!(!is_admin(&api, &account));
        assert_eq!(
            api.accounts.stats(&["kick:123", "kick:456", "123"]).premium,
            2
        );
        assert_eq!(api.accounts.stats(&["123", "456"]).premium, 0);
    }

    #[tokio::test]
    async fn linked_platforms_share_one_account() {
        let twitch = start_twitch().await;
        let (kick, fake) = start_kick().await;
        let (address, api) = start_api_with(
            None,
            None,
            Some(twitch_app(&twitch)),
            Some(kick_app(&kick)),
            None,
        )
        .await;
        let flow = SignInFlow::new(&address, fake.clone());
        let client = reqwest::Client::new();
        let session = session_of(&flow.sign_in("twitch", "c").await);
        let account = account_of(&address, &session).await;
        let id = account["settings"]["id"].as_str().unwrap().to_string();
        let key = key_of(&account);
        let twitch_account = api.auth.session_of(&session).unwrap().account;
        assert_ne!(twitch_account, "42");
        assert_eq!(account["kick"], Value::Null);
        let link = |token: &str, session: &str| {
            client
                .post(format!("http://{address}/account/link"))
                .bearer_auth(session)
                .json(&json!({"token": token}))
                .send()
        };
        let unlink = |platform: &str, session: &str| {
            client
                .post(format!("http://{address}/account/unlink/{platform}"))
                .bearer_auth(session)
                .send()
        };
        let response = unlink("twitch", &session).await.unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.text().await.unwrap(),
            "You can't unlink the only way to sign in. Link another platform first."
        );
        let response = unlink("kick", &session).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let response = unlink("youtube", &session).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let response = link("garbage", &session).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            flow.link("kick", "unknown").await,
            "https://cloud.moblin.app/account/#linkFailed"
        );
        let token = link_of(&flow.link("kick", "kick-a").await);
        assert_eq!(api.auth.link_of(&token).unwrap().user_id, "123".to_string());
        assert_eq!(
            api.accounts.get(&twitch_account).unwrap().identities.len(),
            1
        );
        let response = link(&token, "garbage").await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let account: Value = link(&token, &session).await.unwrap().json().await.unwrap();
        assert_eq!(account["twitch"]["login"], "erik");
        assert_eq!(account["kick"]["login"], "kickerik");
        assert_eq!(account["name"], "Erik");
        assert_eq!(account["settings"]["id"], id);
        assert_eq!(key_of(&account), key);
        let kick_session = session_of(&flow.sign_in("kick", "kick-a").await);
        assert_eq!(
            api.auth.session_of(&kick_session).unwrap().account,
            twitch_account
        );
        let account = account_of(&address, &kick_session).await;
        assert_eq!(account["settings"]["id"], id);
        assert_eq!(account["twitch"]["login"], "erik");
        assert_eq!(key_of(&account), key);
        for path in ["/pages/twitch/erik", "/pages/kick/kickerik"] {
            let (status, name) = get_json(&address, path).await;
            assert_eq!(status, StatusCode::OK, "{path}");
            assert_eq!(name["id"], id, "{path}");
        }
        let (_, profile) = get_json(&address, &format!("/pages/{id}")).await;
        assert_eq!(
            profile["twitch"],
            json!({"login": "erik", "name": "Erik", "avatar": "https://static-cdn.jtvnw.net/erik.png"})
        );
        assert_eq!(
            profile["kick"],
            json!({"login": "kickerik", "name": "KickErik", "avatar": "https://files.kick.com/erik.png"})
        );
        let response = client
            .post(format!("http://{address}/account/settings"))
            .bearer_auth(&session)
            .json(&json!({"subSpins": {"tier1": 1, "tier2": 2, "tier3": 3}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let (_, spin) = get_json(&address, &format!("/pages/{id}/spin")).await;
        assert_eq!(spin["subSpins"]["tier1"], 1);
        let other = session_of(&flow.sign_in("kick", "kick-b").await);
        let other_account = api.auth.session_of(&other).unwrap().account;
        assert_ne!(other_account, twitch_account);
        let token = link_of(&flow.link("twitch", "c").await);
        let response = link(&token, &other).await.unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.text().await.unwrap(),
            "This Twitch account already has its own page. Delete it first."
        );
        let token = link_of(&flow.link("kick", "kick-b").await);
        let response = link(&token, &session).await.unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.text().await.unwrap(),
            "This Kick account already has its own page. Delete it first."
        );
        assert_eq!(
            api.accounts.get(&other_account).unwrap().identities.len(),
            1
        );
        assert_eq!(
            api.accounts.get(&twitch_account).unwrap().identities.len(),
            2
        );
        let account: Value = unlink("twitch", &kick_session)
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(account["twitch"], Value::Null);
        assert_eq!(account["kick"]["login"], "kickerik");
        assert_eq!(account["name"], "KickErik");
        assert_eq!(account["settings"]["id"], id);
        let (status, _) = get_json(&address, "/pages/twitch/erik").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (_, spin) = get_json(&address, &format!("/pages/{id}/spin")).await;
        assert_eq!(spin["subSpins"]["tier1"], 1);
        assert_eq!(account_of(&address, &session).await["settings"]["id"], id);
        let fresh = session_of(&flow.sign_in("twitch", "c").await);
        let fresh_account = api.auth.session_of(&fresh).unwrap().account;
        assert_ne!(fresh_account, twitch_account);
        assert_ne!(account_of(&address, &fresh).await["settings"]["id"], id);
        let (status, name) = get_json(&address, "/pages/twitch/erik").await;
        assert_eq!(status, StatusCode::OK);
        assert_ne!(name["id"], id);
    }

    #[tokio::test]
    async fn sessions_of_twitch_user_ids_keep_working() {
        let twitch = start_twitch().await;
        let path = crate::database::temporary_path("twitch-user-id-sessions");
        let database = Database::open(Some(&path)).unwrap();
        database
            .lock()
            .execute_batch(
                "INSERT INTO accounts (id, page_id, moblin_key, sessions) VALUES
                     ('0123456789abcdef0123456789abcdef', 'oldpage', 'k', 3);
                 INSERT INTO identities (platform, user_id, account, login, name) VALUES
                     ('twitch', '42', '0123456789abcdef0123456789abcdef', 'erik', 'Old Erik');
                 INSERT INTO pages (id, settings) VALUES ('oldpage', '{\"message\":\"Old\"}');
                 INSERT INTO twitch_moderators (broadcaster, moderator, login, name) VALUES ('7', '42', 'erik', 'Erik');",
            )
            .unwrap();
        drop(database);
        let database = Database::open(Some(&path)).unwrap();
        let api = Arc::new(Api {
            app_attest: None,
            require_attested_moblin: false,
            remote_control: Streamers::new(std::time::Duration::ZERO),
            pages: Pages::new(database.clone()),
            paypal: None,
            stripe: None,
            orders: Orders::new(database.clone()),
            allowances: Allowances::new(database.clone()),
            deliveries: Deliveries::new(database.clone()),
            auth: Auth::new(
                reqwest::Client::new(),
                "http://localhost".to_string(),
                "https://cloud.moblin.app".to_string(),
                Some(twitch_app(&twitch)),
                None,
                Some("secret"),
            )
            .unwrap(),
            accounts: Accounts::new(database.clone()),
            counters: Counters::new(database.clone()),
            database,
            admins: vec!["42".to_string()],
            premium_accounts: Vec::new(),
            started: 0,
            tests: Mutex::default(),
            requests: Requests::default(),
            stopping: tokio::sync::watch::channel(false).0,
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("127.0.0.1:{}", listener.local_addr().unwrap().port());
        tokio::spawn({
            let api = api.clone();
            async move { axum::serve(listener, router(api)).await }
        });
        assert_eq!(api.accounts.get("42"), None);
        let id = api
            .accounts
            .account_of_identity(Platform::Twitch, "42")
            .unwrap();
        assert_eq!(id.len(), 32);
        let old_session = api.auth.session("42", 3);
        let account = account_of(&address, &old_session).await;
        assert_eq!(account["settings"]["id"], "oldpage");
        assert_eq!(account["twitch"]["login"], "erik");
        assert_eq!(account["admin"], true);
        let response = reqwest::Client::new()
            .get(format!("http://{address}/account"))
            .bearer_auth(api.auth.session("42", 2))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        create_account(&api, "7", "Streamer", "streamer");
        assert_eq!(
            account_of(&address, &old_session).await["moderating"][0]["login"],
            "streamer"
        );
        let flow = SignInFlow::new(&address, FakeKick::default());
        let session = session_of(&flow.sign_in("twitch", "c").await);
        assert_eq!(api.auth.session_of(&session).unwrap().account, id);
        let account = account_of(&address, &session).await;
        assert_eq!(account["settings"]["id"], "oldpage");
        assert_eq!(account["settings"]["message"], "Old");
        assert_eq!(account["name"], "Erik");
        assert_eq!(key_of(&account), "k");
        assert_eq!(account["admin"], true);
        let (_, name) = get_json(&address, "/pages/twitch/erik").await;
        assert_eq!(name["id"], "oldpage");
        crate::database::remove(&path);
    }
}
