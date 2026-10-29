//! The menu bar icon: what waits for the user and how the repositories stand, at a glance, and a
//! way back into the window. It shows what the app already knows; it never reads anything remote
//! by itself.

use crate::{refresh_status, repo_targets, show_window, steps, with_model};
use openreleasepilot_core::jobs::JobState;
use openreleasepilot_core::repos::{self, HealthState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Wry};

pub const ID: &str = "main";
/// How many repositories or jobs the menu names; the rest are counted.
const LISTED: usize = 6;
/// Longer reasons are cut so the menu stays narrow.
const REASON_CHARS: usize = 70;

static PENDING: AtomicBool = AtomicBool::new(false);

struct Flagged {
    key: String,
    repo: String,
    state: HealthState,
    reason: String,
}

struct OpenJob {
    id: String,
    repos: Vec<String>,
    state: JobState,
    reason: String,
}

/// What the menu shows, read from the model in one go.
struct Summary {
    version: String,
    repos: usize,
    blocked: usize,
    attention: usize,
    unknown: usize,
    healthy: usize,
    reading: bool,
    /// Blocked first, then those needing attention.
    flagged: Vec<Flagged>,
    /// Decisions first, then running, blocked, waiting and planned jobs.
    jobs: Vec<OpenJob>,
    update: Option<String>,
}

impl Summary {
    fn decisions(&self) -> usize {
        self.jobs.iter().filter(|j| j.state == JobState::NeedsApproval).count()
    }
}

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let summary = summary(app);
    TrayIconBuilder::with_id(ID)
        .icon(tauri::include_image!("icons/tray.png"))
        .icon_as_template(true)
        .show_menu_on_left_click(true)
        .menu(&menu(app, &summary)?)
        .on_menu_event(|app, event| clicked(app, event.id().as_ref()))
        .build(app)?;
    decorate(app, &summary);
    Ok(())
}

/// Rebuilds the menu shortly after a change; a burst of changes (status reads, log-heavy job
/// passes) rebuilds it once.
pub fn update(app: &AppHandle) {
    if PENDING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        // A change from now on schedules the next rebuild.
        PENDING.store(false, Ordering::SeqCst);
        let summary = summary(&app);
        if let (Some(tray), Ok(menu)) = (app.tray_by_id(ID), menu(&app, &summary)) {
            let _ = tray.set_menu(Some(menu));
        }
        decorate(&app, &summary);
    });
}

fn summary(app: &AppHandle) -> Summary {
    let version = app.package_info().version.to_string();
    // Reads .git/config of the clones: outside the lock.
    let (site, config) = with_model(app, |m| (m.site.clone(), m.config.clone()));
    let targets = repo_targets(&config, &site);
    let now = steps::now();
    with_model(app, |m| {
        let mut summary = Summary {
            version,
            repos: targets.len(),
            blocked: 0,
            attention: 0,
            unknown: 0,
            healthy: 0,
            reading: false,
            flagged: Vec::new(),
            jobs: Vec::new(),
            update: m.available_update.as_ref().map(|u| u.version.clone()),
        };
        for target in &targets {
            let status = m.repo_status.get(&target.key);
            summary.reading |= status.is_some_and(|s| s.loading);
            let health = repos::health_with(target, status.and_then(|s| s.facts.as_ref()), status.and_then(|s| s.details.as_ref()), crate::upstream_of(m, target), now);
            match health.state {
                HealthState::Blocked => summary.blocked += 1,
                HealthState::NeedsAttention => summary.attention += 1,
                HealthState::Unknown => summary.unknown += 1,
                HealthState::Healthy => summary.healthy += 1,
            }
            if matches!(health.state, HealthState::Blocked | HealthState::NeedsAttention) {
                summary.flagged.push(Flagged {
                    key: target.key.clone(),
                    repo: target.repo.clone(),
                    state: health.state,
                    reason: health.reasons.first().cloned().unwrap_or_default(),
                });
            }
        }
        summary.flagged.sort_by_key(|f| f.state != HealthState::Blocked);
        summary.jobs = m
            .jobs
            .iter()
            .filter(|j| j.open())
            .map(|j| {
                let status = j.status(m.job_running.as_deref() == Some(j.id.as_str()));
                OpenJob { id: j.id.clone(), repos: j.repos.clone(), state: status.state, reason: status.reason }
            })
            .collect();
        summary.jobs.sort_by_key(|j| job_order(j.state));
        summary
    })
}

fn job_order(state: JobState) -> u8 {
    match state {
        JobState::NeedsApproval => 0,
        JobState::Running | JobState::Preparing => 1,
        JobState::Blocked => 2,
        JobState::Waiting => 3,
        _ => 4,
    }
}

/// The job states as the window names them.
fn job_state(state: JobState) -> &'static str {
    match state {
        JobState::Planned => "Geplant",
        JobState::Preparing => "In Vorbereitung",
        JobState::NeedsApproval => "Freigabe nötig",
        JobState::Running => "Läuft",
        JobState::Waiting => "Wartet",
        JobState::Succeeded => "Erfolgreich",
        JobState::Partial => "Teilweise erfolgreich",
        JobState::Blocked => "Blockiert",
        JobState::Failed => "Fehlgeschlagen",
        JobState::Cancelled => "Abgebrochen",
    }
}

fn job_marker(state: JobState) -> &'static str {
    match state {
        JobState::NeedsApproval => "🟠",
        JobState::Running | JobState::Preparing => "🔵",
        JobState::Blocked => "🔴",
        _ => "⚪️",
    }
}

fn cut(text: &str) -> String {
    let text = text.trim();
    if text.chars().count() <= REASON_CHARS {
        return text.to_owned();
    }
    let short: String = text.chars().take(REASON_CHARS - 1).collect();
    format!("{}…", short.trim_end())
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn menu(app: &AppHandle, s: &Summary) -> tauri::Result<Menu<Wry>> {
    let mut items: Vec<Box<dyn IsMenuItem<Wry>>> = Vec::new();
    let info = |text: String| MenuItem::new(app, text, false, None::<&str>);
    let link = |id: String, text: String| MenuItem::with_id(app, id, text, true, None::<&str>);
    let separator = || PredefinedMenuItem::separator(app);

    items.push(Box::new(info(format!("OpenReleasePilot {}", s.version))?));
    if let Some(version) = &s.update {
        items.push(Box::new(link("open".into(), format!("⬆️ Version {version} ist verfügbar …"))?));
    }

    // Repositories
    items.push(Box::new(separator()?));
    let mut head = if s.repos == 0 { "Noch keine Repositories ausgewählt".to_owned() } else { plural(s.repos, "Repository", "Repositories") };
    if s.reading {
        head.push_str(" · Status wird gelesen …");
    }
    items.push(Box::new(info(head)?));
    for (n, marker, one, many) in [
        (s.blocked, "🔴", "blockiert", "blockiert"),
        (s.attention, "🟠", "braucht Aufmerksamkeit", "brauchen Aufmerksamkeit"),
        (s.unknown, "⚪️", "unbekannt", "unbekannt"),
        (s.healthy, "🟢", "in Ordnung", "in Ordnung"),
    ] {
        if n > 0 {
            items.push(Box::new(info(format!("{marker} {}", plural(n, one, many)))?));
        }
    }
    if !s.flagged.is_empty() {
        let sub: Vec<MenuItem<Wry>> = s
            .flagged
            .iter()
            .take(LISTED)
            .map(|f| {
                let marker = if f.state == HealthState::Blocked { "🔴" } else { "🟠" };
                let reason = if f.reason.is_empty() { String::new() } else { format!(" – {}", cut(&f.reason)) };
                link(format!("repo:{}", f.key), format!("{marker} {}{reason}", f.repo))
            })
            .collect::<tauri::Result<_>>()?;
        items.extend(sub.into_iter().map(|i| Box::new(i) as Box<dyn IsMenuItem<Wry>>));
        if s.flagged.len() > LISTED {
            items.push(Box::new(link("repos".into(), format!("… und {} weitere", s.flagged.len() - LISTED))?));
        }
    }

    // Jobs
    items.push(Box::new(separator()?));
    let decisions = s.decisions();
    items.push(Box::new(info(match (s.jobs.len(), decisions) {
        (0, _) => "Keine offenen Aufträge".to_owned(),
        (open, 0) => plural(open, "offener Auftrag", "offene Aufträge"),
        (open, d) => format!(
            "{} · {}",
            plural(open, "offener Auftrag", "offene Aufträge"),
            plural(d, "wartet auf eine Entscheidung", "warten auf eine Entscheidung")
        ),
    })?));
    for job in s.jobs.iter().take(LISTED) {
        let repos = match job.repos.as_slice() {
            [] => "Auftrag".to_owned(),
            [one] => one.clone(),
            [first, rest @ ..] => format!("{first} +{}", rest.len()),
        };
        let reason = if job.reason.is_empty() { job_state(job.state).to_owned() } else { cut(&job.reason) };
        items.push(Box::new(link(format!("job:{}", job.id), format!("{} {repos} – {reason}", job_marker(job.state)))?));
    }
    if s.jobs.len() > LISTED {
        items.push(Box::new(link("runs".into(), format!("… und {} weitere", s.jobs.len() - LISTED))?));
    }

    // Actions
    items.push(Box::new(separator()?));
    items.push(Box::new(MenuItem::with_id(app, "refresh", "Status aktualisieren", s.repos > 0 && !s.reading, None::<&str>)?));
    items.push(Box::new(MenuItem::with_id(app, "open", "OpenReleasePilot öffnen", true, None::<&str>)?));
    items.push(Box::new(separator()?));
    items.push(Box::new(PredefinedMenuItem::quit(app, Some("OpenReleasePilot beenden"))?));

    let refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|i| i.as_ref()).collect();
    Menu::with_items(app, &refs)
}

/// The number next to the icon (decisions waiting for the user) and the tooltip.
fn decorate(app: &AppHandle, s: &Summary) {
    let Some(tray) = app.tray_by_id(ID) else { return };
    let decisions = s.decisions();
    let _ = tray.set_title(if decisions > 0 { Some(decisions.to_string()) } else { None });
    let mut parts = Vec::new();
    if decisions > 0 {
        parts.push(plural(decisions, "Entscheidung offen", "Entscheidungen offen"));
    }
    if s.blocked > 0 {
        parts.push(format!("{} blockiert", s.blocked));
    }
    if s.attention > 0 {
        parts.push(format!("{} mit Aufmerksamkeit", s.attention));
    }
    let tooltip = if parts.is_empty() { "OpenReleasePilot".to_owned() } else { format!("OpenReleasePilot – {}", parts.join(", ")) };
    let _ = tray.set_tooltip(Some(tooltip));
}

fn clicked(app: &AppHandle, id: &str) {
    match id {
        "open" => show_window(app),
        "refresh" => {
            let app = app.clone();
            std::thread::spawn(move || refresh_status(&app, &[]));
        }
        "repos" | "runs" => {
            show_window(app);
            let _ = app.emit_to("main", "show", serde_json::json!({ "area": id }));
        }
        _ => {
            if let Some(key) = id.strip_prefix("repo:") {
                show_window(app);
                let _ = app.emit_to("main", "show", serde_json::json!({ "repo": key }));
            } else if let Some(job) = id.strip_prefix("job:") {
                show_window(app);
                let _ = app.emit_to("main", "show", serde_json::json!({ "job": job }));
            }
        }
    }
}
