use std::collections::HashMap;

use axum::extract::Query;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{Value, json};

use super::serve_local;
use crate::youtube::YouTube;

pub(crate) async fn start_youtube() -> YouTube {
    let router = Router::new().route(
        "/oembed",
        get(|Query(query): Query<HashMap<String, String>>| async move {
            assert_eq!(query["format"], "json");
            let video = query["url"]
                .strip_prefix("https://www.youtube.com/watch?v=")
                .unwrap()
                .to_string();
            match video.as_str() {
                "private0000" => (StatusCode::UNAUTHORIZED, Json(Value::Null)),
                "down0000000" => (StatusCode::INTERNAL_SERVER_ERROR, Json(Value::Null)),
                _ => (
                    StatusCode::OK,
                    Json(json!({"title": format!(" Video {video} ")})),
                ),
            }
        }),
    );
    let address = serve_local(router).await;
    YouTube::new(reqwest::Client::new(), format!("http://{address}"))
}
