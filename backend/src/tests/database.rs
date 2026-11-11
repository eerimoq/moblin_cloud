use super::*;

pub fn temporary_path(name: &str) -> std::path::PathBuf {
    let path =
        std::env::temp_dir().join(format!("moblin-cloud-{name}-{}.sqlite", std::process::id()));
    remove(&path);
    path
}

pub fn remove(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

#[test]
fn files_are_the_files_on_disk() {
    assert!(Database::memory().files().is_empty());
    let path = temporary_path("size");
    let database = Database::open(Some(&path)).unwrap();
    database
        .lock()
        .execute("INSERT INTO pages (id, settings) VALUES ('a', '{}')", [])
        .unwrap();
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    let expected: Vec<(String, u64)> = ["", "-wal", "-shm"]
        .into_iter()
        .filter_map(|suffix| {
            let metadata = std::fs::metadata(format!("{}{suffix}", path.display())).ok()?;
            Some((format!("{name}{suffix}"), metadata.len()))
        })
        .collect();
    assert!(expected.len() > 1);
    assert_eq!(database.files(), expected);
    drop(database);
    remove(&path);
}

#[test]
fn the_old_page_id_index_is_dropped() {
    let path = temporary_path("index");
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TABLE accounts (id TEXT PRIMARY KEY, page_id TEXT, moblin_key TEXT, sessions INTEGER NOT NULL DEFAULT 0, merchant_id TEXT, paypal_tracking_id TEXT, stripe_account_id TEXT, stripe_ready INTEGER NOT NULL DEFAULT 0);
             CREATE INDEX accounts_page_id ON accounts (page_id);
             INSERT INTO accounts (id, page_id) VALUES ('1', 'p'), ('2', 'p');",
        )
        .unwrap();
    assert!(Database::open(Some(&path)).is_err());
    Connection::open(&path)
        .unwrap()
        .execute("DELETE FROM accounts WHERE id = '2'", [])
        .unwrap();
    let database = Database::open(Some(&path)).unwrap();
    let indexes: Vec<String> = database
        .lock()
        .prepare("SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'accounts' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(
        indexes,
        [
            "accounts_moblin_key",
            "accounts_page_id_unique",
            "sqlite_autoindex_accounts_1"
        ]
    );
    remove(&path);
}

#[test]
fn accounts_get_the_premium_column() {
    let path = temporary_path("premium");
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TABLE accounts (id TEXT PRIMARY KEY, page_id TEXT, moblin_key TEXT, sessions INTEGER NOT NULL, merchant_id TEXT, paypal_tracking_id TEXT, stripe_account_id TEXT, stripe_ready INTEGER NOT NULL DEFAULT 0);
             INSERT INTO accounts (id, page_id, moblin_key, sessions) VALUES ('1', 'p', 'k', 0);",
        )
        .unwrap();
    for _ in 0..2 {
        let database = Database::open(Some(&path)).unwrap();
        let premium: bool = database
            .lock()
            .query_row("SELECT premium FROM accounts WHERE id = '1'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(!premium);
    }
    remove(&path);
}

#[test]
fn assistants_get_the_secrets_column() {
    let path = temporary_path("secrets");
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TABLE twitch_assistants (broadcaster TEXT NOT NULL, assistant TEXT NOT NULL, login TEXT NOT NULL, name TEXT NOT NULL, preview INTEGER NOT NULL DEFAULT 0, go_live INTEGER NOT NULL DEFAULT 0, edit_tips INTEGER NOT NULL DEFAULT 0, remote_control INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (broadcaster, assistant));
             CREATE TABLE kick_assistants (broadcaster TEXT NOT NULL, assistant TEXT NOT NULL, login TEXT NOT NULL, name TEXT NOT NULL, preview INTEGER NOT NULL DEFAULT 0, go_live INTEGER NOT NULL DEFAULT 0, edit_tips INTEGER NOT NULL DEFAULT 0, remote_control INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (broadcaster, assistant));
             INSERT INTO twitch_assistants (broadcaster, assistant, login, name) VALUES ('1', '2', 'bob', 'Bob');
             INSERT INTO kick_assistants (broadcaster, assistant, login, name) VALUES ('1', '2', 'bob', 'Bob');",
        )
        .unwrap();
    for _ in 0..2 {
        let database = Database::open(Some(&path)).unwrap();
        for table in ["twitch_assistants", "kick_assistants"] {
            let secrets: bool = database
                .lock()
                .query_row(
                    &format!("SELECT secrets FROM {table} WHERE assistant = '2'"),
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(!secrets);
        }
    }
    remove(&path);
}

impl Database {
    pub fn memory() -> Arc<Self> {
        Self::open(None).unwrap()
    }
}
