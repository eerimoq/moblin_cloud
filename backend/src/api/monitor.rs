use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};

use super::*;

#[derive(Deserialize)]
pub(super) struct MonitorLayoutRequest {
    layout: Value,
}

pub(super) async fn handle_monitor_layout(
    State(api): State<Arc<Api>>,
    Path(page): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    let SignedIn { id, .. } = assistant(&api, &headers, &page)?;
    let layout = api
        .accounts
        .monitor_layout(&id, &page)
        .and_then(|layout| serde_json::from_str::<Value>(&layout).ok())
        .unwrap_or(Value::Null);
    Ok(Json(json!({ "layout": layout })))
}

pub(super) async fn handle_monitor_layout_save(
    State(api): State<Arc<Api>>,
    Path(page): Path<String>,
    headers: HeaderMap,
    Json(request): Json<MonitorLayoutRequest>,
) -> Result<StatusCode, Failure> {
    let SignedIn { id, .. } = assistant(&api, &headers, &page)?;
    if !request.layout.is_object() {
        return Err(failure(
            "invalid-layout",
            StatusCode::UNPROCESSABLE_ENTITY,
            "the layout must be an object",
        ));
    }
    api.accounts
        .set_monitor_layout(&id, &page, &request.layout.to_string())
        .map_err(account_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::{Value, json};

    use crate::accounts::Platform;
    use crate::api::test_utils::*;

    async fn layout(address: &str, session: &str, page: &str) -> Value {
        reqwest::Client::new()
            .get(format!("http://{address}/account/pages/{page}/monitor"))
            .bearer_auth(session)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()["layout"]
            .clone()
    }

    async fn save(address: &str, session: Option<&str>, page: &str, body: &Value) -> StatusCode {
        let mut request = reqwest::Client::new()
            .post(format!("http://{address}/account/pages/{page}/monitor"))
            .json(body);
        if let Some(session) = session {
            request = request.bearer_auth(session);
        }
        request.send().await.unwrap().status()
    }

    #[tokio::test]
    async fn layouts_are_saved_per_page() {
        let (address, api) = start_api(None).await;
        let (session, _, page) = streamer(&api);
        let other = create_account(&api, "2", "Other", "other").page.id;
        api.accounts
            .add_assistant(
                Platform::Twitch,
                "2",
                &twitch_assistant("1", "erik", "Erik"),
            )
            .unwrap();
        assert_eq!(layout(&address, &session, &page).await, Value::Null);
        let first = json!({"panel": "activity"});
        assert_eq!(
            save(&address, Some(&session), &page, &json!({ "layout": first })).await,
            StatusCode::NO_CONTENT
        );
        assert_eq!(layout(&address, &session, &page).await, first);
        assert_eq!(layout(&address, &session, &other).await, first);
        let second = json!({"split": "row", "sizes": [0.5, 0.5], "children": [
            {"panel": "phone"}, {"panel": "graphs"}
        ]});
        assert_eq!(
            save(
                &address,
                Some(&session),
                &other,
                &json!({ "layout": second })
            )
            .await,
            StatusCode::NO_CONTENT
        );
        assert_eq!(layout(&address, &session, &other).await, second);
        assert_eq!(layout(&address, &session, &page).await, first);
        assert_eq!(
            layout(&address, &api.auth.session("2", 0), &other).await,
            Value::Null
        );
    }

    #[tokio::test]
    async fn invalid_layouts_are_refused() {
        let (address, api) = start_api(None).await;
        let (session, _, page) = streamer(&api);
        let stranger = create_account(&api, "2", "Other", "other").page.id;
        assert_eq!(
            save(&address, Some(&session), &page, &json!({"layout": [1, 2]})).await,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(
            save(
                &address,
                Some(&session),
                &page,
                &json!({"layout": {"text": "x".repeat(20_000)}})
            )
            .await,
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert_eq!(
            save(&address, None, &page, &json!({"layout": {}})).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            save(&address, Some(&session), &stranger, &json!({"layout": {}})).await,
            StatusCode::FORBIDDEN
        );
        let response = reqwest::Client::new()
            .get(format!("http://{address}/account/pages/{stranger}/monitor"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(layout(&address, &session, &page).await, Value::Null);
    }

    #[tokio::test]
    async fn layouts_are_deleted_with_the_account() {
        let (address, api) = start_api(None).await;
        let (session, _, page) = streamer(&api);
        create_account(&api, "2", "Other", "other");
        api.accounts
            .add_assistant(
                Platform::Twitch,
                "1",
                &twitch_assistant("2", "other", "Other"),
            )
            .unwrap();
        let assistant = api.auth.session("2", 0);
        let body = json!({"layout": {"panel": "status"}});
        save(&address, Some(&session), &page, &body).await;
        save(&address, Some(&assistant), &page, &body).await;
        let response = reqwest::Client::new()
            .post(format!("http://{address}/account/delete"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(api.accounts.monitor_layout("1", &page).is_none());
        assert!(api.accounts.monitor_layout("2", &page).is_none());
    }
}
