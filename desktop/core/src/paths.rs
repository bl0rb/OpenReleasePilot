use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The repository app; the default identity of a process.
pub const APP_NAME: &str = "OpenReleasePilot";
/// The website app: build, upload, live check.
pub const SITE_APP_NAME: &str = "OpenSitePilot";
/// Name before the rename; its data folder and keychain entries are taken over.
pub const LEGACY_APP_NAME: &str = "OpenFTPUpdater-AI";

static APP: OnceLock<&'static str> = OnceLock::new();

/// Sets which app this process is, once at start, before anything reads a path; the default is
/// OpenReleasePilot. Decides the data folder, keychain service and update package.
pub fn set_app(name: &'static str) {
    let _ = APP.set(name);
}

pub fn app_name() -> &'static str {
    APP.get().copied().unwrap_or(APP_NAME)
}

/// `~/Library/Application Support/<app>` on macOS, `%APPDATA%\<app>` on Windows.
pub fn data_dir() -> PathBuf {
    data_dir_of(app_name())
}

pub fn data_dir_of(app: &str) -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join(app)
}

/// Takes over the data folder of the app under its old name (settings, upload manifests), once.
pub fn migrate_legacy_data() {
    let Some(base) = dirs::data_dir() else { return };
    let (old, new) = (base.join(LEGACY_APP_NAME), base.join(APP_NAME));
    if old.is_dir() && !new.exists() {
        let _ = std::fs::rename(old, new);
    }
}

pub fn config_file() -> PathBuf {
    data_dir().join("config.json")
}

/// One manifest per upload target, see `manifest::last_upload_file`.
pub fn uploads_dir() -> PathBuf {
    data_dir().join("uploads")
}

/// `/Users/me/Code/site` → `~/Code/site` for display.
pub fn abbreviate_home(path: &Path) -> String {
    match dirs::home_dir() {
        Some(home) if path.starts_with(&home) => format!("~/{}", path.strip_prefix(&home).unwrap_or(path).display()),
        _ => path.display().to_string(),
    }
}

/// Expands a leading `~/`.
pub fn expand_home(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), dirs::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(path),
    }
}

/// The app's own notes about one managed repository (profile, memory); never inside the repository.
/// The key is encoded, so `a/b-c` and `a-b/c` stay apart.
pub fn repo_dir(key: &str) -> PathBuf {
    let name: String = key.chars().map(|c| match c {
        '%' => "%25".to_owned(),
        '/' => "%2F".to_owned(),
        ':' => "%3A".to_owned(),
        '\\' => "%5C".to_owned(),
        c => c.to_string(),
    }).collect();
    data_dir().join("repos").join(name)
}
