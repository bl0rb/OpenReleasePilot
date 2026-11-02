//! The core of both apps, without UI: OpenReleasePilot (repository care, content targets) and
//! OpenSitePilot (the website steps). `paths::set_app` says which app a process is.
//!
//! - `config`: the apps' own settings (`Config`: managed repositories, targets; `SiteConfig`: website project, SFTP), each in its app's data folder
//! - `site`: the project's `.openreleasepilot/site.json` (build, output, URL, watched products)
//! - `shell`, `git`, `github`, `gitlab`, `claude`: external tools
//! - `forge`: whether a repository lives on GitHub or on a GitLab instance
//! - `sftp`, `manifest`, `verify`, `secrets`: upload, change tracking, live check, password storage
//! - `tokens`: tokens the user entered for GitHub and GitLab, in the keychain, ahead of the CLIs' tokens
//! - `update`: updates of the app itself from its GitHub releases
//! - `repos`: the managed repositories, what GitHub reports about them, their health (read only)
//! - `security`: security check of the managed repositories (branch protection, scanning, alerts) and its fixes
//! - `repocare`: merging, tagging and branch cleanup in the managed repositories
//! - `proof`: throwaway worktrees in which Claude builds and tests pull requests for its review
//! - `repair`: an update that needs the code adapted gets a repair pull request of its own
//! - `release`: before a tag, a release pull request with the version files and the CHANGELOG
//! - `content`: README/documentation and wiki targets: work folder, checks, proposal, publishing
//! - `profile`: what is in a repository (stack, CI, update bot, tests, CHANGELOG), from its clone, without Claude
//! - `memory`: what earlier jobs found and decided per repository, as material for Claude's prompts
//! - `catalog`: the tasks offered per repository from its profile, each with an autonomy level
//! - `jobs`: care jobs with their steps, approvals and results, stored so they survive a restart

pub mod catalog;
pub mod claude;
pub mod config;
pub mod content;
pub mod forge;
pub mod git;
pub mod github;
pub mod gitlab;
pub mod jobs;
pub mod localrepo;
pub mod manifest;
pub mod memory;
pub mod paths;
pub mod profile;
pub mod proof;
pub mod release;
pub mod repair;
pub mod repocare;
pub mod repos;
pub mod secrets;
pub mod security;
pub mod sftp;
pub mod shell;
pub mod site;
pub mod update;
pub mod tokens;
pub mod verify;
