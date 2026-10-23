//! Release preparation: before the app tags the next version, it can bring the repository's version
//! files and its CHANGELOG to that version, in a release pull request of its own. The user approves
//! it with its diff; the app merges it once its CI is green and tags the merge commit.
//!
//! Nothing is guessed and Claude is not involved: a file changes only where its version is exactly
//! the one of the last tag (`Cargo.toml`, `Cargo.lock`, `package.json`, `package-lock.json`,
//! `tauri.conf.json`, `pyproject.toml`, `Chart.yaml`, `VERSION`), a root README only on lines that
//! name the repository (download and install lines), and the root `CHANGELOG.md` gets a section with
//! the commits since that tag (or its "Unreleased" section is renamed to the version). A version
//! that other lines merely happen to share stays as it is.

use crate::content::{self, PullRequestRef, SourceRev, TargetKind};
use crate::repocare::{iso8601, Plan};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

/// File names whose version the preparation updates, wherever they are in the repository.
const VERSION_FILES: [&str; 9] = ["cargo.toml", "cargo.lock", "package.json", "package-lock.json", "tauri.conf.json", "pyproject.toml", "chart.yaml", "version", "version.txt"];
/// Folders that hold other people's code or build output, never the repository's own version.
const SKIPPED_DIRS: [&str; 6] = ["node_modules", "vendor", "target", "dist", "build", ".venv"];
const MAX_CHANGELOG_ENTRIES: usize = 100;

/// The branch of the release pull request for `tag`; one per version, so an interrupted attempt is found again.
pub fn branch(tag: &str) -> String {
    format!("openreleasepilot/release-{tag}")
}

/// One file the preparation changed, and how.
#[derive(Debug, Clone, PartialEq)]
pub struct Edit {
    pub path: String,
    pub what: String,
}

/// What the preparation found to change in the work folder.
pub enum Prepared {
    /// A proposal to approve, with the edits that make it up.
    Proposal(Box<content::Proposal>, Vec<Edit>),
    /// Nothing to change: the tag follows directly.
    Nothing(String),
    /// The default branch is no longer where the run expects it.
    Moved(String),
}

/// Clones the default branch into the job's work folder, checks it is exactly at `expected` (where
/// this run's merges left it), and brings the version files and the CHANGELOG from `last` to `tag`.
/// Writes only below the work folder.
pub fn prepare(job: &str, plan: &Plan, expected: &str, last: &str, tag: &str) -> Result<Prepared> {
    let old = last.strip_prefix(plan.tag_prefix.as_str()).unwrap_or(last);
    let new = tag.strip_prefix(plan.tag_prefix.as_str()).unwrap_or(tag);
    let work = work_dir(job, &plan.key);
    let prepared = content::prepare(&work, &content::github_remote(&plan.repo), Some(&plan.default_branch), &[], &|_| String::new())?;
    if prepared.base_commit != expected {
        return Ok(Prepared::Moved(format!(
            "{} steht auf {} statt auf {}; die Änderung kam nicht aus diesem Lauf",
            plan.default_branch,
            content::short(&prepared.base_commit),
            content::short(expected)
        )));
    }
    let dir = &prepared.dir;
    let name = plan.repo.rsplit('/').next().unwrap_or(&plan.repo);
    let mut edits = Vec::new();
    for path in tracked(dir)? {
        let Some(text) = fs::read_to_string(dir.join(&path)).ok() else { continue };
        if let Some((changed, what)) = bump_file(&path, &text, old, new, name) {
            fs::write(dir.join(&path), changed).with_context(|| format!("{path} schreiben"))?;
            edits.push(Edit { path, what });
        }
    }
    let mut notes = Vec::new();
    match changelog_file(dir)? {
        None => notes.push("kein CHANGELOG.md im Repository".to_owned()),
        Some(path) => {
            let text = fs::read_to_string(dir.join(&path)).with_context(|| format!("{path} lesen"))?;
            let entries = match content::resolve_tag(&plan.repo, last).and_then(|from| entries(dir, &from)) {
                Ok(entries) => entries,
                Err(e) => {
                    notes.push(format!("Commits seit {last} nicht lesbar ({e:#}); CHANGELOG unverändert"));
                    Vec::new()
                }
            };
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let date = iso8601(now)[..10].to_owned();
            match changelog(&text, new, tag, &date, &entries) {
                Some((changed, what)) => {
                    fs::write(dir.join(&path), changed).with_context(|| format!("{path} schreiben"))?;
                    edits.push(Edit { path, what });
                }
                None => notes.push(format!("{path}: schon auf {new} oder nichts einzutragen")),
            }
        }
    }
    if edits.is_empty() {
        let why = if notes.is_empty() { String::new() } else { format!(" ({})", notes.join("; ")) };
        return Ok(Prepared::Nothing(format!("keine Versionsdatei mit {old} gefunden{why}; der Tag folgt direkt")));
    }
    let paths: Vec<String> = edits.iter().map(|e| e.path.clone()).collect();
    let changes = match content::collect(&prepared, &paths)? {
        Ok(changes) => changes,
        Err(problems) => bail!("Release-Vorbereitung verworfen: {}", problems.join("; ")),
    };
    if changes.files.is_empty() {
        return Ok(Prepared::Nothing("nichts zu ändern; der Tag folgt direkt".to_owned()));
    }
    let checks = content::checks(&prepared, &changes, TargetKind::Docs);
    let source = SourceRev {
        key: plan.key.clone(),
        repo: plan.repo.clone(),
        basis: content::Basis::Commit,
        rev: prepared.base_commit.clone(),
        label: format!("{} {}", plan.default_branch, content::short(&prepared.base_commit)),
    };
    let summary = summary(last, tag, &edits, &notes);
    let proposal = content::Proposal::standalone(&format!("release:{}:{tag}", plan.key), &prepared, vec![source], changes, checks, summary, branch(tag));
    Ok(Prepared::Proposal(Box::new(proposal), edits))
}

/// Opens the release pull request with exactly the approved files on top of the proposal's base
/// commit; picks up the branch or pull request an interrupted attempt left.
pub fn publish(job: &str, plan: &Plan, proposal: &content::Proposal, tag: &str) -> Result<PullRequestRef> {
    let dir = work_dir(job, &plan.key).join("target");
    let title = format!("Release {tag}");
    let body = format!(
        "{}\n\nVorbereitet von OpenReleasePilot (Auftrag {job}); die Änderung wurde in der App geprüft und freigegeben. \
         Ist die CI grün, mergt die App diesen {} und setzt danach den Tag {tag} auf den Merge-Commit, sobald auch dessen CI grün ist.",
        proposal.summary.trim(),
        crate::forge::Forge::of(&plan.repo).request(),
    );
    content::deliver(&plan.repo, &dir, proposal, &title, &body)
}

fn work_dir(job: &str, key: &str) -> std::path::PathBuf {
    content::work_dir(job, &format!("release-{}", key.replace(['/', ':'], "-")))
}

fn summary(last: &str, tag: &str, edits: &[Edit], notes: &[String]) -> String {
    let mut text = format!("Bringt das Repository von {last} auf {tag}:\n");
    for edit in edits {
        text.push_str(&format!("\n- `{}`: {}", edit.path, edit.what));
    }
    if !notes.is_empty() {
        text.push_str(&format!("\n\nHinweis: {}.", notes.join("; ")));
    }
    text
}

/// The repository's files, as Git tracks them, that may carry its version.
fn tracked(dir: &Path) -> Result<Vec<String>> {
    let list = crate::shell::output("git", &["-c", "core.hooksPath=/dev/null", "ls-files", "-z"], dir)?;
    Ok(list
        .split('\0')
        .filter(|path| !path.is_empty())
        .filter(|path| {
            let lower = path.to_lowercase();
            let mut parts: Vec<&str> = lower.split('/').collect();
            let name = parts.pop().unwrap_or_default();
            let readme = parts.is_empty() && name.starts_with("readme") && name.ends_with(".md");
            (VERSION_FILES.contains(&name) || readme) && !parts.iter().any(|p| SKIPPED_DIRS.contains(p))
        })
        .map(str::to_owned)
        .collect())
}

/// The root CHANGELOG, whatever its case.
fn changelog_file(dir: &Path) -> Result<Option<String>> {
    for entry in fs::read_dir(dir).context("Arbeitsordner lesen")? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if name.eq_ignore_ascii_case("changelog.md") {
            return Ok(Some(name));
        }
    }
    Ok(None)
}

/// The changelog lines for the commits since `from`, oldest last, along the default branch: a merge
/// commit counts with its pull request's title, release commits and branch syncs are left out.
fn entries(dir: &Path, from: &str) -> Result<Vec<String>> {
    let range = format!("{from}..HEAD");
    let log = crate::shell::output("git", &["-c", "core.hooksPath=/dev/null", "log", "--first-parent", "--format=%s%x1f%b%x1e", &range], dir)?;
    Ok(log.split('\x1e').filter_map(|record| entry(record.trim_matches('\n'))).take(MAX_CHANGELOG_ENTRIES).collect())
}

/// One changelog line from a commit's subject and body, `None` for commits that say nothing.
fn entry(record: &str) -> Option<String> {
    let (subject, body) = record.split_once('\x1f').unwrap_or((record, ""));
    let subject = subject.trim();
    let first_body_line = body.lines().map(str::trim).find(|l| !l.is_empty());
    if let Some(rest) = subject.strip_prefix("Merge pull request #") {
        let number: String = rest.chars().take_while(char::is_ascii_digit).collect();
        return Some(format!("{} (#{number})", first_body_line?));
    }
    if subject.starts_with("Merge branch ") || subject.starts_with("Merge remote-tracking branch ") {
        // GitLab's merge commits name the merge request in the body; a plain branch sync says nothing.
        let mr = body.lines().find_map(|l| l.trim().strip_prefix("See merge request ")).and_then(|r| r.rsplit_once('!')).map(|(_, n)| n.trim().to_owned());
        let title = first_body_line.filter(|l| !l.starts_with("See merge request "))?;
        return Some(match mr {
            Some(n) => format!("{title} (!{n})"),
            None => title.to_owned(),
        });
    }
    let lower = subject.to_lowercase();
    if subject.is_empty() || lower.starts_with("release ") || lower.starts_with("version ") {
        return None;
    }
    Some(subject.to_owned())
}

// MARK: Version files

/// The file with its version raised from `old` to `new`, and what changed; `None` when it carries
/// no version that is exactly `old`.
pub fn bump_file(path: &str, text: &str, old: &str, new: &str, repo_name: &str) -> Option<(String, String)> {
    let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
    let changed = match name.as_str() {
        "cargo.toml" => toml_version(text, &["package", "workspace.package"], old, new),
        "pyproject.toml" => toml_version(text, &["project", "tool.poetry"], old, new),
        "cargo.lock" => cargo_lock(text, old, new),
        "package.json" | "tauri.conf.json" => json_versions(text, &[&["version"]], old, new),
        "package-lock.json" => json_versions(text, &[&["version"], &["packages", "", "version"]], old, new),
        "chart.yaml" => chart_yaml(text, old, new),
        "version" | "version.txt" => (text.trim() == old).then(|| text.replacen(old, new, 1)),
        _ if name.starts_with("readme") => readme(text, old, new, repo_name),
        _ => None,
    }?;
    let what = if name.starts_with("readme") {
        let lines = changed.lines().zip(text.lines()).filter(|(a, b)| a != b).count();
        format!("{old} → {new} in {lines} Zeile(n), die {repo_name} nennen")
    } else {
        format!("Version {old} → {new}")
    };
    (changed != text).then_some((changed, what))
}

/// `version = "old"` in one of `tables`, line by line, so comments and layout stay.
fn toml_version(text: &str, tables: &[&str], old: &str, new: &str) -> Option<String> {
    let mut table = String::new();
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && !trimmed.starts_with("[[") {
            table = trimmed.trim_start_matches('[').split(']').next().unwrap_or_default().trim().to_owned();
        } else if trimmed.starts_with("[[") {
            table.clear();
        }
        if tables.contains(&table.as_str()) {
            if let Some(replaced) = assignment(line, "version", '=', old, new) {
                out.push_str(&replaced);
                changed = true;
                continue;
            }
        }
        out.push_str(line);
    }
    changed.then_some(out)
}

/// `key = "old"` (or `key: old` / `key: "old"` for YAML) as the whole line, with the value replaced.
fn assignment(line: &str, key: &str, separator: char, old: &str, new: &str) -> Option<String> {
    let rest = line.trim_start().strip_prefix(key)?;
    let rest = rest.trim_start().strip_prefix(separator)?;
    let value = rest.trim_start();
    let quote = value.chars().next().filter(|c| *c == '"' || *c == '\'');
    let inner = match quote {
        Some(q) => value[1..].strip_prefix(old)?.strip_prefix(q)?,
        None if separator == ':' => value.strip_prefix(old)?,
        None => return None,
    };
    let tail = inner.trim();
    if !(tail.is_empty() || tail.starts_with('#')) {
        return None;
    }
    let at = line.len() - value.len();
    Some(format!("{}{}", &line[..at], value.replacen(old, new, 1)))
}

/// The repository's own packages in `Cargo.lock`: those without a `source`, at version `old`.
fn cargo_lock(text: &str, old: &str, new: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    for block in split_keep(text, "[[package]]") {
        let local = !block.lines().any(|l| l.trim_start().starts_with("source = "));
        let mut done = false;
        for line in block.split_inclusive('\n') {
            if local && !done {
                if let Some(replaced) = assignment(line, "version", '=', old, new) {
                    out.push_str(&replaced);
                    changed = true;
                    done = true;
                    continue;
                }
            }
            out.push_str(line);
        }
    }
    changed.then_some(out)
}

/// `text` cut before each occurrence of `marker` at a line start; the pieces join to `text` again.
fn split_keep<'a>(text: &'a str, marker: &str) -> Vec<&'a str> {
    let mut starts = vec![0];
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        if at > 0 && line.starts_with(marker) {
            starts.push(at);
        }
        at += line.len();
    }
    starts.push(text.len());
    starts.windows(2).map(|w| &text[w[0]..w[1]]).collect()
}

/// `version:` and `appVersion:` of a Helm chart, top level only.
fn chart_yaml(text: &str, old: &str, new: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    for line in text.split_inclusive('\n') {
        let top = !line.starts_with([' ', '\t']);
        let replaced = top.then(|| assignment(line, "version", ':', old, new).or_else(|| assignment(line, "appVersion", ':', old, new))).flatten();
        match replaced {
            Some(replaced) => {
                out.push_str(&replaced);
                changed = true;
            }
            None => out.push_str(line),
        }
    }
    changed.then_some(out)
}

/// `old` as a whole version (not part of `10.4.7` or `0.4.71`) on lines that name the repository.
fn readme(text: &str, old: &str, new: &str, repo_name: &str) -> Option<String> {
    let name = repo_name.to_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    for line in text.split_inclusive('\n') {
        if !line.to_lowercase().contains(&name) {
            out.push_str(line);
            continue;
        }
        let replaced = replace_version(line, old, new);
        changed |= replaced != line;
        out.push_str(&replaced);
    }
    changed.then_some(out)
}

fn replace_version(line: &str, old: &str, new: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while let Some(found) = line[i..].find(old) {
        let start = i + found;
        let end = start + old.len();
        let before = start.checked_sub(1).map(|b| bytes[b]);
        let after = bytes.get(end).copied();
        let after_next = bytes.get(end + 1).copied();
        let joined_before = before.is_some_and(|c| c.is_ascii_digit() || c == b'.');
        let joined_after = after.is_some_and(|c| c.is_ascii_digit()) || (after == Some(b'.') && after_next.is_some_and(|c| c.is_ascii_digit()));
        out.push_str(&line[i..start]);
        out.push_str(if joined_before || joined_after { old } else { new });
        i = end;
    }
    out.push_str(&line[i..]);
    out
}

/// The string values at `paths` (keys from the top), replaced where they are exactly `old`;
/// everything else of the file stays byte for byte.
fn json_versions(text: &str, paths: &[&[&str]], old: &str, new: &str) -> Option<String> {
    let mut found = Vec::new();
    let mut scanner = Json { text: text.as_bytes(), at: 0 };
    scanner.value(&mut Vec::new(), paths, &mut found)?;
    let mut ranges: Vec<(usize, usize)> = found.into_iter().filter(|(s, e)| &text[*s..*e] == old).collect();
    if ranges.is_empty() {
        return None;
    }
    ranges.sort_unstable();
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (start, end) in ranges {
        out.push_str(&text[last..start]);
        out.push_str(new);
        last = end;
    }
    out.push_str(&text[last..]);
    Some(out)
}

/// A minimal JSON reader that only notes where the wanted string values are.
struct Json<'a> {
    text: &'a [u8],
    at: usize,
}

impl Json<'_> {
    fn skip_space(&mut self) {
        while self.text.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }

    /// Reads a string at the cursor; returns the byte range of its content (without the quotes).
    fn string(&mut self) -> Option<(usize, usize)> {
        if self.text.get(self.at) != Some(&b'"') {
            return None;
        }
        let start = self.at + 1;
        let mut i = start;
        loop {
            match self.text.get(i)? {
                b'\\' => i += 2,
                b'"' => break,
                _ => i += 1,
            }
        }
        self.at = i + 1;
        Some((start, i))
    }

    fn value(&mut self, path: &mut Vec<String>, wanted: &[&[&str]], found: &mut Vec<(usize, usize)>) -> Option<()> {
        self.skip_space();
        match *self.text.get(self.at)? {
            b'{' => {
                self.at += 1;
                loop {
                    self.skip_space();
                    if self.text.get(self.at) == Some(&b'}') {
                        self.at += 1;
                        return Some(());
                    }
                    let (s, e) = self.string()?;
                    let key = std::str::from_utf8(&self.text[s..e]).ok()?.to_owned();
                    self.skip_space();
                    if self.text.get(self.at) != Some(&b':') {
                        return None;
                    }
                    self.at += 1;
                    path.push(key);
                    self.value(path, wanted, found)?;
                    path.pop();
                    self.skip_space();
                    match self.text.get(self.at)? {
                        b',' => self.at += 1,
                        b'}' => {
                            self.at += 1;
                            return Some(());
                        }
                        _ => return None,
                    }
                }
            }
            b'[' => {
                self.at += 1;
                loop {
                    self.skip_space();
                    if self.text.get(self.at) == Some(&b']') {
                        self.at += 1;
                        return Some(());
                    }
                    // Array elements have no key; nothing wanted lives inside them.
                    path.push("\u{0}".into());
                    self.value(path, wanted, found)?;
                    path.pop();
                    self.skip_space();
                    match self.text.get(self.at)? {
                        b',' => self.at += 1,
                        b']' => {
                            self.at += 1;
                            return Some(());
                        }
                        _ => return None,
                    }
                }
            }
            b'"' => {
                let range = self.string()?;
                if wanted.iter().any(|w| w.len() == path.len() && w.iter().zip(path.iter()).all(|(a, b)| a == b)) {
                    found.push(range);
                }
                Some(())
            }
            _ => {
                while self.text.get(self.at).is_some_and(|c| !matches!(c, b',' | b'}' | b']') && !c.is_ascii_whitespace()) {
                    self.at += 1;
                }
                Some(())
            }
        }
    }
}

// MARK: CHANGELOG

/// The changelog with a section for `version` (`tag` is the full tag, for the heading style of
/// changelogs that write it), and what changed; `None` when it has that version already or there
/// is nothing to write. A non-empty "Unreleased" section becomes the version (the notes written
/// there by hand are better than commit subjects), with a fresh empty one above it; otherwise the
/// commits since the last tag become the section, above the newest one.
pub fn changelog(text: &str, version: &str, tag: &str, date: &str, entries: &[String]) -> Option<(String, String)> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let heading = |line: &str| line.starts_with("## ");
    if lines.iter().filter(|l| heading(l)).any(|l| mentions(l, version)) {
        return None;
    }
    let first_version = lines.iter().find(|l| heading(l) && !is_unreleased(l));
    let brackets = first_version.is_none_or(|l| l.starts_with("## ["));
    // Changelogs that write the tag prefix (`## v1.2.0`) keep it.
    let prefix = tag.strip_suffix(version).unwrap_or_default();
    let prefixed = !prefix.is_empty()
        && first_version.is_some_and(|l| l.trim_start_matches('#').trim().trim_start_matches('[').strip_prefix(prefix).is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit())));
    let label = if prefixed { tag } else { version };
    let title = if brackets { format!("## [{label}] - {date}") } else { format!("## {label} - {date}") };
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    if let Some(at) = lines.iter().position(|l| heading(l) && is_unreleased(l)) {
        let end = lines.iter().skip(at + 1).position(|l| heading(l)).map_or(lines.len(), |n| at + 1 + n);
        let filled = lines[at + 1..end].iter().any(|l| !l.trim().is_empty());
        let mut out: String = lines[..at].concat();
        if filled {
            out.push_str(lines[at]);
            out.push_str(newline);
            out.push_str(&title);
            out.push_str(newline);
            out.push_str(&lines[at + 1..].concat());
            return Some((out, format!("Abschnitt „Unreleased“ wird zu {label}")));
        }
        if entries.is_empty() {
            return None;
        }
        out.push_str(lines[at]);
        out.push_str(newline);
        out.push_str(&section(&title, entries, newline));
        out.push_str(&lines[end.min(lines.len())..].concat());
        return Some((out, format!("Abschnitt {label} mit {} Einträgen seit dem letzten Tag", entries.len())));
    }
    if entries.is_empty() {
        return None;
    }
    let at = lines.iter().position(|l| heading(l)).unwrap_or(lines.len());
    let mut out: String = lines[..at].concat();
    if at == lines.len() && !out.is_empty() {
        if !out.ends_with('\n') {
            out.push_str(newline);
        }
        out.push_str(newline);
    }
    out.push_str(&section(&title, entries, newline));
    out.push_str(&lines[at..].concat());
    Some((out, format!("Abschnitt {label} mit {} Einträgen seit dem letzten Tag", entries.len())))
}

fn section(title: &str, entries: &[String], newline: &str) -> String {
    let mut text = format!("{title}{newline}{newline}");
    for entry in entries {
        text.push_str(&format!("- {entry}{newline}"));
    }
    text.push_str(newline);
    text
}

fn is_unreleased(line: &str) -> bool {
    line.trim_start_matches('#').trim().trim_start_matches('[').to_lowercase().starts_with("unreleased")
}

/// The heading names exactly this version (not `1.2.30` for `1.2.3`).
fn mentions(line: &str, version: &str) -> bool {
    replace_version(line, version, "\u{0}").contains('\u{0}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_files_change_only_where_they_carry_the_old_version() {
        let cargo = "[package]\nname = \"app\"\nversion = \"0.4.7\" # bumped by the app\n\n[dependencies]\nserde = { version = \"0.4.7\" }\nfoo = \"0.4.7\"\n";
        let (changed, what) = bump_file("desktop/Cargo.toml", cargo, "0.4.7", "0.5.0", "app").unwrap();
        assert_eq!(changed, cargo.replacen("version = \"0.4.7\" #", "version = \"0.5.0\" #", 1));
        assert_eq!(what, "Version 0.4.7 → 0.5.0");
        assert!(bump_file("core/Cargo.toml", "[package]\nversion = \"0.1.0\"\n", "0.4.7", "0.5.0", "app").is_none());
        assert!(bump_file("Cargo.toml", "[package]\nversion.workspace = true\n", "0.4.7", "0.5.0", "app").is_none());

        let tauri = "{\n  \"productName\": \"App\",\n  \"version\": \"0.4.7\",\n  \"plugins\": { \"updater\": { \"version\": \"0.4.7\" } },\n  \"list\": [\"0.4.7\"]\n}\n";
        let (changed, _) = bump_file("desktop/src-tauri/tauri.conf.json", tauri, "0.4.7", "0.4.8", "app").unwrap();
        assert_eq!(changed, tauri.replacen("\"version\": \"0.4.7\",", "\"version\": \"0.4.8\",", 1));

        let lock = "{\n  \"name\": \"app\",\n  \"version\": \"1.2.0\",\n  \"packages\": {\n    \"\": { \"name\": \"app\", \"version\": \"1.2.0\" },\n    \"node_modules/x\": { \"version\": \"1.2.0\" }\n  }\n}\n";
        let (changed, _) = bump_file("package-lock.json", lock, "1.2.0", "1.2.1", "app").unwrap();
        assert_eq!(changed.matches("1.2.1").count(), 2, "{changed}");
        assert!(changed.contains("\"node_modules/x\": { \"version\": \"1.2.0\" }"));

        let cargo_lock = "version = 4\n\n[[package]]\nname = \"app\"\nversion = \"0.4.7\"\n\n[[package]]\nname = \"other\"\nversion = \"0.4.7\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n";
        let (changed, _) = bump_file("desktop/Cargo.lock", cargo_lock, "0.4.7", "0.4.8", "app").unwrap();
        assert_eq!(changed.matches("0.4.8").count(), 1);
        assert!(changed.contains("name = \"other\"\nversion = \"0.4.7\""));

        let chart = "apiVersion: v2\nname: app\nversion: 1.7.8\nappVersion: \"1.7.8\"\ndependencies:\n  - name: redis\n    version: 1.7.8\n";
        let (changed, _) = bump_file("charts/app/Chart.yaml", chart, "1.7.8", "1.7.9", "app").unwrap();
        assert_eq!(changed, "apiVersion: v2\nname: app\nversion: 1.7.9\nappVersion: \"1.7.9\"\ndependencies:\n  - name: redis\n    version: 1.7.8\n");

        assert_eq!(bump_file("VERSION", "2.0.1\n", "2.0.1", "2.1.0", "app").unwrap().0, "2.1.0\n");
        assert!(bump_file("pyproject.toml", "[tool.ruff]\nversion = \"0.4.7\"\n", "0.4.7", "0.4.8", "app").is_none());
    }

    #[test]
    fn a_readme_changes_only_on_lines_that_name_the_repository() {
        let readme = "Install:\n\nsudo installer -pkg OpenReleasePilot-0.4.7.pkg -target /\n\nNeeds serde 0.4.7.\nOpenReleasePilot 0.4.71 and 10.4.7 are something else.\n";
        let (changed, what) = bump_file("README.md", readme, "0.4.7", "0.4.8", "OpenReleasePilot").unwrap();
        assert_eq!(changed, readme.replace("OpenReleasePilot-0.4.7.pkg", "OpenReleasePilot-0.4.8.pkg"));
        assert!(what.contains("1 Zeile"), "{what}");
        assert!(bump_file("README.md", "Version 0.4.7\n", "0.4.7", "0.4.8", "OpenReleasePilot").is_none());
    }

    #[test]
    fn the_changelog_gets_a_section_above_the_newest_one() {
        let log = "# Changelog\n\nAll notable changes.\n\n## [0.4.7] - 2026-10-01\n\n- Older\n";
        let entries = vec!["Menu bar icon (#12)".to_owned(), "Fix the update restart".to_owned()];
        let (changed, what) = changelog(log, "0.4.8", "v0.4.8", "2026-10-23", &entries).unwrap();
        assert_eq!(changed, "# Changelog\n\nAll notable changes.\n\n## [0.4.8] - 2026-10-23\n\n- Menu bar icon (#12)\n- Fix the update restart\n\n## [0.4.7] - 2026-10-01\n\n- Older\n");
        assert!(what.contains("2 Einträgen"));
        assert!(changelog(&changed, "0.4.8", "v0.4.8", "2026-10-23", &entries).is_none(), "the version is there already");

        let plain = "# Changes\n\n## v1.2.0\n\n- x\n";
        let (changed, _) = changelog(plain, "1.3.0", "v1.3.0", "2026-10-23", &entries).unwrap();
        assert!(changed.contains("## v1.3.0 - 2026-10-23\n\n- Menu bar icon (#12)"), "{changed}");
        assert!(changelog("# Changes\n", "1.3.0", "1.3.0", "2026-10-23", &[]).is_none());
    }

    #[test]
    fn notes_written_under_unreleased_become_the_version() {
        let log = "# Changelog\n\n## [Unreleased]\n\n### Added\n- S3 storage\n\n## [0.7.13] - 2026-10-20\n";
        let (changed, what) = changelog(log, "0.7.14", "v0.7.14", "2026-10-23", &["ignored".to_owned()]).unwrap();
        assert_eq!(changed, "# Changelog\n\n## [Unreleased]\n\n## [0.7.14] - 2026-10-23\n\n### Added\n- S3 storage\n\n## [0.7.13] - 2026-10-20\n");
        assert!(what.contains("Unreleased"));

        let empty = "# Changelog\n\n## [Unreleased]\n\n## [0.7.13] - 2026-10-20\n";
        let (changed, _) = changelog(empty, "0.7.14", "v0.7.14", "2026-10-23", &["Bump pypdf (#3)".to_owned()]).unwrap();
        assert_eq!(changed, "# Changelog\n\n## [Unreleased]\n\n## [0.7.14] - 2026-10-23\n\n- Bump pypdf (#3)\n\n## [0.7.13] - 2026-10-20\n");
    }

    #[test]
    fn commits_become_changelog_lines() {
        assert_eq!(entry("Merge pull request #12 from o/menu\x1fMenu bar icon\n").as_deref(), Some("Menu bar icon (#12)"));
        assert_eq!(entry("Merge branch 'fix' into 'main'\x1fFix login\n\nSee merge request group/app!7\n").as_deref(), Some("Fix login (!7)"));
        assert_eq!(entry("Merge branch 'main' into feature\x1f").as_deref(), None);
        assert_eq!(entry("Version 0.4.8\x1fCo-Authored-By: x").as_deref(), None);
        assert_eq!(entry("Release v0.4.8\x1f").as_deref(), None);
        assert_eq!(entry("Keep the app in the menu bar (#9)\x1f").as_deref(), Some("Keep the app in the menu bar (#9)"));
    }

    #[test]
    fn prepares_version_files_and_changelog_in_a_clone() {
        use crate::shell::output;
        let root = std::env::temp_dir().join(format!("orp-release-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let dir = root.join("repo");
        fs::create_dir_all(dir.join("node_modules/x")).unwrap();
        let git = |args: &[&str]| output("git", args, &dir).unwrap();
        git(&["init", "-q", "-b", "main"]);
        fs::write(dir.join("package.json"), "{\n  \"name\": \"app\",\n  \"version\": \"1.0.0\"\n}\n").unwrap();
        fs::write(dir.join("node_modules/x/package.json"), "{ \"version\": \"1.0.0\" }\n").unwrap();
        fs::write(dir.join("CHANGELOG.md"), "# Changelog\n").unwrap();
        git(&["add", "-f", "."]);
        git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "Version 1.0.0"]);
        let base = git(&["rev-parse", "HEAD"]);
        git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "Add export"]);
        let files = tracked(&dir).unwrap();
        assert_eq!(files, ["package.json"], "node_modules is someone else's code");
        assert_eq!(changelog_file(&dir).unwrap().as_deref(), Some("CHANGELOG.md"));
        assert_eq!(entries(&dir, base.trim()).unwrap(), ["Add export"]);
        let _ = fs::remove_dir_all(&root);
    }
}
