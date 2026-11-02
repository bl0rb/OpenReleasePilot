//! The passes of care jobs (see `jobrun`). Each runs on its own thread, writes log lines to its
//! job's log as it goes and ends with a status and a one-line summary.

use crate::{changed, emit_to_visible, with_model, LogLine, Model, Status, Step};
use anyhow::Result;
use openreleasepilot_core::claude;
use openreleasepilot_core::config::Config;
use openreleasepilot_core::jobs;
use openreleasepilot_core::shell::{self, Runner};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

const MAX_LOG_LINES: usize = 5000;

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub struct Ctx {
    pub(crate) app: AppHandle,
    step: Step,
    pub(crate) runner: Arc<Runner>,
    /// The care job this pass belongs to; its lines go to the job's own log.
    pub(crate) job: Option<String>,
}

impl Ctx {
    pub(crate) fn log(&self, kind: &'static str, text: impl Into<String>) {
        let text = strip_ansi(&text.into());
        let line = LogLine { kind, text };
        if let Some(job) = &self.job {
            let _ = jobs::append_log(job, &jobs::LogEntry { kind: kind.to_owned(), text: line.text.clone() });
            emit_to_visible(&self.app, "log", serde_json::json!({ "step": self.step, "job": job, "line": line }));
            return;
        }
        with_model(&self.app, |m| {
            if let Some(state) = m.steps.get_mut(&self.step) {
                if state.log.len() >= MAX_LOG_LINES {
                    state.log.drain(..MAX_LOG_LINES / 10);
                }
                state.log.push(line.clone());
            }
        });
        emit_to_visible(&self.app, "log", serde_json::json!({ "step": self.step, "line": line }));
    }
    pub(crate) fn info(&self, text: impl Into<String>) {
        self.log("info", text)
    }
    pub(crate) fn ok(&self, text: impl Into<String>) {
        self.log("ok", text)
    }
    pub(crate) fn warn(&self, text: impl Into<String>) {
        self.log("warn", text)
    }
    pub(crate) fn cmd(&self, text: impl Into<String>) {
        self.log("cmd", text)
    }

    pub(crate) fn config(&self) -> Config {
        with_model(&self.app, |m| m.config.clone())
    }
}

/// One unit of work.
pub enum Work {
    /// One pass of a care job: runs until it needs a decision, waits or is finished.
    Job(String),
}

/// `runner` is set as soon as a start is accepted, `running` only once its thread runs: either means busy.
fn busy(runner: Option<&Arc<Runner>>, running: Option<Step>) -> bool {
    runner.is_some() || running.is_some()
}

/// Runs the work one after another on a background thread; stops at the first error.
pub fn start(app: &AppHandle, work: Vec<Work>) -> Result<(), String> {
    start_with(app, |_| Ok(work))
}

/// Like `start`, but `work` is built under the same lock that marks the app busy: checking a job's
/// state and starting its pass happen together, so a second click finds the app busy.
pub fn start_with(app: &AppHandle, work: impl FnOnce(&mut Model) -> Result<Vec<Work>, String>) -> Result<(), String> {
    let runner = Arc::new(Runner::default());
    let work = with_model(app, |m| {
        if busy(m.runner.as_ref(), m.running) {
            return Err("Es läuft bereits ein Schritt oder Auftrag.".to_owned());
        }
        let work = work(m)?;
        m.runner = Some(runner.clone());
        m.error = None;
        m.notice = None;
        if let Some(Work::Job(id)) = work.first() {
            m.job_running = Some(id.clone());
        }
        Ok(work)
    })?;
    let app = app.clone();
    std::thread::spawn(move || {
        for Work::Job(id) in work {
            let step = Step::Jobs;
            let ctx = Ctx { app: app.clone(), step, runner: runner.clone(), job: Some(id.clone()) };
            with_model(&app, |m| {
                m.running = Some(step);
                let state = m.steps.entry(step).or_default();
                state.status = Status::Running;
                state.summary.clear();
                state.log.clear();
            });
            changed(&app);
            let (status, summary) = match crate::jobrun::pass(&ctx, &id) {
                Ok(done) => done,
                Err(e) => {
                    let message = format!("{e:#}");
                    ctx.log("error", message.clone());
                    (Status::Error, message)
                }
            };
            with_model(&app, |m| {
                let state = m.steps.entry(step).or_default();
                state.status = status;
                state.summary = summary;
                state.finished_at = Some(now());
            });
            changed(&app);
            if status == Status::Error || runner.is_cancelled() {
                break;
            }
        }
        with_model(&app, |m| {
            m.running = None;
            m.runner = None;
            m.job_running = None;
        });
        changed(&app);
    });
    Ok(())
}

/// Brings Claude Code up to date before it runs, at most every six hours: aliases like `opus` or
/// `fable` point to the newest model only in a current CLI, and one that only ever runs headless
/// (as here) does not update itself. A failed update only warns.
pub(crate) fn update_claude(ctx: &Ctx, dir: &std::path::Path) -> Result<()> {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if last.is_some_and(|at| at.elapsed() < Duration::from_secs(6 * 3600)) {
        return Ok(());
    }
    ctx.cmd("$ claude update");
    let mut output = String::new();
    let result = shell::stream("claude", &["update".to_owned()], dir, &HashMap::new(), &ctx.runner, |line| {
        output.push_str(line);
        output.push('\n');
    });
    match result {
        Err(e) if ctx.runner.is_cancelled() => return Err(e),
        Err(e) => ctx.warn(format!("  {e:#}; es läuft die installierte Version")),
        Ok(0) => ctx.info(format!("  {}", claude::update_note(&output))),
        Ok(_) => ctx.warn(format!("  Nicht aktualisiert: {}; es läuft die installierte Version", claude::update_note(&output))),
    }
    *last = Some(Instant::now());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_start_counts_as_busy_before_its_thread_runs() {
        let runner = Arc::new(Runner::default());
        assert!(busy(Some(&runner), None), "a second click right after the first");
        assert!(busy(None, Some(Step::Jobs)));
        assert!(!busy(None, None));
    }
}
