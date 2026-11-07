use super::*;
use crate::database;

fn sign_in(
    accounts: &Accounts,
    id: &str,
    name: &str,
    login: &str,
    page_id: &str,
    moblin_cloud_key: &str,
) -> Result<Account, AccountError> {
    accounts.signed_in(
        &Identity::twitch(id, name, login),
        id.to_string(),
        page_id.to_string(),
        moblin_cloud_key.to_string(),
    )
}

#[test]
fn page_ids_are_unique() {
    let accounts = Accounts::new(Database::memory());
    sign_in(&accounts, "1", "Erik", "erik", "p", "k").unwrap();
    assert_eq!(
        sign_in(&accounts, "2", "Other", "other", "p", "l"),
        Err(AccountError::NotSaved)
    );
    assert_eq!(accounts.get("2"), None);
    assert_eq!(accounts.of_page("p").unwrap().id, "1");
}

#[test]
fn pages_are_kept() {
    let accounts = Accounts::new(Database::memory());
    assert_eq!(accounts.get("1"), None);
    assert_eq!(
        sign_in(&accounts, "1", "Erik", "erik", "p", "k"),
        Ok(Account {
            id: "1".to_string(),
            identities: vec![Identity::twitch("1", "Erik", "erik")],
            page_id: "p".to_string(),
            moblin_cloud_key: "k".to_string(),
            ..Account::default()
        })
    );
    let account = accounts
        .signed_in(
            &Identity::twitch("1", "Eerimoq", "eerimoq"),
            "other".to_string(),
            "q".to_string(),
            "o".to_string(),
        )
        .unwrap();
    assert_eq!(account.id, "1");
    assert_eq!(account.page_id, "p");
    assert_eq!(account.moblin_cloud_key, "k");
    assert_eq!(accounts.get("1").unwrap().name(), "Eerimoq");
    assert_eq!(accounts.get("other"), None);
    assert_eq!(accounts.page_of_login(Platform::Twitch, "erik"), None);
    assert_eq!(
        accounts
            .page_of_login(Platform::Twitch, "eerimoq")
            .as_deref(),
        Some("p")
    );
}

#[test]
fn platforms_are_linked_and_unlinked() {
    let accounts = Accounts::new(Database::memory());
    assert_eq!(
        accounts.link("1", &Identity::kick("7", "Erik", "erik")),
        Err(AccountError::Unknown)
    );
    sign_in(&accounts, "1", "Erik", "erik", "p", "k").unwrap();
    let other = accounts
        .signed_in(
            &Identity::kick("8", "Other", "other"),
            "random".to_string(),
            "q".to_string(),
            "l".to_string(),
        )
        .unwrap();
    assert_eq!(other.id, "random");
    assert_eq!(accounts.get("random").unwrap().name(), "Other");
    assert_eq!(
        accounts.link("1", &Identity::kick("8", "Other", "other")),
        Err(AccountError::IdentityTaken(Platform::Kick))
    );
    assert_eq!(
        accounts.link("random", &Identity::twitch("1", "Erik", "erik")),
        Err(AccountError::IdentityTaken(Platform::Twitch))
    );
    let account = accounts
        .link("1", &Identity::kick("7", "Kick Erik", "erik"))
        .unwrap();
    assert_eq!(
        account.identities,
        [
            Identity::twitch("1", "Erik", "erik"),
            Identity::kick("7", "Kick Erik", "erik")
        ]
    );
    assert_eq!(account.name(), "Erik");
    assert_eq!(
        accounts.page_of_login(Platform::Kick, "erik").as_deref(),
        Some("p")
    );
    assert_eq!(
        accounts.page_of_login(Platform::Twitch, "other").as_deref(),
        None
    );
    let account = accounts
        .signed_in(
            &Identity::kick("7", "Kick Erik", "kickerik"),
            "unused".to_string(),
            "r".to_string(),
            "m".to_string(),
        )
        .unwrap();
    assert_eq!(account.id, "1");
    assert_eq!(account.identities[1].login, "kickerik");
    assert_eq!(accounts.get("unused"), None);
    assert_eq!(
        accounts.unlink("1", Platform::Twitch).unwrap().identities,
        [Identity::kick("7", "Kick Erik", "kickerik")]
    );
    assert_eq!(accounts.get("1").unwrap().name(), "Kick Erik");
    assert_eq!(
        accounts.unlink("1", Platform::Twitch),
        Err(AccountError::UnknownIdentity)
    );
    assert_eq!(
        accounts.unlink("1", Platform::Kick),
        Err(AccountError::LastIdentity)
    );
    assert_eq!(accounts.page_of_login(Platform::Twitch, "erik"), None);
    let account = accounts
        .signed_in(
            &Identity::twitch("1", "Erik", "erik"),
            "fresh".to_string(),
            "s".to_string(),
            "n".to_string(),
        )
        .unwrap();
    assert_eq!(account.id, "fresh");
    assert_eq!(
        accounts.page_of_login(Platform::Twitch, "erik").as_deref(),
        Some("s")
    );
    assert_eq!(
        accounts.link("1", &Identity::twitch("1", "Erik", "erik")),
        Err(AccountError::IdentityTaken(Platform::Twitch))
    );
    let account = accounts
        .link("1", &Identity::kick("7", "Again", "kickerik"))
        .unwrap();
    assert_eq!(account.identities.len(), 1);
    assert_eq!(account.name(), "Again");
}

#[test]
fn assistants_are_kept_per_platform() {
    let accounts = Accounts::new(Database::memory());
    let streamer = accounts
        .signed_in(
            &Identity::twitch("42", "Streamer", "streamer"),
            "s".to_string(),
            "p".to_string(),
            "k".to_string(),
        )
        .unwrap();
    sign_in(&accounts, "1", "Mod", "mod", "q", "l").unwrap();
    let assistant = Assistant {
        user_id: "1".to_string(),
        login: "mod".to_string(),
        name: "Mod".to_string(),
        avatar: None,
        preview: false,
        go_live: false,
        edit_tips: false,
        remote_control: Access::None,
        last_active: None,
        last_remote_control: None,
    };
    accounts
        .add_assistant(Platform::Twitch, "42", &assistant)
        .unwrap();
    assert_eq!(accounts.assisted(Platform::Twitch, "1")[0].page_id, "p");
    assert!(accounts.assists(Platform::Twitch, "1", "42"));
    assert!(!accounts.assists(Platform::Kick, "1", "42"));
    assert!(accounts.assisted(Platform::Kick, "1").is_empty());
    assert_eq!(accounts.assistants(Platform::Twitch, "42"), [assistant]);
    assert!(accounts.assistants(Platform::Kick, "42").is_empty());
    assert_eq!(
        accounts.remove_assistant(Platform::Kick, "42", "1"),
        Err(AccountError::UnknownAssistant)
    );
    accounts
        .link("s", &Identity::kick("9", "Streamer", "streamer"))
        .unwrap();
    accounts.unlink("s", Platform::Twitch).unwrap();
    assert!(accounts.assisted(Platform::Twitch, "1").is_empty());
    assert_eq!(accounts.get(&streamer.id).unwrap().identities.len(), 1);
    accounts
        .link("s", &Identity::twitch("42", "Streamer", "streamer"))
        .unwrap();
    assert_eq!(accounts.assisted(Platform::Twitch, "1")[0].page_id, "p");
    accounts
        .remove_assistant(Platform::Twitch, "42", "1")
        .unwrap();
    assert!(!accounts.assists(Platform::Twitch, "1", "42"));
    assert!(accounts.assisted(Platform::Twitch, "1").is_empty());
}

#[test]
fn assistants_are_an_allow_list() {
    let accounts = Accounts::new(Database::memory());
    let assistant = |id: &str, login: &str| Assistant {
        user_id: id.to_string(),
        login: login.to_string(),
        name: login.to_string(),
        avatar: None,
        preview: false,
        go_live: false,
        edit_tips: false,
        remote_control: Access::None,
        last_active: None,
        last_remote_control: None,
    };
    accounts
        .add_assistant(Platform::Kick, "42", &assistant("1", "erik"))
        .unwrap();
    accounts
        .add_assistant(Platform::Kick, "42", &assistant("2", "bob"))
        .unwrap();
    accounts
        .add_assistant(Platform::Kick, "43", &assistant("1", "erik"))
        .unwrap();
    assert_eq!(
        accounts.assistants(Platform::Kick, "42"),
        [assistant("2", "bob"), assistant("1", "erik")]
    );
    assert!(accounts.assists(Platform::Kick, "1", "42"));
    assert!(accounts.assists(Platform::Kick, "1", "43"));
    assert!(!accounts.assists(Platform::Kick, "2", "43"));
    assert!(!accounts.assists(Platform::Kick, "42", "1"));
    assert_eq!(
        accounts.remove_assistant(Platform::Kick, "42", "3"),
        Err(AccountError::UnknownAssistant)
    );
    accounts
        .remove_assistant(Platform::Kick, "42", "1")
        .unwrap();
    assert!(!accounts.assists(Platform::Kick, "1", "42"));
    assert!(accounts.assists(Platform::Kick, "1", "43"));
    assert_eq!(
        accounts.assistants(Platform::Kick, "42"),
        [assistant("2", "bob")]
    );
    for n in 0..49 {
        accounts
            .add_assistant(
                Platform::Kick,
                "42",
                &assistant(&format!("{}", 100 + n), "m"),
            )
            .unwrap();
    }
    assert_eq!(accounts.assistants(Platform::Kick, "42").len(), 50);
    assert_eq!(
        accounts.add_assistant(Platform::Kick, "42", &assistant("200", "late")),
        Err(AccountError::TooManyAssistants)
    );
    accounts
        .add_assistant(Platform::Kick, "42", &assistant("2", "robert"))
        .unwrap();
    assert_eq!(accounts.assistants(Platform::Kick, "42").len(), 50);
    assert!(
        accounts
            .assistants(Platform::Kick, "42")
            .contains(&assistant("2", "robert"))
    );
}

#[test]
fn assistants_have_permissions_only_when_allowed() {
    let accounts = Accounts::new(Database::memory());
    let assistant = Assistant {
        user_id: "1".to_string(),
        login: "mod".to_string(),
        name: "Mod".to_string(),
        avatar: None,
        preview: false,
        go_live: false,
        edit_tips: false,
        remote_control: Access::None,
        last_active: None,
        last_remote_control: None,
    };
    accounts
        .add_assistant(Platform::Twitch, "42", &assistant)
        .unwrap();
    for permission in [
        Permission::Preview,
        Permission::GoLive,
        Permission::EditTips,
    ] {
        assert!(!accounts.permits(Platform::Twitch, "1", "42", permission));
        assert_eq!(
            accounts.set_assistant_permission(Platform::Twitch, "42", "2", permission, true),
            Err(AccountError::UnknownAssistant)
        );
        assert_eq!(
            accounts.set_assistant_permission(Platform::Kick, "42", "1", permission, true),
            Err(AccountError::UnknownAssistant)
        );
    }
    accounts
        .set_assistant_permission(Platform::Twitch, "42", "1", Permission::Preview, true)
        .unwrap();
    assert!(accounts.permits(Platform::Twitch, "1", "42", Permission::Preview));
    assert!(!accounts.permits(Platform::Twitch, "1", "42", Permission::GoLive));
    assert!(!accounts.permits(Platform::Kick, "1", "42", Permission::Preview));
    assert!(!accounts.permits(Platform::Twitch, "1", "43", Permission::Preview));
    accounts
        .set_assistant_permission(Platform::Twitch, "42", "1", Permission::GoLive, true)
        .unwrap();
    assert!(accounts.permits(Platform::Twitch, "1", "42", Permission::GoLive));
    let shown = Assistant {
        preview: true,
        go_live: true,
        ..assistant.clone()
    };
    assert_eq!(
        accounts.assistants(Platform::Twitch, "42"),
        std::slice::from_ref(&shown)
    );
    accounts
        .add_assistant(Platform::Twitch, "42", &assistant)
        .unwrap();
    assert_eq!(accounts.assistants(Platform::Twitch, "42"), [shown]);
    accounts
        .set_assistant_permission(Platform::Twitch, "42", "1", Permission::Preview, false)
        .unwrap();
    assert!(!accounts.permits(Platform::Twitch, "1", "42", Permission::Preview));
    assert!(accounts.permits(Platform::Twitch, "1", "42", Permission::GoLive));
    accounts
        .set_assistant_permission(Platform::Twitch, "42", "1", Permission::GoLive, false)
        .unwrap();
    assert!(!accounts.permits(Platform::Twitch, "1", "42", Permission::GoLive));
    assert!(accounts.assists(Platform::Twitch, "1", "42"));
}

#[test]
fn assistants_get_remote_control_access_only_when_allowed() {
    let accounts = Accounts::new(Database::memory());
    let assistant = Assistant {
        user_id: "1".to_string(),
        login: "mod".to_string(),
        name: "Mod".to_string(),
        avatar: None,
        preview: false,
        go_live: false,
        edit_tips: false,
        remote_control: Access::None,
        last_active: None,
        last_remote_control: None,
    };
    accounts
        .add_assistant(Platform::Twitch, "42", &assistant)
        .unwrap();
    let granted = |access| accounts.grants(Platform::Twitch, "1", "42", access);
    assert!(granted(Access::None));
    assert!(!granted(Access::View));
    assert_eq!(
        accounts.set_assistant_access(Platform::Twitch, "42", "2", Access::View),
        Err(AccountError::UnknownAssistant)
    );
    accounts
        .set_assistant_access(Platform::Twitch, "42", "1", Access::View)
        .unwrap();
    assert!(granted(Access::View));
    assert!(!granted(Access::Control));
    accounts
        .set_assistant_access(Platform::Twitch, "42", "1", Access::Control)
        .unwrap();
    assert!(granted(Access::View));
    assert!(granted(Access::Control));
    assert!(!accounts.grants(Platform::Kick, "1", "42", Access::View));
    accounts
        .add_assistant(Platform::Twitch, "42", &assistant)
        .unwrap();
    assert_eq!(
        accounts.assistants(Platform::Twitch, "42")[0].remote_control,
        Access::Control
    );
    accounts
        .set_assistant_access(Platform::Twitch, "42", "1", Access::None)
        .unwrap();
    assert!(!granted(Access::View));
}

#[test]
fn kick_assistants_keep_access_when_renamed_and_follow_the_kick_identity() {
    let accounts = Accounts::new(Database::memory());
    let streamer = accounts
        .signed_in(
            &Identity::kick("42", "Streamer", "streamer"),
            "s".to_string(),
            "p".to_string(),
            "k".to_string(),
        )
        .unwrap();
    accounts
        .add_assistant(
            Platform::Kick,
            "42",
            &Assistant {
                user_id: "1".to_string(),
                login: "bob".to_string(),
                name: "bob".to_string(),
                avatar: None,
                preview: false,
                go_live: false,
                edit_tips: false,
                remote_control: Access::None,
                last_active: None,
                last_remote_control: None,
            },
        )
        .unwrap();
    assert_eq!(accounts.assisted(Platform::Kick, "1")[0].page_id, "p");
    assert!(accounts.assists(Platform::Kick, "1", "42"));
    accounts
        .signed_in(
            &Identity::kick("1", "Robert", "robert"),
            "m".to_string(),
            "q".to_string(),
            "l".to_string(),
        )
        .unwrap();
    assert!(accounts.assists(Platform::Kick, "1", "42"));
    assert_eq!(accounts.assisted(Platform::Kick, "1")[0].page_id, "p");
    assert_eq!(
        accounts.assistants(Platform::Kick, "42"),
        [Assistant {
            user_id: "1".to_string(),
            login: "robert".to_string(),
            name: "Robert".to_string(),
            avatar: None,
            preview: false,
            go_live: false,
            edit_tips: false,
            remote_control: Access::None,
            last_active: None,
            last_remote_control: None,
        }]
    );
    assert!(accounts.assisted(Platform::Kick, "2").is_empty());
    accounts
        .link("s", &Identity::twitch("7", "Streamer", "streamer"))
        .unwrap();
    accounts.unlink("s", Platform::Kick).unwrap();
    assert!(accounts.assisted(Platform::Kick, "1").is_empty());
    assert_eq!(accounts.get(&streamer.id).unwrap().identities.len(), 1);
    accounts
        .link("s", &Identity::kick("42", "Streamer", "streamer"))
        .unwrap();
    assert_eq!(accounts.assisted(Platform::Kick, "1")[0].page_id, "p");
    accounts.delete("m").unwrap();
    assert!(accounts.assistants(Platform::Kick, "42").is_empty());
    accounts
        .add_assistant(
            Platform::Kick,
            "42",
            &Assistant {
                user_id: "1".to_string(),
                login: "robert".to_string(),
                name: "Robert".to_string(),
                avatar: None,
                preview: false,
                go_live: false,
                edit_tips: false,
                remote_control: Access::None,
                last_active: None,
                last_remote_control: None,
            },
        )
        .unwrap();
    accounts.delete("s").unwrap();
    assert!(accounts.assistants(Platform::Kick, "42").is_empty());
    assert!(!accounts.assists(Platform::Kick, "1", "42"));
}

#[test]
fn pages_find_their_account() {
    let accounts = Accounts::new(Database::memory());
    accounts
        .signed_in(
            &Identity {
                avatar: Some("https://example.com/a.png".to_string()),
                ..Identity::twitch("1", "Erik", "erik")
            },
            "1".to_string(),
            "p".to_string(),
            "k".to_string(),
        )
        .unwrap();
    assert_eq!(accounts.of_page("q"), None);
    let account = accounts.of_page("p").unwrap();
    assert_eq!(account.avatar(), Some("https://example.com/a.png"));
    assert_eq!(
        account.twitch().unwrap().avatar.as_deref(),
        Some("https://example.com/a.png")
    );
    sign_in(&accounts, "1", "Erik", "erik", "p", "k").unwrap();
    assert_eq!(accounts.of_page("p").unwrap().avatar(), None);
}

#[test]
fn keys_are_replaced() {
    let accounts = Accounts::new(Database::memory());
    sign_in(&accounts, "1", "Erik", "erik", "p", "k").unwrap();
    assert_eq!(accounts.page_of_moblin_cloud_key("k").as_deref(), Some("p"));
    assert_eq!(
        accounts.replace_moblin_cloud_key("2", "n".to_string()),
        Err(AccountError::Unknown)
    );
    let account = accounts
        .replace_moblin_cloud_key("1", "n".to_string())
        .unwrap();
    assert_eq!(account.page_id, "p");
    assert_eq!(account.moblin_cloud_key, "n");
    assert_eq!(accounts.page_of_moblin_cloud_key("k"), None);
    assert_eq!(accounts.page_of_moblin_cloud_key("n").as_deref(), Some("p"));
}

#[test]
fn at_most_20_devices_are_kept_per_account() {
    let accounts = Accounts::new(Database::memory());
    sign_in(&accounts, "1", "Erik", "erik", "p", "k").unwrap();
    for device in 0..25 {
        accounts.seen_device("p", &device.to_string()).unwrap();
    }
    let names: HashMap<String, String> = accounts
        .known_devices("p")
        .into_iter()
        .map(|device| (device.id, device.name))
        .collect();
    assert_eq!(names.len(), 20);
    assert!((0..5).all(|device| !names.contains_key(&device.to_string())));
    assert_eq!(names["5"], "Moblin 6");
    assert_eq!(names["20"], "Moblin 1");
    assert_eq!(names["24"], "Moblin 5");
}

#[test]
fn known_devices_are_listed_last_seen_first() {
    let accounts = Accounts::new(Database::memory());
    sign_in(&accounts, "1", "Erik", "erik", "p", "k").unwrap();
    accounts.seen_device("p", "a").unwrap();
    accounts.seen_device("p", "b").unwrap();
    accounts.left_device("q", "a");
    let ids = |accounts: &Accounts| {
        accounts
            .known_devices("p")
            .into_iter()
            .map(|device| device.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(&accounts), ["b", "a"]);
    accounts
        .database
        .lock()
        .execute("UPDATE devices SET last_seen = 0", [])
        .unwrap();
    accounts.left_device("p", "a");
    assert_eq!(ids(&accounts), ["a", "b"]);
    assert_eq!(accounts.known_devices("p")[0].name, "Moblin 1");
    assert_eq!(accounts.known_devices("p")[1].last_seen, 0);
}

#[test]
fn devices_get_a_name_when_first_seen() {
    let accounts = Accounts::new(Database::memory());
    sign_in(&accounts, "1", "Erik", "erik", "p", "k").unwrap();
    sign_in(&accounts, "2", "Other", "other", "q", "l").unwrap();
    assert_eq!(accounts.seen_device("r", "a"), None);
    let seen = |page_id: &str, device: &str| accounts.seen_device(page_id, device).unwrap();
    assert_eq!(seen("p", "a"), "Moblin 1");
    assert_eq!(seen("p", "b"), "Moblin 2");
    assert_eq!(seen("q", "a"), "Moblin 1");
    assert_eq!(seen("p", "a"), "Moblin 1");
    accounts
        .database
        .lock()
        .execute(
            "UPDATE devices SET last_seen = 0 WHERE account = '1' AND id = 'a'",
            [],
        )
        .unwrap();
    assert_eq!(seen("p", "c"), "Moblin 1");
    assert_eq!(seen("p", "a"), "Moblin 3");
    assert_eq!(
        accounts.rename_device("p", "d", "Backup"),
        Err(AccountError::UnknownDevice)
    );
    accounts.rename_device("p", "b", "Backup").unwrap();
    accounts.rename_device("q", "a", "Other").unwrap();
    assert_eq!(
        accounts.remove_device("p", "x"),
        Err(AccountError::UnknownDevice)
    );
    accounts.seen_device("p", "x").unwrap();
    accounts.remove_device("p", "x").unwrap();
    assert_eq!(seen("p", "d"), "Moblin 2");
    assert_eq!(
        accounts
            .known_devices("p")
            .into_iter()
            .map(|device| (device.id, device.name))
            .collect::<HashMap<_, _>>(),
        HashMap::from([
            ("a".to_string(), "Moblin 3".to_string()),
            ("b".to_string(), "Backup".to_string()),
            ("c".to_string(), "Moblin 1".to_string()),
            ("d".to_string(), "Moblin 2".to_string())
        ])
    );
}

#[test]
fn signing_out_ends_sessions() {
    let accounts = Accounts::new(Database::memory());
    assert_eq!(accounts.sign_out("1"), Err(AccountError::Unknown));
    sign_in(&accounts, "1", "Erik", "erik", "p", "k").unwrap();
    assert_eq!(accounts.sign_out("1").unwrap().sessions, 1);
    assert_eq!(
        sign_in(&accounts, "1", "Erik", "erik", "q", "o")
            .unwrap()
            .sessions,
        1
    );
}

#[test]
fn paypal_connects_only_the_latest_onboarding() {
    let accounts = Accounts::new(Database::memory());
    assert_eq!(
        accounts.start_paypal("1", "a".to_string()),
        Err(AccountError::Unknown)
    );
    sign_in(&accounts, "1", "Erik", "erik", "p", "k").unwrap();
    accounts.start_paypal("1", "a".to_string()).unwrap();
    accounts.start_paypal("1", "b".to_string()).unwrap();
    let account = accounts
        .connect_paypal("1", "a", "MERCHANT00001".to_string())
        .unwrap();
    assert_eq!(account.merchant_id, None);
    let account = accounts
        .connect_paypal("1", "b", "MERCHANT00002".to_string())
        .unwrap();
    assert_eq!(account.merchant_id.as_deref(), Some("MERCHANT00002"));
    assert_eq!(account.paypal_tracking_id, None);
    accounts.start_paypal("1", "c".to_string()).unwrap();
    assert_eq!(
        accounts.get("1").unwrap().merchant_id.as_deref(),
        Some("MERCHANT00002")
    );
    assert_eq!(accounts.of_paypal_tracking_id("c").as_deref(), Some("1"));
    assert_eq!(accounts.of_paypal_tracking_id("b"), None);
    assert!(accounts.disconnect_paypal("MERCHANT00001").is_empty());
    assert_eq!(accounts.disconnect_paypal("MERCHANT00002"), ["1"]);
    assert_eq!(accounts.get("1").unwrap().merchant_id, None);
    accounts.start_paypal("1", "d".to_string()).unwrap();
    accounts
        .connect_paypal("1", "d", "MERCHANT00003".to_string())
        .unwrap();
    accounts.start_paypal("1", "e".to_string()).unwrap();
    let account = accounts.forget_paypal("1").unwrap();
    assert_eq!(account.merchant_id, None);
    assert_eq!(account.paypal_tracking_id, None);
    assert_eq!(accounts.forget_paypal("2"), Err(AccountError::Unknown));
}

#[test]
fn stripe_is_ready_only_for_the_current_account() {
    let accounts = Accounts::new(Database::memory());
    assert_eq!(
        accounts.start_stripe("1", "acct_1".to_string()),
        Err(AccountError::Unknown)
    );
    sign_in(&accounts, "1", "Erik", "erik", "p", "k").unwrap();
    accounts.start_stripe("1", "acct_1".to_string()).unwrap();
    assert!(
        !accounts
            .stripe_ready("1", "acct_2", true)
            .unwrap()
            .stripe_ready
    );
    assert!(
        accounts
            .stripe_ready("1", "acct_1", true)
            .unwrap()
            .stripe_ready
    );
    assert!(
        accounts
            .start_stripe("1", "acct_1".to_string())
            .unwrap()
            .stripe_ready
    );
    let account = accounts.start_stripe("1", "acct_2".to_string()).unwrap();
    assert_eq!(account.stripe_account_id.as_deref(), Some("acct_2"));
    assert!(!account.stripe_ready);
    accounts.stripe_ready("1", "acct_2", true).unwrap();
    let account = accounts.forget_stripe("1").unwrap();
    assert_eq!(account.stripe_account_id, None);
    assert!(!account.stripe_ready);
}

#[test]
fn logins_find_pages() {
    let accounts = Accounts::new(Database::memory());
    assert_eq!(accounts.page_of_login(Platform::Twitch, "erik"), None);
    sign_in(&accounts, "1", "Erik", "erik", "a", "k").unwrap();
    assert_eq!(
        accounts.page_of_login(Platform::Twitch, "ERIK").as_deref(),
        Some("a")
    );
    assert_eq!(accounts.page_of_login(Platform::Kick, "erik"), None);
    assert_eq!(accounts.page_of_login(Platform::Twitch, "other"), None);
    assert_eq!(accounts.page_of_login(Platform::Twitch, ""), None);
    sign_in(&accounts, "2", "Erik", "erik", "c", "l").unwrap();
    assert_eq!(
        accounts.page_of_login(Platform::Twitch, "erik").as_deref(),
        Some("c")
    );
    assert_eq!(accounts.get("1").unwrap().twitch().unwrap().login, "");
    accounts
        .signed_in(
            &Identity::kick("5", "Erik", "erik"),
            "3".to_string(),
            "d".to_string(),
            "m".to_string(),
        )
        .unwrap();
    assert_eq!(
        accounts.page_of_login(Platform::Kick, "erik").as_deref(),
        Some("d")
    );
    assert_eq!(
        accounts.page_of_login(Platform::Twitch, "erik").as_deref(),
        Some("c")
    );
}

#[test]
fn premium_accounts_are_counted() {
    let accounts = Accounts::new(Database::memory());
    assert_eq!(accounts.stats(&["twitch:1"]).premium, 0);
    sign_in(&accounts, "1", "Erik", "erik", "p", "k").unwrap();
    sign_in(&accounts, "2", "Other", "other", "q", "l").unwrap();
    accounts
        .signed_in(
            &Identity::kick("7", "Kicker", "kicker"),
            "random".to_string(),
            "r".to_string(),
            "m".to_string(),
        )
        .unwrap();
    let stats = accounts.stats(&["twitch:1", "twitch:1", "twitch:3"]);
    assert_eq!((stats.total, stats.premium), (3, 1));
    assert_eq!(accounts.stats(&[]).premium, 0);
    assert_eq!(accounts.stats(&["1"]).premium, 0);
    assert_eq!(accounts.stats(&["7"]).premium, 0);
    assert_eq!(accounts.stats(&["twitch:7"]).premium, 0);
    assert_eq!(accounts.stats(&["kick:7"]).premium, 1);
    accounts
        .link("1", &Identity::kick("8", "Erik", "erik"))
        .unwrap();
    assert_eq!(accounts.stats(&["twitch:1", "kick:8"]).premium, 1);
    assert_eq!(accounts.stats(&["kick:8", "kick:7"]).premium, 2);
}

#[test]
fn admin_keys_are_prefixed() {
    assert_eq!(admin_key_of("123"), Ok("twitch:123".to_string()));
    assert_eq!(admin_key_of(" twitch:123 "), Ok("twitch:123".to_string()));
    assert_eq!(admin_key_of("kick:7"), Ok("kick:7".to_string()));
    assert!(admin_key_of("kik:7").is_err());
    assert!(admin_key_of("kick:").is_err());
    assert_eq!(admin_key_of(" "), Ok(String::new()));
    assert_eq!(
        Identity::twitch("123", "Erik", "erik").admin_key(),
        "twitch:123"
    );
    assert_eq!(
        Identity::kick("7", "Kicker", "kicker").admin_key(),
        "kick:7"
    );
}

#[test]
fn accounts_are_saved() {
    let path = database::tests::temporary_path("accounts");
    let accounts = Accounts::new(Database::open(Some(&path)).unwrap());
    let id = "0123456789abcdef0123456789abcdef";
    accounts
        .signed_in(
            &Identity::twitch("3", "erik", "erik"),
            id.to_string(),
            "p".to_string(),
            "k".to_string(),
        )
        .unwrap();
    let accounts = Accounts::new(Database::open(Some(&path)).unwrap());
    assert_eq!(
        accounts.get(id),
        Some(Account {
            id: id.to_string(),
            identities: vec![Identity::twitch("3", "erik", "erik")],
            page_id: "p".to_string(),
            moblin_cloud_key: "k".to_string(),
            ..Account::default()
        })
    );
    database::tests::remove(&path);
}

impl Identity {
    pub fn twitch(id: &str, name: &str, login: &str) -> Self {
        Self {
            platform: Platform::Twitch,
            user_id: id.to_string(),
            login: login.to_string(),
            name: name.to_string(),
            avatar: None,
        }
    }

    pub fn kick(id: &str, name: &str, login: &str) -> Self {
        Self {
            platform: Platform::Kick,
            ..Self::twitch(id, name, login)
        }
    }
}

impl Accounts {
    pub fn assistants(&self, platform: Platform, broadcaster: &str) -> Vec<Assistant> {
        let (twitch, kick) = self.assistants_of(match platform {
            Platform::Twitch => [Some(broadcaster), None],
            Platform::Kick => [None, Some(broadcaster)],
        });
        match platform {
            Platform::Twitch => twitch,
            Platform::Kick => kick,
        }
    }

    pub fn assists(&self, platform: Platform, assistant: &str, broadcaster: &str) -> bool {
        self.rights(platform, assistant, broadcaster).is_some()
    }

    pub fn permits(
        &self,
        platform: Platform,
        assistant: &str,
        broadcaster: &str,
        permission: Permission,
    ) -> bool {
        self.rights(platform, assistant, broadcaster)
            .is_some_and(|rights| rights.permits(permission))
    }

    pub fn grants(
        &self,
        platform: Platform,
        assistant: &str,
        broadcaster: &str,
        access: Access,
    ) -> bool {
        self.rights(platform, assistant, broadcaster)
            .is_some_and(|rights| rights.remote_control >= access)
    }

    pub fn assisted(&self, platform: Platform, assistant: &str) -> Vec<Account> {
        self.assisted_by(match platform {
            Platform::Twitch => [Some(assistant), None],
            Platform::Kick => [None, Some(assistant)],
        })
        .into_iter()
        .map(|(_, account)| account)
        .collect()
    }
}
