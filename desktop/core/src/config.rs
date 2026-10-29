//! The app's own settings. Never contains secrets: the SFTP password lives in the keychain (`secrets`).

use crate::paths;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum AuthKind {
    #[default]
    Password,
    /// Private key file; a passphrase, if any, is stored like a password.
    Key,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SftpConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    /// Folder on the server that receives the build output, e.g. `/example.com`.
    pub remote_dir: String,
    pub auth: AuthKind,
    pub key_path: String,
    /// Trusted host key as `SHA256:<base64>`; empty until the user trusted the server once.
    pub host_key: String,
    /// Whether a password/passphrase is in the keychain. Kept here so that showing the
    /// window never reads the keychain (which can ask the user for permission).
    pub secret_saved: bool,
}

impl Default for SftpConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 22,
            user: String::new(),
            remote_dir: String::new(),
            auth: AuthKind::Password,
            key_path: String::new(),
            host_key: String::new(),
            secret_saved: false,
        }
    }
}

impl SftpConfig {
    pub fn is_complete(&self) -> bool {
        !self.host.trim().is_empty()
            && !self.user.trim().is_empty()
            && !self.remote_dir.trim().is_empty()
            && match self.auth {
                AuthKind::Password => self.secret_saved,
                AuthKind::Key => !self.key_path.trim().is_empty(),
            }
    }

    /// Keychain account for the password or key passphrase.
    pub fn secret_account(&self) -> String {
        format!("sftp:{}@{}:{}", self.user.trim(), self.host.trim(), self.port)
    }
}

/// The user's choice for one update task.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TaskChoice {
    /// `None` = the task's default from site.json.
    pub enabled: Option<bool>,
    /// Model alias; empty = the task's or site's default.
    pub model: String,
}

/// A repository the app looks after, independent of any website.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManagedRepo {
    /// `owner/name` as GitHub reported it when it was added.
    pub repo: String,
    /// GitHub's id, so a rename does not make it a second repository.
    #[serde(default)]
    pub github_id: Option<u64>,
    /// Turned off: stays listed and readable, but no care job takes it.
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Local clone (absolute or `~/…`); empty = none. Status and pull request care work without one.
    #[serde(default)]
    pub local_path: String,
    /// How the repository releases; `None` = no automatic tag (never guessed).
    #[serde(default)]
    pub release: Option<ReleaseRule>,
    /// Key of the managed repository whose release publishes this one (e.g. a public repository
    /// that a private one exports to with the same tag); empty = none. Such a repository has no
    /// release rule of its own: the app never tags it, it waits for its release.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub released_by: String,
}

fn yes() -> bool {
    true
}

impl ManagedRepo {
    pub fn new(repo: &str) -> Self {
        Self { repo: repo.trim().to_owned(), github_id: None, enabled: true, local_path: String::new(), release: None, released_by: String::new() }
    }

    /// The identity: `owner/name` in lower case.
    pub fn key(&self) -> String {
        self.repo.trim().to_lowercase()
    }

    /// The key of the repository whose release publishes this one, in lower case like `key`.
    pub fn released_by_key(&self) -> Option<String> {
        let key = self.released_by.trim().to_lowercase();
        (!key.is_empty()).then_some(key)
    }
}

/// The release line the app may continue: tags `<prefix>x.y.z`, the next patch, minor or major version as each job chooses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseRule {
    /// `v` for `v1.2.0`, empty for `1.2.0`.
    pub tag_prefix: String,
    /// A workflow of the repository publishes a GitHub release for each tag; the app waits for it
    /// before steps that need the release.
    #[serde(default)]
    pub publishes_release: bool,
}

/// A GitLab instance the user entered an own token for, and how it is reached. Exists only while
/// that token is stored.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitlabInstance {
    /// Lower case, with port if any, like `gitlab.example.com` or `192.168.178.88:8929`.
    pub host: String,
    /// `https`, or `http` for a self-hosted instance without TLS.
    pub scheme: String,
    /// Where its token is kept.
    #[serde(default)]
    pub storage: crate::tokens::Storage,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    /// The repositories the app looks after; the basis of everything else.
    pub repositories: Vec<ManagedRepo>,
    /// README/documentation and wiki targets fed by those repositories.
    pub content_targets: Vec<crate::content::ContentTarget>,
    /// Claude model for reviews and content work without a choice in the job; empty = the
    /// website's setting, else Claude Code's default.
    pub claude_model: String,
    /// Cost limit per Claude run of a care job; `None` = the website's setting, if any.
    pub max_budget_usd: Option<f64>,
    /// Local clone of the website repository (contains `.openreleasepilot/site.json`); optional.
    pub project_dir: String,
    /// Folder that holds the local clones (website and products); empty = the project's parent.
    pub workspace_dir: String,
    pub sftp: SftpConfig,
    /// Task id → choice.
    pub tasks: std::collections::BTreeMap<String, TaskChoice>,
    /// Product ids (site.json) the user turned off: Precheck and Update leave them alone.
    pub skipped_products: std::collections::BTreeSet<String>,
    /// No look for a new version of the app at start (e.g. when IT distributes it).
    pub no_update_check: bool,
    /// Whether an own GitHub token is stored. Kept here so that showing the window never
    /// reads a token.
    pub github_token_saved: bool,
    /// Where the own GitHub token is kept.
    pub github_token_storage: crate::tokens::Storage,
    /// GitLab instances with an own token (keychain or app).
    pub gitlab_instances: Vec<GitlabInstance>,
}

impl Config {
    pub fn product_enabled(&self, id: &str) -> bool {
        !self.skipped_products.contains(id)
    }

    /// The managed repository with this key (`owner/name`, any case).
    pub fn repository(&self, key: &str) -> Option<&ManagedRepo> {
        let key = key.trim().to_lowercase();
        self.repositories.iter().find(|r| r.key() == key)
    }

    /// The managed repositories whose release `upstream` publishes, in the order they were added.
    pub fn released_through(&self, upstream: &str) -> Vec<&ManagedRepo> {
        let upstream = upstream.trim().to_lowercase();
        self.repositories.iter().filter(|r| r.released_by_key().as_deref() == Some(upstream.as_str())).collect()
    }

    /// Why `key` cannot be released by `upstream` with the release rule `release`; `None` when it
    /// can. One step only: the upstream tags itself, and nothing waits for itself.
    pub fn released_by_problem(&self, key: &str, upstream: &str, release: Option<&ReleaseRule>) -> Option<String> {
        let (key, upstream) = (key.trim().to_lowercase(), upstream.trim().to_lowercase());
        if upstream.is_empty() {
            return None;
        }
        if upstream == key {
            return Some("ein Repository kann nicht durch sich selbst veröffentlicht werden".into());
        }
        if release.is_some() {
            return Some("mit eigener Release-Regel taggt die App selbst; Release-Regel abwählen".into());
        }
        let Some(source) = self.repository(&upstream) else {
            return Some(format!("{upstream} wird nicht verwaltet"));
        };
        if crate::gitlab::project(&source.repo).is_some() || crate::gitlab::project(&key).is_some() {
            return Some("gibt es nur auf GitHub".into());
        }
        if !source.enabled {
            return Some(format!("{} ist abgeschaltet; ohne Tag dort kommt hier nichts an", source.repo));
        }
        if source.release.is_none() {
            return Some(format!("{} hat keine Release-Regel; ohne Tag dort kommt hier nichts an", source.repo));
        }
        if source.released_by_key().is_some() {
            return Some(format!("{} wird selbst durch {} veröffentlicht; nur eine Stufe", source.repo, source.released_by));
        }
        if let Some(other) = self.released_through(&key).first() {
            return Some(format!("{} wird schon durch dieses Repository veröffentlicht; nur eine Stufe", other.repo));
        }
        None
    }

    /// Adds a repository unless it is managed already (same `owner/name` or same GitHub id);
    /// returns whether it was added. Never changes an existing entry.
    pub fn add_repository(&mut self, repo: ManagedRepo) -> bool {
        let known = self.repositories.iter().any(|r| r.key() == repo.key() || (repo.github_id.is_some() && r.github_id == repo.github_id));
        if !known {
            self.repositories.push(repo);
        }
        !known
    }

    pub fn content_target(&self, id: &str) -> Option<&crate::content::ContentTarget> {
        self.content_targets.iter().find(|t| t.id == id)
    }

    /// Where clones are searched and created.
    pub fn workspace(&self) -> PathBuf {
        if !self.workspace_dir.trim().is_empty() {
            return paths::expand_home(self.workspace_dir.trim());
        }
        if !self.project_dir.trim().is_empty() {
            if let Some(parent) = paths::expand_home(self.project_dir.trim()).parent() {
                return parent.to_path_buf();
            }
        }
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let code = home.join("Documents").join("Code");
        if code.is_dir() {
            code
        } else {
            home
        }
    }
}

pub fn load() -> Config {
    fs::read_to_string(paths::config_file())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(config: &Config) -> Result<()> {
    let file = paths::config_file();
    fs::create_dir_all(file.parent().expect("config file has a parent")).context("App-Datenordner anlegen")?;
    let tmp = file.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(config)?).context("Einstellungen schreiben")?;
    fs::rename(&tmp, &file).context("Einstellungen speichern")?;
    Ok(())
}

/// The cost limit as typed: empty = none; otherwise a finite positive amount, with a decimal point
/// or comma. Anything else is an error, so a typo never removes the limit.
pub fn parse_budget(text: &str) -> Result<Option<f64>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    match text.replace(',', ".").parse::<f64>() {
        Ok(amount) if amount.is_finite() && amount > 0.0 => Ok(Some(amount)),
        _ => Err(format!("Kostengrenze „{text}“: bitte einen Betrag größer als 0 eingeben, z. B. 1,50, oder das Feld leeren")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::Storage;

    #[test]
    fn only_an_empty_budget_removes_the_limit() {
        assert_eq!(parse_budget(""), Ok(None));
        assert_eq!(parse_budget("  "), Ok(None));
        assert_eq!(parse_budget("1,50"), Ok(Some(1.5)));
        assert_eq!(parse_budget("1.50"), Ok(Some(1.5)));
        assert_eq!(parse_budget(" 2 "), Ok(Some(2.0)));
        for bad in ["-5", "0", "0,00", "abc", "1.000,50", "inf", "NaN", "1,5,0"] {
            assert!(parse_budget(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_repository_is_managed_once() {
        let mut config = Config::default();
        assert!(config.add_repository(ManagedRepo { github_id: Some(7), ..ManagedRepo::new("Owner/App") }));
        assert!(!config.add_repository(ManagedRepo::new("owner/app")), "same name, other case");
        assert!(!config.add_repository(ManagedRepo { github_id: Some(7), ..ManagedRepo::new("owner/renamed") }), "same GitHub id");
        assert_eq!(config.repositories.len(), 1);
        assert_eq!(config.repository("OWNER/APP").map(|r| r.repo.as_str()), Some("Owner/App"));
    }

    #[test]
    fn older_settings_load_without_repositories() {
        let config: Config = serde_json::from_str(r#"{"projectDir":"~/site","sftp":{"host":"h"}}"#).unwrap();
        assert!(config.repositories.is_empty() && config.content_targets.is_empty());
        assert_eq!(config.project_dir, "~/site");
        let repo: ManagedRepo = serde_json::from_str(r#"{"repo":"o/a"}"#).unwrap();
        assert!(repo.enabled && repo.release.is_none(), "no release rule is guessed");
        assert!(!config.github_token_saved && config.gitlab_instances.is_empty(), "no own token unless saved");
        assert_eq!(config.github_token_storage, Storage::Keychain);
        let old: GitlabInstance = serde_json::from_str(r#"{"host":"h","scheme":"https"}"#).unwrap();
        assert_eq!(old.storage, Storage::Keychain, "tokens saved before the choice are in the keychain");
    }

    #[test]
    fn own_token_flags_round_trip_and_never_hold_a_secret() {
        let mut config = Config { github_token_saved: true, github_token_storage: Storage::App, ..Config::default() };
        config.gitlab_instances.push(GitlabInstance { host: "192.168.178.88:8929".into(), scheme: "http".into(), storage: Storage::Keychain });
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["githubTokenSaved"], true);
        assert_eq!(json["gitlabInstances"][0], serde_json::json!({ "host": "192.168.178.88:8929", "scheme": "http", "storage": "keychain" }));
        assert_eq!(json["githubTokenStorage"], "app");
        let back: Config = serde_json::from_value(json).unwrap();
        assert!(back.github_token_saved && back.github_token_storage == Storage::App);
        assert_eq!(back.gitlab_instances, config.gitlab_instances);
    }

    #[test]
    fn a_repository_is_released_by_one_other_with_a_release_rule() {
        let rule = || Some(ReleaseRule { tag_prefix: "v".into(), publishes_release: false });
        let mut config = Config::default();
        config.add_repository(ManagedRepo { release: rule(), ..ManagedRepo::new("o/ee") });
        config.add_repository(ManagedRepo::new("o/core"));
        config.add_repository(ManagedRepo::new("o/plain"));
        config.add_repository(ManagedRepo { release: rule(), enabled: false, ..ManagedRepo::new("o/off") });
        config.add_repository(ManagedRepo { release: rule(), ..ManagedRepo::new("gitlab.example.com/g/lab") });
        assert!(config.released_by_problem("o/core", "o/off", None).unwrap().contains("abgeschaltet"));
        assert!(config.released_by_problem("o/core", "gitlab.example.com/g/lab", None).unwrap().contains("nur auf GitHub"));
        assert_eq!(config.released_by_problem("o/core", "O/EE", None), None);
        assert_eq!(config.released_by_problem("o/core", "", rule().as_ref()), None, "none at all is always fine");
        assert!(config.released_by_problem("o/core", "o/core", None).unwrap().contains("durch sich selbst"));
        assert!(config.released_by_problem("o/core", "o/ee", rule().as_ref()).unwrap().contains("eigener Release-Regel"));
        assert!(config.released_by_problem("o/core", "o/plain", None).unwrap().contains("keine Release-Regel"));
        assert!(config.released_by_problem("o/core", "o/gone", None).unwrap().contains("nicht verwaltet"));
        config.repositories[1].released_by = "O/EE".into();
        assert_eq!(config.released_through("o/ee").iter().map(|r| r.repo.as_str()).collect::<Vec<_>>(), ["o/core"]);
        config.repositories[2].release = rule();
        assert!(config.released_by_problem("o/plain", "o/core", None).unwrap().contains("keine Release-Regel"), "o/core never tags itself");
        assert!(config.released_by_problem("o/ee", "o/plain", None).unwrap().contains("nur eine Stufe"), "o/ee releases o/core");
        let json = serde_json::to_value(&config.repositories[2]).unwrap();
        assert!(json.get("releasedBy").is_none(), "older versions read the file unchanged");
    }
}
