use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use anyhow::{bail, Result};

/// Runs git with an argument vector (never a shell string) and returns stdout.
pub fn git(args: &[&str]) -> Result<String> {
    git_in(None, args)
}

pub fn git_in(dir: Option<&Path>, args: &[&str]) -> Result<String> {
    let mut cmd = Command::new("git");
    if let Some(d) = dir {
        cmd.arg("-C").arg(d);
    }
    let out = cmd
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output()?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn git_opt(args: &[&str]) -> Option<String> {
    git(args)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Repository folders. Git runs hooks from the top of the work tree, so they
/// can usually be read straight from `.git` without spawning git (each spawn
/// costs ~10 ms, and a commit runs four hooks).
struct Layout {
    toplevel: PathBuf,
    git_dir: PathBuf,
    common_dir: PathBuf,
}

fn read_layout() -> Option<Layout> {
    if std::env::var_os("GIT_DIR").is_some() || std::env::var_os("GIT_WORK_TREE").is_some() {
        return None;
    }
    let cwd = std::env::current_dir().ok()?;
    let dot = cwd.join(".git");
    let git_dir = if dot.is_dir() {
        dot
    } else {
        let text = std::fs::read_to_string(&dot).ok()?;
        let rel = text.lines().find_map(|l| l.strip_prefix("gitdir:"))?.trim();
        cwd.join(rel)
    };
    if !git_dir.join("HEAD").exists() {
        return None;
    }
    let common_dir = match std::fs::read_to_string(git_dir.join("commondir")) {
        Ok(rel) => git_dir.join(rel.trim()),
        Err(_) => git_dir.clone(),
    };
    Some(Layout {
        toplevel: cwd,
        git_dir,
        common_dir,
    })
}

fn layout() -> Option<&'static Layout> {
    static L: OnceLock<Option<Layout>> = OnceLock::new();
    L.get_or_init(read_layout).as_ref()
}

pub fn git_dir() -> Option<PathBuf> {
    if let Some(l) = layout() {
        return Some(l.git_dir.clone());
    }
    git_opt(&["rev-parse", "--absolute-git-dir"]).map(PathBuf::from)
}

pub fn common_dir() -> Option<PathBuf> {
    if let Some(l) = layout() {
        return Some(l.common_dir.clone());
    }
    let p = PathBuf::from(git_opt(&["rev-parse", "--git-common-dir"])?);
    if p.is_absolute() {
        Some(p)
    } else {
        std::env::current_dir().ok().map(|cwd| cwd.join(p))
    }
}

pub fn toplevel() -> Option<String> {
    if let Some(l) = layout() {
        return Some(l.toplevel.to_string_lossy().into_owned());
    }
    git_opt(&["rev-parse", "--show-toplevel"])
}

/// The repo settings GitAssist reads, fetched with one git call: the comment
/// character and every remote URL, in config order. Keys are lower-case.
pub fn repo_config() -> &'static [(String, String)] {
    static C: OnceLock<Vec<(String, String)>> = OnceLock::new();
    C.get_or_init(|| {
        git(&[
            "config",
            "-z",
            "--get-regexp",
            r"^(core\.commentchar|remote\..*\.url)$",
        ])
        .map(|out| {
            out.split('\0')
                .filter_map(|rec| rec.split_once('\n'))
                .map(|(k, v)| (k.to_lowercase(), v.to_string()))
                .collect()
        })
        .unwrap_or_default()
    })
}

pub fn remote_url(name: &str) -> Option<String> {
    let key = format!("remote.{}.url", name.to_lowercase());
    repo_config()
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.clone())
}

pub fn first_remote_url() -> Option<String> {
    remote_url("origin").or_else(|| {
        repo_config()
            .iter()
            .find(|(k, _)| k.starts_with("remote.") && k.ends_with(".url"))
            .map(|(_, v)| v.clone())
    })
}

pub fn staged_files() -> Vec<String> {
    git(&["diff", "--cached", "--name-only", "-z"])
        .map(|s| {
            s.split('\0')
                .filter(|p| !p.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

pub fn comment_char() -> char {
    let value = repo_config()
        .iter()
        .find(|(k, _)| k == "core.commentchar")
        .map(|(_, v)| v.trim().to_string());
    match value {
        Some(c) if c != "auto" && c.chars().count() == 1 => c.chars().next().unwrap_or('#'),
        _ => '#',
    }
}

/// A merge, squash, rebase or cherry-pick is in progress in this repository.
pub fn in_progress_op() -> Option<&'static str> {
    let dir = git_dir()?;
    for (file, name) in [
        ("MERGE_HEAD", "merge"),
        ("SQUASH_MSG", "squash"),
        ("CHERRY_PICK_HEAD", "cherry-pick"),
        ("REVERT_HEAD", "revert"),
        ("rebase-merge", "rebase"),
        ("rebase-apply", "rebase"),
    ] {
        if dir.join(file).exists() {
            return Some(name);
        }
    }
    None
}

pub fn global_config_get(key: &str) -> Option<String> {
    git_opt(&["config", "--global", "--get", key])
}

/// The checked-out branch name, or None when HEAD is detached.
pub fn current_branch() -> Option<String> {
    let head = std::fs::read_to_string(git_dir()?.join("HEAD")).ok()?;
    head.trim()
        .strip_prefix("ref: refs/heads/")
        .map(str::to_string)
}

/// True before the first commit, when there is nothing to protect yet.
pub fn is_unborn() -> bool {
    git_opt(&["rev-parse", "--verify", "-q", "HEAD"]).is_none()
}
