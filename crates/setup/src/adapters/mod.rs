//! Agent integrations. Each adapter only *declares* what it changes; applying,
//! removing and checking are shared, so every adapter is reversible the same way.

mod aider;
mod claude;
mod codex;
mod copilot;
mod cursor;
mod gemini;
mod yaml;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use gitassist_core::Config;
use gitassist_gateway::state::{FileRecord, State};
use gitassist_gateway::Paths;
use serde::Serialize;
use serde_json::Value;

use crate::blocks::{self, MD};
use crate::{backup, jsonc, Env};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Change {
    /// A GitAssist-marked markdown block in an instruction file.
    Block { path: PathBuf, content: String },
    /// A key in a JSON / JSONC settings file. `key` is a path of object keys.
    Json {
        path: PathBuf,
        key: Vec<String>,
        value: Value,
        only_if_exists: bool,
    },
    /// A top-level scalar in a YAML file; `value` is YAML scalar text.
    Yaml {
        path: PathBuf,
        key: String,
        value: String,
    },
}

impl Change {
    pub fn path(&self) -> &Path {
        match self {
            Change::Block { path, .. } | Change::Json { path, .. } | Change::Yaml { path, .. } => {
                path
            }
        }
    }

    fn state_key(&self) -> String {
        match self {
            Change::Block { path, .. } => format!("{}::block", path.display()),
            Change::Json { path, key, .. } => format!("{}::{}", path.display(), key.join(".")),
            Change::Yaml { path, key, .. } => format!("{}::{key}", path.display()),
        }
    }

    fn skipped(&self) -> bool {
        matches!(self, Change::Json { path, only_if_exists: true, .. } if !path.is_file())
    }

    fn is_current(&self) -> bool {
        match self {
            Change::Block { path, content } => std::fs::read_to_string(path)
                .ok()
                .and_then(|t| blocks::block_content(&t, MD))
                .is_some_and(|c| c.trim() == content.trim()),
            Change::Json {
                path, key, value, ..
            } => {
                let k: Vec<&str> = key.iter().map(String::as_str).collect();
                jsonc::get_file(path, &k).ok().flatten().as_ref() == Some(value)
            }
            Change::Yaml { path, key, value } => {
                yaml::get_file(path, key).as_deref() == Some(value.as_str())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    NotDetected,
    NotApplied,
    Current,
    Stale,
}

#[derive(Debug, Clone, Serialize)]
pub struct Adapter {
    pub id: &'static str,
    pub name: &'static str,
    /// Official docs the targets below were verified against.
    pub docs: Vec<&'static str>,
    pub detected: bool,
    pub changes: Vec<Change>,
    /// Steps with no file-based setting; shown in the app with a copy button.
    pub manual: Vec<String>,
    /// What cannot be enforced locally for this agent.
    pub limits: Vec<String>,
}

pub fn all(env: &Env, cfg: &Config) -> Vec<Adapter> {
    vec![
        claude::adapter(env, cfg),
        cursor::adapter(env, cfg),
        copilot::adapter(env, cfg),
        codex::adapter(env, cfg),
        gemini::adapter(env, cfg),
        aider::adapter(env, cfg),
    ]
}

pub fn find(env: &Env, cfg: &Config, id: &str) -> Option<Adapter> {
    all(env, cfg).into_iter().find(|a| a.id == id)
}

pub fn status(a: &Adapter, paths: &Paths) -> Status {
    if !a.detected {
        return Status::NotDetected;
    }
    let live: Vec<&Change> = a.changes.iter().filter(|c| !c.skipped()).collect();
    if live.iter().all(|c| c.is_current()) {
        return Status::Current;
    }
    let state = State::load(&paths.state_file);
    if state.adapter_previous.contains_key(a.id) || state.adapter_created_files.contains_key(a.id) {
        Status::Stale
    } else {
        Status::NotApplied
    }
}

pub fn apply(a: &Adapter, paths: &Paths) -> Result<()> {
    let mut state = State::load(&paths.state_file);
    let live: Vec<&Change> = a.changes.iter().filter(|c| !c.skipped()).collect();
    for c in &live {
        let key = c.path().to_string_lossy().into_owned();
        let files = state.adapter_files.entry(a.id.to_string()).or_default();
        files.entry(key).or_insert_with(|| FileRecord {
            original: fs::read_to_string(c.path()).ok(),
            written: String::new(),
        });
    }
    for c in live {
        backup::backup_file(paths, c.path())?;
        let existed = c.path().exists();
        let prev = state.adapter_previous.entry(a.id.to_string()).or_default();
        match c {
            Change::Block { path, content } => {
                blocks::upsert_file(path, MD, content)?;
            }
            Change::Json {
                path, key, value, ..
            } => {
                let k: Vec<&str> = key.iter().map(String::as_str).collect();
                let old = jsonc::get_file(path, &k)?.unwrap_or(Value::Null);
                prev.entry(c.state_key()).or_insert(old);
                jsonc::set_file(path, &k, Some(value))?;
            }
            Change::Yaml { path, key, value } => {
                let old = yaml::get_file(path, key)
                    .map(Value::String)
                    .unwrap_or(Value::Null);
                prev.entry(c.state_key()).or_insert(old);
                yaml::set_file(path, key, Some(value))?;
            }
        }
        if !existed {
            let created = state
                .adapter_created_files
                .entry(a.id.to_string())
                .or_default();
            let p = c.path().to_string_lossy().into_owned();
            if !created.contains(&p) {
                created.push(p);
            }
        }
    }
    state.adapter_previous.entry(a.id.to_string()).or_default();
    if let Some(files) = state.adapter_files.get_mut(a.id) {
        for (path, rec) in files.iter_mut() {
            rec.written = fs::read_to_string(path).unwrap_or_default();
        }
    }
    state.save(&paths.state_file)
}

/// Restores every key to its pre-GitAssist value and removes GitAssist blocks.
pub fn remove(a: &Adapter, paths: &Paths) -> Result<()> {
    let mut state = State::load(&paths.state_file);
    let prev = state.adapter_previous.remove(a.id).unwrap_or_default();
    let created = state.adapter_created_files.remove(a.id).unwrap_or_default();
    let records = state.adapter_files.remove(a.id).unwrap_or_default();
    let was_created = |p: &Path| created.iter().any(|c| Path::new(c) == p);

    // Untouched since GitAssist last wrote it: put the original bytes back exactly.
    let mut restored: Vec<String> = Vec::new();
    for (path, rec) in &records {
        let current = fs::read_to_string(path).ok();
        if current.as_deref() == Some(rec.written.as_str()) {
            match &rec.original {
                Some(orig) => fs::write(path, orig)?,
                None => fs::remove_file(path)?,
            }
            restored.push(path.clone());
        }
    }

    // Edited by the user since: undo only GitAssist's own keys and blocks.
    for c in &a.changes {
        if restored.iter().any(|p| Path::new(p) == c.path()) {
            continue;
        }
        match c {
            Change::Block { path, .. } => blocks::remove_file_block(path, MD, was_created(path))?,
            Change::Json { path, key, .. } => {
                let Some(old) = prev.get(&c.state_key()) else {
                    continue;
                };
                let k: Vec<&str> = key.iter().map(String::as_str).collect();
                jsonc::set_file(path, &k, (!old.is_null()).then_some(old))?;
            }
            Change::Yaml { path, key, .. } => {
                let Some(old) = prev.get(&c.state_key()) else {
                    continue;
                };
                yaml::set_file(path, key, old.as_str())?;
            }
        }
    }
    for p in &created {
        let path = Path::new(p);
        if let Ok(t) = std::fs::read_to_string(path) {
            if t.trim().is_empty() || t.trim() == "{}" {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    state.save(&paths.state_file)
}

fn json(path: PathBuf, key: &[&str], value: Value, only_if_exists: bool) -> Change {
    Change::Json {
        path,
        key: key.iter().map(|s| s.to_string()).collect(),
        value,
        only_if_exists,
    }
}

fn strips_attribution(cfg: &Config) -> bool {
    cfg.global.trailers.strip_mode != gitassist_core::Mode::Warn
}

fn env_dir(var: &str, default: PathBuf) -> PathBuf {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or(default)
}
