//! Local unpushed / dirty work for a repo (ahead, dirty files, +/− counts).

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::repo::output_with_timeout;

const TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    /// e.g. M, A, D, ??, MM
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkStatus {
    pub ahead: u32,
    pub behind: u32,
    pub dirty: u32,
    pub staged: u32,
    pub additions: u32,
    pub deletions: u32,
    pub branch: String,
    pub upstream: Option<String>,
    pub files: Vec<FileChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSync {
    pub remote: String,
    pub branch: String,
    pub ref_name: Option<String>, // e.g. origin/main when known
    pub ahead: u32,
    pub behind: u32,
}

fn git(repo: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo).args(args);
    let out = output_with_timeout(cmd, TIMEOUT)?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn parse_numstat(text: &str) -> (u32, u32, Vec<(String, u32, u32)>) {
    let mut add = 0u32;
    let mut del = 0u32;
    let mut rows = Vec::new();
    for line in text.lines() {
        let mut parts = line.splitn(3, '\t');
        let a = parts.next().unwrap_or("0");
        let d = parts.next().unwrap_or("0");
        let path = parts.next().unwrap_or("").to_string();
        if path.is_empty() {
            continue;
        }
        let a_n = if a == "-" {
            0
        } else {
            a.parse().unwrap_or(0)
        };
        let d_n = if d == "-" {
            0
        } else {
            d.parse().unwrap_or(0)
        };
        add = add.saturating_add(a_n);
        del = del.saturating_add(d_n);
        rows.push((path, a_n, d_n));
    }
    (add, del, rows)
}

/// `rev-list --left-right --count A...B` prints `<behind> <ahead>`.
fn parse_left_right(text: &str) -> Option<(u32, u32)> {
    let mut it = text.split_whitespace();
    let behind = it.next()?.parse().ok()?;
    let ahead = it.next()?.parse().ok()?;
    Some((ahead, behind))
}

/// Compare HEAD to `remote/branch` if that ref exists; else ahead vs remote tips for that remote only.
pub fn status_vs_remote(repo: &Path, remote: &str, branch: &str) -> Result<RemoteSync> {
    let tip = format!("{remote}/{branch}");
    let has_tip = git(repo, &["rev-parse", "--verify", &tip]).is_some();
    let (ahead, behind) = if has_tip {
        let spec = format!("{tip}...HEAD");
        git(repo, &["rev-list", "--left-right", "--count", &spec])
            .and_then(|s| parse_left_right(&s))
            .unwrap_or((0, 0))
    } else {
        // commits not reachable from this remote's tips
        let remotes = format!("--remotes={remote}");
        let ahead = git(repo, &["rev-list", "--count", "HEAD", "--not", &remotes])
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        (ahead, 0)
    };
    Ok(RemoteSync {
        remote: remote.to_string(),
        branch: branch.to_string(),
        ref_name: has_tip.then_some(tip),
        ahead,
        behind,
    })
}

/// Snapshot of unpushed commits and uncommitted changes. Best-effort; never blocks long.
pub fn status(repo: &Path) -> Result<WorkStatus> {
    if !repo.join(".git").exists() && !repo.join(".git").is_file() {
        // worktree / bare: still try
        anyhow::ensure!(repo.is_dir(), "not a directory");
    }
    let branch = git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_default()
        .trim()
        .to_string();
    let upstream = git(repo, &["rev-parse", "--abbrev-ref", "@{u}"])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let has_remotes = git(repo, &["remote"])
        .map(|s| s.lines().any(|l| !l.trim().is_empty()))
        .unwrap_or(false);
    let (ahead, behind) = if upstream.is_some() {
        git(repo, &["rev-list", "--left-right", "--count", "@{u}...HEAD"])
            .and_then(|s| {
                let mut it = s.split_whitespace();
                let b = it.next()?.parse().ok()?;
                let a = it.next()?.parse().ok()?;
                Some((a, b))
            })
            .unwrap_or((0, 0))
    } else if has_remotes {
        // No upstream branch: commits not on any remote tip
        let ahead = git(repo, &["rev-list", "--count", "HEAD", "--not", "--remotes"])
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        (ahead, 0)
    } else {
        (0, 0)
    };

    let porcelain = git(repo, &["status", "--porcelain", "-uall"]).unwrap_or_default();
    let mut staged = 0u32;
    let mut dirty = 0u32;
    let mut status_map = std::collections::BTreeMap::<String, String>::new();
    for line in porcelain.lines() {
        if line.len() < 3 {
            continue;
        }
        let st = line[..2].to_string();
        let path = line[3..].trim().to_string();
        if path.is_empty() {
            continue;
        }
        let xy = st.as_bytes();
        if xy[0] != b' ' && xy[0] != b'?' {
            staged += 1;
        }
        if xy[1] != b' ' || xy[0] == b'?' {
            dirty += 1;
        }
        // Prefer showing the new path half for renames "R  old -> new"
        let path = path
            .rsplit_once(" -> ")
            .map(|(_, neu)| neu.to_string())
            .unwrap_or(path);
        status_map.insert(path, st);
    }

    let unstaged = git(repo, &["diff", "--numstat"]).unwrap_or_default();
    let cached = git(repo, &["diff", "--cached", "--numstat"]).unwrap_or_default();
    let (a1, d1, rows1) = parse_numstat(&unstaged);
    let (a2, d2, rows2) = parse_numstat(&cached);
    let mut file_stats = std::collections::BTreeMap::<String, (u32, u32)>::new();
    for (p, a, d) in rows1.into_iter().chain(rows2) {
        let e = file_stats.entry(p).or_default();
        e.0 = e.0.saturating_add(a);
        e.1 = e.1.saturating_add(d);
    }
    // Untracked files: count as additions roughly by line count (cap)
    for (path, st) in &status_map {
        if st.contains('?') && !file_stats.contains_key(path) {
            file_stats.insert(path.clone(), (0, 0));
        }
    }

    let mut files: Vec<FileChange> = file_stats
        .into_iter()
        .map(|(path, (additions, deletions))| FileChange {
            status: status_map.get(&path).cloned().unwrap_or_else(|| " M".into()),
            path,
            additions,
            deletions,
        })
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    if files.len() > 200 {
        files.truncate(200);
    }

    Ok(WorkStatus {
        ahead,
        behind,
        dirty: dirty.max(status_map.len() as u32),
        staged,
        additions: a1.saturating_add(a2),
        deletions: d1.saturating_add(d2),
        branch,
        upstream,
        files,
    })
}

/// Compact summary for list views (no file list).
pub fn summary(repo: &Path) -> WorkStatus {
    match status(repo) {
        Ok(mut w) => {
            w.files.clear();
            w
        }
        Err(_) => WorkStatus::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn clean_repo_summary() {
        let d = tempdir().unwrap();
        let repo = d.path();
        for args in [
            &["init", "-b", "main"][..],
            &["config", "user.email", "t@example.com"][..],
            &["config", "user.name", "t"][..],
        ] {
            assert!(Command::new("git")
                .arg("-C")
                .arg(repo)
                .args(args)
                .status()
                .unwrap()
                .success());
        }
        fs::write(repo.join("a.txt"), "hi\n").unwrap();
        assert!(Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["add", "a.txt"])
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["commit", "-m", "init"])
            .status()
            .unwrap()
            .success());
        let w = status(repo).unwrap();
        assert_eq!(w.branch, "main");
        assert_eq!(w.dirty, 0);
        assert!(w.files.is_empty());
    }

    fn git_ok(repo: &std::path::Path, args: &[&str]) {
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(repo)
                .args(args)
                .status()
                .unwrap()
                .success(),
            "git {args:?} failed"
        );
    }

    #[test]
    fn parse_left_right_swapped_order() {
        // `rev-list --left-right --count A...B` prints behind then ahead.
        assert_eq!(parse_left_right("3\t1\n"), Some((1, 3)));
    }

    #[test]
    fn status_vs_remote_counts_ahead() {
        let d = tempdir().unwrap();
        let bare = d.path().join("bare.git");
        let repo = d.path().join("repo");
        fs::create_dir(&repo).unwrap();
        assert!(Command::new("git")
            .args(["init", "--bare", "-b", "main"])
            .arg(&bare)
            .status()
            .unwrap()
            .success());
        for args in [
            &["init", "-b", "main"][..],
            &["config", "user.email", "t@example.com"][..],
            &["config", "user.name", "t"][..],
        ] {
            git_ok(&repo, args);
        }
        fs::write(repo.join("a.txt"), "hi\n").unwrap();
        git_ok(&repo, &["add", "a.txt"]);
        git_ok(&repo, &["commit", "-m", "init"]);
        git_ok(&repo, &["remote", "add", "origin", bare.to_str().unwrap()]);
        git_ok(&repo, &["push", "-u", "origin", "main"]);
        fs::write(repo.join("a.txt"), "hi\nthere\n").unwrap();
        git_ok(&repo, &["add", "a.txt"]);
        git_ok(&repo, &["commit", "-m", "local"]);

        let sync = status_vs_remote(&repo, "origin", "main").unwrap();
        assert!(sync.ahead >= 1, "ahead was {}", sync.ahead);
        assert_eq!(sync.behind, 0);
        assert_eq!(sync.remote, "origin");
        assert_eq!(sync.branch, "main");
        assert_eq!(sync.ref_name.as_deref(), Some("origin/main"));
    }

    #[test]
    fn missing_remote_ref_is_zeroish() {
        // No commits and no remote: the tip is missing and the fallback count is 0.
        let d = tempdir().unwrap();
        let repo = d.path();
        git_ok(repo, &["init", "-b", "main"]);
        let sync = status_vs_remote(repo, "origin", "main").unwrap();
        assert_eq!(sync.remote, "origin");
        assert_eq!(sync.branch, "main");
        assert_eq!(sync.ref_name, None);
        assert_eq!(sync.ahead, 0);
        assert_eq!(sync.behind, 0);
    }
}
