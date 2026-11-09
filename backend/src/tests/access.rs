use super::*;
use crate::accounts::Accounts;

fn account(accounts: &Accounts, identity: &Identity) {
    accounts
        .signed_in(
            identity,
            format!("account-{}", identity.user_id),
            format!("page-{}", identity.user_id),
            format!("key-{}", identity.user_id),
        )
        .unwrap();
}

fn keys(people: &[Person]) -> Vec<(Platform, &str, &str)> {
    people
        .iter()
        .map(|person| {
            (
                person.platform,
                person.user_id.as_str(),
                person.login.as_str(),
            )
        })
        .collect()
}

#[test]
fn everyone_may_sign_in_until_restricted() {
    let database = Database::memory();
    let access = SignInAccess::new(database.clone());
    assert!(!access.restricted());
    assert_eq!(access.state(), SignInState::default());
    assert!(!access.allows(&[Identity::twitch("1", "Erik", "erik")]));
}

#[test]
fn restricting_allows_the_existing_identities_and_survives_restarts() {
    let database = Database::memory();
    let accounts = Accounts::new(database.clone());
    let erik = Identity::twitch("1", "Erik", "erik");
    let kick = Identity::kick("2", "Kicker", "kicker");
    account(&accounts, &erik);
    account(&accounts, &kick);
    let access = SignInAccess::new(database.clone());
    access.set_restricted(true).unwrap();
    assert!(access.restricted());
    assert!(access.allows(std::slice::from_ref(&erik)));
    assert!(access.allows(&[Identity::twitch("9", "Bob", "bob"), kick.clone()]));
    assert!(!access.allows(&[Identity::twitch("2", "Twitch 2", "two")]));
    assert!(!access.allows(&[]));
    assert_eq!(
        keys(&access.state().allowed),
        [
            (Platform::Twitch, "1", "erik"),
            (Platform::Kick, "2", "kicker")
        ]
    );
    assert!(SignInAccess::new(database.clone()).restricted());
    access.remove(Platform::Twitch, "1").unwrap();
    access.set_restricted(false).unwrap();
    assert!(!SignInAccess::new(database.clone()).restricted());
    assert_eq!(access.state().allowed.len(), 1);
    access.set_restricted(true).unwrap();
    assert_eq!(
        keys(&access.state().allowed),
        [(Platform::Kick, "2", "kicker")]
    );
}

#[test]
fn turning_off_before_ever_restricting_changes_nothing() {
    let database = Database::memory();
    account(
        &Accounts::new(database.clone()),
        &Identity::twitch("1", "Erik", "erik"),
    );
    let access = SignInAccess::new(database);
    access.set_restricted(false).unwrap();
    assert!(!access.restricted());
    access.set_restricted(true).unwrap();
    assert_eq!(access.state().allowed.len(), 1);
}

#[test]
fn requests_are_allowed_or_dismissed() {
    let database = Database::memory();
    let accounts = Accounts::new(database.clone());
    let access = SignInAccess::new(database.clone());
    access.set_restricted(true).unwrap();
    let bob = Identity {
        avatar: Some("https://example.com/bob.png".to_string()),
        ..Identity::twitch("9", "Bob", "bob")
    };
    let alice = Identity::kick("7", "Alice", "alice");
    access.request(&bob).unwrap();
    access
        .request(&Identity::twitch("9", "Bobby", "bobby"))
        .unwrap();
    access.request(&alice).unwrap();
    assert_eq!(access.state().requests.len(), 2);
    assert!(access.allow(Platform::Twitch, "9").unwrap());
    let state = access.state();
    assert_eq!(keys(&state.requests), [(Platform::Kick, "7", "alice")]);
    assert_eq!(keys(&state.allowed), [(Platform::Twitch, "9", "bobby")]);
    assert_eq!(state.allowed[0].name, "Bobby");
    assert!(access.allows(std::slice::from_ref(&bob)));
    access.dismiss(Platform::Kick, "7").unwrap();
    assert!(access.state().requests.is_empty());
    assert!(!access.allows(&[alice]));
    assert!(!access.allow(Platform::Kick, "7").unwrap());
    let erik = Identity::twitch("1", "Erik", "erik");
    account(&accounts, &erik);
    assert!(!access.allows(std::slice::from_ref(&erik)));
    assert!(access.allow(Platform::Twitch, "1").unwrap());
    assert!(access.allows(&[erik]));
    access.remove(Platform::Twitch, "9").unwrap();
    assert!(!access.allows(&[bob]));
}

#[test]
fn only_the_newest_requests_are_kept() {
    let database = Database::memory();
    let access = SignInAccess::new(database.clone());
    for id in 0..105 {
        access
            .request(&Identity::twitch(&id.to_string(), "Someone", "someone"))
            .unwrap();
    }
    let requests = access.state().requests;
    assert_eq!(requests.len(), 100);
    assert!(requests.iter().all(|request| request.user_id != "0"));
    assert!(requests.iter().any(|request| request.user_id == "104"));
}
