use std::time::Duration;

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::accounts::Platform;
use crate::api::Api;
use crate::api::test_utils::*;
use crate::payments::Stage;
use crate::youtube::{LookupError, YouTube};

async fn set_up(
    address: &str,
    api: &Api,
    media: Value,
) -> (reqwest::Client, String, String, String) {
    let client = reqwest::Client::new();
    let (session, moblin_cloud_key, page_id) = streamer(api);
    connect_paypal(api);
    let settings: Value = client
        .post(format!("http://{address}/account/pages/{page_id}/settings"))
        .bearer_auth(&session)
        .json(&json!({"media": media}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(settings["media"]["enabled"], true);
    (client, session, moblin_cloud_key, page_id)
}

async fn take_videos(socket: &mut Socket, state: Value) {
    send_event(socket, json!({"mediaShare": {"state": state}})).await;
    ponged(socket).await;
}

async fn paid(client: &reqwest::Client, orders_url: &str, request: Value) {
    let order: Value = client
        .post(orders_url)
        .json(&request)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(order, json!({"orderId": "ORDER1"}));
    let response = client
        .post(format!("{orders_url}/ORDER1/capture"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn videos_wait_for_approval_before_moblin_plays_them() {
    let (address, api, fake) = start_media().await;
    let (client, session, moblin_cloud_key, page_id) =
        set_up(&address, &api, json!({"price": 300, "seconds": 60})).await;
    let media_url = format!("http://{address}/pages/{page_id}/media");
    let orders_url = format!("{media_url}/orders");
    let public = || async {
        client
            .get(&media_url)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()
    };
    let order = |request: Value| client.post(&orders_url).json(&request).send();
    let link = "https://youtu.be/dQw4w9WgXcQ?t=43";
    let answer_of = public().await;
    assert_eq!(answer_of["media"]["price"], 300);
    assert_eq!(answer_of["seconds"], 60);
    assert_eq!(answer_of["moderated"], true);
    assert_eq!(answer_of["available"], false);
    let response = order(json!({"url": link, "seconds": 30})).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let mut socket = connect(&address, &moblin_cloud_key).await;
    let response = order(json!({"url": link, "seconds": 30})).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    take_videos(&mut socket, json!({"idle": {}})).await;
    assert_eq!(public().await["available"], true);
    for (request, status) in [
        (
            json!({"url": "https://vimeo.com/1", "seconds": 30}),
            StatusCode::BAD_REQUEST,
        ),
        (json!({"url": link, "seconds": 61}), StatusCode::BAD_REQUEST),
        (json!({"url": link, "seconds": 0}), StatusCode::BAD_REQUEST),
        (
            json!({"url": link, "seconds": 30, "price": 200}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"url": link, "seconds": 30, "start": 90000}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"url": "private0000", "seconds": 30}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"url": "down0000000", "seconds": 30}),
            StatusCode::BAD_GATEWAY,
        ),
    ] {
        let response = order(request.clone()).await.unwrap();
        assert_eq!(response.status(), status, "{request}");
    }
    assert_eq!(*fake.order.lock().unwrap(), None);
    paid(
        &client,
        &orders_url,
        json!({"url": link, "seconds": 30, "price": 300, "user": "Viewer", "message": "Look"}),
    )
    .await;
    let sent = fake.order.lock().unwrap().clone().unwrap();
    assert_eq!(
        sent["purchase_units"][0]["description"],
        "Video request by Viewer: Look"
    );
    assert!(api.deliveries.pending().is_empty());
    let queue = || async {
        client
            .get(format!("http://{address}/account/pages/{page_id}/media"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()
    };
    let queued = queue().await;
    assert_eq!(queued["device"], "A");
    assert_eq!(queued["playing"], Value::Null);
    assert_eq!(queued["held"].as_array().unwrap().len(), 1);
    let held = &queued["held"][0];
    assert_eq!(held["id"], "ORDER1");
    assert_eq!(held["user"], "Viewer");
    assert_eq!(held["message"], "Look");
    assert_eq!(held["video"], "dQw4w9WgXcQ");
    assert_eq!(held["start"], 43);
    assert_eq!(held["seconds"], 30);
    assert_eq!(held["title"], "Video dQw4w9WgXcQ");
    assert_eq!(held["amount"], 300);
    assert_eq!(held["currency"], "USD");
    let recent: Value = client
        .get(format!("http://{address}/account/pages/{page_id}/recent"))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(recent["media"]["items"][0]["status"], "held");
    assert_eq!(recent["media"]["items"][0]["video"], "dQw4w9WgXcQ");
    assert_eq!(recent["media"]["items"][0]["title"], "Video dQw4w9WgXcQ");
    let decide = |request: &str, decision: &str| {
        client
            .post(format!(
                "http://{address}/account/pages/{page_id}/media/{request}/{decision}"
            ))
            .bearer_auth(&session)
            .send()
    };
    let response = decide("ORDER2", "approve").await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = decide("ORDER1", "approve").await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let request = receive(&mut socket).await;
    assert_eq!(
        request["request"]["data"],
        json!({"tip": {
            "name": "Viewer",
            "message": "Look",
            "kind": {"mediaShare": {"source": {"youTube": {"videoId": "dQw4w9WgXcQ", "start": 43}}, "duration": 30}},
            "requestId": "ORDER1",
        }})
    );
    answer(&mut socket, &request, Value::Null).await;
    wait_until_added(&api).await;
    let response = decide("ORDER1", "approve").await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = decide("ORDER1", "reject").await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(fake.refunds.lock().unwrap().is_empty());
    take_videos(&mut socket, json!({"playing": {"requestId": "ORDER1"}})).await;
    let queued = queue().await;
    assert_eq!(queued["held"], json!([]));
    assert_eq!(queued["paused"], false);
    assert_eq!(queued["playing"]["id"], "ORDER1");
    assert_eq!(queued["playing"]["title"], "Video dQw4w9WgXcQ");
    assert_eq!(queued["queued"], json!([]));
    assert_eq!(counter(&api, "mediaPayments"), 1);
    assert_eq!(counter(&api, "mediaApprovals"), 1);
    assert_eq!(counter(&api, "mediaDeliveries"), 1);
    take_videos(&mut socket, json!({"unavailable": {}})).await;
    assert_eq!(public().await["available"], false);
    assert_eq!(queue().await["device"], Value::Null);
}

#[tokio::test]
async fn rejected_videos_are_refunded() {
    let (address, api, fake) = start_media().await;
    let (client, session, moblin_cloud_key, page_id) =
        set_up(&address, &api, json!({"price": 300, "seconds": 60})).await;
    let orders_url = format!("http://{address}/pages/{page_id}/media/orders");
    let mut socket = connect(&address, &moblin_cloud_key).await;
    take_videos(&mut socket, json!({"idle": {}})).await;
    paid(
        &client,
        &orders_url,
        json!({"url": "dQw4w9WgXcQ", "seconds": 60}),
    )
    .await;
    let reject = || {
        client
            .post(format!(
                "http://{address}/account/pages/{page_id}/media/ORDER1/reject"
            ))
            .bearer_auth(&session)
            .send()
    };
    *fake.refuse_refund.lock().unwrap() = true;
    let response = reject().await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.text().await.unwrap(),
        "PayPal refused to refund the tip, so the video can only be approved. Refund it at PayPal to reject it."
    );
    assert_eq!(api.deliveries.held(&page_id).len(), 1);
    assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Captured);
    *fake.refuse_refund.lock().unwrap() = false;
    let response = reject().await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(fake.refunds.lock().unwrap().len(), 1);
    assert!(api.deliveries.held(&page_id).is_empty());
    assert!(api.deliveries.pending().is_empty());
    assert_eq!(api.orders.get("ORDER1"), None);
    let response = reject().await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(fake.refunds.lock().unwrap().len(), 1);
    assert_eq!(counter(&api, "mediaRefunds"), 1);
    assert_eq!(counter(&api, "mediaRejections"), 1);
    assert_eq!(counter(&api, "mediaRejectionsFailed:refund-refused"), 1);
}

#[tokio::test]
async fn unmoderated_videos_are_sent_at_once_and_captures_need_moblin() {
    let (address, api, _) = start_media().await;
    let (client, _, moblin_cloud_key, page_id) = set_up(
        &address,
        &api,
        json!({"price": 300, "seconds": 60, "moderated": false}),
    )
    .await;
    let orders_url = format!("http://{address}/pages/{page_id}/media/orders");
    let mut socket = connect(&address, &moblin_cloud_key).await;
    take_videos(&mut socket, json!({"idle": {}})).await;
    let order: Value = client
        .post(&orders_url)
        .json(&json!({"url": "dQw4w9WgXcQ", "seconds": 20, "start": 5}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(order, json!({"orderId": "ORDER1"}));
    take_videos(&mut socket, json!({"unavailable": {}})).await;
    let capture_url = format!("{orders_url}/ORDER1/capture");
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response.text().await.unwrap(),
        "The streamer stopped taking videos, so your tip was not taken."
    );
    assert_eq!(api.orders.get("ORDER1").unwrap().stage, Stage::Created);
    take_videos(&mut socket, json!({"idle": {}})).await;
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let request = receive(&mut socket).await;
    assert_eq!(
        request["request"]["data"]["tip"]["kind"],
        json!({"mediaShare": {"source": {"youTube": {"videoId": "dQw4w9WgXcQ", "start": 5}}, "duration": 20}})
    );
    assert!(api.deliveries.held(&page_id).is_empty());
}

#[tokio::test]
async fn streamers_test_their_videos() {
    let (address, api, _) = start_media().await;
    let (client, session, moblin_cloud_key, page_id) =
        set_up(&address, &api, json!({"price": 300, "seconds": 10})).await;
    let test = |request: Value| {
        client
            .post(format!(
                "http://{address}/account/pages/{page_id}/test/media"
            ))
            .bearer_auth(&session)
            .json(&request)
            .send()
    };
    let response = test(json!({})).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(response.text().await.unwrap(), "Connect Moblin first.");
    let mut socket = connect(&address, &moblin_cloud_key).await;
    let response = test(json!({})).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    take_videos(&mut socket, json!({"idle": {}})).await;
    let response = test(json!({"url": "https://vimeo.com/1"})).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = test(json!({"url": "https://youtu.be/aaaaaaaaaaa?t=9"}))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let request = receive(&mut socket).await;
    assert_eq!(request["request"]["data"]["tip"]["name"], "Erik");
    assert_eq!(
        request["request"]["data"]["tip"]["kind"],
        json!({"mediaShare": {"source": {"youTube": {"videoId": "aaaaaaaaaaa", "start": 9}}, "duration": 10}})
    );
    answer(&mut socket, &request, Value::Null).await;
    let response = test(json!({})).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let request = receive(&mut socket).await;
    assert_eq!(
        request["request"]["data"]["tip"]["kind"]["mediaShare"]["source"]["youTube"]["videoId"],
        "dQw4w9WgXcQ"
    );
    answer(&mut socket, &request, Value::Null).await;
    for times in [
        json!({"seconds": 0}),
        json!({"seconds": 601}),
        json!({"start": 86401}),
    ] {
        let response = test(times).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.text().await.unwrap(),
            "Enter a start time in the first 24 hours and a length from 1 to 600 seconds."
        );
    }
    let response = test(json!({
        "url": "https://youtu.be/aaaaaaaaaaa?t=9",
        "start": 70,
        "seconds": 45
    }))
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let request = receive(&mut socket).await;
    assert_eq!(
        request["request"]["data"]["tip"]["kind"],
        json!({"mediaShare": {"source": {"youTube": {"videoId": "aaaaaaaaaaa", "start": 70}}, "duration": 45}})
    );
    socket.close(None).await.unwrap();
    for _ in 0..100 {
        if !api.remote_control.is_connected(&page_id) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(api.remote_control.media_device(&page_id), None);
}

#[tokio::test]
async fn twitch_clips_are_played_whole() {
    let (address, api) = start_clips().await;
    let client = reqwest::Client::new();
    let (session, moblin_cloud_key, page_id) = streamer(&api);
    let mut socket = connect(&address, &moblin_cloud_key).await;
    take_videos(&mut socket, json!({"idle": {}})).await;
    let test = |url: &str| {
        client
            .post(format!(
                "http://{address}/account/pages/{page_id}/test/media"
            ))
            .bearer_auth(&session)
            .json(&json!({"url": url}))
            .send()
    };
    for (url, status, text) in [
        (
            "https://clips.twitch.tv/Missing",
            StatusCode::BAD_REQUEST,
            "That clip does not exist. Pick another one.",
        ),
        (
            "https://clips.twitch.tv/Broken",
            StatusCode::BAD_GATEWAY,
            "Twitch did not answer. Try again.",
        ),
        (
            "https://kick.com/kickerik/clips/clip_01M86Z1Z62360AM4D5KT55H4GS",
            StatusCode::BAD_REQUEST,
            "That is not a link to a YouTube video or a Twitch clip.",
        ),
        (
            "https://vimeo.com/1",
            StatusCode::BAD_REQUEST,
            "That is not a link to a YouTube video or a Twitch clip.",
        ),
    ] {
        let response = test(url).await.unwrap();
        assert_eq!(response.status(), status, "{url}");
        assert_eq!(response.text().await.unwrap(), text, "{url}");
    }
    let response = test("https://www.twitch.tv/erik/clip/Funny-Clip_1")
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let request = receive(&mut socket).await;
    assert_eq!(
        request["request"]["data"]["tip"]["kind"],
        json!({"mediaShare": {"source": {"twitchClip": {"slug": "Funny-Clip_1"}}, "duration": 29}})
    );
    answer(&mut socket, &request, Value::Null).await;
    wait_until_added(&api).await;
    take_videos(
        &mut socket,
        json!({"playing": {"requestId": request["request"]["data"]["tip"]["requestId"]}}),
    )
    .await;
    let queue: Value = client
        .get(format!("http://{address}/account/pages/{page_id}/media"))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let playing = &queue["playing"];
    assert_eq!(playing["platform"], "twitch");
    assert_eq!(playing["title"], "Clip Funny-Clip_1");
    assert_eq!(playing["seconds"], 29);
    assert_eq!(playing["link"], "https://clips.twitch.tv/Funny-Clip_1");
    assert_eq!(
        playing["thumbnail"],
        "https://static-cdn.jtvnw.net/clip.jpg"
    );
}

#[tokio::test]
async fn clips_need_their_platform_and_fit_the_longest_video() {
    let (address, api, _) = start_media().await;
    let (client, _, moblin_cloud_key, page_id) =
        set_up(&address, &api, json!({"price": 300, "seconds": 30})).await;
    let mut socket = connect(&address, &moblin_cloud_key).await;
    take_videos(&mut socket, json!({"idle": {}})).await;
    let orders_url = format!("http://{address}/pages/{page_id}/media/orders");
    let response = client
        .post(&orders_url)
        .json(&json!({"url": "https://clips.twitch.tv/Funny"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response.text().await.unwrap(),
        "Twitch clips can't be played here. Pick a YouTube video."
    );
    drop(socket);
    let (address, api) = start_clips().await;
    let (session, moblin_cloud_key, page_id) = streamer(&api);
    let response = client
        .post(format!("http://{address}/account/pages/{page_id}/settings"))
        .bearer_auth(&session)
        .json(&json!({
            "media": {"price": 300, "seconds": 30, "enabled": false},
            "subMedia": {"tier1": 1, "tier2": 1, "tier3": 1},
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let mut socket = connect(&address, &moblin_cloud_key).await;
    take_videos(&mut socket, json!({"idle": {}})).await;
    let viewer = api.auth.viewer_session(
        Platform::Twitch,
        "42".to_string(),
        "Fan".to_string(),
        page_id.clone(),
        Some(1),
    );
    let play = |url: &str| {
        client
            .post(format!("http://{address}/pages/{page_id}/media/subscriber"))
            .bearer_auth(&viewer)
            .json(&json!({"url": url}))
            .send()
    };
    let response = play("https://clips.twitch.tv/Long").await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response.text().await.unwrap(),
        "That clip is 60 seconds, and the streamer plays at most 30. Pick a shorter one."
    );
    let response = play("https://clips.twitch.tv/Short").await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let held = api.deliveries.held(&page_id);
    assert_eq!(held.len(), 1);
    assert_eq!(
        serde_json::to_value(&held[0].item).unwrap(),
        json!({
            "user": "Fan",
            "message": "",
            "video": "Short",
            "start": 0,
            "seconds": 29,
            "title": "Clip Short",
            "platform": "twitch",
            "link": "https://clips.twitch.tv/Short",
            "thumbnail": "https://static-cdn.jtvnw.net/clip.jpg",
        })
    );
}

#[tokio::test]
async fn titles_are_looked_up() {
    let youtube = start_youtube().await;
    assert_eq!(
        youtube.title("dQw4w9WgXcQ").await,
        Ok("Video dQw4w9WgXcQ".to_string())
    );
    assert_eq!(
        youtube.title("private0000").await,
        Err(LookupError::Unavailable)
    );
    assert_eq!(
        youtube.title("down0000000").await,
        Err(LookupError::Unknown)
    );
    let gone = YouTube::new(reqwest::Client::new(), "http://127.0.0.1:9".to_string());
    assert_eq!(gone.title("dQw4w9WgXcQ").await, Err(LookupError::Unknown));
}

async fn outcome_of(
    client: &reqwest::Client,
    address: &str,
    session: &str,
    page_id: &str,
    expected: Value,
) -> Value {
    let mut item = Value::Null;
    for _ in 0..200 {
        let recent: Value = client
            .get(format!("http://{address}/account/pages/{page_id}/recent"))
            .bearer_auth(session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        item = recent["media"]["items"][0].clone();
        if item["status"] == "delivered" && item["outcome"] == expected {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    item
}

#[tokio::test]
async fn how_a_video_ended_is_kept_until_it_is_sent_again() {
    let (address, api, _) = start_media().await;
    let (client, session, moblin_cloud_key, page_id) = set_up(
        &address,
        &api,
        json!({"price": 300, "seconds": 60, "moderated": false}),
    )
    .await;
    let orders_url = format!("http://{address}/pages/{page_id}/media/orders");
    let mut socket = connect(&address, &moblin_cloud_key).await;
    take_videos(&mut socket, json!({"idle": {}})).await;
    paid(
        &client,
        &orders_url,
        json!({"url": "dQw4w9WgXcQ", "seconds": 20}),
    )
    .await;
    let request = receive(&mut socket).await;
    assert_eq!(request["request"]["data"]["tip"]["requestId"], "ORDER1");
    take_videos(&mut socket, json!({"playing": {"requestId": "ORDER1"}})).await;
    answer(&mut socket, &request, Value::Null).await;
    let item = outcome_of(&client, &address, &session, &page_id, Value::Null).await;
    assert_eq!(item["status"], "delivered");
    assert_eq!(item["outcome"], Value::Null);
    let failed =
        json!({"idle": {"lastEnded": {"requestId": "ORDER1", "reason": {"failedToStart": {}}}}});
    take_videos(&mut socket, failed.clone()).await;
    let item = outcome_of(
        &client,
        &address,
        &session,
        &page_id,
        json!("failedToStart"),
    )
    .await;
    assert_eq!(item["outcome"], "failedToStart");
    let response = client
        .post(format!(
            "http://{address}/account/pages/{page_id}/recent/ORDER1/resend"
        ))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let request = receive(&mut socket).await;
    assert_eq!(request["request"]["data"]["tip"]["requestId"], "ORDER1-1");
    take_videos(&mut socket, json!({"playing": {"requestId": "ORDER1-1"}})).await;
    answer(&mut socket, &request, Value::Null).await;
    take_videos(&mut socket, failed).await;
    let item = outcome_of(&client, &address, &session, &page_id, Value::Null).await;
    assert_eq!(item["outcome"], Value::Null);
    take_videos(
        &mut socket,
        json!({"idle": {"lastEnded": {"requestId": "ORDER1-1", "reason": {"finished": {}}}}}),
    )
    .await;
    let item = outcome_of(&client, &address, &session, &page_id, json!("finished")).await;
    assert_eq!(item["outcome"], "finished");
}
