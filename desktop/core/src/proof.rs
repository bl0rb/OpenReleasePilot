//! Throwaway worktrees for Claude's review: one per pull request, detached at the commit the preview
//! saw, so Claude can build, test and try a short check program instead of guessing. Nothing in them
//! is pushed; they are removed when the review ends (`Drop`), and leftovers of an interrupted run are
//! removed before the next one.
//!
//! Claude's shell runs in Claude Code's sandbox there (`settings`): writes only inside the worktrees
//! and the proof cache, network only to package registries, credentials unreadable. A dependency's
//! build script runs inside that sandbox too.

use crate::repocare::Plan;
use crate::shell::output;
use anyhow::{Context, Result};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// At most this many worktrees per repository, so a long queue of pull requests does not fill the disk.
pub const MAX_TREES: usize = 10;

/// Where package managers may download to: registries and the code hosts git dependencies live on.
const NETWORK: &[&str] = &[
    "crates.io",
    "index.crates.io",
    "static.crates.io",
    "github.com",
    "codeload.github.com",
    "objects.githubusercontent.com",
    "registry.npmjs.org",
    "registry.yarnpkg.com",
    "proxy.golang.org",
    "sum.golang.org",
    "pypi.org",
    "files.pythonhosted.org",
];

/// Credentials a build script must not read.
const DENY_READ: &[&str] = &[
    "~/.ssh",
    "~/.aws",
    "~/.gnupg",
    "~/.config/gh",
    "~/.config/glab-cli",
    "~/.netrc",
    "~/.git-credentials",
    "~/.npmrc",
    "~/.docker",
    "~/.cargo/credentials",
    "~/.cargo/credentials.toml",
    "~/Library/Keychains",
];

/// The worktrees of one repository's review.
#[derive(Debug)]
pub struct Proofs {
    /// Holds the worktrees; Claude's working directory.
    pub root: PathBuf,
    /// Package caches and build output, kept between runs (only proofs use them).
    pub cache: PathBuf,
    clone: PathBuf,
    /// Pull request number and its worktree.
    pub trees: Vec<(u64, PathBuf)>,
    /// Pull requests without a worktree, and why.
    pub missing: Vec<(u64, String)>,
}

impl Proofs {
    /// One worktree per pull request of the plan, at the head commit the preview saw. Needs the local clone.
    pub fn prepare(plan: &Plan) -> Result<Proofs> {
        let clone = PathBuf::from(plan.clone.as_deref().context("keine lokale Kopie")?);
        let slug = slug(&plan.repo);
        let root = std::env::temp_dir().join("OpenReleasePilot-Proof").join(&slug);
        let cache = std::env::temp_dir().join("OpenReleasePilot-Proof-Cache").join(&slug);
        remove_trees(&clone, &root);
        fs::create_dir_all(&root).context("Ordner für die Nachweise anlegen")?;
        fs::create_dir_all(&cache).context("Cache für die Nachweise anlegen")?;
        // Canonical paths: the sandbox and the permission rules compare real paths (/var is /private/var on macOS).
        let root = root.canonicalize()?;
        let cache = cache.canonicalize()?;
        let mut proofs = Proofs { root, cache, clone, trees: Vec::new(), missing: Vec::new() };

        let prs: Vec<_> = plan.prs.iter().filter(|pr| !pr.head_sha.is_empty()).collect();
        for pr in prs.iter().skip(MAX_TREES) {
            proofs.missing.push((pr.number, format!("mehr als {MAX_TREES} PRs")));
        }
        let prs: Vec<_> = prs.into_iter().take(MAX_TREES).collect();
        if prs.is_empty() {
            return Ok(proofs);
        }
        let gitlab = crate::gitlab::project(&plan.repo).is_some();
        let refs: Vec<String> =
            prs.iter().map(|pr| if gitlab { format!("refs/merge-requests/{}/head", pr.number) } else { format!("refs/pull/{}/head", pr.number) }).collect();
        // Without a destination the fetch only writes FETCH_HEAD and objects: no branch of the clone moves.
        let mut args = vec!["fetch", "--no-tags", "--quiet", "origin"];
        args.extend(refs.iter().map(String::as_str));
        if let Err(e) = output("git", &args, &proofs.clone) {
            let why = format!("PR-Commits nicht geholt: {e:#}");
            proofs.missing.extend(prs.iter().map(|pr| (pr.number, why.clone())));
            return Ok(proofs);
        }
        for pr in prs {
            if output("git", &["cat-file", "-e", &format!("{}^{{commit}}", pr.head_sha)], &proofs.clone).is_err() {
                proofs.missing.push((pr.number, "Commit der Vorschau nicht mehr vorhanden".into()));
                continue;
            }
            let dir = proofs.root.join(format!("pr-{}", pr.number));
            match output("git", &["worktree", "add", "--detach", "--quiet", &dir.to_string_lossy(), &pr.head_sha], &proofs.clone) {
                Ok(_) => proofs.trees.push((pr.number, dir)),
                Err(e) => proofs.missing.push((pr.number, format!("Worktree nicht angelegt: {e:#}"))),
            }
        }
        Ok(proofs)
    }

    /// Claude Code settings for the review: Bash in the sandbox, writes only here, network only to registries.
    /// gh and glab run outside the sandbox (they need their login and API), and only as far as the
    /// review's allow rules reach: with `--permission-mode dontAsk` every other unsandboxed command,
    /// including an attempt to switch the sandbox off, is denied.
    pub fn settings(&self) -> String {
        json!({
            "sandbox": {
                "enabled": true,
                "autoAllowBashIfSandboxed": true,
                // Unsandboxed only through the permission rules (see above), never by prompt.
                "allowUnsandboxedCommands": true,
                "failIfUnavailable": true,
                "excludedCommands": ["gh *", "glab *"],
                "filesystem": {
                    "allowWrite": [self.root.to_string_lossy(), self.cache.to_string_lossy()],
                    "denyRead": DENY_READ,
                },
                "network": { "allowedDomains": NETWORK },
            }
        })
        .to_string()
    }

    /// The package managers' caches and build output in the proof cache, never in the user's.
    pub fn env(&self) -> HashMap<String, String> {
        let at = |name: &str| self.cache.join(name).to_string_lossy().into_owned();
        [
            ("CARGO_HOME", at("cargo")),
            ("CARGO_TARGET_DIR", at("target")),
            ("npm_config_cache", at("npm")),
            ("GOPATH", at("go")),
            ("GOMODCACHE", at("go/pkg/mod")),
            ("GOCACHE", at("go-build")),
            ("PIP_CACHE_DIR", at("pip")),
            ("GRADLE_USER_HOME", at("gradle")),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect()
    }

    /// For the prompt: `#12` and its folder.
    pub fn listing(&self) -> Vec<(u64, String)> {
        self.trees.iter().map(|(n, dir)| (*n, dir.to_string_lossy().into_owned())).collect()
    }
}

impl Drop for Proofs {
    fn drop(&mut self) {
        remove_trees(&self.clone, &self.root);
    }
}

/// Removes the worktrees under `root` from the clone and the folder itself.
fn remove_trees(clone: &Path, root: &Path) {
    if let Ok(entries) = fs::read_dir(root) {
        for dir in entries.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()) {
            let _ = output("git", &["worktree", "remove", "--force", &dir.to_string_lossy()], clone);
        }
    }
    let _ = fs::remove_dir_all(root);
    let _ = output("git", &["worktree", "prune"], clone);
}

/// `owner/name` (or a GitLab URL) as one folder name.
fn slug(repo: &str) -> String {
    repo.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '.' { c } else { '_' }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_one_folder() {
        assert_eq!(slug("bl0rb/OpenReleasePilot"), "bl0rb_OpenReleasePilot");
        assert_eq!(slug("https://gitlab.example.com/a/b"), "https___gitlab.example.com_a_b");
    }

    #[test]
    fn settings_keep_writes_in_the_proof_folders() {
        let proofs = Proofs {
            root: "/tmp/p".into(),
            cache: "/tmp/c".into(),
            clone: "/nonexistent-clone".into(),
            trees: Vec::new(),
            missing: Vec::new(),
        };
        let settings: serde_json::Value = serde_json::from_str(&proofs.settings()).unwrap();
        let sandbox = &settings["sandbox"];
        assert_eq!(sandbox["enabled"], true);
        assert_eq!(sandbox["excludedCommands"], json!(["gh *", "glab *"]));
        assert_eq!(sandbox["filesystem"]["allowWrite"], json!(["/tmp/p", "/tmp/c"]));
        assert!(sandbox["filesystem"]["denyRead"].as_array().unwrap().contains(&json!("~/.ssh")));
        assert!(proofs.env()["CARGO_TARGET_DIR"].starts_with("/tmp/c"));
        // Dropping must not touch a real clone.
        std::mem::forget(proofs);
    }
}
