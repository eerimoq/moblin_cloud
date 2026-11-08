use axum::http::StatusCode;
use futures_util::SinkExt;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

use crate::accounts::Platform;
use crate::api::test_utils::*;
use crate::deliveries;
use crate::payments::{Item, Order, Payee};

#[tokio::test]
async fn recent_payments_are_listed_to_the_streamer() {
    let (address, api, _) = start_paid_fake().await;
    let (session, capture_url) = paypal_tip_order_in(&address, &api, "USD").await;
    let client = reqwest::Client::new();
    let page_id = api.accounts.get("1").unwrap().page_id;
    let recent = || async {
        client
            .get(format!(
                "http://{address}/account/pages/{page_id}/recent?kind=tip"
            ))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()
    };
    assert_eq!(recent().await, json!({"items": [], "more": false}));
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let listed = recent().await;
    assert_eq!(listed["more"], false);
    assert_eq!(listed["items"].as_array().unwrap().len(), 1);
    let tip = &listed["items"][0];
    assert_eq!(tip["id"], "ORDER1");
    assert_eq!(tip["user"], "Viewer");
    assert_eq!(tip["amount"], 700);
    assert_eq!(tip["currency"], "USD");
    assert_eq!(tip["provider"], "paypal");
    assert_eq!(tip["status"], "waiting");
    assert!(tip["at"].as_u64().unwrap() > 0);
    assert!(tip.get("message").is_none());
    create_account(&api, "21", "Mod 21", "mod21");
    api.accounts
        .add_assistant(
            Platform::Twitch,
            "1",
            &twitch_assistant("21", "mod21", "Mod 21"),
        )
        .unwrap();
    let assisted: Value = client
        .get(format!(
            "http://{address}/account/pages/{page_id}/recent?kind=tip"
        ))
        .bearer_auth(api.auth.session("21", 0))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(assisted["items"][0]["id"], "ORDER1");
    assert!(assisted["items"][0].get("provider").is_none());
    assert_eq!(recent().await["items"][0]["devices"], json!([]));
    let moblin_cloud_key = api.accounts.get("1").unwrap().moblin_cloud_key.clone();
    let mut socket = connect(&address, &moblin_cloud_key).await;
    let request = receive(&mut socket).await;
    assert_eq!(request["request"]["data"]["tip"]["requestId"], "ORDER1");
    answer(&mut socket, &request, Value::Null).await;
    wait_until_added(&api).await;
    let tip = &recent().await["items"][0];
    assert_eq!(tip["status"], "delivered");
    assert_eq!(
        tip["devices"],
        json!([{"name": "Moblin 1", "received": true}])
    );
    let response = client
        .get(format!(
            "http://{address}/account/pages/{page_id}/recent?kind=tip"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn recent_payments_can_be_sent_again() {
    let (address, api, _) = start_paid_fake().await;
    let (session, capture_url) = paypal_tip_order_in(&address, &api, "USD").await;
    let client = reqwest::Client::new();
    let page_id = api.accounts.get("1").unwrap().page_id;
    let status = || async {
        client
            .get(format!(
                "http://{address}/account/pages/{page_id}/recent?kind=tip"
            ))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()["items"][0]["status"]
            .clone()
    };
    let response = client.post(&capture_url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let moblin_cloud_key = api.accounts.get("1").unwrap().moblin_cloud_key.clone();
    let mut socket = connect(&address, &moblin_cloud_key).await;
    let acknowledge = async |socket: &mut Socket, request_id: &str| {
        let request = receive(socket).await;
        assert_eq!(request["request"]["data"]["tip"]["requestId"], request_id);
        socket
            .send(Message::text(
                json!({"response": {"id": request["request"]["id"], "result": {"ok": {}}}})
                    .to_string(),
            ))
            .await
            .unwrap();
        wait_until_added(&api).await;
    };
    acknowledge(&mut socket, "ORDER1").await;
    assert_eq!(status().await, "delivered");
    let url = format!("http://{address}/account/pages/{page_id}/recent/ORDER1/resend");
    let response = client
        .post(&url)
        .bearer_auth(&session)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    acknowledge(&mut socket, "ORDER1-1").await;
    assert_eq!(status().await, "delivered");
    for url in [
        format!("http://{address}/account/pages/{page_id}/recent/ORDER2/resend"),
        format!("http://{address}/account/pages/OTHER/recent/ORDER1/resend"),
    ] {
        let response = client
            .post(&url)
            .bearer_auth(&session)
            .send()
            .await
            .unwrap();
        assert!(
            [StatusCode::NOT_FOUND, StatusCode::FORBIDDEN].contains(&response.status()),
            "{url}"
        );
    }
    let response = client.post(&url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(status().await, "delivered");
}

#[tokio::test]
async fn recent_lists_are_paged_per_kind() {
    let (address, api) = start_api(None).await;
    let (session, _, page_id) = streamer(&api);
    let order = |item: Item, price: u32| Order {
        streamer: page_id.clone(),
        item,
        user: "Viewer".to_string(),
        payee: Payee::PayPal("MERCHANT00001".to_string()),
        price,
        currency: "USD".to_string(),
    };
    for n in 0..45 {
        let order_id = format!("TIP{n}");
        api.orders
            .insert(
                &order_id,
                &order(
                    Item::Tip {
                        message: Some(format!("m{n}")),
                    },
                    100 + n,
                ),
            )
            .unwrap();
        api.orders.captured(&order_id, |_| Ok(())).unwrap();
    }
    api.orders
        .insert(
            "SPIN1",
            &order(
                Item::Spin {
                    spins: 2,
                    message: Some("Go!".to_string()),
                },
                800,
            ),
        )
        .unwrap();
    api.orders
        .captured("SPIN1", |connection| {
            deliveries::insert(
                connection,
                &page_id,
                "SPIN1",
                &deliveries::Item::Spin {
                    user: "Viewer".to_string(),
                    message: "Go!".to_string(),
                    spins: 2,
                },
            )
        })
        .unwrap();
    let client = reqwest::Client::new();
    let recent = |query: String| {
        let request = client
            .get(format!(
                "http://{address}/account/pages/{page_id}/recent?{query}"
            ))
            .bearer_auth(&session);
        async move { request.send().await.unwrap() }
    };
    for kind in ["gift", "other"] {
        let response = recent(format!("kind={kind}")).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let response = recent("before=TIP1".to_string()).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let all: Value = recent("limit=5".to_string()).await.json().await.unwrap();
    assert_eq!(all["tip"]["items"].as_array().unwrap().len(), 5);
    assert_eq!(all["tip"]["more"], true);
    assert_eq!(all["tip"]["items"][0]["id"], "TIP44");
    assert_eq!(all["spin"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(all["spin"]["more"], false);
    assert_eq!(all["effect"], json!({"items": [], "more": false}));
    let all: Value = recent(String::new()).await.json().await.unwrap();
    assert_eq!(all["tip"]["items"].as_array().unwrap().len(), 20);
    let mut ids = Vec::new();
    let mut before = String::new();
    for (expected, more) in [(20, true), (20, true), (5, false)] {
        let listed: Value = recent(format!("kind=tip{before}"))
            .await
            .json()
            .await
            .unwrap();
        let items = listed["items"].as_array().unwrap();
        assert_eq!(items.len(), expected);
        assert_eq!(listed["more"], more);
        ids.extend(
            items
                .iter()
                .map(|item| item["id"].as_str().unwrap().to_string()),
        );
        before = format!("&before={}", ids.last().unwrap());
    }
    assert_eq!(
        ids,
        (0..45).rev().map(|n| format!("TIP{n}")).collect::<Vec<_>>()
    );
    let listed: Value = recent("kind=tip&limit=5".to_string())
        .await
        .json()
        .await
        .unwrap();
    let tips = listed["items"].as_array().unwrap();
    assert_eq!(tips.len(), 5);
    assert_eq!(listed["more"], true);
    assert_eq!(tips[0]["id"], "TIP44");
    assert_eq!(tips[0]["message"], "m44");
    assert_eq!(tips[0]["amount"], 144);
    assert_eq!(tips[0]["status"], "delivered");
    let listed: Value = recent("kind=spin".to_string()).await.json().await.unwrap();
    assert_eq!(listed["items"].as_array().unwrap().len(), 1);
    let spin = &listed["items"][0];
    assert_eq!(spin["spins"], 2);
    assert_eq!(spin["message"], "Go!");
    assert_eq!(spin["status"], "waiting");
    assert_eq!(spin["devices"], json!([]));
    api.orders
        .insert(
            "OTHER",
            &Order {
                streamer: "other".to_string(),
                ..order(Item::Tip { message: None }, 100)
            },
        )
        .unwrap();
    api.orders.captured("OTHER", |_| Ok(())).unwrap();
    let listed: Value = recent("kind=tip&before=OTHER".to_string())
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(listed, json!({"items": [], "more": false}));
    let listed: Value = recent("kind=tip".to_string()).await.json().await.unwrap();
    assert_eq!(listed["items"][0]["id"], "TIP44");
}
