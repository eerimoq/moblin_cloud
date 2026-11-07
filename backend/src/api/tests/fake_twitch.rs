use std::collections::HashMap;

use axum::extract::Query;
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;

use super::serve_local;
use crate::auth::OAuthApp;

pub(crate) async fn start_twitch() -> String {
    let router = Router::new()
        .route(
            "/token",
            post(|body: String| async move {
                if body.contains("grant_type=client_credentials") {
                    assert!(body.contains("client_id=id&client_secret=secret"));
                    return Json(json!({"access_token": "app", "token_type": "bearer"}));
                }
                assert!(body.contains("code=c"));
                Json(json!({"access_token": "u", "token_type": "bearer"}))
            }),
        )
        .route(
            "/users",
            get(
                |headers: HeaderMap, Query(query): Query<HashMap<String, String>>| async move {
                    assert_eq!(headers["client-id"], "id");
                    let Some(login) = query.get("login") else {
                        assert_eq!(headers[AUTHORIZATION], "Bearer u");
                        return Json(json!({"data": [{
                            "id": "42",
                            "login": "erik",
                            "display_name": "Erik",
                            "profile_image_url": "https://static-cdn.jtvnw.net/erik.png",
                        }]}))
                        .into_response();
                    };
                    assert_eq!(headers[AUTHORIZATION], "Bearer app");
                    let user = match login.as_str() {
                        "nobody" => return Json(json!({"data": []})).into_response(),
                        "broken" => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                        "bob" => json!({"id": "9", "login": "Bob", "display_name": "Bob"}),
                        "erik" => json!({"id": "1", "login": "erik", "display_name": "Erik"}),
                        "mod21" => {
                            json!({"id": "21", "login": "mod21", "display_name": "Mod 21"})
                        }
                        login => json!({"id": login, "login": login, "display_name": ""}),
                    };
                    Json(json!({"data": [user]})).into_response()
                },
            ),
        )
        .route(
            "/subscriptions",
            get(
                |headers: HeaderMap, Query(query): Query<HashMap<String, String>>| async move {
                    assert_eq!(headers[AUTHORIZATION], "Bearer u");
                    assert_eq!(headers["client-id"], "id");
                    assert_eq!(query["user_id"], "42");
                    match query["broadcaster_id"].as_str() {
                        "1" => Json(json!({"data": [{"tier": "2000"}]})).into_response(),
                        "3" => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                        _ => StatusCode::NOT_FOUND.into_response(),
                    }
                },
            ),
        );
    let address = serve_local(router).await;
    format!("http://{address}")
}

pub(crate) fn twitch_app(twitch: &str) -> OAuthApp {
    OAuthApp {
        client_id: "id".to_string(),
        client_secret: "secret".to_string(),
        authorize_url: format!("{twitch}/authorize"),
        token_url: format!("{twitch}/token"),
        users_url: format!("{twitch}/users"),
        subscriptions_url: format!("{twitch}/subscriptions"),
    }
}
