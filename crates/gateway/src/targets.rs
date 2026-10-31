//! Per-repo publish targets: where to push, with which rules and filters.
//! Stored in GitAssist app data only — never in the git tree.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use gitassist_core::config::{BranchConfig, Mode, TabConfig, Tabs};
use gitassist_core::Config;
use serde::{Deserialize, Serialize};

use crate::paths::Paths;
use crate::secure;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TargetRole {
    Personal,
    Public,
    Organization,
    #[default]
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PushPolicy {
    #[default]
    Manual,
    AfterCommit,
    WithAll,
    AgentOnly,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CheckCommand {
    pub name: String,
    pub shell: String,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub env: std::collections::BTreeMap<String, String>,
}

impl Default for CheckCommand {
    fn default() -> Self {
        Self {
            name: String::new(),
            shell: String::new(),
            env: Default::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TargetChecks {
    pub enabled: bool,
    pub on_fail: Mode,
    pub timeout_secs: u64,
    pub commands: Vec<CheckCommand>,
}

impl Default for TargetChecks {
    fn default() -> Self {
        Self {
            enabled: false,
            on_fail: Mode::Block,
            timeout_secs: 600,
            commands: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PublishTarget {
    pub id: String,
    pub label: String,
    pub role: TargetRole,
    pub remote_name: String,
    pub url: String,
    pub enabled: bool,
    pub push_policy: PushPolicy,
    pub include_in_push_all: bool,
    pub publish_filter: Vec<String>,
    /// When true (default if filter non-empty), push a cleaned mirror ref.
    pub use_mirror: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tabs: Option<Tabs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branches: Option<BranchConfig>,
    pub checks: TargetChecks,
}

impl Default for PublishTarget {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            role: TargetRole::Custom,
            remote_name: String::new(),
            url: String::new(),
            enabled: true,
            push_policy: PushPolicy::Manual,
            include_in_push_all: true,
            publish_filter: Vec::new(),
            use_mirror: false,
            tabs: None,
            branches: None,
            checks: TargetChecks::default(),
        }
    }
}

impl PublishTarget {
    pub fn needs_mirror(&self) -> bool {
        self.use_mirror
            || (!self.publish_filter.is_empty()
                && self
                    .publish_filter
                    .iter()
                    .any(|p| !p.trim().is_empty() && !p.trim().starts_with('#')))
    }

    pub fn new_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("t{n}")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RepoTargets {
    pub repo_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_target: Option<String>,
    pub targets: Vec<PublishTarget>,
}

pub fn key_for(repo: &Path) -> String {
    let canon = repo
        .canonicalize()
        .unwrap_or_else(|_| repo.to_path_buf())
        .to_string_lossy()
        .to_string();
    let mut h = DefaultHasher::new();
    canon.hash(&mut h);
    format!("{:x}", h.finish())
}

fn file(paths: &Paths, repo: &Path) -> PathBuf {
    paths.data_dir.join("targets").join(format!("{}.toml", key_for(repo)))
}

pub fn load(paths: &Paths, repo: &Path) -> RepoTargets {
    let f = file(paths, repo);
    std::fs::read_to_string(&f)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_else(|| RepoTargets {
            repo_path: repo.to_string_lossy().into_owned(),
            ..Default::default()
        })
}

pub fn save(paths: &Paths, repo: &Path, data: &RepoTargets) -> Result<()> {
    let dir = paths.data_dir.join("targets");
    std::fs::create_dir_all(&dir).context("creating targets directory")?;
    let f = file(paths, repo);
    let mut owned = data.clone();
    owned.repo_path = repo.to_string_lossy().into_owned();
    for t in &mut owned.targets {
        if t.id.is_empty() {
            t.id = PublishTarget::new_id();
        }
        if t.remote_name.trim().is_empty() {
            bail!("target `{}` needs a git remote name", t.label);
        }
        if t.label.trim().is_empty() {
            bail!("each target needs a label");
        }
        if !t.publish_filter.is_empty() && !t.use_mirror {
            // Filters without an explicit mirror still publish via mirror.
            t.use_mirror = true;
        }
    }
    let body = toml::to_string_pretty(&owned).context("serializing targets")?;
    gitassist_core::config::write_atomic(&f, body.as_bytes())?;
    secure::lock_file(&f);
    Ok(())
}

pub fn find_by_label<'a>(data: &'a RepoTargets, label: &str) -> Option<&'a PublishTarget> {
    let needle = label.trim().to_lowercase();
    data.targets
        .iter()
        .find(|t| t.enabled && t.label.eq_ignore_ascii_case(&needle))
}

pub fn find_by_remote<'a>(data: &'a RepoTargets, remote: &str) -> Option<&'a PublishTarget> {
    data.targets
        .iter()
        .find(|t| t.enabled && t.remote_name == remote)
}

pub fn for_push_all(data: &RepoTargets) -> Vec<&PublishTarget> {
    data.targets
        .iter()
        .filter(|t| t.enabled && t.include_in_push_all)
        .collect()
}

/// Merge target overlays onto a copy of the global config.
pub fn effective_config(base: &Config, target: &PublishTarget) -> Config {
    let mut cfg = base.clone();
    if let Some(tabs) = &target.tabs {
        merge_tabs(&mut cfg.tabs, tabs);
    }
    if let Some(branches) = &target.branches {
        cfg.branches = branches.clone();
    }
    cfg
}

fn merge_tab(dst: &mut TabConfig, src: &TabConfig) {
    if src.enabled.is_some() {
        dst.enabled = src.enabled;
    }
    if src.subject_max.is_some() {
        dst.subject_max = src.subject_max.clone();
    }
    if src.body_max.is_some() {
        dst.body_max = src.body_max.clone();
    }
    if src.total_max.is_some() {
        dst.total_max = src.total_max.clone();
    }
    if src.wrap.is_some() {
        dst.wrap = src.wrap.clone();
    }
    if src.conventional.is_some() {
        dst.conventional = src.conventional.clone();
    }
    if src.template.is_some() {
        dst.template = src.template.clone();
    }
    if src.sections_mode.is_some() {
        dst.sections_mode = src.sections_mode;
    }
    if src.managed_table.is_some() {
        dst.managed_table = src.managed_table;
    }
    if src.banned_phrases.is_some() {
        dst.banned_phrases = src.banned_phrases.clone();
    }
    if src.growth_max.is_some() {
        dst.growth_max = src.growth_max.clone();
    }
    if src.strip_trailers.is_some() {
        dst.strip_trailers = src.strip_trailers;
    }
    if src.add_trailers.is_some() {
        dst.add_trailers = src.add_trailers.clone();
    }
    if src.style.is_some() {
        dst.style = src.style.clone();
    }
}

fn merge_tabs(dst: &mut Tabs, src: &Tabs) {
    merge_tab(&mut dst.commit, &src.commit);
    merge_tab(&mut dst.docs_commit, &src.docs_commit);
    merge_tab(&mut dst.merge, &src.merge);
    merge_tab(&mut dst.pr_create, &src.pr_create);
    merge_tab(&mut dst.pr_update, &src.pr_update);
    merge_tab(&mut dst.pr_comment, &src.pr_comment);
    merge_tab(&mut dst.review, &src.review);
    merge_tab(&mut dst.issue, &src.issue);
}

/// Ensure each enabled target's remote exists with the configured URL.
pub fn ensure_remotes(repo: &Path, data: &RepoTargets) -> Result<Vec<String>> {
    let mut notes = Vec::new();
    for t in data.targets.iter().filter(|t| t.enabled) {
        if t.url.trim().is_empty() {
            continue;
        }
        let name = t.remote_name.trim();
        let existing = crate::git::git_in(Some(repo), &["remote", "get-url", name]);
        match existing {
            Ok(url) if url.trim() == t.url.trim() => {}
            Ok(_) => {
                crate::git::git_in(Some(repo), &["remote", "set-url", name, t.url.trim()])?;
                notes.push(format!("updated remote `{name}`"));
            }
            Err(_) => {
                crate::git::git_in(Some(repo), &["remote", "add", name, t.url.trim()])?;
                notes.push(format!("added remote `{name}`"));
            }
        }
    }
    Ok(notes)
}

/// Markdown section for agent instruction files (no private URLs).
pub fn render_targets_section(data: &RepoTargets) -> String {
    if data.targets.is_empty() {
        return String::new();
    }
    let mut out = vec![
        "### Publish targets".to_string(),
        String::new(),
        "This repo has named push destinations. Prefer the GitAssist CLI:".into(),
        "- `gitassist push <label>` for one target (for example `personal`, `public`, `org`)".into(),
        "- `gitassist push --all` for every target marked include-in-push-all (all-or-nothing)".into(),
        "- Plain `git push <remote>` still works when the remote matches a target; that target's rules apply.".into(),
        String::new(),
    ];
    for t in data.targets.iter().filter(|t| t.enabled) {
        let role = match t.role {
            TargetRole::Personal => "personal",
            TargetRole::Public => "public",
            TargetRole::Organization => "organization",
            TargetRole::Custom => "custom",
        };
        let mut line = format!(
            "- **{}** (role: {role}, remote: `{}`, policy: {:?})",
            t.label, t.remote_name, t.push_policy
        );
        if t.needs_mirror() {
            line.push_str(&format!(
                "; publish filter: {}",
                t.publish_filter
                    .iter()
                    .filter(|p| !p.trim().is_empty())
                    .take(8)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if t.checks.enabled && !t.checks.commands.is_empty() {
            let names: Vec<_> = t.checks.commands.iter().map(|c| c.name.as_str()).collect();
            line.push_str(&format!("; checks: {}", names.join(", ")));
        }
        if t.branches.is_some() {
            line.push_str("; has branch rule overrides");
        }
        if t.tabs.is_some() {
            line.push_str("; has message rule overrides");
        }
        out.push(line);
    }
    out.push(String::new());
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn round_trip_save_load() {
        let dir = tempdir().unwrap();
        let paths = Paths::from_dirs(dir.path().join("c"), dir.path().join("d"));
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let mut data = RepoTargets {
            repo_path: repo.to_string_lossy().into(),
            default_target: Some("personal".into()),
            targets: vec![PublishTarget {
                id: "t1".into(),
                label: "personal".into(),
                role: TargetRole::Personal,
                remote_name: "origin".into(),
                url: "git@github.com:me/private.git".into(),
                include_in_push_all: true,
                ..Default::default()
            }],
        };
        save(&paths, &repo, &data).unwrap();
        let loaded = load(&paths, &repo);
        assert_eq!(loaded.targets.len(), 1);
        assert_eq!(loaded.targets[0].label, "personal");
        assert_eq!(loaded.default_target.as_deref(), Some("personal"));
        data.targets[0].publish_filter = vec![".env".into()];
        save(&paths, &repo, &data).unwrap();
        let loaded = load(&paths, &repo);
        assert!(loaded.targets[0].use_mirror);
    }
}
