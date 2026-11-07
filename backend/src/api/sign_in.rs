use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderName, StatusCode};
use axum::response::Redirect;
use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::accounts::{AccountError, Platform};
use crate::auth::{ConnectPoll, STATE_COOKIE, SignIn, StateError, Terminal};

use super::*;

#[derive(Deserialize)]
pub(super) struct LoginQuery {
    #[serde(rename = "page")]
    page_id: Option<String>,
    link: Option<String>,
    port: Option<u16>,
    challenge: Option<String>,
}

pub(super) async fn handle_twitch_login(
    State(api): State<Arc<Api>>,
    Query(query): Query<LoginQuery>,
) -> Result<([(HeaderName, String); 1], Redirect), StatusCode> {
    login(&api, Platform::Twitch, query)
}

pub(super) async fn handle_kick_login(
    State(api): State<Arc<Api>>,
    Query(query): Query<LoginQuery>,
) -> Result<([(HeaderName, String); 1], Redirect), StatusCode> {
    login(&api, Platform::Kick, query)
}

fn login(
    api: &Api,
    platform: Platform,
    query: LoginQuery,
) -> Result<([(HeaderName, String); 1], Redirect), StatusCode> {
    if let Some(page_id) = &query.page_id
        && api
            .accounts
            .of_page(page_id)
            .is_none_or(|account| account.identity(platform).is_none())
    {
        return Err(StatusCode::NOT_FOUND);
    }
    let terminal = match (query.port, query.challenge) {
        (Some(_), _) | (_, Some(_)) if query.page_id.is_some() || query.link.is_some() => {
            return Err(StatusCode::BAD_REQUEST);
        }
        (Some(port), Some(challenge)) => {
            Some(Terminal::new(port, challenge).ok_or(StatusCode::BAD_REQUEST)?)
        }
        (None, None) => None,
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    let (url, state) = api
        .auth
        .login_url(
            platform,
            query.page_id.as_deref(),
            query.link.is_some(),
            terminal,
        )
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
    let failed = |link: bool, port: Option<u16>| {
        Redirect::to(&match port {
            Some(port) => format!("http://127.0.0.1:{port}/?signInFailed"),
            None if link => format!("{account}#linkFailed"),
            None => format!("{dashboard}#signInFailed"),
        })
    };
    let Some(state) = query.state else {
        return failed(false, None);
    };
    let state = match api.auth.consume_state(&state, browser_state) {
        Ok(state) => state,
        Err(StateError {
            page_id: Some(page_id),
            error,
        }) => return viewer_redirect(api, platform, &page_id, Err(error)).await,
        Err(StateError { error, .. }) => {
            warn!("signing in with {} failed: {error:#}", platform.title());
            return failed(false, None);
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
    if let Some(page_id) = state.page_id {
        return viewer_redirect(api, platform, &page_id, sign_in).await;
    }
    let port = state.terminal.as_ref().map(|terminal| terminal.port);
    let sign_in = match sign_in {
        Ok(sign_in) => sign_in,
        Err(error) => {
            warn!(
                "{} with {} failed: {error:#}",
                if state.link { "linking" } else { "signing in" },
                platform.title()
            );
            return failed(state.link, port);
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
    let moblin_cloud_key = api.remote_control.new_moblin_cloud_key();
    let (Some(page_id), Some(new_id)) = (
        api.remote_control.new_page_id(),
        api.remote_control.new_account_id(),
    ) else {
        warn!(
            "no page could be created for {} user {}",
            platform.title(),
            identity.user_id
        );
        return failed(false, port);
    };
    let account = match api
        .accounts
        .signed_in(identity, new_id, page_id, moblin_cloud_key)
    {
        Ok(signed_in) => signed_in,
        Err(error) => {
            warn!(
                "saving the account of {} user {} failed: {error:?}",
                platform.title(),
                identity.user_id
            );
            return failed(false, port);
        }
    };
    info!(
        "{} user {} ({}) signed in to account {}",
        platform.title(),
        identity.user_id,
        identity.login,
        account.id
    );
    let session = api.auth.session(&account.id, account.sessions);
    let Some(terminal) = state.terminal else {
        return Redirect::to(&format!("{dashboard}#session={session}"));
    };
    match api.auth.terminal_code(terminal.challenge, session) {
        Some(code) => Redirect::to(&format!("http://127.0.0.1:{}/?code={code}", terminal.port)),
        None => failed(false, Some(terminal.port)),
    }
}

#[derive(Deserialize)]
pub(super) struct SessionRequest {
    code: String,
    verifier: String,
}

#[derive(Serialize)]
pub(super) struct SessionResponse {
    session: String,
}

pub(super) async fn handle_session(
    State(api): State<Arc<Api>>,
    Json(request): Json<SessionRequest>,
) -> Result<Json<SessionResponse>, (StatusCode, String)> {
    let session = api
        .auth
        .session_of_code(&request.code, &request.verifier)
        .ok_or((
            StatusCode::BAD_REQUEST,
            "Signing in failed. Try again.".to_string(),
        ))?;
    Ok(Json(SessionResponse { session }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ConnectResponse {
    device_code: String,
    user_code: String,
    url: String,
    interval: u64,
    expires_in: u64,
}

pub(super) async fn handle_connect(
    State(api): State<Arc<Api>>,
) -> Result<Json<ConnectResponse>, (StatusCode, String)> {
    let (device_code, user_code) = api.auth.start_connect().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Signing in is busy. Try again in a minute.".to_string(),
    ))?;
    let url = format!("{}/connect/", api.auth.frontend_url);
    Ok(Json(ConnectResponse {
        device_code,
        user_code: format!("{}-{}", &user_code[..4], &user_code[4..]),
        url,
        interval: 5,
        expires_in: 600,
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ConnectSessionRequest {
    device_code: String,
}

pub(super) async fn handle_connect_session(
    State(api): State<Arc<Api>>,
    Json(request): Json<ConnectSessionRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, String)> {
    match api.auth.connect_session(&request.device_code) {
        ConnectPoll::Pending => Ok(accepted()),
        ConnectPoll::Session(session) => Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "session": session })),
        )),
        ConnectPoll::Denied => Err((StatusCode::FORBIDDEN, "Signing in was denied.".to_string())),
        ConnectPoll::Gone => Err((
            StatusCode::GONE,
            "The code expired. Start signing in again.".to_string(),
        )),
    }
}

#[derive(Deserialize)]
pub(super) struct DecideConnectRequest {
    code: String,
    approve: bool,
}

pub(super) async fn handle_account_connect(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<DecideConnectRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let SignedIn { account, .. } = signed_in(&api, &headers)?;
    let id = account.id.as_str();
    if !within_limit(&api.connect_tries, id, 10) {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "Too many tries. Try again in a minute.".to_string(),
        ));
    }
    let session = request
        .approve
        .then(|| api.auth.session(id, account.sessions));
    if !api.auth.decide_connect(&request.code, session) {
        return Err((
            StatusCode::NOT_FOUND,
            "There is no such code. Check it, or start signing in again in mobtop.".to_string(),
        ));
    }
    info!(
        "{id} {} a sign in on another device",
        if request.approve {
            "approved"
        } else {
            "denied"
        }
    );
    Ok(Json(serde_json::json!({})))
}

pub(super) enum ViewerSignInError {
    Failed(anyhow::Error),
    Unchecked(anyhow::Error),
}

async fn viewer_redirect(
    api: &Api,
    platform: Platform,
    page_id: &str,
    sign_in: Result<SignIn>,
) -> Redirect {
    let account = api.accounts.of_page(page_id);
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
        None => format!("{}/page/?streamer={page_id}", api.auth.frontend_url),
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
                "{} user {} ({}) signed in as a viewer of page {page_id} with subscription tier {tier:?}",
                platform.title(),
                user.user_id,
                user.login
            );
            Redirect::to(&format!(
                "{url}#viewer={}",
                api.auth.viewer_session(
                    platform,
                    user.user_id,
                    user.name,
                    page_id.to_string(),
                    tier
                )
            ))
        }
        Err(ViewerSignInError::Unchecked(error)) => {
            warn!("checking the Kick subscription of a viewer of page {page_id} failed: {error:#}");
            Redirect::to(&format!("{url}#viewerCheckFailed"))
        }
        Err(ViewerSignInError::Failed(error)) => {
            warn!(
                "signing in with {} as a viewer of page {page_id} failed: {error:#}",
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
    let signed_in = signed_in(&api, &headers)?;
    let identity = api.auth.link_of(&request.token).ok_or((
        StatusCode::BAD_REQUEST,
        "Linking took too long. Try again.".to_string(),
    ))?;
    let account = api
        .accounts
        .link(&signed_in.account.id, &identity)
        .map_err(account_error)?;
    api.remote_control
        .changes
        .notify(&account.page_id, Change::Page);
    info!(
        "{} linked {} user {} ({})",
        account.id,
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
    let SignedIn { account, .. } = signed_in(&api, &headers)?;
    let platform = platform_of(&platform)?;
    if account.identity(platform).is_some()
        && !account
            .identities
            .iter()
            .any(|identity| identity.platform != platform && api.auth.has(identity.platform))
    {
        return Err(account_error(AccountError::LastIdentity));
    }
    let account = api
        .accounts
        .unlink(&account.id, platform)
        .map_err(account_error)?;
    api.remote_control
        .changes
        .notify(&account.page_id, Change::Page);
    info!("{} unlinked {}", account.id, platform.title());
    Ok(account_response(&api, account))
}

#[cfg(test)]
#[path = "tests/sign_in.rs"]
mod tests;
