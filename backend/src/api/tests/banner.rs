use axum::http::StatusCode;
use serde_json::Value;

use crate::api::test_utils::*;
use crate::images;

#[tokio::test]
async fn banner_images() {
    let (address, api) = start_api(None).await;
    let client = reqwest::Client::new();
    let upload = |session: String, page_id: String, body: Vec<u8>| {
        client
            .post(format!("http://{address}/account/pages/{page_id}/image"))
            .bearer_auth(session)
            .header("content-type", "image/png")
            .body(body)
            .send()
    };
    let (session, _, page_id) = streamer(&api);
    let response = upload(session, page_id.clone(), images::tests::png(1200, 400))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    create_account(&api, "premium", "Premium", "premium");
    api.accounts.set_premium("premium", true).unwrap();
    let premium = api.auth.session("premium", 0);
    let account: Value = client
        .get(format!("http://{address}/account"))
        .bearer_auth(&premium)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(account["premium"], true);
    assert_eq!(account["settings"]["image"], Value::Null);
    let premium_page_id = account["settings"]["id"].as_str().unwrap().to_string();
    let image_url = format!("http://{address}/pages/{premium_page_id}/image");
    assert_eq!(
        client.get(&image_url).send().await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    let response = upload(
        premium.clone(),
        premium_page_id.clone(),
        b"not an image".to_vec(),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    match upload(
        premium.clone(),
        premium_page_id.clone(),
        vec![0; 3 * 1024 * 1024],
    )
    .await
    {
        Ok(response) => assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE),
        Err(error) => assert!(error.is_request(), "{error:?}"),
    }
    let settings: Value = upload(
        premium.clone(),
        premium_page_id.clone(),
        images::tests::png(1200, 400),
    )
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    let first = settings["image"].as_str().unwrap().to_string();
    let profile: Value = client
        .get(format!("http://{address}/pages/{premium_page_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(profile["image"], first);
    let response = client.get(&image_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/jpeg");
    assert!(!response.bytes().await.unwrap().is_empty());
    let settings: Value = upload(
        premium.clone(),
        premium_page_id.clone(),
        images::tests::png(2400, 800),
    )
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_ne!(settings["image"], first);
    let other: Value = client
        .get(format!("http://{address}/pages/{page_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(other["image"], Value::Null);
    let settings: Value = client
        .post(format!(
            "http://{address}/account/pages/{premium_page_id}/image/remove"
        ))
        .bearer_auth(&premium)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(settings["image"], Value::Null);
    assert_eq!(
        client.get(&image_url).send().await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(counter(&api, "imageUploads"), 2);
    assert_eq!(counter(&api, "imageUploadsFailed:not-premium"), 1);
    assert_eq!(counter(&api, "imageUploadsFailed:invalid-image"), 1);
}
