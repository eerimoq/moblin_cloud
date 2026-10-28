use std::sync::Arc;

use anyhow::anyhow;
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::Response;
use log::info;

use super::*;

pub(super) async fn handle_remote_control_streamer(
    State(api): State<Arc<Api>>,
    Path(key): Path<String>,
    upgrade: WebSocketUpgrade,
) -> Result<Response, StatusCode> {
    let Some(page) = api.accounts.page_of_key(&key) else {
        api.counters.bump("moblinConnectionsFailed:unknown-key");
        return Err(StatusCode::NOT_FOUND);
    };
    Ok(upgrade.on_upgrade(move |mut socket| async move {
        let identity = match api.remote_control.identify(&mut socket).await {
            Ok(identity) => identity,
            Err(error) => {
                info!("remote control streamer {page} failed to identify: {error:#}");
                return;
            }
        };
        let Some(device) = identity.device else {
            api.counters.bump("moblinConnectionsFailed:no-device-id");
            api.remote_control.refuse(socket).await;
            return;
        };
        if let Some(app_attest) = &api.app_attest {
            let attested = match &identity.app_attest {
                Some(attestation) => app_attest
                    .verify(attestation, &identity.challenge)
                    .map_err(|error| ("invalid", error)),
                None => Err(("missing", anyhow!("no app attest in identify"))),
            };
            match attested {
                Ok(()) => api.counters.bump("moblinAttestations"),
                Err((reason, error)) => {
                    info!(
                        "remote control streamer {page} device {device} is not attested: {error:#}"
                    );
                    api.counters
                        .bump(&format!("moblinAttestationsFailed:{reason}"));
                    if api.require_attested_moblin {
                        api.counters.bump("moblinConnectionsFailed:unattested");
                        api.remote_control.refuse(socket).await;
                        return;
                    }
                }
            }
        }
        let Some(connection) = api.remote_control.connect(&page, &device) else {
            info!("remote control streamer {page} device {device} rejected, too many devices");
            api.counters
                .bump("moblinConnectionsFailed:too-many-devices");
            api.remote_control.refuse(socket).await;
            return;
        };
        api.accounts.seen_device(&page, &device);
        api.counters.bump("moblinConnections");
        api.deliveries.notify();
        api.remote_control
            .serve(connection, socket, |icon| {
                api.pages.set_icon(&page, icon.id)
            })
            .await;
        api.accounts.left_device(&page, &device);
        api.remote_control.changes.notify(&page, Change::Page);
    }))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use axum::http::StatusCode;
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{Value, json};
    use tokio_tungstenite::tungstenite::Message;

    use crate::api::Api;
    use crate::api::test_utils::*;
    use crate::app_attest::{AppAttest, Environment};
    use crate::deliveries;

    async fn refused(address: &str, key: &str, identify: Value) {
        let (mut socket, _) = tokio_tungstenite::connect_async(format!(
            "ws://{address}/remote-control/streamer/{key}"
        ))
        .await
        .unwrap();
        assert_eq!(receive(&mut socket).await["hello"]["apiVersion"], "0.1");
        socket
            .send(Message::text(identify.to_string()))
            .await
            .unwrap();
        assert_eq!(
            receive(&mut socket).await,
            json!({"identified": {"result": {"error": {}}}})
        );
        assert!(matches!(
            socket.next().await,
            None | Some(Err(_)) | Some(Ok(Message::Close(_)))
        ));
    }

    #[tokio::test]
    async fn the_pages_of_logins_are_answered_with_their_addresses() {
        let (address, api) = start_api(None).await;
        streamer(&address, &api).await;
        create_account(&api, "2", "Anna", "anna");
        let (status, body) = get_json(
            &address,
            "/pages/addresses?logins=kick:nobody,twitch:erik,youtube:anna,twitch:anna,twitch,kick:erik",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            json!({
                "twitch:erik": {"url": "https://cloud.moblin.app/twitch/erik", "icon": null},
                "twitch:anna": {"url": "https://cloud.moblin.app/twitch/anna", "icon": null},
            })
        );
    }

    #[tokio::test]
    async fn moblins_icon_is_shown_on_the_page_and_in_its_address() {
        let (address, api) = start_api(None).await;
        let (_, key, id) = streamer(&address, &api).await;
        let icon = async || {
            let (_, profile) = get_json(&address, &format!("/pages/{id}")).await;
            let (_, addresses) = get_json(&address, "/pages/addresses?logins=twitch:erik").await;
            assert_eq!(
                addresses["twitch:erik"]["icon"],
                match &profile["icon"] {
                    Value::String(icon) => json!(format!("http://localhost/icons/{icon}")),
                    _ => Value::Null,
                }
            );
            profile["icon"].clone()
        };
        assert_eq!(icon().await, Value::Null);
        let mut socket = connect(&address, &key).await;
        for moblin in ["AppIconKing", "AppIconNope"] {
            socket
                .send(Message::text(
                    json!({"event": {"data": {"icon": {"id": moblin}}}}).to_string(),
                ))
                .await
                .unwrap();
        }
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(&mut socket).await, json!({"pong": {}}));
        assert_eq!(icon().await, json!("king"));
        socket.close(None).await.unwrap();
        assert_eq!(icon().await, json!("king"));
        let response = reqwest::get(format!("http://{address}/icons/king"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "image/png");
        assert!(response.bytes().await.unwrap().starts_with(b"\x89PNG"));
        let response = reqwest::get(format!("http://{address}/icons/nope"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn the_frontends_origin_is_allowed() {
        let mut api = build_api(None, None, None, None, None, Duration::ZERO);
        api.auth.frontend_url = "https://example.com/cloud".to_string();
        let (address, _) = start_api_built(api).await;
        let allowed = async |origin: &str| {
            let response = reqwest::Client::new()
                .get(format!("http://{address}/icons/king"))
                .header("origin", origin)
                .send()
                .await
                .unwrap();
            response
                .headers()
                .get("access-control-allow-origin")
                .map(|value| value.to_str().unwrap().to_string())
        };
        for origin in ["https://example.com", "https://cloud.moblin.app"] {
            assert_eq!(
                allowed(origin).await,
                Some("https://example.com".to_string())
            );
        }
    }

    #[tokio::test]
    async fn preflights_allow_the_session_and_are_cached() {
        let mut api = build_api(None, None, None, None, None, Duration::ZERO);
        api.auth.frontend_url = "https://example.com".to_string();
        let (address, _) = start_api_built(api).await;
        let response = reqwest::Client::new()
            .request(
                reqwest::Method::OPTIONS,
                format!("http://{address}/account"),
            )
            .header("origin", "https://example.com")
            .header("access-control-request-method", "POST")
            .header(
                "access-control-request-headers",
                "authorization,content-type",
            )
            .send()
            .await
            .unwrap();
        let headers = response.headers();
        assert_eq!(
            headers["access-control-allow-headers"],
            "authorization,content-type"
        );
        assert_eq!(headers["access-control-max-age"], "7200");
    }

    #[tokio::test]
    async fn up_to_5_devices_connect_with_one_key() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        let mut sockets = Vec::new();
        for device in ["a", "b", "c", "d", "e"] {
            sockets.push(connect_as(&address, &key, device).await);
        }
        refused(
            &address,
            &key,
            json!({"identify": {"streamerId": "f", "authentication": "x"}}),
        )
        .await;
        refused(&address, &key, json!({"identify": {"authentication": "x"}})).await;
        sockets.push(connect_as(&address, &key, "a").await);
        let account: Value = client
            .get(format!("http://{address}/account"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(account["key"], key);
        assert_eq!(
            account["devices"],
            json!([
                {"id": "a", "name": "Moblin 1", "connected": true, "reconnecting": false, "wheel": false, "lastSeen": null},
                {"id": "b", "name": "Moblin 2", "connected": true, "reconnecting": false, "wheel": false, "lastSeen": null},
                {"id": "c", "name": "Moblin 3", "connected": true, "reconnecting": false, "wheel": false, "lastSeen": null},
                {"id": "d", "name": "Moblin 4", "connected": true, "reconnecting": false, "wheel": false, "lastSeen": null},
                {"id": "e", "name": "Moblin 5", "connected": true, "reconnecting": false, "wheel": false, "lastSeen": null},
            ])
        );
        assert_eq!(api.remote_control.connected_count(), 5);
        let counters = api.counters.all();
        let count = |name: &str| {
            counters
                .iter()
                .find(|counter| counter.name == name)
                .map(|counter| counter.total)
        };
        assert_eq!(count("moblinConnections"), Some(6));
        assert_eq!(count("moblinConnectionsFailed:too-many-devices"), Some(1));
        assert_eq!(count("moblinConnectionsFailed:no-device-id"), Some(1));
        let rename = |device: &str, name: &str| {
            client
                .post(format!("http://{address}/account/devices/{device}/name"))
                .bearer_auth(&session)
                .json(&json!({"name": name}))
                .send()
        };
        let response = rename("x", "Backup").await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let response = rename("b", &"x".repeat(31)).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let account: Value = rename("b", " Backup ").await.unwrap().json().await.unwrap();
        assert_eq!(account["devices"][1]["name"], "Backup");
        let response = rename("b", " ").await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let remove = |device: &str| {
            client
                .post(format!("http://{address}/account/devices/{device}/remove"))
                .bearer_auth(&session)
                .send()
        };
        let response = remove("b").await.unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let response = client
            .post(format!("http://{address}/account/key"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let account: Value = response.json().await.unwrap();
        assert_ne!(account["key"], key);
        assert!(!api.remote_control.is_connected(&id));
        let devices = account["devices"].as_array().unwrap();
        assert_eq!(devices.len(), 5);
        assert!(
            devices
                .iter()
                .all(|device| device["connected"] == false && device["lastSeen"].is_u64())
        );
        assert!(
            devices
                .iter()
                .any(|device| device["id"] == "b" && device["name"] == "Backup")
        );
        let response = remove("x").await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let account: Value = remove("b").await.unwrap().json().await.unwrap();
        let devices = account["devices"].as_array().unwrap();
        assert_eq!(devices.len(), 4);
        assert!(devices.iter().all(|device| device["id"] != "b"));
    }

    async fn start_attested(require: bool) -> (String, Arc<Api>) {
        let mut api = build_api(None, None, None, None, None, Duration::ZERO);
        api.app_attest = Some(AppAttest::new(
            "35MFYY2JY5.co.chiff.attestation-test",
            Environment::Development,
        ));
        api.require_attested_moblin = require;
        start_api_built(api).await
    }

    fn bogus_identify(device: &str) -> Value {
        json!({"identify": {"streamerId": device, "authentication": "x", "appAttest": {
            "keyId": "eA==", "attestation": "eA==", "attestationChallenge": "eA==", "assertion": "eA=="
        }}})
    }

    async fn connect_with(address: &str, key: &str, identify: Value) -> Socket {
        let (mut socket, _) = tokio_tungstenite::connect_async(format!(
            "ws://{address}/remote-control/streamer/{key}"
        ))
        .await
        .unwrap();
        receive(&mut socket).await;
        socket
            .send(Message::text(identify.to_string()))
            .await
            .unwrap();
        assert_eq!(
            receive(&mut socket).await,
            json!({"identified": {"result": {"ok": {}}}})
        );
        socket
    }

    fn count(api: &Api, name: &str) -> Option<u64> {
        api.counters
            .all()
            .into_iter()
            .find(|counter| counter.name == name)
            .map(|counter| counter.total)
    }

    #[tokio::test]
    async fn unattested_moblins_are_counted_until_required() {
        let (address, api) = start_attested(false).await;
        let (_, key, id) = streamer(&address, &api).await;
        let _plain = connect_as(&address, &key, "a").await;
        let _bogus = connect_with(&address, &key, bogus_identify("b")).await;
        let _malformed = connect_with(
            &address,
            &key,
            json!({"identify": {"streamerId": "c", "authentication": "x", "appAttest": {"keyId": 1}}}),
        )
        .await;
        assert_eq!(api.remote_control.devices(&id).len(), 3);
        assert_eq!(count(&api, "moblinConnections"), Some(3));
        assert_eq!(count(&api, "moblinAttestations"), None);
        assert_eq!(count(&api, "moblinAttestationsFailed:missing"), Some(1));
        assert_eq!(count(&api, "moblinAttestationsFailed:invalid"), Some(2));
    }

    #[tokio::test]
    async fn unattested_moblins_are_refused_when_required() {
        let (address, api) = start_attested(true).await;
        let (_, key, id) = streamer(&address, &api).await;
        refused(
            &address,
            &key,
            json!({"identify": {"streamerId": "a", "authentication": "x"}}),
        )
        .await;
        refused(&address, &key, bogus_identify("a")).await;
        assert!(!api.remote_control.is_connected(&id));
        assert_eq!(count(&api, "moblinConnections"), None);
        assert_eq!(count(&api, "moblinConnectionsFailed:unattested"), Some(2));
        assert_eq!(count(&api, "moblinAttestationsFailed:missing"), Some(1));
        assert_eq!(count(&api, "moblinAttestationsFailed:invalid"), Some(1));
    }

    async fn send_event(socket: &mut Socket, event: Value) {
        socket
            .send(Message::text(json!({"event": {"data": event}}).to_string()))
            .await
            .unwrap();
        socket
            .send(Message::text(json!({"ping": {}}).to_string()))
            .await
            .unwrap();
        assert_eq!(receive(socket).await, json!({"pong": {}}));
    }

    #[tokio::test]
    async fn a_moblin_that_stops_reading_is_disconnected() {
        let (address, api) = start_api(None).await;
        let (_, key, id) = streamer(&address, &api).await;
        let mut socket = connect(&address, &key).await;
        let api_tips = api.clone();
        let page = id.clone();
        tokio::spawn(async move {
            for number in 0..50 {
                let api = api_tips.clone();
                let page = page.clone();
                tokio::spawn(async move {
                    let _ = api
                        .remote_control
                        .tip(
                            (&page, "A"),
                            "Erik".to_string(),
                            "x".repeat(1_000_000),
                            crate::remote_control::TipKind::WheelOfLuck { count: 1 },
                            number.to_string(),
                        )
                        .await;
                });
            }
        });
        for _ in 0..30 {
            if !api.remote_control.is_connected(&id) {
                return;
            }
            socket
                .send(Message::text(json!({"ping": {}}).to_string()))
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        panic!("still connected");
    }

    #[tokio::test]
    async fn spins_go_to_the_first_device_with_a_wheel_and_tips_to_all() {
        let (address, api) = start_api(None).await;
        let client = reqwest::Client::new();
        let (session, key, id) = streamer(&address, &api).await;
        let mut main = connect_as(&address, &key, "main").await;
        let mut other = connect_as(&address, &key, "other").await;
        send_event(
            &mut other,
            json!({"wheelOfLuck": {"wheel": {"options": [{"text": "A", "weight": 1}]}}}),
        )
        .await;
        let account: Value = client
            .get(format!("http://{address}/account"))
            .bearer_auth(&session)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            account["devices"],
            json!([
                {"id": "main", "name": "Moblin 1", "connected": true, "reconnecting": false, "wheel": false, "lastSeen": null},
                {"id": "other", "name": "Moblin 2", "connected": true, "reconnecting": false, "wheel": true, "lastSeen": null},
            ])
        );
        let spin_url = format!("http://{address}/pages/{id}/spin");
        let spin: Value = client
            .get(&spin_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(spin["wheel"]["options"][0]["text"], "A");
        api.deliveries.add_test(
            id.clone(),
            "TIP1".to_string(),
            deliveries::Item::Tip {
                user: "Viewer".to_string(),
                message: String::new(),
                amount: 500,
                currency: "USD".to_string(),
            },
        );
        for socket in [&mut main, &mut other] {
            let request = receive(socket).await;
            assert_eq!(request["request"]["data"]["tip"]["requestId"], "TIP1");
            answer(socket, &request, Value::Null).await;
        }
        wait_until_added(&api).await;
        other.close(None).await.unwrap();
        for _ in 0..100 {
            if api.remote_control.devices(&id).len() == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let spin: Value = client
            .get(&spin_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(spin["wheel"], Value::Null);
        assert_eq!(spin["connected"], true);
    }
}
