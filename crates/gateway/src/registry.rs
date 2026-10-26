//! Every repo GitAssist has seen: where it pushes and when it was last
//! committed to and pushed. Hooks keep it current as they run, so it needs no
//! extra permissions; a scan can add repos you haven't touched yet.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::paths::Paths;
use crate::{git, secure};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Remote {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RepoRecord {
    pub path: String,
    pub remotes: Vec<Remote>,
    pub branch: Option<String>,
    pub last_commit: Option<u64>,
    pub last_push: Option<u64>,
    pub last_push_remote: Option<String>,
    /// Remote-tracking refs last updated (push or fetch), from a scan.
    pub last_sync: Option<u64>,
    /// Set when the repo uses its own hooks folder (Husky, lefthook).
    pub local_hooks_path: Option<String>,
    pub seen: u64,
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn file(paths: &Paths) -> std::path::PathBuf {
    paths.data_dir.join("repos-index.json")
}

pub fn load(paths: &Paths) -> BTreeMap<String, RepoRecord> {
    std::fs::read_to_string(file(paths))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(paths: &Paths, map: &BTreeMap<String, RepoRecord>) {
    if let Ok(s) = serde_json::to_string_pretty(map) {
        let _ = gitassist_core::config::write_atomic(&file(paths), s.as_bytes());
        secure::lock_file(&file(paths));
    }
}

/// Merges a record in, keeping the newest timestamps from both.
pub fn merge(map: &mut BTreeMap<String, RepoRecord>, rec: RepoRecord) {
    let e = map.entry(rec.path.clone()).or_default();
    let keep_max = |a: Option<u64>, b: Option<u64>| a.max(b);
    e.path = rec.path;
    if !rec.remotes.is_empty() {
        e.remotes = rec.remotes;
    }
    if rec.branch.is_some() {
        e.branch = rec.branch;
    }
    if rec.last_push > e.last_push {
        e.last_push_remote = rec.last_push_remote.or(e.last_push_remote.take());
    }
    e.last_commit = keep_max(e.last_commit, rec.last_commit);
    e.last_push = keep_max(e.last_push, rec.last_push);
    e.last_sync = keep_max(e.last_sync, rec.last_sync);
    e.local_hooks_path = rec.local_hooks_path.or(e.local_hooks_path.take());
    e.seen = e.seen.max(rec.seen);
}

/// Updates the current repo from inside a hook. Never fails the hook.
pub fn touch(paths: &Paths, update: impl FnOnce(&mut RepoRecord)) {
    let Some(top) = git::toplevel() else { return };
    let mut rec = RepoRecord {
        path: top,
        seen: now(),
        branch: git::current_branch(),
        ..Default::default()
    };
    rec.remotes = git::repo_config()
        .iter()
        .filter_map(|(k, v)| {
            let name = k.strip_prefix("remote.")?.strip_suffix(".url")?;
            Some(Remote {
                name: name.to_string(),
                url: v.clone(),
            })
        })
        .collect();
    update(&mut rec);
    let mut map = load(paths);
    merge(&mut map, rec);
    save(paths, &map);
}

/// Drops repos whose folder no longer exists (only checked outside folders macOS guards).
pub fn prune(
    map: &mut BTreeMap<String, RepoRecord>,
    home: &Path,
    protected: impl Fn(&Path, &Path) -> bool,
) {
    map.retain(|path, _| {
        let p = Path::new(path);
        protected(p, home) || p.join(".git").exists()
    });
}
