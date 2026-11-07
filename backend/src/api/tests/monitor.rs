use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::accounts::Platform;
use crate::api::test_utils::*;

async fn layout(address: &str, session: &str, page_id: &str) -> Value {
    reqwest::Client::new()
        .get(format!("http://{address}/account/pages/{page_id}/monitor"))
        .bearer_auth(session)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["layout"]
        .clone()
}

async fn save(address: &str, session: Option<&str>, page_id: &str, body: &Value) -> StatusCode {
    let mut request = reqwest::Client::new()
        .post(format!("http://{address}/account/pages/{page_id}/monitor"))
        .json(body);
    if let Some(session) = session {
        request = request.bearer_auth(session);
    }
    request.send().await.unwrap().status()
}

#[tokio::test]
async fn layouts_are_saved_per_page() {
    let (address, api) = start_api(None).await;
    let (session, _, page_id) = streamer(&api);
    let other = create_account(&api, "2", "Other", "other").page_id;
    api.accounts
        .add_assistant(
            Platform::Twitch,
            "2",
            &twitch_assistant("1", "erik", "Erik"),
        )
        .unwrap();
    assert_eq!(layout(&address, &session, &page_id).await, Value::Null);
    let first = json!({"panel": "activity"});
    assert_eq!(
        save(
            &address,
            Some(&session),
            &page_id,
            &json!({ "layout": first })
        )
        .await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(layout(&address, &session, &page_id).await, first);
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
    assert_eq!(layout(&address, &session, &page_id).await, first);
    assert_eq!(
        layout(&address, &api.auth.session("2", 0), &other).await,
        Value::Null
    );
}

#[tokio::test]
async fn invalid_layouts_are_refused() {
    let (address, api) = start_api(None).await;
    let (session, _, page_id) = streamer(&api);
    let stranger = create_account(&api, "2", "Other", "other").page_id;
    assert_eq!(
        save(
            &address,
            Some(&session),
            &page_id,
            &json!({"layout": [1, 2]})
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        save(
            &address,
            Some(&session),
            &page_id,
            &json!({"layout": {"text": "x".repeat(20_000)}})
        )
        .await,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(
        save(&address, None, &page_id, &json!({"layout": {}})).await,
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
    assert_eq!(layout(&address, &session, &page_id).await, Value::Null);
}

#[tokio::test]
async fn layouts_are_deleted_with_the_account() {
    let (address, api) = start_api(None).await;
    let (session, _, page_id) = streamer(&api);
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
    save(&address, Some(&session), &page_id, &body).await;
    save(&address, Some(&assistant), &page_id, &body).await;
    let response = reqwest::Client::new()
        .post(format!("http://{address}/account/delete"))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(api.accounts.monitor_layout("1", &page_id).is_none());
    assert!(api.accounts.monitor_layout("2", &page_id).is_none());
}
