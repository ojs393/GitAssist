//! Per-repo rule overlays stored in app data (not in the project tree).
//! Header Commits/PR pages remain global defaults; these customize one repo.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use gitassist_core::TabConfig;
use serde::{Deserialize, Serialize};

use crate::paths::Paths;
use crate::secure;
use crate::targets;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepoOverrides {
    pub repo_path: String,
    /// Partial tab configs keyed by tab id (`commit`, `pr_create`, …).
    #[serde(default)]
    pub tabs: HashMap<String, TabConfig>,
}

fn file(paths: &Paths, repo: &Path) -> PathBuf {
    paths
        .data_dir
        .join("overrides")
        .join(format!("{}.toml", targets::key_for(repo)))
}

pub fn load(paths: &Paths, repo: &Path) -> RepoOverrides {
    let f = file(paths, repo);
    std::fs::read_to_string(&f)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_else(|| RepoOverrides {
            repo_path: repo.to_string_lossy().into_owned(),
            ..Default::default()
        })
}

pub fn save(paths: &Paths, repo: &Path, data: &RepoOverrides) -> Result<()> {
    let dir = paths.data_dir.join("overrides");
    std::fs::create_dir_all(&dir).context("creating overrides directory")?;
    secure::lock_dir(&dir);
    let mut owned = data.clone();
    owned.repo_path = repo.to_string_lossy().into_owned();
    let body = toml::to_string_pretty(&owned).context("serializing overrides")?;
    std::fs::write(file(paths, repo), body).context("writing overrides")?;
    Ok(())
}

pub fn clear_tab(paths: &Paths, repo: &Path, tab: &str) -> Result<()> {
    let mut data = load(paths, repo);
    data.tabs.remove(tab);
    save(paths, repo, &data)
}
