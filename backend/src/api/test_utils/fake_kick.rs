use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query};
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use serde_json::{Value, json};

use crate::auth::KickApp;

#[derive(Clone, Default)]
pub(crate) struct FakeKick {
    pub(crate) challenge: Arc<std::sync::Mutex<Option<String>>>,
    pub(crate) tokens: Arc<std::sync::Mutex<Vec<Value>>>,
}

pub(crate) async fn start_kick() -> (String, FakeKick) {
    let fake = FakeKick::default();
    let user_of = |token: &str| match token {
        "kick-a" => Some(json!({
            "user_id": 123,
            "name": "KickErik",
            "email": "erik@example.com",
            "profile_picture": "https://files.kick.com/erik.png",
        })),
        "kick-b" => Some(json!({
            "user_id": "456",
            "name": "",
            "profile_picture": "http://files.kick.com/other.png",
        })),
        "kick-sub" => Some(json!({"user_id": 777, "name": "Sub"})),
        "kick-lapsed" => Some(json!({"user_id": 778, "name": "Lapsed"})),
        "kick-broken" => Some(json!({"user_id": 779, "name": "Broken"})),
        "kick-mod" => Some(json!({"user_id": 9, "name": "Robert"})),
        _ => None,
    };
    let channel_of = |token: &str| match token {
        "kick-a" => Some((json!(123), "KickErik")),
        "kick-b" => Some((json!(456), "other-kick")),
        "kick-sub" => Some((json!(777), "subbed")),
        "kick-lapsed" => Some((json!(778), "lapsed")),
        "kick-broken" => Some((json!(779), "broken")),
        "kick-mod" => Some((json!(9), "robert")),
        _ => None,
    };
    let slug_of = |slug: &str| match slug {
        "bob" => Some((json!(9), "bob")),
        "robert" => Some((json!(9), "robert")),
        "kickerik" => Some((json!(123), "KickErik")),
        _ => None,
    };
    let bearer = |headers: &HeaderMap| {
        headers[AUTHORIZATION]
            .to_str()
            .unwrap()
            .strip_prefix("Bearer ")
            .unwrap()
            .to_string()
    };
    let channel_json = |(id, slug): (Value, &str)| {
        json!({
            "broadcaster_user_id": id,
            "slug": slug,
            "channel_description": "",
            "stream": {"is_live": false},
        })
    };
    let router = Router::new()
        .route(
            "/token",
            post({
                let challenge = fake.challenge.clone();
                let tokens = fake.tokens.clone();
                move |axum::Form(form): axum::Form<HashMap<String, String>>| async move {
                    tokens.lock().unwrap().push(json!(form));
                    if form["grant_type"] == "client_credentials" {
                        if form["client_id"] != "kid" || form["client_secret"] != "ksecret" {
                            return (
                                StatusCode::UNAUTHORIZED,
                                Json(json!({"error": "invalid_client"})),
                            );
                        }
                        return (
                            StatusCode::OK,
                            Json(json!({
                                "access_token": "app",
                                "token_type": "Bearer",
                                "expires_in": 3600,
                            })),
                        );
                    }
                    let expected = challenge.lock().unwrap().clone().unwrap();
                    let verifier = &form["code_verifier"];
                    if BASE64_URL_SAFE_NO_PAD.encode(ring::digest::digest(
                        &ring::digest::SHA256,
                        verifier.as_bytes(),
                    )) != expected
                        || !form["code"].starts_with("kick-")
                    {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({"error": "invalid_grant"})),
                        );
                    }
                    (
                        StatusCode::OK,
                        Json(json!({
                            "access_token": form["code"],
                            "token_type": "Bearer",
                            "expires_in": 7200,
                            "scope": "user:read channel:read",
                        })),
                    )
                }
            }),
        )
        .route(
            "/users",
            get(move |headers: HeaderMap| async move {
                match user_of(&bearer(&headers)) {
                    Some(user) => (
                        StatusCode::OK,
                        Json(json!({"data": [user], "message": "OK"})),
                    ),
                    None => (
                        StatusCode::UNAUTHORIZED,
                        Json(json!({"message": "Unauthorized"})),
                    ),
                }
            }),
        )
        .route(
            "/channels",
            get(
                move |headers: HeaderMap, Query(query): Query<HashMap<String, String>>| async move {
                    let token = bearer(&headers);
                    if let Some(slug) = query.get("slug") {
                        if token != "app" {
                            return StatusCode::UNAUTHORIZED.into_response();
                        }
                        if slug == "broken" {
                            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                        }
                        let data: Vec<Value> =
                            slug_of(slug).map(channel_json).into_iter().collect();
                        return Json(json!({"data": data, "message": "OK"})).into_response();
                    }
                    let Some(channel) = channel_of(&token) else {
                        return StatusCode::UNAUTHORIZED.into_response();
                    };
                    Json(json!({"data": [channel_json(channel)], "message": "OK"}))
                        .into_response()
                },
            ),
        )
        .route(
            "/api/v2/channels/{streamer}/users/{viewer}",
            get(
                |Path((streamer, viewer)): Path<(String, String)>| async move {
                    if streamer != "kickerik" {
                        return StatusCode::NOT_FOUND.into_response();
                    }
                    let badges = match viewer.as_str() {
                        "subbed" => json!([
                            {"type": "vip", "text": "VIP", "active": true},
                            {"type": "subscriber", "text": "Subscriber", "count": 3, "active": true},
                        ]),
                        "lapsed" => json!([
                            {"type": "subscriber", "text": "Subscriber", "count": 1, "active": false},
                        ]),
                        "other-kick" => json!([]),
                        "broken" => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                        _ => return StatusCode::NOT_FOUND.into_response(),
                    };
                    Json(json!({
                        "id": 1,
                        "username": viewer,
                        "slug": viewer,
                        "is_moderator": false,
                        "badges": badges,
                    }))
                    .into_response()
                },
            ),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await });
    (format!("http://{address}"), fake)
}

pub(crate) fn kick_app(kick: &str) -> KickApp {
    KickApp {
        client_id: "kid".to_string(),
        client_secret: "ksecret".to_string(),
        authorize_url: format!("{kick}/authorize"),
        token_url: format!("{kick}/token"),
        users_url: format!("{kick}/users"),
        channels_url: format!("{kick}/channels"),
        channel_users_url: format!("{kick}/api/v2/channels"),
    }
}
