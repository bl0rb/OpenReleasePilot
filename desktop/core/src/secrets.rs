//! The SFTP password (or key passphrase) in the system keychain, never in a file.

use crate::config::SftpConfig;
use crate::paths::{app_name, APP_NAME, LEGACY_APP_NAME};
use anyhow::{Context, Result};

fn entry(sftp: &SftpConfig) -> Result<keyring::Entry> {
    keyring::Entry::new(app_name(), &sftp.secret_account()).context("Schlüsselbund nicht verfügbar")
}

/// Where the password was kept before: by OpenReleasePilot (before OpenSitePilot took the website
/// over) and by the app under its old name.
fn earlier(sftp: &SftpConfig) -> impl Iterator<Item = keyring::Entry> + '_ {
    [APP_NAME, LEGACY_APP_NAME]
        .into_iter()
        .filter(|name| *name != app_name())
        .filter_map(|name| keyring::Entry::new(name, &sftp.secret_account()).ok())
}

pub fn get(sftp: &SftpConfig) -> Option<String> {
    if let Ok(secret) = entry(sftp).ok()?.get_password() {
        return Some(secret);
    }
    // Saved under another app name: move it over once.
    let (old, secret) = earlier(sftp).find_map(|old| old.get_password().ok().map(|secret| (old, secret)))?;
    if set(sftp, &secret).is_ok() {
        let _ = old.delete_credential();
    }
    Some(secret)
}

pub fn set(sftp: &SftpConfig, secret: &str) -> Result<()> {
    entry(sftp)?.set_password(secret).context("Passwort konnte nicht im Schlüsselbund gespeichert werden")
}

pub fn delete(sftp: &SftpConfig) -> Result<()> {
    // Otherwise `get` would bring a not yet moved entry back.
    for old in earlier(sftp) {
        let _ = old.delete_credential();
    }
    match entry(sftp)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e).context("Passwort konnte nicht entfernt werden"),
    }
}
