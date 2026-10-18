//! Repairs: when an update needs the code adapted (a major whose API changed), Claude writes the
//! adaptation in a throwaway worktree on the update's commit, and the app checks the changed files
//! and opens a pull request of its own from `openreleasepilot/fix-<n>`. The update's own branch stays
//! untouched (Dependabot stops rebasing a branch with foreign commits); once the repair is merged,
//! the bot closes its pull request itself.
//!
//! The repair is a new pull request like any other: the next care plan waits for its CI, a separate
//! Claude review judges its diff, and the user approves the merge. A repair that touches update,
//! signature or credential code (`sensitive`) is only merged when the user ticks it.

use crate::content::{self, ChangedFile, Changes, Proposal, PullRequestRef};
use crate::repocare::{Plan, PullRequest};
use crate::shell::output;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

const FIX_PREFIX: &str = "openreleasepilot/fix-";
const MAX_FILES: usize = 40;
const MAX_FILE: u64 = 1024 * 1024;
const MAX_DIFF: usize = 400 * 1024;

/// The branch of the repair of pull request `number`; one per pull request, so an interrupted
/// attempt is found again.
pub fn fix_branch(number: u64) -> String {
    format!("{FIX_PREFIX}{number}")
}

/// The pull request a repair branch repairs.
pub fn fix_of(branch: &str) -> Option<u64> {
    branch.strip_prefix(FIX_PREFIX)?.parse().ok()
}

/// Code that checks updates, signatures or credentials: a repair there needs the user's explicit choice.
pub fn sensitive(path: &str) -> bool {
    let name = Path::new(path).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    ["update", "verify", "signature", "sign", "secret", "token", "auth", "crypto", "keychain", "credential"].iter().any(|word| name.contains(word))
}

/// Paths a repair must never change, whatever Claude wrote.
fn forbidden(path: &str) -> Option<&'static str> {
    let lower = path.to_lowercase();
    let first = lower.split('/').next().unwrap_or_default();
    if lower.split('/').any(|part| part == ".git") {
        Some("Git-Interna")
    } else if first == ".github" || first == ".gitlab" || lower == ".gitlab-ci.yml" || lower.ends_with("/.gitlab-ci.yml") {
        Some("Pipeline-Dateien")
    } else {
        None
    }
}

/// The repair's changes in the worktree, or what makes them unacceptable (every reason).
pub fn collect(dir: &Path) -> Result<Result<Changes, Vec<String>>> {
    let status = output("git", &["status", "--porcelain=v1", "-z", "--untracked-files=all", "--no-renames"], dir)?;
    let mut problems = Vec::new();
    let mut paths = Vec::new();
    for entry in status.split('\0').filter(|e| e.len() > 3) {
        let (code, path) = (&entry[..2], &entry[3..]);
        if let Some(what) = forbidden(path) {
            problems.push(format!("{path}: {what} ändert eine Reparatur nicht"));
        } else if code.contains('D') {
            problems.push(format!("{path}: gelöscht; eine Reparatur löscht keine Dateien"));
        } else if fs::symlink_metadata(dir.join(path)).map(|m| !m.is_file()).unwrap_or(true) {
            problems.push(format!("{path}: keine normale Datei (z. B. ein Link)"));
        } else {
            paths.push((path.to_owned(), code == "??" || code.starts_with('A')));
        }
    }
    if paths.len() > MAX_FILES {
        problems.push(format!("{} Dateien, mehr als die erlaubten {MAX_FILES}", paths.len()));
    }
    let mut files = Vec::new();
    for (path, added) in &paths {
        let data = fs::read(dir.join(path)).with_context(|| format!("{path} lesen"))?;
        let size = data.len() as u64;
        if size > MAX_FILE {
            problems.push(format!("{path}: {} KB, mehr als die erlaubten {} KB", size / 1024, MAX_FILE / 1024));
        }
        if !data.contains(&0) && String::from_utf8_lossy(&data).lines().any(|l| l.starts_with("<<<<<<< ") || l.starts_with(">>>>>>> ")) {
            problems.push(format!("{path}: enthält Konfliktmarker"));
        }
        let mode = match (added, output("git", &["ls-tree", "HEAD", "--", path], dir)?.split_whitespace().next()) {
            (false, Some("100755")) => "100755",
            _ => "100644",
        };
        files.push(ChangedFile {
            path: path.clone(),
            added: *added,
            size,
            binary: data.contains(&0) || std::str::from_utf8(&data).is_err(),
            sha256: hex::encode(Sha256::digest(&data)),
            mode: mode.into(),
        });
    }
    if !problems.is_empty() {
        return Ok(Err(problems));
    }
    if files.is_empty() {
        return Ok(Ok(Changes::default()));
    }
    // Intent-to-add, so new files show in the diff without staging anything for real.
    let mut add = vec!["add", "--intent-to-add", "--"];
    add.extend(paths.iter().filter(|(_, added)| *added).map(|(p, _)| p.as_str()));
    if add.len() > 3 {
        output("git", &add, dir)?;
    }
    let diff = output("git", &["diff", "--no-color", "--no-ext-diff", "--no-textconv"], dir)?;
    if diff.len() > MAX_DIFF {
        return Ok(Err(vec![format!("Änderung zu groß für eine prüfbare Reparatur ({} KB Diff, erlaubt {} KB)", diff.len() / 1024, MAX_DIFF / 1024)]));
    }
    let stat = output("git", &["diff", "--no-color", "--stat"], dir)?;
    Ok(Ok(Changes { files, diff, stat }))
}

/// The repair's pull request that is open already (an earlier or interrupted attempt), if any.
pub fn existing(plan: &Plan, number: u64) -> Result<Option<PullRequestRef>> {
    let branch = fix_branch(number);
    if let Some(p) = crate::gitlab::project(&plan.repo) {
        let list = crate::gitlab::get(&p.host, &format!("{}/merge_requests?state=opened&source_branch={}&per_page=5", p.api(), crate::gitlab::encode(&branch)))?
            .unwrap_or_default();
        return Ok(list.as_array().into_iter().flatten().next().map(|mr| PullRequestRef {
            number: mr["iid"].as_u64().unwrap_or(0),
            url: mr["web_url"].as_str().unwrap_or_default().to_owned(),
            commit: mr["sha"].as_str().unwrap_or_default().to_owned(),
        }));
    }
    let owner = plan.repo.split('/').next().unwrap_or_default();
    let list = crate::github::get(&format!("repos/{}/pulls?state=open&head={owner}:{}&per_page=5", plan.repo, crate::github::encode_ref(&branch)))?.unwrap_or_default();
    Ok(list.as_array().into_iter().flatten().next().map(|pr| PullRequestRef {
        number: pr["number"].as_u64().unwrap_or(0),
        url: pr["html_url"].as_str().unwrap_or_default().to_owned(),
        commit: pr["head"]["sha"].as_str().unwrap_or_default().to_owned(),
    }))
}

/// Opens the repair's pull request: a commit with exactly `changes` on top of the update's head
/// commit (so it carries the update too), on `fix_branch`, into the default branch.
pub fn publish(plan: &Plan, pr: &PullRequest, dir: &Path, changes: Changes, summary: &str, proof: &str) -> Result<PullRequestRef> {
    let (word, mark) = if crate::gitlab::project(&plan.repo).is_some() { ("MR", "!") } else { ("PR", "#") };
    let title = format!("{} (mit Code-Anpassung)", pr.title);
    let sensitive: Vec<&str> = changes.files.iter().map(|f| f.path.as_str()).filter(|p| sensitive(p)).collect();
    let mut body = format!(
        "Reparatur von {mark}{number}: Das Update braucht eine Anpassung des Codes. Dieser {word} enthält den Commit von \
         {mark}{number} und darauf die Anpassung, geschrieben von Claude in einer Wegwerf-Kopie und von OpenReleasePilot \
         geprüft (keine Pipeline-Dateien, keine gelöschten Dateien).\n\n**Änderung:** {summary}\n\n**Nachweis:** {proof}\n\n\
         **Dateien:**\n```\n{stat}\n```\n\nWird dieser {word} gemergt, schließt der Bot {mark}{number} selbst. Der nächste \
         Pflegeplan bewertet ihn wie jeden anderen {word} und mergt ihn nur nach deiner Freigabe.",
        number = pr.number,
        stat = changes.stat.trim_end(),
    );
    if !sensitive.is_empty() {
        body.push_str(&format!(
            "\n\n⚠️ Berührt sicherheitskritische Dateien ({}): gemergt wird nur auf ausdrücklichen Wunsch.",
            sensitive.join(", ")
        ));
    }
    let proposal = Proposal {
        base_branch: plan.default_branch.clone(),
        base_commit: pr.head_sha.clone(),
        sources: Vec::new(),
        files: changes.files,
        diff: changes.diff,
        stat: changes.stat,
        checks: Vec::new(),
        summary: summary.to_owned(),
        cost_usd: None,
        branch: fix_branch(pr.number),
        target: String::new(),
        fingerprint: String::new(),
    };
    content::deliver(&plan.repo, dir, &proposal, &title, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fix_branch_names_its_pull_request() {
        assert_eq!(fix_branch(2), "openreleasepilot/fix-2");
        assert_eq!(fix_of("openreleasepilot/fix-2"), Some(2));
        assert_eq!(fix_of("openreleasepilot/fix-x"), None);
        assert_eq!(fix_of("dependabot/cargo/desktop/ureq-3.4.2"), None);
    }

    #[test]
    fn update_and_signature_code_is_sensitive() {
        assert!(sensitive("desktop/core/src/update.rs"));
        assert!(sensitive("desktop/core/src/verify.rs"));
        assert!(sensitive("src/Signature.swift"));
        assert!(!sensitive("desktop/core/src/github.rs"));
        assert!(!sensitive("desktop/Cargo.lock"));
    }

    #[test]
    fn pipelines_and_git_stay_out() {
        assert!(forbidden(".github/workflows/ci.yml").is_some());
        assert!(forbidden(".gitlab-ci.yml").is_some());
        assert!(forbidden("sub/.gitlab-ci.yml").is_some());
        assert!(forbidden("a/.git/config").is_some());
        assert!(forbidden("desktop/core/src/github.rs").is_none());
    }

    #[test]
    fn collects_changed_and_new_files_and_refuses_deletions() {
        let dir = std::env::temp_dir().join(format!("orp-repair-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| output("git", args, &dir).unwrap();
        git(&["init", "-q"]);
        fs::write(dir.join("a.rs"), "old\n").unwrap();
        fs::write(dir.join("b.rs"), "keep\n").unwrap();
        git(&["add", "."]);
        git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "base"]);

        fs::write(dir.join("a.rs"), "new\n").unwrap();
        fs::write(dir.join("c.rs"), "added\n").unwrap();
        let changes = collect(&dir).unwrap().unwrap();
        let paths: Vec<(&str, bool)> = changes.files.iter().map(|f| (f.path.as_str(), f.added)).collect();
        assert_eq!(paths, [("a.rs", false), ("c.rs", true)]);
        assert!(changes.diff.contains("+new") && changes.diff.contains("+added"), "{}", changes.diff);

        fs::remove_file(dir.join("b.rs")).unwrap();
        let problems = collect(&dir).unwrap().unwrap_err();
        assert!(problems.iter().any(|p| p.starts_with("b.rs: gelöscht")), "{problems:?}");
        let _ = fs::remove_dir_all(&dir);
    }
}
