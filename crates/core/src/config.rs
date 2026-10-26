use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::tab::Tab;

pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Fix,
    Block,
    Warn,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleSetting<T> {
    pub value: T,
    pub mode: Mode,
}

impl<T> RuleSetting<T> {
    pub fn new(value: T, mode: Mode) -> Self {
        Self { value, mode }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Conventional {
    Required,
    Forbidden,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    #[default]
    Neutral,
    Friendly,
    Formal,
    Terse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Voice {
    #[default]
    Imperative,
    Past,
    We,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Detail {
    Low,
    #[default]
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Style {
    pub tone: Tone,
    pub voice: Voice,
    pub detail: Detail,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Identity {
    pub author_name: String,
    pub author_email: String,
    pub enforce_author: bool,
}

impl Identity {
    pub fn is_set(&self) -> bool {
        !self.author_name.trim().is_empty() && !self.author_email.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Trailers {
    pub strip_patterns: Vec<String>,
    pub strip_mode: Mode,
    pub add: Vec<String>,
}

pub fn default_strip_patterns() -> Vec<String> {
    [
        r"^\s*Co-authored-by:.*$",
        r"^.*Generated with \[?Claude Code\]?.*$",
        r"^.*Made with \[?Cursor\]?.*$",
        r"^\s*Made-with:\s*Cursor.*$",
        r"^\s*\x{1F916}.*$",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

impl Default for Trailers {
    fn default() -> Self {
        Self {
            strip_patterns: default_strip_patterns(),
            strip_mode: Mode::Fix,
            add: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Docs {
    pub patterns: Vec<String>,
}

impl Default for Docs {
    fn default() -> Self {
        Self {
            patterns: vec!["*.md".into(), "docs/**".into(), "README*".into()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct GlobalConfig {
    pub identity: Identity,
    pub trailers: Trailers,
    pub style: Style,
    pub docs: Docs,
}

/// Files that must never be committed, scoped by who owns the GitHub repo and
/// whether it is public. Lists use .gitignore syntax (including `!` negation).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IgnoreConfig {
    pub enabled: bool,
    /// Fix: unstage the file and add it to the repo's local exclude list.
    pub mode: Mode,
    /// Repos owned by this GitHub account count as personal; others as organization.
    pub github_user: String,
    /// Ask the user's own `gh` whether a repo is public (once per repo, cached).
    pub lookup_visibility: bool,
    pub every_repo: Vec<String>,
    pub personal: Vec<String>,
    pub organization: Vec<String>,
    pub public: Vec<String>,
    pub private: Vec<String>,
}

/// Which branch to work off, which branches never get direct commits or
/// pushes, how new branches are named, and what agents do with new ideas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BranchConfig {
    pub enabled: bool,
    pub base: String,
    pub protected: Vec<String>,
    pub protect_mode: Mode,
    /// Allowed first parts of new branch names (`feat/…`); empty = any name.
    pub prefixes: Vec<String>,
    pub naming_mode: Mode,
    pub ideas: String,
}

impl Default for BranchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            base: "main".into(),
            protected: strings(&["main", "master"]),
            protect_mode: Mode::Warn,
            prefixes: Vec::new(),
            naming_mode: Mode::Warn,
            ideas: "Open an issue for the idea first (gh issue create --label idea), then branch off the base branch and link the issue in the pull request.".into(),
        }
    }
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

impl Default for IgnoreConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: Mode::Fix,
            github_user: String::new(),
            lookup_visibility: true,
            every_repo: strings(&[".DS_Store", "._*", "Thumbs.db", "desktop.ini", "*.swp"]),
            personal: Vec::new(),
            organization: strings(&[
                "CLAUDE.local.md",
                ".claude/settings.local.json",
                ".aider*",
                "*.local.md",
            ]),
            public: strings(&[
                ".env",
                ".env.*",
                "!.env.example",
                "!.env.sample",
                "*.pem",
                "*.key",
                "*.p12",
                "*.pfx",
                "id_rsa*",
                "id_ed25519*",
                "*.keystore",
                "credentials.json",
                "secrets.*",
            ]),
            private: Vec::new(),
        }
    }
}

/// Per-tab overrides. `None` means "inherit the built-in default for this tab"
/// (or the global value, for style and trailers).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TabConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_max: Option<RuleSetting<usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_max: Option<RuleSetting<usize>>,
    /// Whole message: subject/title plus body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_max: Option<RuleSetting<usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap: Option<RuleSetting<usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conventional: Option<RuleSetting<Conventional>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sections_mode: Option<Mode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub managed_table: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banned_phrases: Option<RuleSetting<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub growth_max: Option<RuleSetting<usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strip_trailers: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add_trailers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<Style>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Tabs {
    pub commit: TabConfig,
    pub docs_commit: TabConfig,
    pub merge: TabConfig,
    pub pr_create: TabConfig,
    pub pr_update: TabConfig,
    pub pr_comment: TabConfig,
    pub review: TabConfig,
    pub issue: TabConfig,
}

impl Tabs {
    pub fn get(&self, tab: Tab) -> &TabConfig {
        match tab {
            Tab::Commit => &self.commit,
            Tab::DocsCommit => &self.docs_commit,
            Tab::Merge => &self.merge,
            Tab::PrCreate => &self.pr_create,
            Tab::PrUpdate => &self.pr_update,
            Tab::PrComment => &self.pr_comment,
            Tab::Review => &self.review,
            Tab::Issue => &self.issue,
        }
    }

    pub fn get_mut(&mut self, tab: Tab) -> &mut TabConfig {
        match tab {
            Tab::Commit => &mut self.commit,
            Tab::DocsCommit => &mut self.docs_commit,
            Tab::Merge => &mut self.merge,
            Tab::PrCreate => &mut self.pr_create,
            Tab::PrUpdate => &mut self.pr_update,
            Tab::PrComment => &mut self.pr_comment,
            Tab::Review => &mut self.review,
            Tab::Issue => &mut self.issue,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub enabled: bool,
    pub allow_project_overrides: bool,
    pub log_bodies: bool,
    pub watched_repos: Vec<String>,
    pub global: GlobalConfig,
    pub tabs: Tabs,
    pub ignore: IgnoreConfig,
    pub branches: BranchConfig,
    /// Re-check every few minutes and quietly reinstall anything another tool undid.
    pub auto_repair: bool,
    /// Write the rules into each repo as `.cursor/rules/gitassist.mdc` (kept out
    /// of commits), because Cursor reads no global instruction file.
    pub repo_rule_files: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            enabled: true,
            allow_project_overrides: false,
            log_bodies: false,
            watched_repos: Vec::new(),
            global: GlobalConfig::default(),
            tabs: Tabs::default(),
            ignore: IgnoreConfig::default(),
            branches: BranchConfig::default(),
            auto_repair: true,
            repo_rule_files: true,
        }
    }
}

impl Config {
    pub fn from_toml_str(s: &str) -> Result<Self> {
        let mut table: toml::Table = toml::from_str(s).context("config is not valid TOML")?;
        let version = table
            .get("version")
            .and_then(|v| v.as_integer())
            .unwrap_or(0);
        if version > CONFIG_VERSION as i64 {
            bail!(
                "config version {version} is newer than this GitAssist supports ({CONFIG_VERSION})"
            );
        }
        if version < CONFIG_VERSION as i64 {
            table.insert(
                "version".into(),
                toml::Value::Integer(CONFIG_VERSION as i64),
            );
        }
        let cfg: Config = toml::Value::Table(table)
            .try_into()
            .context("config has invalid values")?;
        Ok(cfg)
    }

    pub fn to_toml_string(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// Missing file means defaults; any other problem is an error.
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(s) => Self::from_toml_str(&s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    /// Writes atomically and keeps keys from the existing file that this
    /// version does not know about.
    pub fn save_atomic(&self, path: &Path) -> Result<()> {
        let mut new: toml::Table = toml::Table::try_from(self)?;
        if let Ok(existing) = fs::read_to_string(path) {
            if let Ok(old) = toml::from_str::<toml::Table>(&existing) {
                let known = toml::Table::try_from(Self::schema_example())?;
                merge_unknown(&mut new, &old, &known);
            }
        }
        write_atomic(path, toml::to_string_pretty(&new)?.as_bytes())
    }

    /// A config with every optional field populated, used to tell keys this
    /// version understands apart from keys written by a newer version.
    fn schema_example() -> Self {
        let full = TabConfig {
            enabled: Some(true),
            subject_max: Some(RuleSetting::new(0, Mode::Warn)),
            body_max: Some(RuleSetting::new(0, Mode::Warn)),
            total_max: Some(RuleSetting::new(0, Mode::Warn)),
            wrap: Some(RuleSetting::new(0, Mode::Warn)),
            conventional: Some(RuleSetting::new(Conventional::Off, Mode::Warn)),
            template: Some(String::new()),
            sections_mode: Some(Mode::Warn),
            managed_table: Some(false),
            banned_phrases: Some(RuleSetting::new(Vec::new(), Mode::Warn)),
            growth_max: Some(RuleSetting::new(0, Mode::Warn)),
            strip_trailers: Some(true),
            add_trailers: Some(Vec::new()),
            style: Some(Style::default()),
        };
        let mut cfg = Self::default();
        for tab in Tab::ALL {
            *cfg.tabs.get_mut(tab) = full.clone();
        }
        cfg
    }
}

fn merge_unknown(new: &mut toml::Table, old: &toml::Table, known: &toml::Table) {
    for (k, v) in old {
        match known.get(k) {
            None => {
                new.entry(k.clone()).or_insert_with(|| v.clone());
            }
            Some(toml::Value::Table(known_child)) => {
                if let (Some(toml::Value::Table(n)), toml::Value::Table(o)) = (new.get_mut(k), v) {
                    merge_unknown(n, o, known_child);
                } else if let toml::Value::Table(o) = v {
                    let mut n = toml::Table::new();
                    merge_unknown(&mut n, o, known_child);
                    if !n.is_empty() {
                        new.insert(k.clone(), toml::Value::Table(n));
                    }
                }
            }
            Some(_) => {}
        }
    }
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().context("path has no parent directory")?;
    fs::create_dir_all(dir)?;
    let file_name = path
        .file_name()
        .context("path has no file name")?
        .to_string_lossy();
    let tmp = dir.join(format!(".{file_name}.{}.tmp", std::process::id()));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })?;
    // Settings and GitAssist's own records are readable by the owner only.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    }
    Ok(())
}
