use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use gitassist_gateway::state::State;
use gitassist_gateway::Paths;
use serde::Serialize;

use crate::hooks_install::portable;

const MARK: &str = "# gitassist-connect";
/// pre-push is not connected: it reads stdin, which the repo's own hook may need.
const CONNECT_HOOKS: [&str; 3] = ["prepare-commit-msg", "commit-msg", "post-commit"];

#[derive(Debug, Clone, Serialize)]
pub struct RepoStatus {
    pub path: String,
    /// Set when the repo overrides the global hooks path (Husky, lefthook, ...).
    pub local_hooks_path: Option<String>,
    pub connected: bool,
}

/// Never blocks: a repo in a privacy-protected folder (Desktop, Documents…)
/// can make git wait on a macOS permission prompt, so the call has a time limit.
fn local_hooks_dir(repo: &Path) -> Option<PathBuf> {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("-C")
        .arg(repo)
        .args(["config", "--local", "--get", "core.hooksPath"]);
    let out = gitassist_gateway::repo::output_with_timeout(cmd, std::time::Duration::from_secs(3))?;
    let v = String::from_utf8_lossy(&out.stdout).into_owned();
    let v = v.trim();
    if v.is_empty() {
        return None;
    }
    let p = PathBuf::from(v);
    Some(if p.is_absolute() { p } else { repo.join(p) })
}

pub fn status(repo: &Path) -> RepoStatus {
    let dir = local_hooks_dir(repo);
    let connected = dir.as_ref().is_some_and(|d| {
        CONNECT_HOOKS
            .iter()
            .all(|h| fs::read_to_string(d.join(h)).is_ok_and(|t| t.contains(MARK)))
    });
    RepoStatus {
        path: repo.to_string_lossy().into_owned(),
        local_hooks_path: dir.map(|d| d.to_string_lossy().into_owned()),
        connected,
    }
}

/// Git repositories under `root`, up to `depth` levels deep. Skips heavy folders.
pub fn scan(root: &Path, depth: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(root, depth, &mut out);
    out
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if dir.join(".git").exists() {
        out.push(dir.to_path_buf());
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.')
            || matches!(
                name.as_str(),
                "node_modules" | "target" | "Library" | "vendor"
            )
        {
            continue;
        }
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            walk(&e.path(), depth - 1, out);
        }
    }
}

fn line(cli: &Path, hook: &str) -> String {
    format!(
        "'{}' hook {hook} --no-chain \"$@\" || exit $? {MARK}",
        portable(cli).replace('\'', r"'\''")
    )
}

pub fn connect(repo: &Path, paths: &Paths) -> Result<()> {
    let dir = local_hooks_dir(repo)
        .ok_or_else(|| anyhow::anyhow!("repo does not set a local core.hooksPath"))?;
    fs::create_dir_all(&dir)?;
    for hook in CONNECT_HOOKS {
        let file = dir.join(hook);
        let existing = fs::read_to_string(&file).unwrap_or_default();
        if existing.contains(MARK) {
            continue;
        }
        let ours = line(&paths.cli_exe(), hook);
        let new = if existing.is_empty() {
            format!("#!/bin/sh\n{ours}\n")
        } else if existing.starts_with("#!") {
            let (first, rest) = existing.split_once('\n').unwrap_or((&existing, ""));
            format!("{first}\n{ours}\n{rest}")
        } else {
            format!("{ours}\n{existing}")
        };
        fs::write(&file, new)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&file, fs::Permissions::from_mode(0o755))?;
        }
    }
    let mut state = State::load(&paths.state_file);
    let key = repo.to_string_lossy().into_owned();
    if !state.connected_repos.contains(&key) {
        state.connected_repos.push(key);
    }
    state.save(&paths.state_file)
}

pub fn disconnect(repo: &Path, paths: &Paths) -> Result<()> {
    if let Some(dir) = local_hooks_dir(repo) {
        for hook in CONNECT_HOOKS {
            let file = dir.join(hook);
            let Ok(text) = fs::read_to_string(&file) else {
                continue;
            };
            let kept: Vec<&str> = text.lines().filter(|l| !l.contains(MARK)).collect();
            if kept
                .iter()
                .all(|l| l.trim().is_empty() || l.starts_with("#!"))
            {
                fs::remove_file(&file)?;
            } else {
                fs::write(&file, format!("{}\n", kept.join("\n")))?;
            }
        }
    }
    let mut state = State::load(&paths.state_file);
    let key = repo.to_string_lossy().into_owned();
    state.connected_repos.retain(|r| r != &key);
    state.save(&paths.state_file)
}

/// Removes GitAssist's per-repo rule file and exclude block from a repo.
pub fn remove_repo_rule(repo: &Path) {
    use gitassist_core::instructions::{is_repo_rule, REPO_RULE_PATH};
    let file = repo.join(REPO_RULE_PATH);
    if fs::read_to_string(&file).is_ok_and(|t| is_repo_rule(&t)) {
        let _ = fs::remove_file(&file);
        for dir in [file.parent(), file.parent().and_then(Path::parent)]
            .into_iter()
            .flatten()
        {
            let _ = fs::remove_dir(dir);
        }
    }
    let exclude = repo.join(".git/info/exclude");
    if let Ok(text) = fs::read_to_string(&exclude) {
        let begin = "# >>> gitassist ignore >>>";
        let end = "# <<< gitassist ignore <<<";
        if let (Some(s), Some(e)) = (text.find(begin), text.find(end)) {
            if e > s {
                let mut stop = e + end.len();
                if text[stop..].starts_with('\n') {
                    stop += 1;
                }
                let _ = fs::write(&exclude, format!("{}{}", &text[..s], &text[stop..]));
            }
        }
    }
}

/// Folders macOS guards with a permission prompt. Background checks stay out
/// of them unless the user connected or added a repo there themselves.
pub fn in_protected_folder(repo: &Path, home: &Path) -> bool {
    [
        "Desktop",
        "Documents",
        "Downloads",
        "Pictures",
        "Movies",
        "Music",
        "Library/Mobile Documents",
        "Library/CloudStorage",
    ]
    .iter()
    .any(|d| repo.starts_with(home.join(d)))
}

fn mtime(p: &Path) -> Option<u64> {
    fs::metadata(p)
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

fn newest_in(dir: &Path, depth: usize) -> Option<u64> {
    let mut best = mtime(dir);
    if depth == 0 {
        return best;
    }
    for e in fs::read_dir(dir).ok()?.flatten() {
        let t = if e.file_type().is_ok_and(|t| t.is_dir()) {
            newest_in(&e.path(), depth - 1)
        } else {
            mtime(&e.path())
        };
        best = best.max(t);
    }
    best
}

/// Reads a repo's details from its `.git` folder without running git.
pub fn read_record(repo: &Path) -> Option<gitassist_gateway::registry::RepoRecord> {
    use gitassist_gateway::registry::{Remote, RepoRecord};
    let dot = repo.join(".git");
    let git_dir = if dot.is_dir() {
        dot
    } else {
        let text = fs::read_to_string(&dot).ok()?;
        let rel = text
            .lines()
            .find_map(|l| l.strip_prefix("gitdir:"))?
            .trim()
            .to_string();
        repo.join(rel)
    };
    let common = match fs::read_to_string(git_dir.join("commondir")) {
        Ok(rel) => git_dir.join(rel.trim()),
        Err(_) => git_dir.clone(),
    };
    let config = fs::read_to_string(common.join("config")).unwrap_or_default();
    let mut remotes = Vec::new();
    let mut section = String::new();
    let mut hooks_path = None;
    for line in config.lines().map(str::trim) {
        if line.starts_with('[') {
            section = line.trim_matches(['[', ']']).to_string();
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let (k, v) = (
            k.trim().to_lowercase(),
            v.trim().trim_matches('"').to_string(),
        );
        if let Some(name) = section
            .strip_prefix("remote \"")
            .and_then(|s| s.strip_suffix('"'))
        {
            if k == "url" {
                remotes.push(Remote {
                    name: name.to_string(),
                    url: v,
                });
            }
        } else if section.eq_ignore_ascii_case("core") && k == "hookspath" {
            hooks_path = Some(v);
        }
    }
    let branch = fs::read_to_string(git_dir.join("HEAD")).ok().and_then(|h| {
        h.trim()
            .strip_prefix("ref: refs/heads/")
            .map(str::to_string)
    });
    Some(RepoRecord {
        path: repo.to_string_lossy().into_owned(),
        remotes,
        branch,
        last_commit: mtime(&git_dir.join("logs/HEAD")),
        last_sync: newest_in(&common.join("refs/remotes"), 2),
        local_hooks_path: hooks_path,
        seen: gitassist_gateway::registry::now(),
        ..Default::default()
    })
}

/// Folders searched when you press "Find repos" without choosing one: your
/// home folder, skipping the ones macOS guards and app data.
pub fn default_roots(home: &Path) -> Vec<PathBuf> {
    let Ok(it) = fs::read_dir(home) else {
        return Vec::new();
    };
    it.flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|p| {
            let n = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            !n.starts_with('.')
                && !matches!(n.as_str(), "Library" | "Applications" | "Public")
                && !in_protected_folder(p, home)
        })
        .collect()
}

/// Finds repos under the roots and merges them into GitAssist's repo list.
pub fn discover(paths: &Paths, roots: &[PathBuf], depth: usize) -> usize {
    use gitassist_gateway::registry;
    let mut map = registry::load(paths);
    let mut found = 0;
    for root in roots {
        for repo in scan(root, depth) {
            if let Some(rec) = read_record(&repo) {
                registry::merge(&mut map, rec);
                found += 1;
            }
        }
    }
    registry::save(paths, &map);
    found
}
