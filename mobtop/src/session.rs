use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("mobtop").join("sessions.json"))
}

fn read_all() -> BTreeMap<String, String> {
    path()
        .and_then(|path| fs::read(path).ok())
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default()
}

pub fn read(backend_url: &str) -> Option<String> {
    read_all().remove(backend_url)
}

pub fn write(backend_url: &str, session: Option<&str>) -> std::io::Result<()> {
    let Some(path) = path() else {
        return Ok(());
    };
    let mut sessions = read_all();
    match session {
        Some(session) => sessions.insert(backend_url.to_string(), session.to_string()),
        None => sessions.remove(backend_url),
    };
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(path)?;
    #[cfg(unix)]
    file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    file.write_all(&serde_json::to_vec_pretty(&sessions)?)
}
