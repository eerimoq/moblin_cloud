use super::*;

fn new_auth(secret: &str) -> Auth {
    Auth::new(
        Client::new(),
        "https://api.example.com/".to_string(),
        "https://example.com".to_string(),
        Some(OAuthApp::twitch("id".to_string(), "secret".to_string())),
        Some(KickApp::kick("kid".to_string(), "ksecret".to_string())),
        Some(secret),
    )
    .unwrap()
}

fn query_of(url: &str) -> HashMap<String, String> {
    Url::parse(url)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}

#[test]
fn sessions_are_verified() {
    let auth = new_auth("secret");
    let session = auth.session_of(&auth.session("1", 3)).unwrap();
    assert_eq!(session.account, "1");
    assert_eq!(session.generation, 3);
    assert!(session.is_fresh());
    let session = auth.session_at("1", 0, now() - 601);
    assert!(!auth.session_of(&session).unwrap().is_fresh());
    assert_eq!(new_auth("other").session_of(&session), None);
    let expired = auth.session_at("1", 0, now() - SESSION_LIFETIME);
    assert_eq!(auth.session_of(&expired), None);
    let (_, tag) = session.split_once('.').unwrap();
    let forged = format!(
        "{}.{tag}",
        BASE64_URL_SAFE_NO_PAD.encode(format!("2:0:{}", now()))
    );
    assert_eq!(auth.session_of(&forged), None);
    assert_eq!(auth.session_of("garbage"), None);
}

#[test]
fn connect_codes_are_decided_once_and_expire() {
    let auth = new_auth("secret");
    let (device_code, user_code) = auth.start_connect().unwrap();
    assert_eq!(user_code.len(), 8);
    assert!(
        user_code
            .bytes()
            .all(|byte| b"BCDFGHJKLMNPQRSTVWXZ".contains(&byte))
    );
    assert_eq!(auth.connect_session(&device_code), ConnectPoll::Pending);
    let typed = format!(" {}-{} ", user_code[..4].to_lowercase(), &user_code[4..]);
    assert_eq!(user_code_of(&typed), user_code);
    assert!(auth.decide_connect(&typed, Some("session".to_string())));
    assert!(!auth.decide_connect(&user_code, None));
    assert_eq!(
        auth.connect_session(&device_code),
        ConnectPoll::Session("session".to_string())
    );
    assert_eq!(auth.connect_session(&device_code), ConnectPoll::Gone);
    assert!(!auth.decide_connect(&user_code, Some("session".to_string())));
    let (device_code, user_code) = auth.start_connect().unwrap();
    assert!(auth.decide_connect(&user_code, None));
    assert_eq!(auth.connect_session(&device_code), ConnectPoll::Denied);
    assert_eq!(auth.connect_session(&device_code), ConnectPoll::Gone);
    assert!(!auth.decide_connect("BBBBBBBB", None));
    assert_eq!(auth.connect_session("unknown"), ConnectPoll::Gone);
    let (device_code, user_code) = auth.start_connect().unwrap();
    auth.connects
        .lock()
        .unwrap()
        .by_device
        .get_mut(&device_code)
        .unwrap()
        .created = Instant::now() - CONNECT_LIFETIME * 2;
    assert!(!auth.decide_connect(&user_code, Some("session".to_string())));
    assert_eq!(auth.connect_session(&device_code), ConnectPoll::Gone);
    let codes: std::collections::HashSet<_> =
        (0..100).map(|_| auth.start_connect().unwrap().1).collect();
    assert_eq!(codes.len(), 100);
}

#[test]
fn terminal_codes_are_single_use_and_need_the_verifier() {
    let auth = new_auth("secret");
    let challenge = challenge_of("verifier");
    assert!(Terminal::new(1234, challenge.clone()).is_some());
    assert!(Terminal::new(1234, "short".to_string()).is_none());
    assert!(Terminal::new(1234, format!("{}+", &challenge[1..])).is_none());
    let code = auth
        .terminal_code(challenge.clone(), "session".to_string())
        .unwrap();
    assert_eq!(auth.session_of_code(&code, "other"), None);
    assert_eq!(auth.session_of_code(&code, "verifier"), None);
    let code = auth
        .terminal_code(challenge.clone(), "session".to_string())
        .unwrap();
    assert_eq!(
        auth.session_of_code(&code, "verifier"),
        Some("session".to_string())
    );
    assert_eq!(auth.session_of_code(&code, "verifier"), None);
    auth.codes.lock().unwrap().insert(
        "old".to_string(),
        TerminalCode {
            created: Instant::now() - CODE_LIFETIME * 2,
            challenge,
            session: "session".to_string(),
        },
    );
    assert_eq!(auth.session_of_code("old", "verifier"), None);
}

#[test]
fn states_are_single_use() {
    let auth = new_auth("secret");
    let (url, state) = auth.twitch_login_url(None, false, None).unwrap();
    let url = Url::parse(&url).unwrap();
    let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(url.host_str(), Some("id.twitch.tv"));
    assert_eq!(
        query["redirect_uri"],
        "https://api.example.com/auth/twitch/callback"
    );
    assert_eq!(query["scope"], "openid");
    assert_eq!(query["state"], state);
    let state = Some(state.as_str());
    assert_eq!(
        auth.consume_state(&query["state"], state).unwrap(),
        SignInState::default()
    );
    assert!(auth.consume_state(&query["state"], state).is_err());
    auth.pending.lock().unwrap().insert(
        "old".to_string(),
        (Instant::now() - STATE_LIFETIME * 2, SignInState::default()),
    );
    assert!(auth.consume_state("old", Some("old")).is_err());
    let (url, state) = auth.twitch_login_url(Some("page"), false, None).unwrap();
    let url = Url::parse(&url).unwrap();
    let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(query["scope"], "user:read:subscriptions");
    assert_eq!(
        auth.consume_state(&state, Some("other"))
            .unwrap_err()
            .page_id
            .as_deref(),
        Some("page")
    );
    let (_, state) = auth.twitch_login_url(Some("page"), false, None).unwrap();
    assert_eq!(
        auth.consume_state(&state, Some(&state))
            .unwrap()
            .page_id
            .as_deref(),
        Some("page")
    );
    let (_, state) = auth.twitch_login_url(None, true, None).unwrap();
    assert!(auth.consume_state(&state, Some(&state)).unwrap().link);
}

#[test]
fn kick_signs_in_with_pkce() {
    let auth = new_auth("secret");
    let (url, state) = auth.kick_login_url(None, true, None).unwrap();
    let parsed = Url::parse(&url).unwrap();
    assert_eq!(parsed.host_str(), Some("id.kick.com"));
    assert_eq!(parsed.path(), "/oauth/authorize");
    let query = query_of(&url);
    assert_eq!(query["client_id"], "kid");
    assert_eq!(
        query["redirect_uri"],
        "https://api.example.com/auth/kick/callback"
    );
    assert_eq!(query["response_type"], "code");
    assert_eq!(query["scope"], "user:read channel:read");
    assert_eq!(query["code_challenge_method"], "S256");
    assert_eq!(query["state"], state);
    let pending = auth.consume_state(&state, Some(&state)).unwrap();
    assert!(pending.link);
    assert_eq!(pending.page_id, None);
    let verifier = pending.verifier.unwrap();
    assert!(verifier.len() >= 43);
    assert_eq!(
        query["code_challenge"],
        BASE64_URL_SAFE_NO_PAD.encode(digest(&SHA256, verifier.as_bytes()))
    );
    let (other, _) = auth.kick_login_url(None, false, None).unwrap();
    assert_ne!(query_of(&other)["code_challenge"], query["code_challenge"]);
    let (_, state) = auth.kick_login_url(Some("page"), false, None).unwrap();
    let pending = auth.consume_state(&state, Some(&state)).unwrap();
    assert_eq!(pending.page_id.as_deref(), Some("page"));
    assert!(!pending.link);
    let without = Auth::new(
        Client::new(),
        "https://api.example.com".to_string(),
        "https://example.com".to_string(),
        None,
        None,
        None,
    )
    .unwrap();
    assert!(without.kick_login_url(None, false, None).is_none());
    assert!(without.twitch_login_url(None, false, None).is_none());
}

#[test]
fn viewer_sessions_are_not_streamer_sessions() {
    let auth = new_auth("secret");
    let session = auth.viewer_session(
        Platform::Twitch,
        "7".to_string(),
        "Viewer".to_string(),
        "page".to_string(),
        Some(2),
    );
    let viewer = auth.viewer_of(&session).unwrap();
    assert_eq!(
        (
            viewer.platform,
            viewer.id.as_str(),
            viewer.name.as_str(),
            viewer.page_id.as_str(),
            viewer.tier
        ),
        (Platform::Twitch, "7", "Viewer", "page", Some(2))
    );
    assert_eq!(viewer.key(), "7");
    let kick = auth.viewer_session(
        Platform::Kick,
        "7".to_string(),
        "Viewer".to_string(),
        "page".to_string(),
        Some(1),
    );
    let kick = auth.viewer_of(&kick).unwrap();
    assert_eq!(kick.platform, Platform::Kick);
    assert_eq!(kick.key(), "kick:7");
    assert_eq!(auth.session_of(&session), None);
    assert_eq!(new_auth("other").viewer_of(&session), None);
    assert_eq!(auth.viewer_of(&auth.session("1", 0)), None);
    let expired = auth.sign(&format!(
        "{VIEWER_PREFIX}{}",
        serde_json::to_string(&Viewer {
            issued: now() - VIEWER_LIFETIME,
            ..viewer
        })
        .unwrap()
    ));
    assert_eq!(auth.viewer_of(&expired), None);
}

#[test]
fn link_tokens_carry_the_identity_for_ten_minutes() {
    let auth = new_auth("secret");
    let identity = Identity {
        platform: Platform::Kick,
        user_id: "123".to_string(),
        login: "erik".to_string(),
        name: "Erik".to_string(),
        avatar: Some("https://files.kick.com/erik.png".to_string()),
    };
    let token = auth.link_token(identity.clone());
    assert_eq!(auth.link_of(&token), Some(identity.clone()));
    assert_eq!(auth.session_of(&token), None);
    assert_eq!(auth.viewer_of(&token), None);
    assert_eq!(new_auth("other").link_of(&token), None);
    assert_eq!(auth.link_of(&auth.session("1", 0)), None);
    let expired = auth.sign(&format!(
        "{LINK_PREFIX}{}",
        serde_json::to_string(&Link {
            identity,
            issued: now() - LINK_LIFETIME,
        })
        .unwrap()
    ));
    assert_eq!(auth.link_of(&expired), None);
}

#[test]
fn state_cookie_is_secure_over_https() {
    assert_eq!(
        new_auth("secret").state_cookie("s"),
        "moblin_sign_in_state=s; Max-Age=600; HttpOnly; SameSite=Lax; Secure"
    );
    let local = Auth::new(
        Client::new(),
        "http://localhost:5173/api".to_string(),
        "http://localhost:5173".to_string(),
        None,
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        local.state_cookie("s"),
        "moblin_sign_in_state=s; Max-Age=600; HttpOnly; SameSite=Lax"
    );
}

#[test]
fn oauth_app_at_another_twitch() {
    let app = OAuthApp::at(
        "http://localhost:8081/",
        "http://localhost:8081",
        "id".to_string(),
        "secret".to_string(),
    );
    assert_eq!(app.authorize_url, "http://localhost:8081/oauth2/authorize");
    assert_eq!(app.token_url, "http://localhost:8081/oauth2/token");
    assert_eq!(app.users_url, "http://localhost:8081/helix/users");
    assert_eq!(
        app.subscriptions_url,
        "http://localhost:8081/helix/subscriptions/user"
    );
    assert_eq!(
        OAuthApp::twitch("id".to_string(), "secret".to_string()).authorize_url,
        "https://id.twitch.tv/oauth2/authorize"
    );
}

#[test]
fn kick_app_at_another_kick() {
    let app = KickApp::at(
        "http://localhost:8082/",
        "http://localhost:8082",
        "id".to_string(),
        "secret".to_string(),
    );
    assert_eq!(app.authorize_url, "http://localhost:8082/oauth/authorize");
    assert_eq!(app.token_url, "http://localhost:8082/oauth/token");
    assert_eq!(app.users_url, "http://localhost:8082/public/v1/users");
    assert_eq!(app.channels_url, "http://localhost:8082/public/v1/channels");
    assert_eq!(
        app.channel_users_url,
        "http://localhost:8082/api/v2/channels"
    );
    let app = KickApp::kick("id".to_string(), "secret".to_string());
    assert_eq!(app.authorize_url, "https://id.kick.com/oauth/authorize");
    assert_eq!(app.token_url, "https://id.kick.com/oauth/token");
    assert_eq!(app.users_url, "https://api.kick.com/public/v1/users");
    assert_eq!(app.channels_url, "https://api.kick.com/public/v1/channels");
    assert_eq!(app.channel_users_url, "https://kick.com/api/v2/channels");
}
