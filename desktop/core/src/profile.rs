//! The repository profile: what is in a managed repository (stack, CI, update bot, tests,
//! CHANGELOG, version files, notes for Claude), read from its local clone without Claude and
//! without network. It describes exactly one commit and lives in the app's data folder
//! (`repos/<key>/profile.json`), never in the repository. The task catalog (`catalog`) offers
//! tasks from it.

use crate::paths;
use crate::release::{in_skipped_dir, is_version_file};
use crate::repos::RepoTarget;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// More of one kind is not listed; the count says how many there were.
const MAX_LISTED: usize = 12;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum StackKind {
    Rust,
    Node,
    Python,
    Go,
    Docker,
}

impl StackKind {
    pub fn name(self) -> &'static str {
        match self {
            StackKind::Rust => "Rust",
            StackKind::Node => "Node",
            StackKind::Python => "Python",
            StackKind::Go => "Go",
            StackKind::Docker => "Docker",
        }
    }

    /// Dependabot's `package-ecosystem` for it.
    pub fn ecosystem(self) -> &'static str {
        match self {
            StackKind::Rust => "cargo",
            StackKind::Node => "npm",
            StackKind::Python => "pip",
            StackKind::Go => "gomod",
            StackKind::Docker => "docker",
        }
    }

    fn of(name: &str) -> Option<StackKind> {
        match name {
            "cargo.toml" => Some(StackKind::Rust),
            "package.json" => Some(StackKind::Node),
            "pyproject.toml" | "requirements.txt" | "setup.py" | "pipfile" => Some(StackKind::Python),
            "go.mod" => Some(StackKind::Go),
            n if n == "dockerfile" || n.starts_with("dockerfile.") => Some(StackKind::Docker),
            _ => None,
        }
    }
}

/// One stack and the manifests that show it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stack {
    pub kind: StackKind,
    pub manifests: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Profile {
    /// The commit the profile describes.
    pub commit: String,
    /// Where that commit came from: `origin/main` (as last fetched) or `HEAD`.
    pub rev: String,
    /// Unix seconds.
    pub read_at: u64,
    pub stacks: Vec<Stack>,
    /// CI configuration: `.github/workflows/*.yml`, `.gitlab-ci.yml` and the like.
    pub ci: Vec<String>,
    /// `.github/dependabot.yml`, if any, and the ecosystems it covers.
    pub dependabot: Option<String>,
    pub dependabot_ecosystems: Vec<String>,
    pub renovate: Option<String>,
    /// Evidence of tests: test folders, test files, an npm test script, pytest configuration.
    pub tests: Vec<String>,
    /// The root CHANGELOG.md, whatever its case (the one the release preparation updates).
    pub changelog: Option<String>,
    /// The files whose version the release preparation updates.
    pub version_files: Vec<String>,
    /// `CLAUDE.md`, `AGENTS.md`.
    pub agent_docs: Vec<String>,
    /// Names of the repository's own Claude skills (`.claude/skills/<name>/SKILL.md`).
    pub skills: Vec<String>,
    /// A website project of OpenSitePilot (`.openreleasepilot/site.json`).
    pub site: bool,
}

impl Profile {
    pub fn has(&self, kind: StackKind) -> bool {
        self.stacks.iter().any(|s| s.kind == kind)
    }

    /// Dependabot's ecosystems with the folders it would look in: each stack (a Cargo workspace
    /// once, at its root), and GitHub Actions when there are workflows.
    pub fn update_targets(&self) -> Vec<(&'static str, Vec<String>)> {
        let mut targets: Vec<(&'static str, Vec<String>)> = self
            .stacks
            .iter()
            .map(|stack| {
                let mut dirs: Vec<String> = stack.manifests.iter().map(|m| folder(m)).collect();
                dirs.sort();
                dirs.dedup();
                (stack.kind.ecosystem(), dirs)
            })
            .collect();
        if self.ci.iter().any(|c| c.starts_with(".github/workflows/")) {
            targets.push(("github-actions", vec!["/".to_owned()]));
        }
        targets
    }
}

/// `/` for a root file, `/desktop` for `desktop/Cargo.toml`.
fn folder(path: &str) -> String {
    match path.rsplit_once('/') {
        Some((dir, _)) => format!("/{dir}"),
        None => "/".to_owned(),
    }
}

// MARK: Detection

/// What detection reads: the files of one commit, the content of single files, and a search.
pub trait Tree {
    fn files(&self) -> &[String];
    fn read(&self, path: &str) -> Option<String>;
    /// Files ending in `suffix` that contain `needle`.
    fn grep(&self, needle: &str, suffix: &str) -> Vec<String>;
}

/// The profile of a tree; `commit`, `rev` and `read_at` stay empty for the caller.
pub fn detect(tree: &dyn Tree) -> Profile {
    let mut profile = Profile::default();
    let mut stacks: Vec<Stack> = Vec::new();
    let mut tests: Vec<String> = Vec::new();
    for path in tree.files() {
        let lower = path.to_lowercase();
        let mut parts: Vec<&str> = lower.split('/').collect();
        let name = parts.pop().unwrap_or_default();
        if in_skipped_dir(&parts) || parts.iter().any(|p| matches!(*p, "fixtures" | "testdata" | "__fixtures__")) {
            continue;
        }
        if let Some(kind) = StackKind::of(name) {
            match stacks.iter_mut().find(|s| s.kind == kind) {
                Some(stack) => stack.manifests.push(path.clone()),
                None => stacks.push(Stack { kind, manifests: vec![path.clone()] }),
            }
        }
        let workflow = parts == [".github", "workflows"] && (name.ends_with(".yml") || name.ends_with(".yaml"));
        let other_ci = matches!(lower.as_str(), ".gitlab-ci.yml" | ".travis.yml" | "azure-pipelines.yml" | "jenkinsfile" | ".circleci/config.yml" | "bitbucket-pipelines.yml");
        if workflow || other_ci {
            profile.ci.push(path.clone());
        }
        if matches!(lower.as_str(), ".github/dependabot.yml" | ".github/dependabot.yaml") {
            profile.dependabot = Some(path.clone());
        }
        if matches!(
            lower.as_str(),
            "renovate.json" | "renovate.json5" | ".renovaterc" | ".renovaterc.json" | ".github/renovate.json" | ".github/renovate.json5" | ".gitlab/renovate.json" | ".gitlab/renovate.json5"
        ) {
            profile.renovate = Some(path.clone());
        }
        if parts.is_empty() && name == "changelog.md" {
            profile.changelog = Some(path.clone());
        }
        if is_version_file(path) {
            profile.version_files.push(path.clone());
        }
        if matches!(name, "claude.md" | "agents.md") && !parts.iter().any(|p| p.starts_with('.')) {
            profile.agent_docs.push(path.clone());
        }
        if parts.len() == 3 && parts[0] == ".claude" && parts[1] == "skills" && name == "skill.md" {
            let skill = path.split('/').nth(2).unwrap_or_default().to_owned();
            profile.skills.push(skill);
        }
        if lower == ".openreleasepilot/site.json" {
            profile.site = true;
        }
        if let Some(dir) = parts.iter().position(|p| matches!(*p, "tests" | "test" | "__tests__" | "spec")) {
            let evidence = format!("{}/", path.split('/').take(dir + 1).collect::<Vec<_>>().join("/"));
            if !tests.contains(&evidence) {
                tests.push(evidence);
            }
        } else if name.ends_with("_test.go") || name.contains(".test.") || name.contains(".spec.") || (name.starts_with("test_") && name.ends_with(".py")) {
            tests.push(path.clone());
        }
        if matches!(name, "pytest.ini" | "conftest.py" | "tox.ini") {
            tests.push(path.clone());
        }
    }
    // A Cargo workspace covers its members: only its root counts.
    for stack in stacks.iter_mut().filter(|s| s.kind == StackKind::Rust) {
        let roots: Vec<String> = stack.manifests.iter().filter(|m| tree.read(m).is_some_and(|t| t.lines().any(|l| l.trim() == "[workspace]"))).map(|m| folder(m)).collect();
        stack.manifests.retain(|m| {
            let dir = folder(m);
            !roots.iter().any(|root| dir != *root && (root == "/" || dir.starts_with(&format!("{root}/"))))
        });
    }
    for stack in stacks.iter().filter(|s| s.kind == StackKind::Node) {
        if stack.manifests.iter().any(|m| tree.read(m).is_some_and(|text| has_npm_test(&text))) {
            tests.push("npm test".to_owned());
        }
    }
    if let Some(pyproject) = stacks.iter().filter(|s| s.kind == StackKind::Python).flat_map(|s| &s.manifests).find(|m| m.to_lowercase().ends_with("pyproject.toml")) {
        if tree.read(pyproject).is_some_and(|t| t.contains("[tool.pytest")) {
            tests.push(format!("{pyproject} (pytest)"));
        }
    }
    if stacks.iter().any(|s| s.kind == StackKind::Rust) {
        let files = tree.grep("#[test]", ".rs");
        if !files.is_empty() {
            tests.push(format!("#[test] in {} Rust-Datei(en)", files.len()));
        }
    }
    if let Some(path) = &profile.dependabot {
        profile.dependabot_ecosystems = tree.read(path).map(|text| ecosystems(&text)).unwrap_or_default();
    }
    stacks.sort_by_key(|s| s.kind);
    for stack in &mut stacks {
        stack.manifests.truncate(MAX_LISTED);
    }
    profile.stacks = stacks;
    tests.truncate(MAX_LISTED);
    profile.tests = tests;
    profile.version_files.truncate(MAX_LISTED);
    profile.agent_docs.truncate(MAX_LISTED);
    profile.skills.truncate(MAX_LISTED);
    profile
}

/// A `test` script in package.json that is not npm's placeholder.
fn has_npm_test(text: &str) -> bool {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else { return false };
    json["scripts"]["test"].as_str().is_some_and(|script| !script.trim().is_empty() && !script.contains("no test specified"))
}

/// The `package-ecosystem` values of a dependabot.yml, without a YAML parser.
fn ecosystems(text: &str) -> Vec<String> {
    let mut found: Vec<String> = text
        .lines()
        .filter_map(|line| line.trim().trim_start_matches("- ").trim().strip_prefix("package-ecosystem:"))
        .map(|value| value.split('#').next().unwrap_or_default().trim().trim_matches(['"', '\'']).to_owned())
        .filter(|value| !value.is_empty())
        .collect();
    found.dedup();
    found
}

// MARK: Reading a clone

/// One commit of a local clone, read with git only (no checkout, no network).
struct GitTree<'a> {
    dir: &'a Path,
    commit: String,
    files: Vec<String>,
}

impl Tree for GitTree<'_> {
    fn files(&self) -> &[String] {
        &self.files
    }

    fn read(&self, path: &str) -> Option<String> {
        git(self.dir, &["show", &format!("{}:{path}", self.commit)]).ok()
    }

    fn grep(&self, needle: &str, suffix: &str) -> Vec<String> {
        // Exit code 1 means nothing found.
        let glob = format!("*{suffix}");
        git(self.dir, &["grep", "-l", "-F", needle, &self.commit, "--", &glob]).map(|out| out.lines().map(str::to_owned).collect()).unwrap_or_default()
    }
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let mut all = vec!["-c", "core.hooksPath=/dev/null", "-c", "core.fsmonitor=false"];
    all.extend_from_slice(args);
    crate::shell::output("git", &all, dir)
}

/// The commit a profile of the clone describes: the default branch as last fetched
/// (`origin/HEAD`), else the checked-out `HEAD`.
pub fn current_commit(dir: &Path) -> Result<(String, String)> {
    if let Ok(name) = git(dir, &["symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"]) {
        if let Ok(commit) = git(dir, &["rev-parse", "--verify", "--quiet", "refs/remotes/origin/HEAD^{commit}"]) {
            return Ok((commit, name));
        }
    }
    let commit = git(dir, &["rev-parse", "--verify", "HEAD^{commit}"]).context("Die lokale Kopie hat keinen Commit")?;
    Ok((commit, "HEAD".to_owned()))
}

/// Reads the profile of `commit` in the clone at `dir`.
pub fn read(dir: &Path, commit: &str, rev: &str, now: u64) -> Result<Profile> {
    let list = git(dir, &["ls-tree", "-r", "-z", "--name-only", commit]).context("Dateien der lokalen Kopie lesen")?;
    let tree = GitTree { dir, commit: commit.to_owned(), files: list.split('\0').filter(|p| !p.is_empty()).map(str::to_owned).collect() };
    let mut profile = detect(&tree);
    profile.commit = commit.to_owned();
    profile.rev = rev.to_owned();
    profile.read_at = now;
    Ok(profile)
}

// MARK: Store

fn file(key: &str) -> std::path::PathBuf {
    paths::repo_dir(key).join("profile.json")
}

pub fn load(key: &str) -> Option<Profile> {
    serde_json::from_slice(&fs::read(file(key)).ok()?).ok()
}

pub fn save(key: &str, profile: &Profile) -> Result<()> {
    crate::content::write_atomic(&file(key), &serde_json::to_vec_pretty(profile)?)
}

pub fn remove(key: &str) {
    let _ = fs::remove_file(file(key));
}

/// The profile of the repository's clone as it is now: the stored one while the commit is the
/// same, otherwise read anew and stored. `Ok(None)` without a usable clone.
pub fn refresh(target: &RepoTarget, now: u64) -> Result<Option<Profile>> {
    let Some(dir) = target.clone_dir() else { return Ok(None) };
    let (commit, rev) = current_commit(&dir)?;
    if let Some(stored) = load(&target.key).filter(|p| p.commit == commit && p.rev == rev) {
        return Ok(Some(stored));
    }
    let profile = read(&dir, &commit, &rev, now)?;
    save(&target.key, &profile)?;
    Ok(Some(profile))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    pub(crate) struct Files(pub BTreeMap<String, String>, Vec<String>);

    impl Files {
        pub(crate) fn new(entries: &[(&str, &str)]) -> Self {
            let map: BTreeMap<String, String> = entries.iter().map(|(p, t)| (p.to_string(), t.to_string())).collect();
            let list = map.keys().cloned().collect();
            Files(map, list)
        }
    }

    impl Tree for Files {
        fn files(&self) -> &[String] {
            &self.1
        }
        fn read(&self, path: &str) -> Option<String> {
            self.0.get(path).cloned()
        }
        fn grep(&self, needle: &str, suffix: &str) -> Vec<String> {
            self.0.iter().filter(|(p, t)| p.ends_with(suffix) && t.contains(needle)).map(|(p, _)| p.clone()).collect()
        }
    }

    #[test]
    fn a_tauri_app_with_a_cargo_workspace() {
        let tree = Files::new(&[
            ("desktop/Cargo.toml", "[workspace]\nmembers = [\"core\", \"src-tauri\"]\n"),
            ("desktop/core/Cargo.toml", "[package]\nname = \"core\"\n"),
            ("desktop/core/src/lib.rs", "#[cfg(test)]\nmod tests { #[test] fn t() {} }\n"),
            ("desktop/src-tauri/Cargo.toml", "[package]\n"),
            ("desktop/src-tauri/tauri.conf.json", "{}"),
            ("desktop/package.json", r#"{"scripts":{"dev":"tauri dev"}}"#),
            ("desktop/node_modules/x/package.json", "{}"),
            (".github/workflows/release.yml", "on: push"),
            ("CHANGELOG.md", "# Changes"),
            ("CLAUDE.md", "Build with cargo"),
            (".claude/skills/release/SKILL.md", "---"),
            (".openreleasepilot/site.json", "{}"),
        ]);
        let p = detect(&tree);
        let rust = p.stacks.iter().find(|s| s.kind == StackKind::Rust).unwrap();
        assert_eq!(rust.manifests, ["desktop/Cargo.toml"], "workspace members are covered by its root");
        assert_eq!(p.stacks.iter().find(|s| s.kind == StackKind::Node).unwrap().manifests, ["desktop/package.json"], "node_modules is not the repository's own");
        assert_eq!(p.ci, [".github/workflows/release.yml"]);
        assert_eq!(p.changelog.as_deref(), Some("CHANGELOG.md"));
        assert!(p.version_files.contains(&"desktop/src-tauri/tauri.conf.json".to_owned()) && p.version_files.contains(&"desktop/package.json".to_owned()));
        assert!(!p.version_files.iter().any(|f| f.contains("node_modules")));
        assert_eq!(p.agent_docs, ["CLAUDE.md"]);
        assert_eq!(p.skills, ["release"]);
        assert!(p.site);
        assert_eq!(p.tests, ["#[test] in 1 Rust-Datei(en)"], "no npm test script, but Rust unit tests");
        assert_eq!(p.dependabot, None);
        assert_eq!(
            p.update_targets(),
            vec![("cargo", vec!["/desktop".to_owned()]), ("npm", vec!["/desktop".to_owned()]), ("github-actions", vec!["/".to_owned()])]
        );
    }

    #[test]
    fn python_go_and_the_update_bots() {
        let tree = Files::new(&[
            ("pyproject.toml", "[project]\nname='x'\n[tool.pytest.ini_options]\n"),
            ("svc/go.mod", "module x"),
            ("svc/handler_test.go", "package x"),
            ("Dockerfile", "FROM alpine"),
            (".gitlab-ci.yml", "test:"),
            ("renovate.json", "{}"),
            (".github/dependabot.yml", "version: 2\nupdates:\n  - package-ecosystem: \"pip\" # Python\n    directory: /\n  - package-ecosystem: gomod\n"),
            ("tests/fixtures/package.json", "{}"),
            ("tests/test_api.py", ""),
            ("Changelog.md", ""),
            ("docs/CHANGELOG.md", ""),
        ]);
        let p = detect(&tree);
        let kinds: Vec<StackKind> = p.stacks.iter().map(|s| s.kind).collect();
        assert_eq!(kinds, [StackKind::Python, StackKind::Go, StackKind::Docker], "no Node: the package.json is a test fixture");
        assert_eq!(p.ci, [".gitlab-ci.yml"]);
        assert_eq!(p.renovate.as_deref(), Some("renovate.json"));
        assert_eq!(p.dependabot_ecosystems, ["pip", "gomod"]);
        assert_eq!(p.changelog.as_deref(), Some("Changelog.md"), "only the root CHANGELOG, whatever its case");
        assert!(p.tests.contains(&"tests/".to_owned()));
        assert!(p.tests.contains(&"svc/handler_test.go".to_owned()));
        assert!(p.tests.contains(&"pyproject.toml (pytest)".to_owned()));
    }

    #[test]
    fn npm_placeholder_is_no_test() {
        assert!(!has_npm_test(r#"{"scripts":{"test":"echo \"Error: no test specified\" && exit 1"}}"#));
        assert!(has_npm_test(r#"{"scripts":{"test":"vitest run"}}"#));
        assert!(!has_npm_test("not json"));
    }

    #[test]
    fn a_real_clone_is_read_at_its_commit_only() {
        let dir = std::env::temp_dir().join(format!("orp-profile-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        let run = |args: &[&str]| git(&dir, args).unwrap();
        run(&["init", "--quiet", "--initial-branch=main"]);
        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        fs::write(dir.join("src/lib.rs"), "#[test]\nfn t() {}\n").unwrap();
        run(&["add", "."]);
        run(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "--quiet", "-m", "init"]);
        // Not committed: not part of the profile.
        fs::write(dir.join("package.json"), "{}").unwrap();
        let (commit, rev) = current_commit(&dir).unwrap();
        assert_eq!(rev, "HEAD");
        let p = read(&dir, &commit, &rev, 7).unwrap();
        assert_eq!(p.commit, commit);
        assert_eq!(p.stacks.iter().map(|s| s.kind).collect::<Vec<_>>(), [StackKind::Rust]);
        assert_eq!(p.tests, ["#[test] in 1 Rust-Datei(en)"]);
        let _ = fs::remove_dir_all(&dir);
    }
}
