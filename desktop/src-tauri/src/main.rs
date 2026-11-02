// No console window next to the app on Windows (release builds).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod jobrun;
mod steps;
mod tray;

use steps::Work;

use openreleasepilot_core::config::{self, Config, ManagedRepo, ReleaseRule};
use openreleasepilot_core::content::{self, ContentTarget, TargetState};
use openreleasepilot_core::jobs::{self, Job, JobOptions};
use openreleasepilot_core::update::{self, ReleaseInfo};
use openreleasepilot_core::forge::{self, Forge};
use openreleasepilot_core::{github, gitlab, localrepo, repos, security, site};
use openreleasepilot_core::paths::{self, abbreviate_home, expand_home};
use openreleasepilot_core::tokens;
use openreleasepilot_core::shell::Runner;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Step {
    /// A pass of a care job; its lines go to the job's own log.
    Jobs,
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
    #[serde(skip)]
    pub log: Vec<LogLine>,
}

/// What the Repositories area knows about one repository; read only, kept in memory.
#[derive(Default)]
pub struct RepoStatus {
    pub facts: Option<repos::Facts>,
    pub loading: bool,
    /// The last refresh failed entirely; `facts`, if any, are from before.
    pub error: Option<String>,
    /// Answers to an older request are dropped.
    pub token: u64,
    pub details: Option<repos::Details>,
    pub details_loading: bool,
    pub details_token: u64,
    /// The last security check; read only on request.
    pub security: Option<security::Report>,
    pub security_loading: bool,
    /// The last security check failed; `security`, if any, is from before.
    pub security_error: Option<String>,
    pub security_token: u64,
}

static NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);

/// A number never handed out before: request tokens, preview ids.
pub fn next_token() -> u64 {
    NEXT_TOKEN.fetch_add(1, Ordering::SeqCst)
}

pub struct Model {
    pub config: Config,
    pub steps: BTreeMap<Step, StepState>,
    pub running: Option<Step>,
    pub runner: Option<Arc<Runner>>,
    /// Repository key → what the Repositories area read about it.
    pub repo_status: BTreeMap<String, RepoStatus>,
    /// Care jobs, newest first; each also stored as a file.
    pub jobs: Vec<Job>,
    /// The job whose pass is running.
    pub job_running: Option<String>,
    /// Job files that could not be read at start.
    pub jobs_unreadable: Vec<String>,
    /// Content target id → what it was last brought to.
    pub content_states: BTreeMap<String, TargetState>,
    /// Newer version of the app itself, found by the update check.
    pub available_update: Option<ReleaseInfo>,
    pub checking_update: bool,
    pub installing_update: bool,
    pub notice: Option<String>,
    pub error: Option<String>,
}

impl Model {
    fn load() -> Self {
        let config = config::load();
        // A job that was running when the app stopped is marked, never continued by itself.
        let (mut jobs, jobs_unreadable) = jobs::load_all();
        for job in &mut jobs {
            if job.mark_interrupted() {
                let _ = jobs::save(job);
            }
        }
        jobs::prune(&mut jobs);
        Self {
            config,
            steps: BTreeMap::from([(Step::Jobs, StepState::default())]),
            running: None,
            runner: None,
            repo_status: BTreeMap::new(),
            jobs,
            job_running: None,
            jobs_unreadable,
            content_states: content::load_states(),
            available_update: None,
            checking_update: false,
            installing_update: false,
            notice: None,
            error: None,
        }
    }
}

fn save_config(app: &AppHandle, change: impl FnOnce(&mut Config) -> Result<(), String>) -> Result<(), String> {
    let mut next = with_model(app, |m| m.config.clone());
    change(&mut next)?;
    config::save(&next).map_err(|e| format!("{e:#}"))?;
    with_model(app, |m| m.config = next);
    changed(app);
    Ok(())
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
    tray::update(app);
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

/// Brings the main window to the front, also from the menu bar.
pub fn show_window(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
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
struct View {
    version: String,
    /// Debug build: no update check.
    dev_build: bool,
    /// `macos`, `windows` or `linux`.
    platform: &'static str,
    /// The system material shows behind the window; the page leaves its sidebar transparent.
    vibrancy: bool,
    config: Config,
    /// Folder with the clones, as displayed (auto value if not set).
    workspace_display: String,
    running: Option<Step>,
    repositories: Vec<RepoView>,
    targets: Vec<TargetView>,
    jobs: Vec<JobSummary>,
    job_running: Option<String>,
    jobs_unreadable: Vec<String>,
    /// Whether gh is signed in, once known.
    signed_in: Option<bool>,
    /// The GitLab instances: those of managed repositories and those glab is signed in to.
    gitlab_hosts: Vec<GitlabHost>,
    /// Facts older than this many seconds are shown as old.
    fresh_secs: u64,
    /// Unix seconds when the view was made.
    now: u64,
    available_update: Option<ReleaseInfo>,
    checking_update: bool,
    installing_update: bool,
    notice: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GitlabHost {
    host: String,
    /// Whether glab has a token for it, once asked.
    signed_in: Option<bool>,
    /// glab knows the host (its configuration names it).
    configured: bool,
    /// An own token for it is stored.
    own_token: bool,
    /// Where that token is kept; `None` without an own token.
    own_storage: Option<tokens::Storage>,
}

fn gitlab_hosts(config: &Config) -> Vec<GitlabHost> {
    let configured = gitlab::configured_hosts();
    let mut hosts: Vec<String> = config
        .repositories
        .iter()
        .filter_map(|r| gitlab::project(&r.repo).map(|p| p.host))
        .chain(configured.iter().cloned())
        .chain(config.gitlab_instances.iter().map(|i| i.host.to_lowercase()))
        .collect();
    hosts.sort();
    hosts.dedup();
    hosts
        .into_iter()
        .map(|host| GitlabHost {
            signed_in: if config.gitlab_instances.iter().any(|i| i.host.eq_ignore_ascii_case(&host)) { Some(true) } else { gitlab::signed_in(&host) },
            configured: configured.contains(&host),
            own_token: config.gitlab_instances.iter().any(|i| i.host.eq_ignore_ascii_case(&host)),
            own_storage: config.gitlab_instances.iter().find(|i| i.host.eq_ignore_ascii_case(&host)).map(|i| i.storage),
            host,
        })
        .collect()
}

/// One repository in the Repositories area.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RepoView {
    #[serde(flatten)]
    target: repos::RepoTarget,
    facts: Option<repos::Facts>,
    health: repos::Health,
    capabilities: repos::Capabilities,
    loading: bool,
    error: Option<String>,
    details: Option<repos::Details>,
    details_loading: bool,
    security: Option<security::Report>,
    security_loading: bool,
    security_error: Option<String>,
    /// The open job that holds it, if any.
    job: Option<JobRef>,
    /// Ids of the content targets it feeds.
    targets: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct JobRef {
    id: String,
    status: jobs::Status,
}

/// A content target with what it was last brought to and how that compares with now.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TargetView {
    #[serde(flatten)]
    target: ContentTarget,
    state: TargetState,
    /// Why it cannot be used as configured.
    problem: Option<String>,
    /// Per source: whether the published state is behind what GitHub reports now.
    freshness: Vec<String>,
    job: Option<JobRef>,
}

/// A job without its proposals, for lists; `get_job` has all of it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JobSummary {
    id: String,
    created_at: u64,
    updated_at: u64,
    repos: Vec<String>,
    options: JobOptions,
    status: jobs::Status,
    running: bool,
    steps: Vec<StepSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StepSummary {
    id: String,
    kind: jobs::StepKind,
    label: String,
    state: jobs::StepState,
    detail: String,
    links: Vec<jobs::Link>,
    /// Care: pull requests Claude advises to keep open, which wait for the user (the full job has the plan instead).
    waiting: usize,
}

fn summary(job: &Job, running: bool) -> JobSummary {
    JobSummary {
        id: job.id.clone(),
        created_at: job.created_at,
        updated_at: job.updated_at,
        repos: job.repos.clone(),
        options: job.options.clone(),
        status: job.status(running),
        running,
        steps: job
            .steps
            .iter()
            .map(|s| StepSummary {
                id: s.id.clone(),
                kind: s.kind,
                label: s.label.clone(),
                state: s.state,
                detail: s.detail.clone(),
                links: s.links.clone(),
                waiting: if s.kind == jobs::StepKind::Care { waiting(job) } else { 0 },
            })
            .collect(),
    }
}

/// Pull requests of the care plan that Claude advises to keep open and the plan does not touch.
fn waiting(job: &Job) -> usize {
    job.plan.iter().flat_map(|set| &set.plans).flat_map(|p| &p.prs).filter(|pr| pr.action == openreleasepilot_core::repocare::Action::Keep && !pr.optional).count()
}

/// What a target is behind, judged by the last status read of its sources.
fn freshness(state: &TargetState, status: &BTreeMap<String, RepoStatus>) -> Vec<String> {
    content::behind(state, &|key| {
        let facts = status.get(key)?.facts.as_ref()?;
        Some(content::Latest { release: facts.release.ok().cloned().flatten(), head: facts.head.ok().cloned() })
    })
}

#[tauri::command]
fn get_view(app: AppHandle) -> View {
    let version = app.package_info().version.to_string();
    // Reads .git/config of the clones: outside the lock.
    let targets = repos::targets(&with_model(&app, |m| m.config.clone()));
    let now = steps::now();
    with_model(&app, |m| {
        let open_job = |pred: &dyn Fn(&Job) -> bool| {
            m.jobs.iter().find(|j| j.open() && pred(j)).map(|j| JobRef { id: j.id.clone(), status: j.status(m.job_running.as_deref() == Some(j.id.as_str())) })
        };
        let managed: Vec<String> = m.config.repositories.iter().map(|r| r.key()).collect();
        View {
            version,
            dev_build: cfg!(debug_assertions),
            platform: std::env::consts::OS,
            vibrancy: VIBRANCY.load(Ordering::Relaxed),
            config: m.config.clone(),
            workspace_display: abbreviate_home(&m.config.workspace()),
            running: m.running,
            repositories: targets
                .into_iter()
                .map(|target| {
                    let status = m.repo_status.get(&target.key);
                    let facts = status.and_then(|s| s.facts.clone());
                    let details = status.and_then(|s| s.details.clone());
                    let key = target.key.clone();
                    RepoView {
                        health: repos::health_with(&target, facts.as_ref(), details.as_ref(), upstream_of(m, &target), now),
                        capabilities: repos::capabilities(&target, facts.as_ref(), target.forge.signed_in()),
                        loading: status.is_some_and(|s| s.loading),
                        error: status.and_then(|s| s.error.clone()),
                        details_loading: status.is_some_and(|s| s.details_loading),
                        security: status.and_then(|s| s.security.clone()),
                        security_loading: status.is_some_and(|s| s.security_loading),
                        security_error: status.and_then(|s| s.security_error.clone()),
                        job: open_job(&|j| j.held().contains(&key)),
                        targets: m.config.content_targets.iter().filter(|t| t.sources.contains(&key)).map(|t| t.id.clone()).collect(),
                        facts,
                        details,
                        target,
                    }
                })
                .collect(),
            targets: m
                .config
                .content_targets
                .iter()
                .map(|target| {
                    let state = m.content_states.get(&target.id).cloned().unwrap_or_default();
                    TargetView {
                        problem: content::validate(target, &managed).err(),
                        freshness: freshness(&state, &m.repo_status),
                        job: open_job(&|j| j.options.targets.iter().any(|t| t.id == target.id)),
                        target: target.clone(),
                        state,
                    }
                })
                .collect(),
            jobs: m.jobs.iter().map(|j| summary(j, m.job_running.as_deref() == Some(j.id.as_str()))).collect(),
            job_running: m.job_running.clone(),
            jobs_unreadable: m.jobs_unreadable.clone(),
            signed_in: if m.config.github_token_saved { Some(true) } else { github::signed_in() },
            gitlab_hosts: gitlab_hosts(&m.config),
            fresh_secs: repos::FRESH_SECS,
            now,
            available_update: m.available_update.clone(),
            checking_update: m.checking_update,
            installing_update: m.installing_update,
            notice: m.notice.clone(),
            error: m.error.clone(),
        }
    })
}

// MARK: Settings

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsInput {
    #[serde(default)]
    workspace_dir: String,
    /// Look for a new app version at start.
    #[serde(default = "yes")]
    update_check: bool,
    /// Claude model for care jobs without a choice; empty = Claude Code's default.
    #[serde(default)]
    claude_model: String,
    /// Cost limit per Claude run of a care job, as typed; empty = none.
    #[serde(default)]
    max_budget_usd: String,
}

fn yes() -> bool {
    true
}

/// The settings `input` makes of `current`, without saving anything or touching the keychain.
fn settings_config(current: &Config, input: &SettingsInput) -> Result<Config, String> {
    let mut next = current.clone();
    next.workspace_dir = input.workspace_dir.trim().to_owned();
    next.no_update_check = !input.update_check;
    next.claude_model = input.claude_model.trim().to_owned();
    next.max_budget_usd = config::parse_budget(&input.max_budget_usd)?;
    Ok(next)
}

fn apply_settings(app: &AppHandle, input: SettingsInput) -> Result<(), String> {
    let next = settings_config(&with_model(app, |m| m.config.clone()), &input)?;
    config::save(&next).map_err(|e| format!("{e:#}"))?;
    with_model(app, |m| m.config = next);
    changed(app);
    Ok(())
}

#[tauri::command]
async fn save_settings(app: AppHandle, input: SettingsInput) -> Result<(), String> {
    blocking(move || apply_settings(&app, input)).await
}

/// The account the token is stored under, and the host for GitLab.
fn token_account(forge: &Forge) -> Result<(String, Option<String>), String> {
    match forge {
        Forge::Github => Ok((tokens::github_account(), None)),
        Forge::Gitlab { host } => {
            let host = host.trim().to_lowercase();
            if !gitlab::is_host(&host) {
                return Err("Kein gültiger GitLab-Host (z. B. gitlab.example.com oder 192.168.178.88:8929)".into());
            }
            Ok((tokens::gitlab_account(&host), Some(host)))
        }
    }
}

/// Checks the token against the service, then stores it in the chosen place (keychain or app)
/// and notes that in the settings. Returns the login name. Stores nothing if the check fails.
#[tauri::command]
async fn save_forge_token(app: AppHandle, forge: Forge, token: String, scheme: Option<String>, storage: Option<String>) -> Result<String, String> {
    blocking(move || {
        let token = token.trim().to_owned();
        if token.is_empty() {
            return Err("Kein Token eingegeben".to_owned());
        }
        let (account, host) = token_account(&forge)?;
        let scheme = match scheme.as_deref().map(|s| s.trim().to_lowercase()).as_deref() {
            None | Some("") | Some("https") => "https",
            Some("http") => "http",
            Some(_) => return Err("Schema muss https oder http sein".to_owned()),
        };
        let storage = tokens::Storage::parse(storage.as_deref()).ok_or("Ablage muss keychain oder app sein")?;
        let login = match &host {
            None => github::validate_token(&token),
            Some(host) => gitlab::validate_token(host, scheme, &token),
        }
        .map_err(|e| format!("{e:#}"))?;
        // Clear the other place first, so no stale copy stays behind.
        tokens::delete(&account).map_err(|e| format!("{e:#}"))?;
        tokens::set(&account, storage, &token).map_err(|e| format!("{e:#}"))?;
        let mut next = with_model(&app, |m| m.config.clone());
        match &host {
            None => {
                next.github_token_saved = true;
                next.github_token_storage = storage;
            }
            Some(host) => {
                let instance = config::GitlabInstance { host: host.clone(), scheme: scheme.to_owned(), storage };
                match next.gitlab_instances.iter_mut().find(|i| i.host.eq_ignore_ascii_case(host)) {
                    Some(existing) => *existing = instance,
                    None => next.gitlab_instances.push(instance),
                }
            }
        }
        config::save(&next).map_err(|e| format!("{e:#}"))?;
        with_model(&app, |m| m.config = next);
        match &host {
            None => tokens::remember_github(Some(token)),
            Some(host) => tokens::remember_gitlab(host, Some((token, scheme.to_owned()))),
        }
        changed(&app);
        Ok(login)
    })
    .await
}

/// Removes the own token from the keychain and the app; the CLI's token, if any, applies again.
#[tauri::command]
async fn forget_forge_token(app: AppHandle, forge: Forge) -> Result<(), String> {
    blocking(move || {
        let (account, host) = token_account(&forge)?;
        tokens::delete(&account).map_err(|e| format!("{e:#}"))?;
        let mut next = with_model(&app, |m| m.config.clone());
        match &host {
            None => {
                next.github_token_saved = false;
                next.github_token_storage = tokens::Storage::default();
            }
            Some(host) => next.gitlab_instances.retain(|i| !i.host.eq_ignore_ascii_case(host)),
        }
        config::save(&next).map_err(|e| format!("{e:#}"))?;
        with_model(&app, |m| m.config = next);
        match &host {
            None => tokens::remember_github(None),
            Some(host) => tokens::remember_gitlab(host, None),
        }
        changed(&app);
        Ok(())
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
                    m.notice = Some(format!("OpenReleasePilot {version} ist aktuell."));
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
/// fails, the package is opened in the macOS installer instead. Not while a step or a job pass
/// runs: the restart would cut it off.
#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    let release = with_model(&app, |m| {
        if m.installing_update || m.running.is_some() || m.job_running.is_some() {
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
                            "{e:#}. Installer für OpenReleasePilot {} geöffnet; nach der Installation die App neu starten.",
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

// MARK: Repositories

/// The repository that publishes `target`'s release, with what was last read of it.
pub(crate) fn upstream_of<'a>(m: &'a Model, target: &repos::RepoTarget) -> Option<repos::Upstream<'a>> {
    let key = target.released_by.as_deref()?;
    let managed = m.config.repository(key)?;
    Some(repos::Upstream { repo: &managed.repo, facts: m.repo_status.get(key).and_then(|s| s.facts.as_ref()) })
}

/// Reads the facts of the repositories (all when `keys` is empty), four at a time, each stored as
/// soon as it arrives. Read only and without Claude; runs beside a step, never under the model lock.
/// Also looks whether pull requests delivered by content targets were merged meanwhile.
pub fn refresh_status(app: &AppHandle, keys: &[String]) {
    let config = with_model(app, |m| m.config.clone());
    let targets: Vec<repos::RepoTarget> = repos::targets(&config).into_iter().filter(|t| keys.is_empty() || keys.contains(&t.key)).collect();
    if targets.is_empty() {
        return;
    }
    let mut hosts: Vec<String> = targets.iter().filter_map(|t| gitlab::project(&t.repo).map(|p| p.host)).collect();
    hosts.dedup();
    let full = keys.is_empty();
    let jobs: VecDeque<(repos::RepoTarget, u64)> = with_model(app, |m| {
        targets
            .into_iter()
            .map(|target| {
                let token = next_token();
                let status = m.repo_status.entry(target.key.clone()).or_default();
                status.token = token;
                status.loading = true;
                (target, token)
            })
            .collect()
    });
    changed(app);
    let app = app.clone();
    std::thread::spawn(move || {
        // Asks gh and glab once, so the view knows whether they are signed in; reading everything
        // asks glab again, so a login since then counts.
        if full {
            gitlab::forget_tokens();
        }
        // The own tokens come out of the keychain here, never when merely showing the window.
        tokens::ensure_loaded();
        github::token();
        for host in &hosts {
            gitlab::token(host);
        }
        let queue = Mutex::new(jobs);
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| loop {
                    let Some((target, token)) = queue.lock().unwrap_or_else(|e| e.into_inner()).pop_front() else { break };
                    let current = |m: &Model| m.repo_status.get(&target.key).is_some_and(|s| s.token == token);
                    if !with_model(&app, |m| current(m)) {
                        continue;
                    }
                    let facts = repos::fetch_facts(&target, steps::now());
                    with_model(&app, |m| {
                        let Some(status) = m.repo_status.get_mut(&target.key).filter(|s| s.token == token) else { return };
                        status.loading = false;
                        match &facts.access {
                            // An older reading is worth more than none; it shows its age.
                            repos::Fetch::Failed(why) if facts.failed_entirely() && status.facts.is_some() => status.error = Some(why.clone()),
                            _ => {
                                status.error = None;
                                status.facts = Some(facts);
                                status.details = None;
                            }
                        }
                    });
                    changed(&app);
                });
            }
        });
        refresh_deliveries(&app);
    });
}

/// A docs pull request the app delivered counts as published once it is merged; read only.
fn refresh_deliveries(app: &AppHandle) {
    let (config, states) = with_model(app, |m| (m.config.clone(), m.content_states.clone()));
    let mut changed_any = false;
    let mut next = states.clone();
    for (id, state) in &states {
        let (Some(target), Some(delivered)) = (config.content_target(id), &state.delivered) else { continue };
        let Some(number) = delivered.pr else { continue };
        match content::pull_request_state(&target.repo, number) {
            Ok(content::PrState::Merged) => {
                let entry = next.get_mut(id).expect("same keys");
                entry.published = Some(content::Delivery { pr: None, what: format!("{} (gemergt)", delivered.what), ..delivered.clone() });
                entry.delivered = Some(content::Delivery { pr: None, ..delivered.clone() });
                changed_any = true;
            }
            Ok(content::PrState::Closed) => {
                let entry = next.get_mut(id).expect("same keys");
                entry.delivered = Some(content::Delivery { pr: None, what: format!("{} (ohne Merge geschlossen)", delivered.what), ..delivered.clone() });
                changed_any = true;
            }
            _ => {}
        }
    }
    if changed_any {
        let _ = content::save_states(&next);
        with_model(app, |m| m.content_states = next);
        changed(app);
    }
}

/// "Status aktualisieren": all repositories, or the given ones.
#[tauri::command]
fn refresh_repos(app: AppHandle, keys: Vec<String>) {
    refresh_status(&app, &keys);
}

/// Drops the answers of running status reads; what is already shown stays.
#[tauri::command]
fn cancel_repo_refresh(app: AppHandle) {
    with_model(&app, |m| {
        for status in m.repo_status.values_mut() {
            if status.loading {
                status.token = next_token();
                status.loading = false;
            }
            if status.details_loading {
                status.details_token = next_token();
                status.details_loading = false;
            }
            if status.security_loading {
                status.security_token = next_token();
                status.security_loading = false;
            }
        }
    });
    changed(&app);
}

/// Mergeability and CI of a repository's open pull requests, for its detail view.
#[tauri::command]
fn load_repo_details(app: AppHandle, key: String) -> Result<(), String> {
    let (repo, prs, token) = with_model(&app, |m| {
        let status = m.repo_status.get_mut(&key)?;
        let facts = status.facts.as_ref()?;
        let repo = facts.access.ok()?.full_name.clone();
        let prs = facts.prs.ok()?.clone();
        let token = next_token();
        status.details_token = token;
        status.details_loading = true;
        Some((repo, prs, token))
    })
    .ok_or("Erst den Status des Repositorys lesen")?;
    changed(&app);
    std::thread::spawn(move || {
        let details = repos::fetch_details(&repo, &prs, steps::now());
        with_model(&app, |m| {
            if let Some(status) = m.repo_status.get_mut(&key).filter(|s| s.details_token == token) {
                status.details_loading = false;
                status.details = Some(details);
            }
        });
        changed(&app);
    });
    Ok(())
}

// MARK: Security

/// The security check of the repositories (all when `keys` is empty), four at a time, each stored
/// as soon as it arrives. Read only and without Claude.
fn check_security_of(app: &AppHandle, keys: &[String]) {
    let config = with_model(app, |m| m.config.clone());
    let targets: Vec<repos::RepoTarget> = repos::targets(&config).into_iter().filter(|t| keys.is_empty() || keys.contains(&t.key)).collect();
    if targets.is_empty() {
        return;
    }
    let jobs: VecDeque<(repos::RepoTarget, u64)> = with_model(app, |m| {
        targets
            .into_iter()
            .map(|target| {
                let token = next_token();
                let status = m.repo_status.entry(target.key.clone()).or_default();
                status.security_token = token;
                status.security_loading = true;
                (target, token)
            })
            .collect()
    });
    changed(app);
    let app = app.clone();
    std::thread::spawn(move || {
        tokens::ensure_loaded();
        github::token();
        let queue = Mutex::new(jobs);
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| loop {
                    let Some((target, token)) = queue.lock().unwrap_or_else(|e| e.into_inner()).pop_front() else { break };
                    if !with_model(&app, |m| m.repo_status.get(&target.key).is_some_and(|s| s.security_token == token)) {
                        continue;
                    }
                    let report = security::check_repo(&target.repo, target.website, steps::now());
                    with_model(&app, |m| {
                        let Some(status) = m.repo_status.get_mut(&target.key).filter(|s| s.security_token == token) else { return };
                        status.security_loading = false;
                        match report {
                            Ok(report) => {
                                status.security = Some(report);
                                status.security_error = None;
                            }
                            // An older check is worth more than none; it shows its age.
                            Err(e) => status.security_error = Some(format!("{e:#}")),
                        }
                    });
                    changed(&app);
                });
            }
        });
    });
}

/// "Sicherheit prüfen": all repositories, or the given ones.
#[tauri::command]
fn check_security(app: AppHandle, keys: Vec<String>) {
    check_security_of(&app, &keys);
}

/// Carries out fixes the user confirmed for one repository, then checks it again. Not while an
/// open job holds it: a new rule could refuse a step the job's plan counts on.
#[tauri::command]
async fn fix_security(app: AppHandle, key: String, fixes: Vec<security::Fix>) -> Result<Vec<security::FixResult>, String> {
    if fixes.is_empty() {
        return Err("Nichts ausgewählt".into());
    }
    if let Some(job) = with_model(&app, |m| held_by_job(m, &key)) {
        return Err(format!("Gehört zum offenen Auftrag {job}; erst abschließen oder beenden"));
    }
    let repo = with_model(&app, |m| m.config.repositories.iter().find(|r| r.key() == key).map(|r| r.repo.trim().to_owned())).ok_or("Repository wird nicht verwaltet")?;
    let results = blocking(move || security::apply(&repo, &fixes)).await.map_err(|e| format!("{e:#}"))?;
    check_security_of(&app, &[key]);
    Ok(results)
}

// MARK: Managing repositories

/// Whether an open job holds the repository; changing it under the job would invalidate its plan.
fn held_by_job(m: &Model, key: &str) -> Option<String> {
    m.jobs.iter().find(|j| j.open() && j.held().iter().any(|r| r == key)).map(|j| j.id.clone())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalRepo {
    /// `owner/name` (GitHub) or `host/group/…/name` (GitLab) as the clone's origin names it.
    repo: String,
    forge: Forge,
    path: String,
    /// Further clones of the same repository.
    others: Vec<String>,
    managed: bool,
    /// A website project: the clone has `.openreleasepilot/site.json`.
    is_website: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalRepoList {
    /// The folder that was looked at, as displayed.
    folder: String,
    repos: Vec<LocalRepo>,
}

/// The clones in a folder (the repo folder unless `dir` names another), GitHub and GitLab: the
/// selection the app offers. Reads only the clones' `.git/config`.
#[tauri::command]
async fn list_local_repos(app: AppHandle, dir: Option<String>) -> LocalRepoList {
    let config = with_model(&app, |m| m.config.clone());
    blocking(move || {
        let folder = match dir.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
            Some(dir) => expand_home(dir),
            None => config.workspace(),
        };
        let repos = localrepo::clones(&folder)
            .into_iter()
            .map(|clone| LocalRepo {
                forge: Forge::of(&clone.repo),
                managed: config.repository(&clone.repo).is_some(),
                is_website: site::is_project(&clone.path),
                path: clone.path.display().to_string(),
                others: clone.others.iter().map(|p| p.display().to_string()).collect(),
                repo: clone.repo,
            })
            .collect();
        LocalRepoList { folder: abbreviate_home(&folder), repos }
    })
    .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddRepo {
    repo: String,
    /// The local clone it was picked from; empty for a name typed in.
    #[serde(default)]
    local_path: String,
}

/// A repository as typed: `owner/name`, `host/group/name`, or the URL of either (`https://…`,
/// `git@host:group/name.git`).
fn typed_repo(text: &str) -> Result<String, String> {
    let text = text.trim();
    let repo = if text.contains("://") || text.starts_with("git@") { localrepo::repo_from_url(text) } else { Some(text.trim_end_matches('/').trim_end_matches(".git").to_owned()) };
    repo.filter(|r| forge::is_repo_id(r))
        .ok_or_else(|| format!("„{text}“ ist kein Repository: owner/name (GitHub), host/gruppe/name (GitLab) oder deren Adresse"))
}

/// Adds repositories to the managed list; one already managed is left as it is. A name typed in
/// must exist on GitHub or its GitLab instance; a local clone counts by its origin, and the
/// service's spelling (and GitHub's id) are taken when it can be asked (a private one without
/// login cannot). Returns the ones added.
#[tauri::command]
async fn add_repositories(app: AppHandle, repos: Vec<AddRepo>) -> Result<Vec<String>, String> {
    blocking(move || {
        let mut found = Vec::new();
        for entry in &repos {
            let repo = typed_repo(&entry.repo)?;
            let local = entry.local_path.trim();
            let origin = if local.is_empty() { None } else { localrepo::origin_repo(&expand_home(local)) };
            if !local.is_empty() && !origin.as_deref().is_some_and(|o| o.eq_ignore_ascii_case(&repo)) {
                return Err(format!("{local} ist keine Kopie von {repo}"));
            }
            match gitlab::project(&repo) {
                Some(project) => match (gitlab::project_info(&project), origin) {
                    (Ok(info), _) => found.push((info.id, None, local.to_owned())),
                    (Err(_), Some(origin)) => found.push((origin, None, local.to_owned())),
                    (Err(e), None) => return Err(format!("{e:#}{}", if gitlab::token(&project.host).is_none() { format!(" ({})", Forge::of(&repo).sign_in_hint()) } else { String::new() })),
                },
                None => match (github::repo_info(&repo), origin) {
                    (Ok(info), _) => found.push((info.full_name, Some(info.id), local.to_owned())),
                    (Err(_), Some(origin)) => found.push((origin, None, local.to_owned())),
                    (Err(e), None) => return Err(format!("{e:#}")),
                },
            }
        }
        let mut added = Vec::new();
        save_config(&app, |config| {
            for (repo, github_id, local_path) in found {
                if config.add_repository(ManagedRepo { github_id, local_path, ..ManagedRepo::new(&repo) }) {
                    added.push(repo);
                }
            }
            Ok(())
        })?;
        let keys: Vec<String> = added.iter().map(|r| r.to_lowercase()).collect();
        if !keys.is_empty() {
            refresh_status(&app, &keys);
        }
        Ok(added)
    })
    .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepoSettings {
    enabled: bool,
    local_path: String,
    release: Option<ReleaseRule>,
    /// Key of the managed repository whose release publishes this one; empty = none.
    #[serde(default)]
    released_by: String,
}

/// Changes how a repository is managed: on/off, local clone, release rule or the repository that
/// releases it. Not while an open job holds it.
#[tauri::command]
fn update_repository(app: AppHandle, key: String, settings: RepoSettings) -> Result<(), String> {
    if let Some(job) = with_model(&app, |m| held_by_job(m, &key)) {
        return Err(format!("Gehört zum offenen Auftrag {job}; erst abschließen oder beenden"));
    }
    if let Some(rule) = &settings.release {
        if rule.tag_prefix.trim() != rule.tag_prefix || rule.tag_prefix.chars().any(|c| !c.is_ascii_alphanumeric() && !"-_./".contains(c)) {
            return Err("Tag-Präfix: nur Buchstaben, Ziffern und - _ . /, keine Leerzeichen".into());
        }
    }
    save_config(&app, |config| {
        let released_by = settings.released_by.trim().to_lowercase();
        if let Some(why) = config.released_by_problem(&key, &released_by, settings.release.as_ref()) {
            return Err(format!("Veröffentlicht durch: {why}"));
        }
        if !released_by.is_empty() {
            let docs: Vec<String> = config
                .content_targets
                .iter()
                .filter(|t| t.kind == content::TargetKind::Docs && t.repo.eq_ignore_ascii_case(&key))
                .map(|t| t.name.clone())
                .collect();
            if !docs.is_empty() {
                return Err(format!("{} liegt hier und würde beim Export ersetzt; erst in {released_by} anlegen und hier entfernen", docs.join(", ")));
            }
        }
        let followers: Vec<String> = config.released_through(&key).iter().map(|r| r.repo.clone()).collect();
        if !followers.is_empty() && (settings.release.is_none() || !settings.enabled) {
            let what = if settings.enabled { "Release-Regel" } else { "Abschalten" };
            return Err(format!("{what}: veröffentlicht {}; ohne Tag hier kommt dort nichts an", followers.join(", ")));
        }
        let repo = config.repositories.iter_mut().find(|r| r.key() == key).ok_or("Repository wird nicht verwaltet")?;
        repo.enabled = settings.enabled;
        repo.local_path = settings.local_path.trim().to_owned();
        repo.release = settings.release;
        repo.released_by = released_by;
        Ok(())
    })
}

/// Takes a repository out of the managed list. Deletes nothing: not on GitHub, not the clone.
#[tauri::command]
fn remove_repository(app: AppHandle, key: String) -> Result<(), String> {
    if let Some(job) = with_model(&app, |m| held_by_job(m, &key)) {
        return Err(format!("Gehört zum offenen Auftrag {job}; erst abschließen oder beenden"));
    }
    save_config(&app, |config| {
        let users: Vec<String> = config.content_targets.iter().filter(|t| t.sources.contains(&key)).map(|t| t.name.clone()).collect();
        if !users.is_empty() {
            return Err(format!("Quelle von {}; dort erst entfernen", users.join(", ")));
        }
        let followers: Vec<String> = config.released_through(&key).iter().map(|r| r.repo.clone()).collect();
        if !followers.is_empty() {
            return Err(format!("veröffentlicht {}; dort erst „Veröffentlicht durch“ leeren", followers.join(", ")));
        }
        config.repositories.retain(|r| r.key() != key);
        Ok(())
    })?;
    with_model(&app, |m| m.repo_status.remove(&key));
    changed(&app);
    Ok(())
}

// MARK: Content targets

/// Saves a content target; an empty id makes a new one. Checked before it is stored.
#[tauri::command]
fn save_target(app: AppHandle, target: ContentTarget) -> Result<String, String> {
    let mut target = target;
    target.name = target.name.trim().to_owned();
    target.repo = target.repo.trim().to_owned();
    target.branch = target.branch.trim().to_owned();
    target.paths = target.paths.iter().map(|p| p.trim().to_owned()).filter(|p| !p.is_empty()).collect();
    target.sources.dedup();
    if !target.id.is_empty() {
        let id = target.id.clone();
        if with_model(&app, |m| m.jobs.iter().any(|j| j.open() && j.options.targets.iter().any(|t| t.id == id))) {
            return Err("Gehört zu einem offenen Auftrag; erst abschließen oder beenden".into());
        }
    }
    let mut id = target.id.clone();
    save_config(&app, |config| {
        let managed: Vec<String> = config.repositories.iter().map(|r| r.key()).collect();
        content::validate(&target, &managed)?;
        if let Some(why) = jobs::generated_target(&target, config) {
            return Err(why);
        }
        if target.id.is_empty() {
            let next = (1..).find(|n| !config.content_targets.iter().any(|t| t.id == format!("t{n}"))).expect("a free id");
            target.id = format!("t{next}");
            id = target.id.clone();
            config.content_targets.push(target);
        } else {
            let slot = config.content_targets.iter_mut().find(|t| t.id == target.id).ok_or("Ziel nicht gefunden")?;
            *slot = target;
        }
        Ok(())
    })?;
    Ok(id)
}

/// Removes a content target from the app; nothing in the repository or wiki changes.
#[tauri::command]
fn remove_target(app: AppHandle, id: String) -> Result<(), String> {
    if with_model(&app, |m| m.jobs.iter().any(|j| j.open() && j.options.targets.iter().any(|t| t.id == id))) {
        return Err("Gehört zu einem offenen Auftrag; erst abschließen oder beenden".into());
    }
    save_config(&app, |config| {
        config.content_targets.retain(|t| t.id != id);
        Ok(())
    })
}

// MARK: Jobs

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobInput {
    repos: Vec<String>,
    options: JobOptions,
}

/// Starts a care job for exactly these repositories and choices; its first pass begins at once.
#[tauri::command]
fn start_job(app: AppHandle, input: JobInput) -> Result<String, String> {
    if input.options.prs || input.options.cleanup || input.options.release {
        jobrun::require_logins(&input.repos).map_err(|e| format!("{e:#}"))?;
    }
    let mut created = String::new();
    steps::start_with(&app, |m| {
        let open: Vec<&Job> = m.jobs.iter().filter(|j| j.open()).collect();
        let scope = jobs::Scope { config: &m.config };
        let now = steps::now();
        let job = jobs::make(&jobs::new_id(now, next_token()), now, &input.repos, &input.options, &scope)?;
        if let Some(why) = jobs::overlap(&open, &job.held(), &job.options) {
            return Err(why);
        }
        jobs::save(&job).map_err(|e| format!("{e:#}"))?;
        created = job.id.clone();
        m.jobs.insert(0, job);
        Ok(vec![Work::Job(created.clone())])
    })?;
    Ok(created)
}

/// Continues a job that is ready, waits or was interrupted.
#[tauri::command]
fn continue_job(app: AppHandle, id: String) -> Result<(), String> {
    steps::start_with(&app, |m| {
        let job = m.jobs.iter().find(|j| j.id == id).ok_or("Auftrag nicht gefunden")?;
        if job.ended.is_some() {
            return Err("Der Auftrag ist beendet".into());
        }
        Ok(vec![Work::Job(id.clone())])
    })
}

/// Changes a job that is not running, stores it, and continues it when nothing else waits for a
/// decision.
fn decide(app: &AppHandle, id: &str, change: impl FnOnce(&mut Job, u64) -> Result<(), String>) -> Result<(), String> {
    let go_on = with_model(app, |m| {
        if m.job_running.as_deref() == Some(id) {
            return Err("Der Auftrag läuft gerade".to_owned());
        }
        let job = m.jobs.iter_mut().find(|j| j.id == id).ok_or("Auftrag nicht gefunden")?;
        let mut next = job.clone();
        change(&mut next, steps::now())?;
        jobs::save(&next).map_err(|e| format!("{e:#}"))?;
        *job = next;
        Ok(!job.steps.iter().any(|s| s.state == jobs::StepState::NeedsApproval) && job.next_runnable().is_some())
    })?;
    changed(app);
    if go_on {
        if let Err(e) = steps::start(app, vec![Work::Job(id.to_owned())]) {
            with_model(app, |m| m.notice = Some(format!("Entscheidung gespeichert; fortgesetzt wird, sobald der laufende Schritt fertig ist ({e}).")));
            changed(app);
        }
    }
    Ok(())
}

#[tauri::command]
fn approve_plan(app: AppHandle, id: String, plan_id: u64, excluded: Vec<String>, included: Vec<String>) -> Result<(), String> {
    decide(&app, &id, |job, now| jobs::approve_plan(job, plan_id, &excluded, &included, now))
}

#[tauri::command]
fn reject_plan(app: AppHandle, id: String, plan_id: u64) -> Result<(), String> {
    decide(&app, &id, |job, now| jobs::reject_plan(job, plan_id, now))
}

#[tauri::command]
fn approve_step(app: AppHandle, id: String, step: String, fingerprint: String) -> Result<(), String> {
    decide(&app, &id, |job, now| jobs::approve_step(job, &step, &fingerprint, now))
}

#[tauri::command]
fn reject_step(app: AppHandle, id: String, step: String) -> Result<(), String> {
    decide(&app, &id, |job, now| jobs::reject_step(job, &step, now))
}

#[tauri::command]
fn redo_step(app: AppHandle, id: String, step: String) -> Result<(), String> {
    decide(&app, &id, |job, now| jobs::redo_step(job, &step, now))
}

/// Ends a job: a running pass is cancelled (it ends the job itself), a waiting one is ended here.
#[tauri::command]
fn end_job(app: AppHandle, id: String) -> Result<(), String> {
    let runner = with_model(&app, |m| if m.job_running.as_deref() == Some(id.as_str()) { m.runner.clone() } else { None });
    if let Some(runner) = runner {
        runner.cancel();
        return Ok(());
    }
    with_model(&app, |m| {
        let job = m.jobs.iter_mut().find(|j| j.id == id).ok_or("Auftrag nicht gefunden")?;
        jobs::end(job, "von dir beendet", steps::now());
        jobs::save(job).map_err(|e| format!("{e:#}"))?;
        content::remove_work(&job.id);
        Ok::<(), String>(())
    })?;
    changed(&app);
    Ok(())
}

/// Removes a finished job from the history, with its log and work folder.
#[tauri::command]
fn delete_job(app: AppHandle, id: String) -> Result<(), String> {
    with_model(&app, |m| {
        let job = m.jobs.iter().find(|j| j.id == id).ok_or("Auftrag nicht gefunden")?;
        if job.open() {
            return Err("Nur abgeschlossene oder beendete Aufträge lassen sich entfernen".to_owned());
        }
        jobs::remove(&id);
        m.jobs.retain(|j| j.id != id);
        Ok(())
    })?;
    changed(&app);
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JobView {
    #[serde(flatten)]
    job: Job,
    status: jobs::Status,
    running: bool,
}

/// A job with everything: plan, items, proposals with their diffs.
#[tauri::command]
fn get_job(app: AppHandle, id: String) -> Result<JobView, String> {
    with_model(&app, |m| {
        let running = m.job_running.as_deref() == Some(id.as_str());
        let job = m.jobs.iter().find(|j| j.id == id).ok_or("Auftrag nicht gefunden")?;
        Ok(JobView { status: job.status(running), running, job: job.clone() })
    })
}

#[tauri::command]
async fn get_job_log(id: String) -> Vec<LogLine> {
    blocking(move || {
        jobs::read_log(&id)
            .into_iter()
            .map(|e| LogLine { kind: log_kind(&e.kind), text: e.text })
            .collect()
    })
    .await
}

fn log_kind(kind: &str) -> &'static str {
    match kind {
        "ok" => "ok",
        "warn" => "warn",
        "error" => "error",
        "cmd" => "cmd",
        "out" => "out",
        "claude" => "claude",
        _ => "info",
    }
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

#[tauri::command]
fn cancel(app: AppHandle) {
    if let Some(runner) = with_model(&app, |m| m.runner.clone()) {
        runner.cancel();
    }
}

fn main() {
    paths::migrate_legacy_data();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Shared(Mutex::new(Model::load())))
        .setup(|app| {
            let material = app.get_webview_window("main").is_some_and(|w| apply_window_material(&w));
            VIBRANCY.store(material, Ordering::Relaxed);
            // Without a menu bar icon (a Linux desktop without one) the app works as before.
            if let Err(e) = tray::init(app.handle()) {
                eprintln!("no menu bar icon: {e}");
            }
            // Like ClaudeKeyZapper: look for a new version once at start, quietly.
            let handle = app.handle().clone();
            if !cfg!(debug_assertions) && !with_model(&handle, |m| m.config.no_update_check) {
                std::thread::spawn(move || check_updates(&handle, false, false));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_view,
            save_settings,
            save_forge_token,
            forget_forge_token,
            pick_folder,
            pick_file,
            open_url,
            dismiss,
            dismiss_notice,
            refresh_repos,
            cancel_repo_refresh,
            load_repo_details,
            check_security,
            fix_security,
            add_repositories,
            update_repository,
            remove_repository,
            list_local_repos,
            save_target,
            remove_target,
            start_job,
            continue_job,
            approve_plan,
            reject_plan,
            approve_step,
            reject_step,
            redo_step,
            end_job,
            delete_job,
            get_job,
            get_job_log,
            check_for_updates,
            install_update,
            dismiss_update,
            clone_repo,
            cancel
        ])
        // Closing the window keeps the app in the menu bar, with running jobs going on; it ends
        // with "Beenden" there or Cmd+Q.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                if window.label() == "main" && app.tray_by_id(tray::ID).is_some() {
                    api.prevent_close();
                    let _ = window.hide();
                    // Without a window, no Dock icon either.
                    #[cfg(target_os = "macos")]
                    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while running OpenReleasePilot")
        .run(|_app, _event| {
            // A click on the Dock icon or opening the app again brings the window back.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = _event {
                show_window(_app);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(changes: serde_json::Value) -> SettingsInput {
        let mut base = serde_json::json!({ "workspaceDir": "", "updateCheck": true, "claudeModel": "", "maxBudgetUsd": "2" });
        base.as_object_mut().unwrap().extend(changes.as_object().unwrap().clone());
        serde_json::from_value(base).unwrap()
    }

    fn saved() -> Config {
        Config { max_budget_usd: Some(2.0), project_dir: "~/site".into(), ..Default::default() }
    }

    #[test]
    fn settings_are_built_without_changing_the_saved_ones() {
        let current = saved();
        let next = settings_config(&current, &input(serde_json::json!({ "workspaceDir": "~/Code", "maxBudgetUsd": "1,50" }))).unwrap();
        assert_eq!(next.workspace_dir, "~/Code");
        assert_eq!(next.max_budget_usd, Some(1.5));
        assert_eq!(current.max_budget_usd, Some(2.0), "the saved settings stay as they are");
        assert_eq!(next.project_dir, "~/site", "the old website settings stay for OpenSitePilot");
    }

    #[test]
    fn an_invalid_budget_is_refused_instead_of_removing_the_limit() {
        for bad in ["-5", "0", "viel"] {
            assert!(settings_config(&saved(), &input(serde_json::json!({ "maxBudgetUsd": bad }))).is_err(), "{bad}");
        }
        let next = settings_config(&saved(), &input(serde_json::json!({ "maxBudgetUsd": "" }))).unwrap();
        assert_eq!(next.max_budget_usd, None, "only an empty field removes it");
    }
}
