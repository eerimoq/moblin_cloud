use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use serde_json::{Value, json};

use super::*;
use crate::access::SignInAccess;
use crate::accounts::{Accounts, Identity, Platform};
use crate::api::test_utils::*;
use crate::auth::Auth;
use crate::counters::Counters;
use crate::database::Database;
use crate::deliveries::Deliveries;
use crate::pages::Pages;
use crate::payments::{DEADLINE, Orders};
use crate::remote_control::Streamers;
use crate::requests::Requests;
use crate::subscribers::Allowances;

#[tokio::test]
async fn streamers_sign_in_and_manage_their_page() {
    let twitch = start_twitch().await;
    let (address, api) = start_api_with(None, None, Some(twitch_app(&twitch)), None, None).await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let unconfigured = start().await;
    let response = client
        .get(format!("http://{unconfigured}/auth/twitch/login"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let response = client
        .get(format!("http://{address}/auth/twitch/login"))
        .send()
        .await
        .unwrap();
    let login = location(&response);
    let cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    assert!(login.starts_with(&format!("{twitch}/authorize?")));
    let state = reqwest::Url::parse(&login)
        .unwrap()
        .query_pairs()
        .find(|(name, _)| name == "state")
        .unwrap()
        .1
        .into_owned();
    assert_eq!(cookie, format!("moblin_sign_in_state={state}"));
    let callback = format!("http://{address}/auth/twitch/callback?code=c&state={state}");
    let page = location(
        &client
            .get(&callback)
            .header("cookie", &cookie)
            .send()
            .await
            .unwrap(),
    );
    let session = page
        .strip_prefix("https://cloud.moblin.app/dashboard/#session=")
        .unwrap()
        .to_string();
    create_account(&api, "1", "One", "one");
    api.accounts
        .add_assistant(
            Platform::Twitch,
            "1",
            &twitch_assistant("42", "fortytwo", "42"),
        )
        .unwrap();
    assert!(api.accounts.assists(Platform::Twitch, "42", "1"));
    assert!(!api.accounts.assists(Platform::Twitch, "42", "9"));
    let response = client
        .get(format!("http://{address}/auth/twitch/login"))
        .send()
        .await
        .unwrap();
    let again = reqwest::Url::parse(&location(&response)).unwrap();
    let again_cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let again_state = again
        .query_pairs()
        .find(|(name, _)| name == "state")
        .unwrap()
        .1
        .into_owned();
    let again = location(
        &client
            .get(format!(
                "http://{address}/auth/twitch/callback?code=c&state={again_state}"
            ))
            .header("cookie", &again_cookie)
            .send()
            .await
            .unwrap(),
    );
    assert!(
        again.starts_with("https://cloud.moblin.app/dashboard/#session="),
        "{again}"
    );
    assert!(api.accounts.assists(Platform::Twitch, "42", "1"));
    assert_eq!(
        location(
            &client
                .get(&callback)
                .header("cookie", &cookie)
                .send()
                .await
                .unwrap()
        ),
        "https://cloud.moblin.app/dashboard/#signInFailed"
    );
    let response = client
        .get(format!("http://{address}/auth/twitch/login"))
        .send()
        .await
        .unwrap();
    let state = reqwest::Url::parse(&location(&response))
        .unwrap()
        .query_pairs()
        .find(|(name, _)| name == "state")
        .unwrap()
        .1
        .into_owned();
    assert_eq!(
        location(
            &client
                .get(format!(
                    "http://{address}/auth/twitch/callback?code=c&state={state}"
                ))
                .header("cookie", "moblin_sign_in_state=other")
                .send()
                .await
                .unwrap()
        ),
        "https://cloud.moblin.app/dashboard/#signInFailed"
    );
    let response = client
        .get(format!("http://{address}/account"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let account: Value = client
        .get(format!("http://{address}/account"))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let moblin_cloud_key = moblin_cloud_key_of(&account);
    let page_id = account["settings"]["id"].as_str().unwrap().to_string();
    assert_eq!(page_id.len(), 16);
    assert_eq!(account["name"], "Erik");
    assert_eq!(account["avatar"], "https://static-cdn.jtvnw.net/erik.png");
    assert_eq!(
        account["twitch"],
        json!({"login": "erik", "name": "Erik", "avatar": "https://static-cdn.jtvnw.net/erik.png"})
    );
    assert_eq!(account["kick"], Value::Null);
    assert_eq!(account["devices"], json!([]));
    assert_eq!(account["admin"], false);
    assert_eq!(account["premium"], false);
    let settings: Value = client
        .post(format!("http://{address}/account/pages/{page_id}/settings"))
        .bearer_auth(&session)
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(settings, account["settings"]);
    let settings: Value = client
        .post(format!("http://{address}/account/pages/{page_id}/settings"))
        .bearer_auth(&session)
        .json(&json!({
            "message": "Thanks for hanging out!",
            "accent": "pink",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(settings["message"], "Thanks for hanging out!");
    assert_eq!(settings["accent"], "pink");
    assert_eq!(settings["font"], Value::Null);
    let response = client
        .post(format!("http://{address}/account/pages/{page_id}/settings"))
        .bearer_auth(&session)
        .json(&json!({"accent": "teal"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let name: Value = client
        .get(format!("http://{address}/pages/twitch/Erik"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(name["id"], account["settings"]["id"]);
    let profile: Value = client
        .get(format!(
            "http://{address}/pages/{}",
            name["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        profile,
        json!({
            "twitch": {
                "name": "Erik",
                "login": "erik",
                "avatar": "https://static-cdn.jtvnw.net/erik.png",
            },
            "kick": null,
            "message": "Thanks for hanging out!",
            "accent": "pink",
            "font": null,
            "image": null,
            "icon": null,
        })
    );
    let response = client
        .get(format!("http://{address}/pages/unknown"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = client
        .get(format!("http://{address}/pages/twitch/other"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let again: Value = client
        .get(format!("http://{address}/account"))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(moblin_cloud_key_of(&again), moblin_cloud_key);
}

fn link_of(account: &str) -> String {
    account
        .strip_prefix("https://cloud.moblin.app/account/#link=")
        .unwrap_or_else(|| panic!("{account}"))
        .to_string()
}

#[tokio::test]
async fn kick_streamers_sign_in_with_pkce() {
    let (kick, fake) = start_kick().await;
    let (address, api) = start_api_with(None, None, None, Some(kick_app(&kick)), None).await;
    let flow = SignInFlow::new(&address, fake.clone());
    let unconfigured = start().await;
    let response = reqwest::Client::new()
        .get(format!("http://{unconfigured}/auth/kick/login"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let (query, cookie) = flow.start("/auth/kick/login").await;
    assert_eq!(query["client_id"], "kid");
    assert_eq!(query["redirect_uri"], "http://localhost/auth/kick/callback");
    assert_eq!(query["scope"], "user:read channel:read");
    assert_eq!(query["code_challenge_method"], "S256");
    assert_eq!(cookie, format!("moblin_sign_in_state={}", query["state"]));
    assert_eq!(
        flow.finish(
            "kick",
            "kick-a",
            &query["state"],
            "moblin_sign_in_state=other"
        )
        .await,
        "https://cloud.moblin.app/dashboard/#signInFailed"
    );
    let session = session_of(&flow.sign_in("kick", "kick-a").await);
    let token = fake.tokens.lock().unwrap().last().unwrap().clone();
    assert_eq!(token["client_id"], "kid");
    assert_eq!(token["client_secret"], "ksecret");
    assert_eq!(token["grant_type"], "authorization_code");
    assert_eq!(token["redirect_uri"], "http://localhost/auth/kick/callback");
    let account = account_of(&address, &session).await;
    assert_eq!(account["name"], "KickErik");
    assert_eq!(account["avatar"], "https://files.kick.com/erik.png");
    assert_eq!(account["twitch"], Value::Null);
    assert_eq!(
        account["kick"],
        json!({"login": "kickerik", "name": "KickErik", "avatar": "https://files.kick.com/erik.png"})
    );
    assert_eq!(account["twitchAssistants"], json!([]));
    assert_eq!(account["assisting"], json!([]));
    let id = account["settings"]["id"].as_str().unwrap().to_string();
    let session_account = api.auth.session_of(&session).unwrap().account;
    assert_ne!(session_account, "123");
    assert_eq!(session_account.len(), 32);
    assert!(session_account.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(api.accounts.get(&session_account).unwrap().page_id, id);
    assert_eq!(api.accounts.get("123"), None);
    let (status, name) = get_json(&address, "/pages/kick/KickErik").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(name["id"], id);
    let (status, _) = get_json(&address, "/pages/twitch/kickerik").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, profile) = get_json(&address, &format!("/pages/{id}")).await;
    assert_eq!(profile["twitch"], Value::Null);
    assert_eq!(profile["kick"]["login"], "kickerik");
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{address}/account/pages/{id}/settings"))
        .bearer_auth(&session)
        .json(&json!({"subSpins": {"tier1": 1, "tier2": 2, "tier3": 3}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let (_, spin) = get_json(&address, &format!("/pages/{id}/spin")).await;
    assert_eq!(
        spin["subSpins"],
        json!({"tier1": 1, "tier2": 2, "tier3": 3})
    );
    let response = client
        .get(format!("http://{address}/auth/twitch/login?page={id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = client
        .post(format!("http://{address}/account/assistants/twitch"))
        .bearer_auth(&session)
        .json(&json!({"username": "bob"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(response.text().await.unwrap(), "Link Twitch first.");
    let response = client
        .post(format!("http://{address}/account/assistants/youtube"))
        .bearer_auth(&session)
        .json(&json!({"username": "bob"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let again = session_of(&flow.sign_in("kick", "kick-a").await);
    assert_eq!(
        api.auth.session_of(&again).unwrap().account,
        session_account
    );
    assert_eq!(account_of(&address, &again).await["settings"]["id"], id);
    assert_eq!(
        flow.sign_in("kick", "unknown").await,
        "https://cloud.moblin.app/dashboard/#signInFailed"
    );
    let other = session_of(&flow.sign_in("kick", "kick-b").await);
    let other = account_of(&address, &other).await;
    assert_eq!(
        other["kick"],
        json!({"login": "other-kick", "name": "other-kick", "avatar": null})
    );
    assert_ne!(other["settings"]["id"], id);
    let account = api.accounts.get(&session_account).unwrap();
    assert_eq!(account.identities[0].admin_key(), "kick:123");
    assert!(!is_admin(&api, &account));
    assert_eq!(
        api.accounts.stats(&["kick:123", "kick:456", "123"]).premium,
        2
    );
    assert_eq!(api.accounts.stats(&["123", "456"]).premium, 0);
}

#[tokio::test]
async fn only_configured_platforms_are_offered() {
    let (kick, fake) = start_kick().await;
    let (address, api) = start_api_with(None, None, None, Some(kick_app(&kick)), None).await;
    let client = reqwest::Client::new();
    let config: Value = client
        .get(format!("http://{address}/config"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(config["signIn"], json!({"twitch": false, "kick": true}));
    let session = session_of(
        &SignInFlow::new(&address, fake)
            .sign_in("kick", "kick-a")
            .await,
    );
    let account = account_of(&address, &session).await;
    let page_id = account["settings"]["id"].as_str().unwrap().to_string();
    let id = api.auth.session_of(&session).unwrap().account;
    api.accounts
        .link(&id, &Identity::twitch("9", "Nine", "nine"))
        .unwrap();
    let response = client
        .post(format!("http://{address}/account/pages/{page_id}/settings"))
        .bearer_auth(&session)
        .json(&json!({"subSpins": {"tier1": 1, "tier2": 2, "tier3": 3}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let sub_spins = || async {
        let spin: Value = client
            .get(format!("http://{address}/pages/{page_id}/spin"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        spin["subSpins"].clone()
    };
    assert_eq!(sub_spins().await["tier1"], 1);
    let unlink = |platform: &str| {
        client
            .post(format!("http://{address}/account/unlink/{platform}"))
            .bearer_auth(&session)
            .send()
    };
    let response = unlink("kick").await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.text().await.unwrap(),
        "You can't unlink the only way to sign in. Link another platform first."
    );
    api.accounts.unlink(&id, Platform::Kick).unwrap();
    assert_eq!(sub_spins().await, Value::Null);
    assert_eq!(account_of(&address, &session).await["kick"], Value::Null);
    api.accounts
        .link(&id, &Identity::kick("8", "Eight", "eight"))
        .unwrap();
    let response = unlink("twitch").await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn linked_platforms_share_one_account() {
    let twitch = start_twitch().await;
    let (kick, fake) = start_kick().await;
    let (address, api) = start_api_with(
        None,
        None,
        Some(twitch_app(&twitch)),
        Some(kick_app(&kick)),
        None,
    )
    .await;
    let flow = SignInFlow::new(&address, fake.clone());
    let client = reqwest::Client::new();
    let session = session_of(&flow.sign_in("twitch", "c").await);
    let account = account_of(&address, &session).await;
    let id = account["settings"]["id"].as_str().unwrap().to_string();
    let moblin_cloud_key = moblin_cloud_key_of(&account);
    let twitch_account = api.auth.session_of(&session).unwrap().account;
    assert_ne!(twitch_account, "42");
    assert_eq!(account["kick"], Value::Null);
    let link = |token: &str, session: &str| {
        client
            .post(format!("http://{address}/account/link"))
            .bearer_auth(session)
            .json(&json!({"token": token}))
            .send()
    };
    let unlink = |platform: &str, session: &str| {
        client
            .post(format!("http://{address}/account/unlink/{platform}"))
            .bearer_auth(session)
            .send()
    };
    let response = unlink("twitch", &session).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.text().await.unwrap(),
        "You can't unlink the only way to sign in. Link another platform first."
    );
    let response = unlink("kick", &session).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = unlink("youtube", &session).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = link("garbage", &session).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        flow.link("kick", "unknown").await,
        "https://cloud.moblin.app/account/#linkFailed"
    );
    let token = link_of(&flow.link("kick", "kick-a").await);
    assert_eq!(api.auth.link_of(&token).unwrap().user_id, "123".to_string());
    assert_eq!(
        api.accounts.get(&twitch_account).unwrap().identities.len(),
        1
    );
    let response = link(&token, "garbage").await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let account: Value = link(&token, &session).await.unwrap().json().await.unwrap();
    assert_eq!(account["twitch"]["login"], "erik");
    assert_eq!(account["kick"]["login"], "kickerik");
    assert_eq!(account["name"], "Erik");
    assert_eq!(account["settings"]["id"], id);
    assert_eq!(moblin_cloud_key_of(&account), moblin_cloud_key);
    let kick_session = session_of(&flow.sign_in("kick", "kick-a").await);
    assert_eq!(
        api.auth.session_of(&kick_session).unwrap().account,
        twitch_account
    );
    let account = account_of(&address, &kick_session).await;
    assert_eq!(account["settings"]["id"], id);
    assert_eq!(account["twitch"]["login"], "erik");
    assert_eq!(moblin_cloud_key_of(&account), moblin_cloud_key);
    for path in ["/pages/twitch/erik", "/pages/kick/kickerik"] {
        let (status, name) = get_json(&address, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(name["id"], id, "{path}");
    }
    let (_, profile) = get_json(&address, &format!("/pages/{id}")).await;
    assert_eq!(
        profile["twitch"],
        json!({"login": "erik", "name": "Erik", "avatar": "https://static-cdn.jtvnw.net/erik.png"})
    );
    assert_eq!(
        profile["kick"],
        json!({"login": "kickerik", "name": "KickErik", "avatar": "https://files.kick.com/erik.png"})
    );
    let response = client
        .post(format!("http://{address}/account/pages/{id}/settings"))
        .bearer_auth(&session)
        .json(&json!({"subSpins": {"tier1": 1, "tier2": 2, "tier3": 3}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let (_, spin) = get_json(&address, &format!("/pages/{id}/spin")).await;
    assert_eq!(spin["subSpins"]["tier1"], 1);
    let other = session_of(&flow.sign_in("kick", "kick-b").await);
    let other_account = api.auth.session_of(&other).unwrap().account;
    assert_ne!(other_account, twitch_account);
    let token = link_of(&flow.link("twitch", "c").await);
    let response = link(&token, &other).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.text().await.unwrap(),
        "This Twitch account already has its own page. Delete it first."
    );
    let token = link_of(&flow.link("kick", "kick-b").await);
    let response = link(&token, &session).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.text().await.unwrap(),
        "This Kick account already has its own page. Delete it first."
    );
    assert_eq!(
        api.accounts.get(&other_account).unwrap().identities.len(),
        1
    );
    assert_eq!(
        api.accounts.get(&twitch_account).unwrap().identities.len(),
        2
    );
    let account: Value = unlink("twitch", &kick_session)
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(account["twitch"], Value::Null);
    assert_eq!(account["kick"]["login"], "kickerik");
    assert_eq!(account["name"], "KickErik");
    assert_eq!(account["settings"]["id"], id);
    let (status, _) = get_json(&address, "/pages/twitch/erik").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, spin) = get_json(&address, &format!("/pages/{id}/spin")).await;
    assert_eq!(spin["subSpins"]["tier1"], 1);
    assert_eq!(account_of(&address, &session).await["settings"]["id"], id);
    let fresh = session_of(&flow.sign_in("twitch", "c").await);
    let fresh_account = api.auth.session_of(&fresh).unwrap().account;
    assert_ne!(fresh_account, twitch_account);
    assert_ne!(account_of(&address, &fresh).await["settings"]["id"], id);
    let (status, name) = get_json(&address, "/pages/twitch/erik").await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(name["id"], id);
}

#[tokio::test]
async fn sessions_of_twitch_user_ids_keep_working() {
    let twitch = start_twitch().await;
    let path = crate::database::tests::temporary_path("twitch-user-id-sessions");
    let database = Database::open(Some(&path)).unwrap();
    database
        .lock()
        .execute_batch(
            "INSERT INTO accounts (id, page_id, moblin_key, sessions) VALUES
                 ('0123456789abcdef0123456789abcdef', 'oldpage', 'k', 3);
             INSERT INTO identities (platform, user_id, account, login, name) VALUES
                 ('twitch', '42', '0123456789abcdef0123456789abcdef', 'erik', 'Old Erik');
             INSERT INTO pages (id, settings) VALUES ('oldpage', '{\"message\":\"Old\"}');
             INSERT INTO twitch_assistants (broadcaster, assistant, login, name) VALUES ('7', '42', 'erik', 'Erik');",
        )
        .unwrap();
    drop(database);
    let database = Database::open(Some(&path)).unwrap();
    let api = Arc::new(Api {
        app_attest: None,
        require_attested_moblin: false,
        remote_control: Streamers::new(std::time::Duration::ZERO),
        pages: Pages::new(database.clone()),
        paypal: None,
        stripe: None,
        orders: Orders::new(database.clone(), DEADLINE),
        allowances: Allowances::new(database.clone()),
        deliveries: Deliveries::new(database.clone()),
        auth: Auth::new(
            reqwest::Client::new(),
            "http://localhost".to_string(),
            "https://cloud.moblin.app".to_string(),
            Some(twitch_app(&twitch)),
            None,
            Some("secret"),
        )
        .unwrap(),
        accounts: Accounts::new(database.clone()),
        access: SignInAccess::new(database.clone()),
        counters: Counters::new(database.clone()),
        database,
        admins: vec!["twitch:42".to_string()],
        started: 0,
        tests: Mutex::default(),
        connect_tries: Mutex::default(),
        requests: Requests::default(),
        stopping: tokio::sync::watch::channel(false).0,
        timing: Timing::default(),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn({
        let api = api.clone();
        async move { axum::serve(listener, router(api)).await }
    });
    assert_eq!(api.accounts.get("42"), None);
    let id = api
        .accounts
        .account_of_identity(Platform::Twitch, "42")
        .unwrap();
    assert_eq!(id.len(), 32);
    let old_session = api.auth.session("42", 3);
    let account = account_of(&address, &old_session).await;
    assert_eq!(account["settings"]["id"], "oldpage");
    assert_eq!(account["twitch"]["login"], "erik");
    assert_eq!(account["admin"], true);
    let response = reqwest::Client::new()
        .get(format!("http://{address}/account"))
        .bearer_auth(api.auth.session("42", 2))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    create_account(&api, "7", "Streamer", "streamer");
    assert_eq!(
        account_of(&address, &old_session).await["assisting"][0]["login"],
        "streamer"
    );
    let flow = SignInFlow::new(&address, FakeKick::default());
    let session = session_of(&flow.sign_in("twitch", "c").await);
    assert_eq!(api.auth.session_of(&session).unwrap().account, id);
    let account = account_of(&address, &session).await;
    assert_eq!(account["settings"]["id"], "oldpage");
    assert_eq!(account["settings"]["message"], "Old");
    assert_eq!(account["name"], "Erik");
    assert_eq!(moblin_cloud_key_of(&account), "k");
    assert_eq!(account["admin"], true);
    let (_, name) = get_json(&address, "/pages/twitch/erik").await;
    assert_eq!(name["id"], "oldpage");
    crate::database::tests::remove(&path);
}

#[tokio::test]
async fn terminals_sign_in_through_a_loopback_redirect() {
    let twitch = start_twitch().await;
    let (kick, fake) = start_kick().await;
    let (address, _) = start_api_with(
        None,
        None,
        Some(twitch_app(&twitch)),
        Some(kick_app(&kick)),
        None,
    )
    .await;
    let flow = SignInFlow::new(&address, fake);
    let challenge = crate::auth::challenge_of("verifier");
    let exchange = async |code: &str, verifier: &str| {
        let response = flow
            .client
            .post(format!("http://{address}/auth/session"))
            .json(&json!({"code": code, "verifier": verifier}))
            .send()
            .await
            .unwrap();
        let status = response.status();
        (status, response.text().await.unwrap())
    };
    let (query, cookie) = flow
        .start(&format!(
            "/auth/twitch/login?port=1234&challenge={challenge}"
        ))
        .await;
    let terminal = flow.finish("twitch", "c", &query["state"], &cookie).await;
    let code = terminal
        .strip_prefix("http://127.0.0.1:1234/?code=")
        .unwrap_or_else(|| panic!("{terminal}"));
    assert_eq!(
        exchange(code, "other").await,
        (
            StatusCode::BAD_REQUEST,
            "Signing in failed. Try again.".to_string()
        )
    );
    assert_eq!(exchange(code, "verifier").await.0, StatusCode::BAD_REQUEST);
    let (query, cookie) = flow
        .start(&format!(
            "/auth/twitch/login?port=1234&challenge={challenge}"
        ))
        .await;
    let terminal = flow.finish("twitch", "c", &query["state"], &cookie).await;
    let code = terminal
        .strip_prefix("http://127.0.0.1:1234/?code=")
        .unwrap();
    let (status, body) = exchange(code, "verifier").await;
    assert_eq!(status, StatusCode::OK);
    let session = serde_json::from_str::<Value>(&body).unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    let twitch_account = account_of(&address, &session).await;
    assert!(twitch_account["twitch"].is_object());
    assert_eq!(exchange(code, "verifier").await.0, StatusCode::BAD_REQUEST);
    let (query, cookie) = flow
        .start(&format!("/auth/kick/login?port=4321&challenge={challenge}"))
        .await;
    let terminal = flow
        .finish("kick", "kick-a", &query["state"], &cookie)
        .await;
    let code = terminal
        .strip_prefix("http://127.0.0.1:4321/?code=")
        .unwrap();
    let (_, body) = exchange(code, "verifier").await;
    let session = serde_json::from_str::<Value>(&body).unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        account_of(&address, &session).await["kick"]["login"],
        "kickerik"
    );
    let (query, cookie) = flow
        .start(&format!("/auth/kick/login?port=4321&challenge={challenge}"))
        .await;
    assert_eq!(
        flow.finish("kick", "unknown", &query["state"], &cookie)
            .await,
        "http://127.0.0.1:4321/?signInFailed"
    );
    let (query, _) = flow
        .start(&format!(
            "/auth/twitch/login?port=1234&challenge={challenge}"
        ))
        .await;
    assert_eq!(
        flow.finish("twitch", "c", &query["state"], "moblin_sign_in_state=other")
            .await,
        "https://cloud.moblin.app/dashboard/#signInFailed"
    );
    let id = twitch_account["settings"]["id"].as_str().unwrap();
    for path in [
        format!("/auth/twitch/login?port=1&challenge={challenge}&page={id}"),
        format!("/auth/twitch/login?port=1&challenge={challenge}&link=1"),
        format!("/auth/twitch/login?challenge={challenge}&link=1"),
        "/auth/twitch/login?port=1".to_string(),
        format!("/auth/twitch/login?challenge={challenge}"),
        "/auth/twitch/login?port=1&challenge=short".to_string(),
    ] {
        let response = flow
            .client
            .get(format!("http://{address}{path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path}");
    }
}

#[tokio::test]
async fn terminals_sign_in_with_a_code_approved_on_another_device() {
    let twitch = start_twitch().await;
    let (address, _) = start_api_with(None, None, Some(twitch_app(&twitch)), None, None).await;
    let flow = SignInFlow::new(&address, FakeKick::default());
    let session = session_of(&flow.sign_in("twitch", "c").await);
    let client = reqwest::Client::new();
    let post = async |path: &str, session: Option<&str>, body: Value| {
        let mut request = client.post(format!("http://{address}{path}")).json(&body);
        if let Some(session) = session {
            request = request.bearer_auth(session);
        }
        let response = request.send().await.unwrap();
        let status = response.status();
        (status, response.text().await.unwrap())
    };
    let start = async || {
        let (status, body) = post("/auth/connect", None, json!({})).await;
        assert_eq!(status, StatusCode::OK);
        serde_json::from_str::<Value>(&body).unwrap()
    };
    let connect = start().await;
    let device_code = connect["deviceCode"].as_str().unwrap();
    let user_code = connect["userCode"].as_str().unwrap();
    assert_eq!(user_code.len(), 9);
    assert_eq!(&user_code[4..5], "-");
    assert_eq!(connect["url"], "https://cloud.moblin.app/connect/");
    assert_eq!(connect.get("codeUrl"), None);
    assert_eq!(connect["interval"], 5);
    let poll = json!({"deviceCode": device_code});
    assert_eq!(
        post("/auth/connect/session", None, poll.clone()).await,
        (StatusCode::ACCEPTED, "{}".to_string())
    );
    assert_eq!(
        post(
            "/account/connect",
            None,
            json!({"code": user_code, "approve": true})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post(
            "/account/connect",
            Some(&session),
            json!({"code": user_code.to_lowercase(), "approve": true})
        )
        .await,
        (StatusCode::OK, "{}".to_string())
    );
    let (status, body) = post("/auth/connect/session", None, poll.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let terminal = serde_json::from_str::<Value>(&body).unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        account_of(&address, &terminal).await["settings"]["id"],
        account_of(&address, &session).await["settings"]["id"]
    );
    assert_eq!(
        post("/auth/connect/session", None, poll).await,
        (
            StatusCode::GONE,
            "The code expired. Start signing in again.".to_string()
        )
    );
    let connect = start().await;
    assert_eq!(
        post(
            "/account/connect",
            Some(&session),
            json!({"code": connect["userCode"], "approve": false})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        post(
            "/auth/connect/session",
            None,
            json!({"deviceCode": connect["deviceCode"]})
        )
        .await,
        (StatusCode::FORBIDDEN, "Signing in was denied.".to_string())
    );
    assert_eq!(
        post(
            "/account/connect",
            Some(&session),
            json!({"code": connect["userCode"], "approve": true})
        )
        .await,
        (
            StatusCode::NOT_FOUND,
            "There is no such code. Check it, or start signing in again in mobtop.".to_string()
        )
    );
    for _ in 0..7 {
        assert_eq!(
            post(
                "/account/connect",
                Some(&session),
                json!({"code": "BBBB-BBBB", "approve": true})
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        post(
            "/account/connect",
            Some(&session),
            json!({"code": "BBBB-BBBB", "approve": true})
        )
        .await,
        (
            StatusCode::TOO_MANY_REQUESTS,
            "Too many tries. Try again in a minute.".to_string()
        )
    );
}

#[tokio::test]
async fn only_allowed_accounts_sign_in_while_restricted() {
    let twitch = start_twitch().await;
    let (kick, fake) = start_kick().await;
    let (address, api) = start_api_with(
        None,
        None,
        Some(twitch_app(&twitch)),
        Some(kick_app(&kick)),
        None,
    )
    .await;
    let flow = SignInFlow::new(&address, fake);
    let client = reqwest::Client::new();
    let admin_page = create_account(&api, "admin", "Admin", "admin").page_id;
    let admin = api.auth.session("admin", 0);
    create_account(&api, "premium", "Premium", "premium");
    let premium = api.auth.session("premium", 0);
    let kicker = session_of(&flow.sign_in("kick", "kick-a").await);
    let post = async |path: &str, session: &str, body: Value| {
        let response = client
            .post(format!("http://{address}{path}"))
            .bearer_auth(session)
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = response.status();
        let text = response.text().await.unwrap();
        (
            status,
            serde_json::from_str(&text).unwrap_or(Value::String(text)),
        )
    };
    let status_of = async |session: &str| {
        client
            .get(format!("http://{address}/account"))
            .bearer_auth(session)
            .send()
            .await
            .unwrap()
            .status()
    };
    let users = |people: &Value| {
        people
            .as_array()
            .unwrap()
            .iter()
            .map(|person| {
                format!(
                    "{}:{}",
                    person["platform"].as_str().unwrap(),
                    person["userId"].as_str().unwrap()
                )
            })
            .collect::<Vec<_>>()
    };
    let allowed_users = |groups: &Value| {
        groups
            .as_array()
            .unwrap()
            .iter()
            .flat_map(users)
            .collect::<Vec<_>>()
    };
    let restrict = json!({"restricted": true});
    assert_eq!(
        post("/admin/sign-in", &kicker, restrict.clone()).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post("/admin/sign-in/allowed/twitch/42", &kicker, Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, state) = post("/admin/sign-in", &admin, restrict).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(state["restricted"], true);
    let mut allowed = allowed_users(&state["allowed"]);
    allowed.sort();
    assert_eq!(allowed, ["kick:123", "twitch:admin", "twitch:premium"]);
    assert_eq!(state["requests"], json!([]));
    assert_eq!(status_of(&kicker).await, StatusCode::OK);
    assert_eq!(status_of(&premium).await, StatusCode::OK);
    let (status, _) = post(
        "/admin/sign-in/allowed/twitch/premium/remove",
        &admin,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(status_of(&premium).await, StatusCode::UNAUTHORIZED);
    assert_eq!(
        flow.sign_in("twitch", "c").await,
        "https://cloud.moblin.app/dashboard/#signInRefused"
    );
    assert_eq!(
        api.accounts.account_of_identity(Platform::Twitch, "42"),
        None
    );
    let challenge = crate::auth::challenge_of("verifier");
    let (query, cookie) = flow
        .start(&format!(
            "/auth/twitch/login?port=1234&challenge={challenge}"
        ))
        .await;
    assert_eq!(
        flow.finish("twitch", "c", &query["state"], &cookie).await,
        "http://127.0.0.1:1234/?signInRefused"
    );
    let (query, cookie) = flow
        .start(&format!("/auth/twitch/login?page={admin_page}"))
        .await;
    assert!(
        flow.finish("twitch", "c", &query["state"], &cookie)
            .await
            .starts_with("https://cloud.moblin.app/twitch/admin#viewer=")
    );
    let requests = api.access.state().requests;
    assert_eq!(requests.len(), 1);
    assert_eq!(
        (requests[0].user_id.as_str(), requests[0].login.as_str()),
        ("42", "erik")
    );
    for path in [
        "/admin/sign-in/allowed/twitch/nobody",
        "/admin/sign-in/allowed/youtube/42",
    ] {
        assert_eq!(
            post(path, &admin, Value::Null).await.0,
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
    let (status, state) = post("/admin/sign-in/allowed/twitch/42", &admin, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(state["requests"], json!([]));
    assert!(allowed_users(&state["allowed"]).contains(&"twitch:42".to_string()));
    let erik = session_of(&flow.sign_in("twitch", "c").await);
    assert_eq!(status_of(&erik).await, StatusCode::OK);
    post(
        "/admin/sign-in/allowed/twitch/42/remove",
        &admin,
        Value::Null,
    )
    .await;
    assert_eq!(status_of(&erik).await, StatusCode::UNAUTHORIZED);
    assert_eq!(
        flow.sign_in("twitch", "c").await,
        "https://cloud.moblin.app/dashboard/#signInRefused"
    );
    let (status, state) = post(
        "/admin/sign-in/requests/twitch/42/remove",
        &admin,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(state["requests"], json!([]));
    post(
        "/admin/sign-in/allowed/twitch/admin/remove",
        &admin,
        Value::Null,
    )
    .await;
    assert_eq!(status_of(&admin).await, StatusCode::OK);
    assert_eq!(
        post("/admin/sign-in", &admin, json!({"restricted": false}))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(status_of(&erik).await, StatusCode::OK);
    assert_eq!(status_of(&premium).await, StatusCode::OK);
}
