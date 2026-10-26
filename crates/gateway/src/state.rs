use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// What the installer changed, so it can be chained to and undone.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct State {
    /// `core.hooksPath` that was set globally before GitAssist took over.
    pub previous_global_hooks_path: Option<String>,
    pub hooks_installed: bool,
    /// The user pressed "Turn on": keep everything installed and self-repairing.
    pub turned_on: bool,
    pub excludes_file_set: bool,
    pub shim_installed: bool,
    /// Shell profile files GitAssist added its PATH block to.
    pub profiles: Vec<String>,
    pub windows_user_path: bool,
    pub autostart: bool,
    /// agent id -> setting key -> previous JSON value (`null` = key was absent).
    pub adapter_previous: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
    /// agent id -> files GitAssist created (deleted again on removal if left empty).
    pub adapter_created_files: BTreeMap<String, Vec<String>>,
    /// agent id -> file -> original and last-written contents, for exact restore.
    pub adapter_files: BTreeMap<String, BTreeMap<String, FileRecord>>,
    /// Repos with a local hooks path that GitAssist was connected into.
    pub connected_repos: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FileRecord {
    /// `None` when the file did not exist before GitAssist.
    pub original: Option<String>,
    pub written: String,
}

impl State {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        gitassist_core::config::write_atomic(path, serde_json::to_string_pretty(self)?.as_bytes())?;
        crate::secure::lock_file(path);
        Ok(())
    }
}
