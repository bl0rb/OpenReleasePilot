//! The task catalog: which tasks the app offers for a repository, from what is in it (its
//! `profile`) and what GitHub or GitLab reported (`repos::Facts`). Each task has an autonomy level:
//!
//! - nur vorschlagen (`Propose`): only a hint; the app does nothing.
//! - mit Freigabe (`Approve`): the app prepares a proposal; anything outside the app happens only
//!   after the user approved exactly that proposal.
//! - automatisch (`Auto`): runs without asking; only for steps that read or stay reversible inside
//!   the app (e.g. building in a throwaway worktree). A task that writes is never `Auto`.
//!
//! The tasks that map onto existing job options (`review-prs`, `proof`, `release`,
//! `prepare-release`) only set their defaults and explain them; `dependabot-config` is a step of
//! its own (`task:<repo>:dependabot-config`), prepared in the job's work folder and delivered as a
//! pull request through the same path as the release preparation.

use crate::content::{self, PullRequestRef, SourceRev, TargetKind};
use crate::forge::Forge;
use crate::profile::{self, Profile};
use crate::repos::{CiState, Facts, RepoTarget};
use anyhow::Result;
use serde::Serialize;

pub const REVIEW_PRS: &str = "review-prs";
pub const PROOF: &str = "proof";
pub const RELEASE: &str = "release";
pub const PREPARE_RELEASE: &str = "prepare-release";
pub const DEPENDABOT_CONFIG: &str = "dependabot-config";
pub const SETUP_CI: &str = "setup-ci";

/// The tasks that are steps of their own in a job (`JobOptions::tasks`).
pub const STEP_TASKS: &[&str] = &[DEPENDABOT_CONFIG];

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Autonomy {
    Propose,
    Approve,
    Auto,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Offer {
    pub id: &'static str,
    pub title: String,
    /// Why it is offered, or why not now, from the profile and the facts.
    pub why: String,
    pub autonomy: Autonomy,
    /// Writes outside the app (on GitHub/GitLab); then never `Auto`.
    pub writes: bool,
    /// Preselected in the job dialog.
    pub default_on: bool,
    /// Why it cannot be chosen now; `None` = it can (a `Propose` task is never chosen).
    pub blocked: Option<String>,
}

fn offer(id: &'static str, title: &str, autonomy: Autonomy, writes: bool, why: impl Into<String>) -> Offer {
    Offer { id, title: title.to_owned(), why: why.into(), autonomy, writes, default_on: false, blocked: None }
}

/// Whether the repository has CI: `Some(true)` when the profile shows a configuration or the head
/// commit has checks, `Some(false)` when the profile shows none and the head has no checks either
/// (or was not asked), `None` when nothing tells.
pub fn has_ci(profile: Option<&Profile>, facts: Option<&Facts>) -> Option<bool> {
    let checks = facts.and_then(|f| f.ci.ok()).map(|ci| ci.state);
    if matches!(checks, Some(CiState::Passed | CiState::Failed | CiState::Pending)) || profile.is_some_and(|p| !p.ci.is_empty()) {
        return Some(true);
    }
    match (profile, checks) {
        (Some(_), None | Some(CiState::NoChecks)) => Some(false),
        _ => None,
    }
}

/// The offers for one repository, in the order the job dialog shows them.
pub fn offers(target: &RepoTarget, profile: Option<&Profile>, facts: Option<&Facts>) -> Vec<Offer> {
    let mut list = Vec::new();
    let word = Forge::of(&target.repo).request();

    let mut review = offer(REVIEW_PRS, &format!("{word}s prüfen und übernehmen"), Autonomy::Approve, true, "");
    match facts.and_then(|f| f.prs.ok()) {
        Some(prs) if prs.items.is_empty() => review.why = format!("Keine offenen {word}s."),
        Some(prs) => {
            review.default_on = true;
            review.why = format!("{} offene {word}s; Claude bewertet, gemergt wird erst nach deiner Freigabe des Plans.", prs.items.len());
        }
        None => {
            review.default_on = true;
            review.why = format!("Offene {word}s noch nicht gelesen; Claude bewertet, gemergt wird erst nach deiner Freigabe.");
        }
    }
    list.push(review);

    let mut proof = offer(PROOF, "Mit Nachweisen prüfen", Autonomy::Auto, false, "");
    match (target.clone_dir(), profile) {
        (None, _) => proof.blocked = Some("Keine lokale Kopie: Claude kann nur lesen.".into()),
        (Some(_), Some(p)) if !p.tests.is_empty() => {
            proof.default_on = true;
            proof.why = format!("Tests erkannt ({}): Claude baut und testet jeden {word} in einer Wegwerf-Kopie, ohne zu pushen.", p.tests.join(", "));
        }
        (Some(_), Some(_)) => proof.why = "Keine Tests erkannt: ein Build ist möglich, belegt aber wenig.".into(),
        (Some(_), None) => {
            proof.default_on = true;
            proof.why = "Noch keine Analyse der lokalen Kopie; Claude baut und testet in einer Wegwerf-Kopie.".into();
        }
    }
    list.push(proof);

    let ci = has_ci(profile, facts);
    let mut release = offer(RELEASE, "Release: nächste Version taggen", Autonomy::Approve, true, "");
    if let Some(by) = &target.released_by {
        release.blocked = Some(format!("Wird durch {by} veröffentlicht; die App taggt hier nicht."));
    } else if target.release.is_none() {
        release.blocked = Some("Keine Release-Regel; einrichten unter Verwaltung.".into());
    } else if ci == Some(false) {
        release.blocked = Some("Keine CI erkannt: ohne grüne CI genau des Ziel-Commits setzt die App keinen Tag.".into());
    } else {
        release.why = match profile.map(|p| &p.ci) {
            Some(files) if !files.is_empty() => format!("CI erkannt ({}); getaggt wird nur auf grüner CI.", files.join(", ")),
            _ => "Getaggt wird nur auf grüner CI des Ziel-Commits.".into(),
        };
    }
    list.push(release);

    if target.release.is_some() && target.released_by.is_none() {
        let mut prepare = offer(PREPARE_RELEASE, "Vorher Versionsdateien und CHANGELOG anpassen", Autonomy::Approve, true, "");
        match profile {
            Some(p) if p.changelog.is_some() || !p.version_files.is_empty() => {
                prepare.default_on = true;
                let mut found: Vec<String> = p.changelog.iter().cloned().collect();
                found.extend(p.version_files.iter().take(4).cloned());
                prepare.why = format!("Gefunden: {}; der Diff kommt vor dem Tag zur Freigabe.", found.join(", "));
            }
            Some(_) => prepare.why = "Kein CHANGELOG.md und keine Versionsdatei erkannt; der Tag folgt direkt.".into(),
            None => {
                prepare.default_on = true;
                prepare.why = "Noch keine Analyse; ohne passende Datei folgt der Tag direkt.".into();
            }
        }
        list.push(prepare);
    }

    if let Some(dependabot) = dependabot_offer(&target.repo, target.released_by.as_deref(), profile) {
        list.push(dependabot);
    }

    if ci == Some(false) {
        list.push(offer(
            SETUP_CI,
            "CI einrichten",
            Autonomy::Propose,
            false,
            "Keine CI-Konfiguration und keine Checks auf dem Default-Branch: Merges und Tags bleiben ohne Nachweis. Die App richtet keine CI ein; das ist ein Hinweis.",
        ));
    }
    list
}

/// The offer to add a Dependabot configuration: GitHub, ecosystems detected, neither Dependabot
/// nor Renovate configured. `None` when it does not apply.
pub fn dependabot_offer(repo: &str, released_by: Option<&str>, profile: Option<&Profile>) -> Option<Offer> {
    let p = profile?;
    if Forge::of(repo) != Forge::Github || p.dependabot.is_some() || p.renovate.is_some() {
        return None;
    }
    let targets = p.update_targets();
    if targets.is_empty() {
        return None;
    }
    let names: Vec<&str> = targets.iter().map(|(ecosystem, _)| *ecosystem).collect();
    let mut o = offer(
        DEPENDABOT_CONFIG,
        "Dependabot-Konfiguration vorschlagen",
        Autonomy::Approve,
        true,
        format!(
            "Keine dependabot.yml und kein Renovate, aber {} erkannt. Die App schreibt .github/dependabot.yml (wöchentlich), zeigt den Diff und eröffnet nach deiner Freigabe einen PR; mergen tust du.",
            names.join(", ")
        ),
    );
    if let Some(by) = released_by {
        o.blocked = Some(format!("Wird durch {by} veröffentlicht, dessen Export die Dateien ersetzt; dort einrichten."));
    }
    Some(o)
}

/// `.github/dependabot.yml` for the profile's ecosystems, weekly.
pub fn dependabot_yml(profile: &Profile) -> String {
    let mut text = String::from("# Vorgeschlagen von OpenReleasePilot: wöchentliche Updates für die erkannten Abhängigkeiten.\nversion: 2\nupdates:\n");
    for (ecosystem, dirs) in profile.update_targets() {
        text.push_str(&format!("  - package-ecosystem: \"{ecosystem}\"\n"));
        match dirs.as_slice() {
            [dir] => text.push_str(&format!("    directory: \"{dir}\"\n")),
            dirs => {
                text.push_str("    directories:\n");
                for dir in dirs {
                    text.push_str(&format!("      - \"{dir}\"\n"));
                }
            }
        }
        text.push_str("    schedule:\n      interval: \"weekly\"\n");
    }
    text
}

// MARK: Running a task step

pub const DEPENDABOT_FILE: &str = ".github/dependabot.yml";

/// The branch of the task's pull request; one per task, so an interrupted attempt is found again.
pub fn branch(task: &str) -> String {
    format!("openreleasepilot/{task}")
}

fn work_dir(job: &str, key: &str, task: &str) -> std::path::PathBuf {
    content::work_dir(job, &format!("task-{}-{task}", key.replace(['/', ':'], "-")))
}

pub enum Prepared {
    Proposal(Box<content::Proposal>),
    /// Nothing to do any more, and why.
    Nothing(String),
}

/// Clones the default branch into the job's work folder, profiles exactly that commit and writes
/// the task's file there. Writes only below the work folder.
pub fn prepare(job: &str, key: &str, repo: &str, task: &str, now: u64) -> Result<Prepared> {
    anyhow::ensure!(task == DEPENDABOT_CONFIG, "Unbekannte Aufgabe {task}");
    let work = work_dir(job, key, task);
    let prepared = content::prepare(&work, &content::github_remote(repo), None, &[], &|_| String::new())?;
    let dir = &prepared.dir;
    let current = profile::read(dir, &prepared.base_commit, &prepared.base_branch, now)?;
    if let Some(file) = current.dependabot.as_ref().or(current.renovate.as_ref()) {
        return Ok(Prepared::Nothing(format!("{file} gibt es inzwischen; nichts vorzuschlagen")));
    }
    if current.update_targets().is_empty() {
        return Ok(Prepared::Nothing("keine Abhängigkeiten erkannt; nichts vorzuschlagen".into()));
    }
    let path = dir.join(DEPENDABOT_FILE);
    std::fs::create_dir_all(path.parent().expect("has a folder"))?;
    std::fs::write(&path, dependabot_yml(&current))?;
    let changes = match content::collect_files(&prepared, &[DEPENDABOT_FILE])? {
        Ok(changes) => changes,
        Err(problems) => anyhow::bail!("Vorschlag verworfen: {}", problems.join("; ")),
    };
    let checks = content::checks(&prepared, &changes, TargetKind::Docs);
    let source = SourceRev {
        key: key.to_owned(),
        repo: repo.to_owned(),
        basis: content::Basis::Commit,
        rev: prepared.base_commit.clone(),
        label: format!("{} {}", prepared.base_branch, content::short(&prepared.base_commit)),
    };
    let ecosystems: Vec<String> = current.update_targets().iter().map(|(e, dirs)| format!("{e} ({})", dirs.join(", "))).collect();
    let summary = format!(
        "Fügt `{DEPENDABOT_FILE}` hinzu: Dependabot schlägt wöchentlich Updates vor für {}.\n\nDie App hat die Datei aus den erkannten Manifesten erzeugt, ohne Claude.",
        ecosystems.join(", ")
    );
    let proposal = content::Proposal::standalone(&format!("task:{key}:{task}"), &prepared, vec![source], changes, checks, summary, branch(task));
    Ok(Prepared::Proposal(Box::new(proposal)))
}

/// Opens the task's pull request with exactly the approved files on top of the proposal's base;
/// stops when someone changed the same file meanwhile, picks up what an interrupted attempt left.
pub fn publish(job: &str, key: &str, repo: &str, task: &str, proposal: &content::Proposal) -> Result<PullRequestRef> {
    let dir = work_dir(job, key, task).join("target");
    let title = "Dependabot-Konfiguration hinzufügen".to_owned();
    let body = format!(
        "{}\n\nVorgeschlagen von OpenReleasePilot (Auftrag {job}); die Änderung wurde in der App geprüft und freigegeben. \
         Die App mergt diesen PR nicht selbst.",
        proposal.summary.trim()
    );
    content::publish_docs(repo, &dir, proposal, &title, &body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ReleaseRule;
    use crate::profile::{Stack, StackKind};
    use crate::repos::{Ci, Fetch, LocalCopy};

    fn target(release: bool) -> RepoTarget {
        RepoTarget {
            key: "o/app".into(),
            repo: "o/app".into(),
            forge: Forge::Github,
            github_id: None,
            enabled: true,
            release: release.then(|| ReleaseRule { tag_prefix: "v".into(), publishes_release: false }),
            released_by: None,
            local: LocalCopy::Found { path: "/tmp/app".into() },
            website: false,
        }
    }

    fn rust_app() -> Profile {
        Profile {
            commit: "abc".into(),
            stacks: vec![Stack { kind: StackKind::Rust, manifests: vec!["Cargo.toml".into()] }],
            ci: vec![".github/workflows/ci.yml".into()],
            tests: vec!["tests/".into()],
            changelog: Some("CHANGELOG.md".into()),
            version_files: vec!["Cargo.toml".into()],
            ..Profile::default()
        }
    }

    fn facts_with_ci(state: CiState) -> Facts {
        let mut facts = Facts::new(1, Fetch::Failed("x".into()), false);
        facts.ci = Fetch::Ok(Ci { state, sha: "abc".into(), detail: String::new() });
        facts
    }

    fn find<'a>(offers: &'a [Offer], id: &str) -> Option<&'a Offer> {
        offers.iter().find(|o| o.id == id)
    }

    #[test]
    fn a_writing_task_is_never_automatic() {
        for profile in [None, Some(rust_app()), Some(Profile { ci: vec![], ..rust_app() })] {
            for o in offers(&target(true), profile.as_ref(), None) {
                assert!(!(o.writes && o.autonomy == Autonomy::Auto), "{} writes and runs automatically", o.id);
                assert!(!(o.autonomy == Autonomy::Propose && o.writes), "{} only proposes but writes", o.id);
            }
        }
    }

    #[test]
    fn no_ci_means_no_tag_explained_and_a_hint() {
        let bare = Profile { ci: vec![], ..rust_app() };
        let list = offers(&target(true), Some(&bare), Some(&facts_with_ci(CiState::NoChecks)));
        assert!(find(&list, RELEASE).unwrap().blocked.as_deref().unwrap().contains("Keine CI"));
        assert_eq!(find(&list, SETUP_CI).unwrap().autonomy, Autonomy::Propose);
        // Checks on the head commit count as CI even without a configuration file (an external CI).
        let list = offers(&target(true), Some(&bare), Some(&facts_with_ci(CiState::Passed)));
        assert_eq!(find(&list, RELEASE).unwrap().blocked, None);
        assert!(find(&list, SETUP_CI).is_none());
        // Without a profile, missing checks prove nothing.
        let list = offers(&target(true), None, Some(&facts_with_ci(CiState::NoChecks)));
        assert_eq!(find(&list, RELEASE).unwrap().blocked, None);
        assert!(offers(&target(false), Some(&rust_app()), None).iter().any(|o| o.id == RELEASE && o.blocked.as_deref().is_some_and(|b| b.contains("Release-Regel"))));
    }

    #[test]
    fn changelog_or_version_files_turn_the_preparation_on() {
        let list = offers(&target(true), Some(&rust_app()), None);
        let prepare = find(&list, PREPARE_RELEASE).unwrap();
        assert!(prepare.default_on && prepare.why.contains("CHANGELOG.md"));
        let plain = Profile { changelog: None, version_files: vec![], ..rust_app() };
        assert!(!find(&offers(&target(true), Some(&plain), None), PREPARE_RELEASE).unwrap().default_on);
        assert!(find(&offers(&target(false), Some(&rust_app()), None), PREPARE_RELEASE).is_none(), "only with a release rule");
    }

    #[test]
    fn proofs_by_default_only_with_tests_and_a_clone() {
        assert!(find(&offers(&target(true), Some(&rust_app()), None), PROOF).unwrap().default_on);
        let untested = Profile { tests: vec![], ..rust_app() };
        assert!(!find(&offers(&target(true), Some(&untested), None), PROOF).unwrap().default_on);
        let without_clone = RepoTarget { local: LocalCopy::NotConfigured, ..target(true) };
        assert!(find(&offers(&without_clone, Some(&rust_app()), None), PROOF).unwrap().blocked.is_some());
    }

    #[test]
    fn dependabot_is_offered_only_where_nothing_updates_dependencies() {
        let o = dependabot_offer("o/app", None, Some(&rust_app())).unwrap();
        assert_eq!((o.autonomy, o.writes, o.default_on, o.blocked.clone()), (Autonomy::Approve, true, false, None));
        assert!(o.why.contains("cargo, github-actions"), "{}", o.why);
        assert!(dependabot_offer("o/app", None, Some(&Profile { dependabot: Some(".github/dependabot.yml".into()), ..rust_app() })).is_none());
        assert!(dependabot_offer("o/app", None, Some(&Profile { renovate: Some("renovate.json".into()), ..rust_app() })).is_none());
        assert!(dependabot_offer("o/app", None, Some(&Profile { stacks: vec![], ci: vec![], ..rust_app() })).is_none(), "nothing to update");
        assert!(dependabot_offer("o/app", None, None).is_none(), "not without a profile");
        assert!(dependabot_offer("o/app", Some("o/ee"), Some(&rust_app())).unwrap().blocked.is_some());
    }

    #[test]
    fn the_dependabot_file_covers_each_ecosystem() {
        let profile = Profile {
            stacks: vec![
                Stack { kind: StackKind::Rust, manifests: vec!["desktop/Cargo.toml".into()] },
                Stack { kind: StackKind::Node, manifests: vec!["web/package.json".into(), "package.json".into()] },
            ],
            ..rust_app()
        };
        let yml = dependabot_yml(&profile);
        assert_eq!(
            yml,
            "# Vorgeschlagen von OpenReleasePilot: wöchentliche Updates für die erkannten Abhängigkeiten.\nversion: 2\nupdates:\n\
             \x20 - package-ecosystem: \"cargo\"\n    directory: \"/desktop\"\n    schedule:\n      interval: \"weekly\"\n\
             \x20 - package-ecosystem: \"npm\"\n    directories:\n      - \"/\"\n      - \"/web\"\n    schedule:\n      interval: \"weekly\"\n\
             \x20 - package-ecosystem: \"github-actions\"\n    directory: \"/\"\n    schedule:\n      interval: \"weekly\"\n"
        );
    }
}
