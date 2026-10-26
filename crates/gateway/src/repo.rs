use std::collections::BTreeMap;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gitassist_core::config::Config;
use gitassist_core::scope::{parse_remote, scope_for, RemoteRepo, Scope, Visibility};
use serde::{Deserialize, Serialize};

use crate::gh::find_real_gh;
use crate::paths::Paths;
use crate::{git, INTERNAL_ENV};

const FRESH_SECS: u64 = 7 * 24 * 3600;
const RETRY_UNKNOWN_SECS: u64 = 3600;
const LOOKUP_TIMEOUT: Duration = Duration::from_millis(1500);

/// Runs a command, killing it if it outlives `timeout`.
pub fn output_with_timeout(mut cmd: Command, timeout: Duration) -> Option<Output> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().ok(),
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(15)),
            Err(_) => return None,
        }
    }
}

pub fn origin_url() -> Option<String> {
    git::first_remote_url()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Cached {
    visibility: String,
    checked: u64,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn parse_visibility(s: &str) -> Visibility {
    match s.trim().to_lowercase().as_str() {
        "public" => Visibility::Public,
        "private" | "internal" => Visibility::Private,
        _ => Visibility::Unknown,
    }
}

/// Asks the user's own `gh` once per repo and caches the answer, so commits
/// stay fast and offline after the first check.
pub fn visibility(paths: &Paths, repo: &RemoteRepo) -> Visibility {
    let file = paths.data_dir.join("repos.json");
    let mut cache: BTreeMap<String, Cached> = std::fs::read_to_string(&file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let key = format!("{}/{}", repo.host, repo.slug());
    if let Some(c) = cache.get(&key) {
        let v = parse_visibility(&c.visibility);
        let ttl = if v == Visibility::Unknown {
            RETRY_UNKNOWN_SECS
        } else {
            FRESH_SECS
        };
        if now().saturating_sub(c.checked) < ttl {
            return v;
        }
    }
    let v = lookup(paths, repo);
    cache.insert(
        key,
        Cached {
            visibility: match v {
                Visibility::Public => "public",
                Visibility::Private => "private",
                Visibility::Unknown => "unknown",
            }
            .into(),
            checked: now(),
        },
    );
    if let Ok(s) = serde_json::to_string_pretty(&cache) {
        let _ = gitassist_core::config::write_atomic(&file, s.as_bytes());
    }
    v
}

/// The remembered answer only; never asks gh.
pub fn cached_visibility(paths: &Paths, repo: &RemoteRepo) -> Visibility {
    let cache: BTreeMap<String, Cached> =
        std::fs::read_to_string(paths.data_dir.join("repos.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
    cache
        .get(&format!("{}/{}", repo.host, repo.slug()))
        .map(|c| parse_visibility(&c.visibility))
        .unwrap_or(Visibility::Unknown)
}

fn lookup(paths: &Paths, repo: &RemoteRepo) -> Visibility {
    let Some(gh) = find_real_gh(Some(&paths.bin_dir)) else {
        return Visibility::Unknown;
    };
    let target = if repo.host == "github.com" {
        repo.slug()
    } else {
        format!("{}/{}", repo.host, repo.slug())
    };
    let mut cmd = Command::new(gh);
    cmd.args([
        "repo",
        "view",
        &target,
        "--json",
        "visibility",
        "--jq",
        ".visibility",
    ])
    .env("GH_PROMPT_DISABLED", "1")
    .env(INTERNAL_ENV, "1");
    match output_with_timeout(cmd, LOOKUP_TIMEOUT) {
        Some(o) if o.status.success() => parse_visibility(&String::from_utf8_lossy(&o.stdout)),
        _ => Visibility::Unknown,
    }
}

pub fn scope_for_url(cfg: &Config, paths: &Paths, url: Option<&str>) -> Scope {
    let remote = url.and_then(parse_remote);
    let vis = match &remote {
        Some(r) if cfg.ignore.lookup_visibility => visibility(paths, r),
        _ => Visibility::Unknown,
    };
    scope_for(remote.as_ref(), &cfg.ignore.github_user, vis)
}

pub fn current_scope(cfg: &Config, paths: &Paths) -> Scope {
    scope_for_url(cfg, paths, origin_url().as_deref())
}
