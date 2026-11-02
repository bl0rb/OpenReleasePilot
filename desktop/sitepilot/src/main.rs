// No console window next to the app on Windows (release builds).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! OpenSitePilot: the website of `.openreleasepilot/site.json` in four steps (Prüfen, Inhalte
//! aktualisieren, Veröffentlichen, Live-Seite prüfen) and new products for it. The repositories
//! themselves are OpenReleasePilot's.

mod steps;

use steps::{NewProductInput, Work};

use openreleasepilot_core::claude::{Outcome, Outdated, ReleaseCheck};
use openreleasepilot_core::config::{self, AuthKind, SiteConfig};
use openreleasepilot_core::paths::{self, abbreviate_home, expand_home};
use openreleasepilot_core::secrets;
use openreleasepilot_core::sftp::{self, ConnectError};
use openreleasepilot_core::shell::Runner;
use openreleasepilot_core::site::{self, Site};
use openreleasepilot_core::update::{self, ReleaseInfo};
use openreleasepilot_core::verify::Report;
use openreleasepilot_core::{forge, github, localrepo, manifest, tokens};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Step {
    Precheck,
    Update,
    Upload,
    Verify,
}

impl Step {
    pub const ALL: [Step; 4] = [Step::Precheck, Step::Update, Step::Upload, Step::Verify];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    #[default]
    Idle,
    Running,
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogLine {
    /// `info`, `ok`, `warn`, `error`, `cmd`, `out` or `claude`.
    pub kind: &'static str,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StepState {
    pub status: Status,
    pub summary: String,
    /// Unix seconds.
    pub finished_at: Option<u64>,
    /// The result predates a later Update or Upload (only used for Precheck).
    pub stale: bool,
    #[serde(skip)]
    pub log: Vec<LogLine>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub label: String,
    pub status: Status,
    pub detail: String,
}

pub struct Model {
    pub config: SiteConfig,
    pub site: Result<Site, String>,
    pub steps: BTreeMap<Step, StepState>,
    pub running: Option<Step>,
    pub runner: Option<Arc<Runner>>,
    pub checks: Vec<Check>,
    /// `None` until a precheck has looked at the releases.
    pub outdated: Option<ReleaseCheck>,
    /// Product id → newest release tag, as last looked up.
    pub latest_tags: BTreeMap<String, String>,
    /// Claude runs of the last Update or New product, one per task.
    pub claude_runs: Vec<ClaudeRun>,
    pub upload: Option<UploadResult>,
    pub verify: Option<Report>,
    /// Fingerprint the user has to confirm before the first SFTP connection.
    pub pending_host_key: Option<String>,
    /// Newer version of the app itself, found by the update check.
    pub available_update: Option<ReleaseInfo>,
    pub checking_update: bool,
    pub installing_update: bool,
    pub notice: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeRun {
    pub task: String,
    pub model: String,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UploadResult {
    pub uploaded: usize,
    pub deleted: usize,
    pub unchanged: usize,
    /// What happened with git, e.g. `2 Commit(s) gepusht`.
    pub pushed: String,
}

impl Model {
    fn load() -> Self {
        let config = config::load_site();
        let site = load_site(&config);
        Self {
            config,
            site,
            steps: Step::ALL.iter().map(|s| (*s, StepState::default())).collect(),
            running: None,
            runner: None,
            checks: Vec::new(),
            outdated: None,
            latest_tags: BTreeMap::new(),
            claude_runs: Vec::new(),
            upload: None,
            verify: None,
            pending_host_key: None,
            available_update: None,
            checking_update: false,
            installing_update: false,
            notice: None,
            error: None,
        }
    }
}

pub fn load_site(config: &SiteConfig) -> Result<Site, String> {
    if config.project_dir.trim().is_empty() {
        return Err("Keine Website eingerichtet".into());
    }
    site::load(&expand_home(&config.project_dir)).map_err(|e| format!("{e:#}"))
}

struct Shared(Mutex<Model>);

pub fn with_model<T>(app: &AppHandle, f: impl FnOnce(&mut Model) -> T) -> T {
    let shared = app.state::<Shared>();
    let mut model = shared.0.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut model)
}

/// Tells the window to fetch a new view.
pub fn changed(app: &AppHandle) {
    emit_to_visible(app, "changed", ());
}

/// Sends an event to the open windows only. A window waiting hidden in the tray is left alone:
/// after an event, WebKit keeps a hidden window on the display link for good, which costs CPU
/// with nothing to see. The window catches up when it becomes visible again.
pub fn emit_to_visible(app: &AppHandle, event: &str, payload: impl Serialize + Clone) {
    for (label, window) in app.webview_windows() {
        if window.is_visible().unwrap_or(false) {
            let _ = app.emit_to(label.as_str(), event, payload.clone());
        }
    }
}

static VIBRANCY: AtomicBool = AtomicBool::new(false);

/// Puts the system's translucent material behind the window: vibrancy on macOS, Mica on Windows 11.
/// False where it is not available (Windows 10, Linux); the page then paints its own background.
fn apply_window_material(window: &tauri::WebviewWindow) -> bool {
    use window_vibrancy::{NSVisualEffectMaterial, NSVisualEffectState};
    if cfg!(windows) {
        window_vibrancy::apply_mica(window, None).is_ok()
    } else {
        window_vibrancy::apply_vibrancy(window, NSVisualEffectMaterial::Sidebar, Some(NSVisualEffectState::FollowsWindowActiveState), None).is_ok()
    }
}

/// Runs blocking work (files, network, dialogs) off the main thread.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tauri::async_runtime::spawn_blocking(f).await.expect("background task panicked")
}

// MARK: View

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LastUpload {
    commit: Option<String>,
    uploaded_at: Option<u64>,
    files: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct View {
    version: String,
    /// Debug build: no update check.
    dev_build: bool,
    /// `macos`, `windows` or `linux`.
    platform: &'static str,
    /// The system material shows behind the window; the page leaves its sidebar transparent.
    vibrancy: bool,
    config: SiteConfig,
    project_display: String,
    /// Folder with the clones, as displayed (auto value if not set).
    workspace_display: String,
    /// `owner/name` of the website repository (origin of the project folder).
    site_repo: Option<String>,
    site: Option<Site>,
    /// Project file in use: `.openreleasepilot/site.json`, or the old path while only that exists.
    site_file: &'static str,
    site_error: Option<String>,
    sftp_ready: bool,
    steps: BTreeMap<Step, StepState>,
    running: Option<Step>,
    checks: Vec<Check>,
    outdated: Option<Vec<Outdated>>,
    /// Products whose newest release the last lookup could not read.
    release_unknown: Vec<String>,
    products: Vec<steps::ProductView>,
    tasks: Vec<steps::ResolvedTask>,
    claude_runs: Vec<ClaudeRun>,
    upload: Option<UploadResult>,
    verify: Option<Report>,
    /// Whether gh is signed in, once known.
    signed_in: Option<bool>,
    last_upload: Option<LastUpload>,
    pending_host_key: Option<String>,
    available_update: Option<ReleaseInfo>,
    checking_update: bool,
    installing_update: bool,
    notice: Option<String>,
    error: Option<String>,
}

#[tauri::command]
fn get_view(app: AppHandle) -> View {
    let version = app.package_info().version.to_string();
    with_model(&app, |m| View {
        version,
        dev_build: cfg!(debug_assertions),
        platform: std::env::consts::OS,
        vibrancy: VIBRANCY.load(Ordering::Relaxed),
        config: m.config.clone(),
        project_display: if m.config.project_dir.is_empty() { String::new() } else { abbreviate_home(&expand_home(&m.config.project_dir)) },
        workspace_display: abbreviate_home(&m.config.workspace()),
        site_repo: if m.config.project_dir.is_empty() { None } else { localrepo::origin_repo(&expand_home(&m.config.project_dir)) },
        site: m.site.as_ref().ok().cloned(),
        site_file: if m.config.project_dir.is_empty() { site::SITE_FILE } else { site::file_name(&expand_home(&m.config.project_dir)) },
        site_error: if m.config.project_dir.is_empty() { None } else { m.site.as_ref().err().cloned() },
        sftp_ready: m.config.sftp.is_complete(),
        steps: m.steps.clone(),
        running: m.running,
        checks: m.checks.clone(),
        outdated: m.outdated.as_ref().map(|c| c.outdated.clone()),
        release_unknown: m.outdated.as_ref().map(|c| c.unknown.clone()).unwrap_or_default(),
        products: m.site.as_ref().map(|site| steps::resolve_products(site, &m.config, &m.latest_tags)).unwrap_or_default(),
        tasks: m.site.as_ref().map(|site| steps::resolve_tasks(site, &m.config)).unwrap_or_default(),
        claude_runs: m.claude_runs.clone(),
        upload: m.upload.clone(),
        verify: m.verify.clone(),
        signed_in: github::signed_in(),
        last_upload: manifest::load_last(&m.config.sftp).map(|last| LastUpload {
            commit: last.commit.map(|c| c.chars().take(7).collect()),
            uploaded_at: last.uploaded_at,
            files: last.files.len(),
        }),
        pending_host_key: m.pending_host_key.clone(),
        available_update: m.available_update.clone(),
        checking_update: m.checking_update,
        installing_update: m.installing_update,
        notice: m.notice.clone(),
        error: m.error.clone(),
    })
}

#[tauri::command]
fn get_log(app: AppHandle, step: Step) -> Vec<LogLine> {
    with_model(&app, |m| m.steps.get(&step).map(|s| s.log.clone()).unwrap_or_default())
}

// MARK: Settings

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsInput {
    project_dir: String,
    #[serde(default)]
    workspace_dir: String,
    host: String,
    port: u16,
    user: String,
    remote_dir: String,
    auth: AuthKind,
    key_path: String,
    /// New password or passphrase; empty keeps the stored one.
    secret: String,
    /// Look for a new app version at start.
    #[serde(default = "yes")]
    update_check: bool,
    /// A server key the user confirmed in the dialog for a server not saved yet; saved with the rest.
    #[serde(default)]
    host_key: String,
}

fn yes() -> bool {
    true
}

/// The settings `input` makes of `current`, without saving anything or touching the keychain.
fn settings_config(current: &SiteConfig, input: &SettingsInput) -> SiteConfig {
    let mut next = current.clone();
    let previous_account = next.sftp.secret_account();
    let target_changed = next.sftp.host.trim() != input.host.trim() || next.sftp.port != input.port;
    next.project_dir = input.project_dir.trim().to_owned();
    next.workspace_dir = input.workspace_dir.trim().to_owned();
    next.sftp.host = input.host.trim().to_owned();
    next.sftp.port = if input.port == 0 { 22 } else { input.port };
    next.sftp.user = input.user.trim().to_owned();
    next.sftp.remote_dir = input.remote_dir.trim().to_owned();
    next.sftp.auth = input.auth.clone();
    next.sftp.key_path = input.key_path.trim().to_owned();
    next.no_update_check = !input.update_check;
    if target_changed {
        // Another server needs its own confirmed host key.
        next.sftp.host_key.clear();
    }
    if !input.host_key.trim().is_empty() {
        next.sftp.host_key = input.host_key.trim().to_owned();
    }
    if next.sftp.secret_account() != previous_account && next.sftp.secret_saved && input.secret.is_empty() {
        // The keychain entry belongs to the old user/host; ask for the password again.
        next.sftp.secret_saved = false;
    }
    next
}

fn save(app: &AppHandle, next: SiteConfig) -> Result<(), String> {
    config::save_site(&next).map_err(|e| format!("{e:#}"))?;
    with_model(app, |m| m.config = next);
    changed(app);
    Ok(())
}

fn apply_settings(app: &AppHandle, input: SettingsInput) -> Result<(), String> {
    let mut next = settings_config(&with_model(app, |m| m.config.clone()), &input);
    if !input.secret.is_empty() {
        secrets::set(&next.sftp, &input.secret).map_err(|e| format!("{e:#}"))?;
        next.sftp.secret_saved = true;
    }
    config::save_site(&next).map_err(|e| format!("{e:#}"))?;
    let site = load_site(&next);
    with_model(app, |m| {
        m.config = next;
        m.site = site;
        m.pending_host_key = None;
    });
    changed(app);
    Ok(())
}

#[tauri::command]
async fn save_settings(app: AppHandle, input: SettingsInput) -> Result<(), String> {
    blocking(move || apply_settings(&app, input)).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
enum ConnectionTest {
    Connected { message: String },
    /// The server is not trusted yet: the dialog shows its key; trusted, it goes along with the next test and save.
    UnknownHostKey { fingerprint: String },
}

/// Connects once with the dialog's entries and lists the target folder. Saves nothing:
/// only "Sichern" does.
#[tauri::command]
async fn test_connection(app: AppHandle, input: SettingsInput) -> Result<ConnectionTest, String> {
    blocking(move || {
        let sftp_config = settings_config(&with_model(&app, |m| m.config.clone()), &input).sftp;
        let secret = if !input.secret.is_empty() {
            Some(input.secret.clone())
        } else if sftp_config.secret_saved {
            secrets::get(&sftp_config)
        } else {
            None
        };
        match sftp::connect(&sftp_config, secret.as_deref()) {
            Ok(connection) => {
                let names = connection.list_root().map_err(|e| format!("{e:#}"))?;
                Ok(ConnectionTest::Connected {
                    message: format!("Verbunden. Zielordner enthält {} Einträge{}", names.len(), steps::preview(&names)),
                })
            }
            Err(ConnectError::UnknownHostKey(fingerprint)) => Ok(ConnectionTest::UnknownHostKey { fingerprint }),
            Err(e) => Err(e.to_string()),
        }
    })
    .await
}

#[tauri::command]
fn trust_host_key(app: AppHandle, fingerprint: String) -> Result<(), String> {
    let mut next = with_model(&app, |m| {
        m.pending_host_key = None;
        m.config.clone()
    });
    next.sftp.host_key = fingerprint;
    save(&app, next)
}

#[tauri::command]
fn reject_host_key(app: AppHandle) {
    with_model(&app, |m| m.pending_host_key = None);
    changed(&app);
}

#[tauri::command]
async fn forget_secret(app: AppHandle) -> Result<(), String> {
    blocking(move || {
        let sftp_config = with_model(&app, |m| m.config.sftp.clone());
        secrets::delete(&sftp_config).map_err(|e| format!("{e:#}"))?;
        let mut next = with_model(&app, |m| m.config.clone());
        next.sftp.secret_saved = false;
        save(&app, next)
    })
    .await
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Option<String> {
    blocking(move || app.dialog().file().blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())).await
}

#[tauri::command]
async fn pick_file(app: AppHandle) -> Option<String> {
    blocking(move || app.dialog().file().blocking_pick_file().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())).await
}

#[tauri::command]
fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("Nur http(s)-Adressen".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

/// Turns an update task on or off and sets its model (empty = default).
#[tauri::command]
fn set_task(app: AppHandle, id: String, enabled: bool, model: String) -> Result<(), String> {
    let mut next = with_model(&app, |m| m.config.clone());
    let choice = next.tasks.entry(id).or_default();
    choice.enabled = Some(enabled);
    choice.model = model.trim().to_owned();
    save(&app, next)
}

/// Turns watched products on or off; turned-off products are neither checked nor updated.
#[tauri::command]
fn set_products(app: AppHandle, ids: Vec<String>, enabled: bool) -> Result<(), String> {
    let mut next = with_model(&app, |m| m.config.clone());
    for id in ids {
        if enabled {
            next.skipped_products.remove(&id);
        } else {
            next.skipped_products.insert(id);
        }
    }
    save(&app, next)
}

// MARK: App updates

/// Looks for a newer release of the app. Betas count when asked for or when a beta is installed.
/// A check at start stays silent unless it finds something.
fn check_updates(app: &AppHandle, beta: bool, user_initiated: bool) {
    let version = app.package_info().version.to_string();
    if cfg!(debug_assertions) {
        if user_initiated {
            with_model(app, |m| m.notice = Some("Entwicklungsversion – keine Update-Prüfung.".into()));
            changed(app);
        }
        return;
    }
    with_model(app, |m| m.checking_update = true);
    changed(app);
    let result = update::latest_release(beta || update::is_prerelease(&version));
    with_model(app, |m| {
        m.checking_update = false;
        match result {
            Ok(Some(release)) if update::is_newer(&release.version, &version) => m.available_update = Some(release),
            Ok(_) => {
                m.available_update = None;
                if user_initiated {
                    m.notice = Some(format!("OpenSitePilot {version} ist aktuell."));
                }
            }
            Err(e) if user_initiated => m.error = Some(format!("Update-Prüfung fehlgeschlagen: {e:#}")),
            Err(_) => {}
        }
    });
    changed(app);
}

#[tauri::command]
async fn check_for_updates(app: AppHandle, beta: bool) {
    blocking(move || check_updates(&app, beta, true)).await
}

/// Downloads the package of the available update, checks it and installs it behind the
/// administrator prompt; the app then starts again as the new version. If installing in place
/// fails, the package is opened in the macOS installer instead. Not while a step runs: the
/// restart would cut it off.
#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    let release = with_model(&app, |m| {
        if m.installing_update || m.running.is_some() {
            return None;
        }
        let release = m.available_update.clone()?;
        m.installing_update = true;
        Some(release)
    });
    let Some(release) = release else { return Ok(()) };
    changed(&app);
    blocking(move || {
        let path = match update::download_package(&release) {
            Ok(path) => path,
            Err(e) => {
                with_model(&app, |m| {
                    m.installing_update = false;
                    m.error = Some(format!("Update konnte nicht geladen werden: {e:#}"));
                });
                changed(&app);
                return Ok(());
            }
        };
        match update::install(&path, &release.version) {
            Ok(update::Installed::Done) => app.restart(),
            Ok(update::Installed::Cancelled) => with_model(&app, |m| m.notice = Some("Installation abgebrochen.".into())),
            Err(e) => {
                let opened = app.opener().open_path(path.to_string_lossy(), None::<&str>);
                with_model(&app, |m| match opened {
                    Ok(()) => {
                        m.available_update = None;
                        m.notice = Some(format!(
                            "{e:#}. Installer für OpenSitePilot {} geöffnet; nach der Installation die App neu starten.",
                            release.version
                        ));
                    }
                    Err(_) => m.error = Some(format!("{e:#}")),
                });
            }
        }
        with_model(&app, |m| m.installing_update = false);
        changed(&app);
        Ok(())
    })
    .await
}

/// Hides the update banner until the next check.
#[tauri::command]
fn dismiss_update(app: AppHandle) {
    with_model(&app, |m| m.available_update = None);
    changed(&app);
}

/// The toast with a notice closed (by itself or by the user); an error stays.
#[tauri::command]
fn dismiss_notice(app: AppHandle) {
    with_model(&app, |m| m.notice = None);
    changed(&app);
}

#[tauri::command]
fn dismiss(app: AppHandle) {
    with_model(&app, |m| {
        m.notice = None;
        m.error = None;
    });
    changed(&app);
}

// MARK: Steps

#[tauri::command]
fn run_step(app: AppHandle, step: Step, instructions: String, everything: Option<bool>) -> Result<(), String> {
    let work = match step {
        Step::Precheck => Work::Precheck,
        Step::Update => Work::Update(instructions),
        Step::Upload => Work::Upload(everything.unwrap_or(false)),
        Step::Verify => Work::Verify,
    };
    steps::start(&app, vec![work])
}

/// Precheck, then Update (only if something is outdated or there are instructions), Upload, Verify.
#[tauri::command]
fn run_all(app: AppHandle, instructions: String) -> Result<(), String> {
    steps::start(&app, vec![Work::Precheck, Work::Update(instructions), Work::Upload(false), Work::Verify])
}

/// Claude adds a repository as a new product (runs in the Update slot, ends with a commit).
#[tauri::command]
fn run_add_product(app: AppHandle, input: NewProductInput) -> Result<(), String> {
    if !github::is_repo_name(input.repo.trim()) {
        return Err("Repository im Format owner/name angeben".into());
    }
    steps::start(&app, vec![Work::AddProduct(input)])
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RepoEntry {
    #[serde(flatten)]
    info: github::RepoInfo,
    /// Absolute path of the clone in the repo folder, if any.
    local_path: Option<String>,
    on_site: bool,
    is_website: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RepoList {
    repos: Vec<RepoEntry>,
    workspace: String,
    /// Why only public repositories are listed, if so.
    note: Option<String>,
}

/// The user's GitHub repositories with their local clone and their role on the website. Without a
/// gh login: the public repositories of the owners of the website's products.
#[tauri::command]
async fn list_repos(app: AppHandle) -> Result<RepoList, String> {
    let (site, config) = with_model(&app, |m| (m.site.clone().ok(), m.config.clone()));
    blocking(move || {
        let workspace = config.workspace();
        let (repos, note) = match github::user_repos() {
            Ok(repos) => (repos, None),
            Err(e) => {
                let mut owners: Vec<String> = site
                    .iter()
                    .flat_map(|s| s.products.iter().map(|p| p.repo.clone()))
                    .filter_map(|repo| repo.split('/').next().map(str::to_owned))
                    .collect();
                owners.sort();
                owners.dedup();
                if owners.is_empty() {
                    return Err(format!("{e:#}"));
                }
                let mut repos = Vec::new();
                for owner in &owners {
                    repos.extend(github::owner_repos(owner).map_err(|e| format!("{e:#}"))?);
                }
                (repos, Some(format!("{e:#}. Gezeigt werden nur öffentliche Repositories.")))
            }
        };
        let mut clones = localrepo::clones_in(&workspace);
        let website = if config.project_dir.is_empty() { None } else { localrepo::origin_repo(&expand_home(&config.project_dir)) };
        let repos = repos
            .into_iter()
            .map(|info| {
                let key = info.full_name.to_lowercase();
                RepoEntry {
                    local_path: clones.remove(&key).map(|p| p.display().to_string()),
                    on_site: site.iter().flat_map(|s| s.products.iter()).any(|p| p.repo.eq_ignore_ascii_case(&info.full_name)),
                    is_website: website.as_ref().is_some_and(|w| w.eq_ignore_ascii_case(&info.full_name)),
                    info,
                }
            })
            .collect();
        Ok(RepoList { repos, workspace: abbreviate_home(&workspace), note })
    })
    .await
}

/// Clones a repository into the repo folder and returns its path.
#[tauri::command]
async fn clone_repo(app: AppHandle, repo: String, workspace_dir: String) -> Result<String, String> {
    let mut config = with_model(&app, |m| m.config.clone());
    if !workspace_dir.trim().is_empty() {
        config.workspace_dir = workspace_dir;
    }
    blocking(move || {
        if !forge::is_repo_id(repo.trim()) {
            return Err("Repository im Format owner/name (GitHub) oder host/gruppe/name (GitLab) angeben".to_owned());
        }
        localrepo::clone(repo.trim(), &config.workspace()).map(|p| p.display().to_string()).map_err(|e| format!("{e:#}"))
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RepoCheck {
    info: github::RepoInfo,
    latest_tag: Option<String>,
    /// Absolute path of a clone found next to the project.
    local_path: Option<String>,
    on_site: bool,
}

#[tauri::command]
async fn inspect_repo(app: AppHandle, repo: String) -> Result<RepoCheck, String> {
    let (site, workspace) = with_model(&app, |m| (m.site.clone(), m.config.workspace()));
    let site = site.map_err(|e| format!("Website nicht lesbar: {e}"))?;
    blocking(move || {
        let info = github::repo_info(repo.trim()).map_err(|e| format!("{e:#}"))?;
        let latest_tag = github::latest_release(&info.full_name).map_err(|e| format!("{e:#}"))?;
        let local_path = localrepo::clones_in(&workspace).remove(&info.full_name.to_lowercase()).map(|p| p.display().to_string());
        let on_site = site.products.iter().any(|p| p.repo.eq_ignore_ascii_case(&info.full_name));
        Ok(RepoCheck { info, latest_tag, local_path, on_site })
    })
    .await
}

#[tauri::command]
fn cancel(app: AppHandle) {
    if let Some(runner) = with_model(&app, |m| m.runner.clone()) {
        runner.cancel();
    }
}

fn main() {
    paths::set_app(paths::SITE_APP_NAME);
    // GitHub is asked with the GitHub CLI's login; the own tokens are OpenReleasePilot's.
    tokens::load(&config::Config::default());
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Shared(Mutex::new(Model::load())))
        .setup(|app| {
            let material = app.get_webview_window("main").is_some_and(|w| apply_window_material(&w));
            VIBRANCY.store(material, Ordering::Relaxed);
            // Like ClaudeKeyZapper: look for a new version once at start, quietly.
            let handle = app.handle().clone();
            if !cfg!(debug_assertions) && !with_model(&handle, |m| m.config.no_update_check) {
                std::thread::spawn(move || check_updates(&handle, false, false));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_view,
            get_log,
            save_settings,
            test_connection,
            trust_host_key,
            reject_host_key,
            forget_secret,
            pick_folder,
            pick_file,
            open_url,
            set_task,
            set_products,
            check_for_updates,
            install_update,
            dismiss_update,
            dismiss_notice,
            dismiss,
            run_step,
            run_all,
            run_add_product,
            list_repos,
            clone_repo,
            inspect_repo,
            cancel
        ])
        .run(tauri::generate_context!())
        .expect("error while running OpenSitePilot");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(changes: serde_json::Value) -> SettingsInput {
        let mut base = serde_json::json!({
            "projectDir": "~/site", "workspaceDir": "", "host": "h", "port": 22, "user": "u", "remoteDir": "/w",
            "auth": "password", "keyPath": "", "secret": "", "updateCheck": true,
        });
        base.as_object_mut().unwrap().extend(changes.as_object().unwrap().clone());
        serde_json::from_value(base).unwrap()
    }

    fn saved() -> SiteConfig {
        SiteConfig {
            project_dir: "~/site".into(),
            sftp: config::SftpConfig {
                host: "h".into(),
                user: "u".into(),
                remote_dir: "/w".into(),
                host_key: "SHA256:old".into(),
                secret_saved: true,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn settings_are_built_without_changing_the_saved_ones() {
        let current = saved();
        let next = settings_config(&current, &input(serde_json::json!({ "projectDir": "~/other" })));
        assert_eq!(next.project_dir, "~/other");
        assert_eq!(current.project_dir, "~/site", "the saved settings stay as they are");
        assert_eq!(next.sftp.host_key, "SHA256:old", "same server, same key");
    }

    #[test]
    fn another_server_needs_its_own_key() {
        let next = settings_config(&saved(), &input(serde_json::json!({ "host": "other" })));
        assert!(next.sftp.host_key.is_empty());
        assert!(!next.sftp.secret_saved, "the stored password belongs to the old server");
        let trusted = settings_config(&saved(), &input(serde_json::json!({ "host": "other", "hostKey": "SHA256:new" })));
        assert_eq!(trusted.sftp.host_key, "SHA256:new", "a key confirmed in the dialog is kept");
    }
}
