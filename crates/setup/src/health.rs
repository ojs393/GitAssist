use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use gitassist_core::Config;
use gitassist_gateway::gh::find_real_gh;
use gitassist_gateway::state::State;
use gitassist_gateway::{log, Paths};
use serde::Serialize;

use crate::adapters::{self, Status};
use crate::hooks_install::{hook_script, hooks_active};
use crate::repos::{self, RepoStatus};
use crate::{autostart, shim_install, Env};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State3 {
    On,
    Off,
    Attention,
    NotNeeded,
}

#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub id: String,
    pub label: String,
    pub ok: bool,
    pub state: State3,
    /// One short line for the main screen.
    pub summary: String,
    /// Paths and specifics, shown only behind the info button.
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdapterHealth {
    pub id: String,
    pub name: String,
    pub status: Status,
    pub manual: Vec<String>,
    pub limits: Vec<String>,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub ok: bool,
    pub turned_on: bool,
    pub enabled: bool,
    pub github_user: String,
    pub items: Vec<Item>,
    pub adapters: Vec<AdapterHealth>,
    pub repos: Vec<RepoStatus>,
    pub limits: Vec<String>,
}

fn item(
    id: &str,
    label: &str,
    state: State3,
    summary: impl Into<String>,
    details: Vec<String>,
) -> Item {
    Item {
        id: id.into(),
        label: label.into(),
        ok: matches!(state, State3::On | State3::NotNeeded),
        state,
        summary: summary.into(),
        details,
    }
}

fn hook_scripts_current(paths: &Paths) -> bool {
    gitassist_gateway::ACTIVE_HOOKS.iter().all(|h| {
        fs::read_to_string(paths.hooks_dir.join(h))
            .is_ok_and(|t| t == hook_script(&paths.cli_exe(), h))
    })
}

/// Repos that use their own hooks path (Husky, lefthook, …) and therefore need
/// an explicit connect before GitAssist can see commits. Shared by the status
/// report and by `repair` so "Fix all" actually connects them.
pub fn hooked_repos(paths: &Paths, env: &Env, cfg: &Config, state: &State) -> Vec<RepoStatus> {
    let mut repo_paths: BTreeSet<String> = cfg.watched_repos.iter().cloned().collect();
    repo_paths.extend(state.connected_repos.iter().cloned());
    repo_paths.extend(
        log::read_recent(&paths.log_file, 500)
            .into_iter()
            .map(|e| e.repo)
            .filter(|r| !r.is_empty() && !repos::in_protected_folder(Path::new(r), &env.home)),
    );
    // Repos with their own hooks never run GitAssist's, so they come from the repo list.
    repo_paths.extend(
        gitassist_gateway::registry::load(paths)
            .into_values()
            .filter(|r| {
                r.local_hooks_path.is_some()
                    && !repos::in_protected_folder(Path::new(&r.path), &env.home)
            })
            .map(|r| r.path),
    );
    repo_paths
        .iter()
        .filter(|p| Path::new(p).join(".git").exists())
        .map(|p| repos::status(Path::new(p)))
        .filter(|r| r.local_hooks_path.is_some())
        .collect()
}

pub fn check(paths: &Paths, env: &Env) -> Report {
    check_with(paths, env, true)
}

/// `probe_shells` runs real shells to see which `gh` they pick up (slower; the
/// tray monitor skips it).
pub fn check_with(paths: &Paths, env: &Env, probe_shells: bool) -> Report {
    let state = State::load(&paths.state_file);
    let mut items = Vec::new();

    let (cfg, cfg_err) = match Config::load(&paths.config_file) {
        Ok(c) => (c, None),
        Err(e) => (Config::default(), Some(format!("{e:#}"))),
    };
    if let Some(e) = &cfg_err {
        items.push(item(
            "config",
            "Settings",
            State3::Attention,
            "Settings file can't be read; commits pass through unchecked",
            vec![e.clone(), paths.config_file.display().to_string()],
        ));
    }

    let installed = hooks_active(paths) && hook_scripts_current(paths) && paths.cli_exe().is_file();
    items.push(item(
        "hooks",
        "Commits & pushes",
        if installed {
            State3::On
        } else if state.hooks_installed || state.turned_on {
            State3::Attention
        } else {
            State3::Off
        },
        if installed {
            "Every commit, merge and push is checked"
        } else if state.hooks_installed || state.turned_on {
            "Another tool changed git's hooks setting"
        } else {
            "Not set up"
        },
        vec![
            format!("Global hooks folder: {}", paths.hooks_dir.display()),
            format!("Command: {}", paths.cli_exe().display()),
            "Your repos' own hooks still run after GitAssist.".into(),
        ],
    ));

    let real_gh = find_real_gh(Some(&paths.bin_dir));
    let coverage = if probe_shells && real_gh.is_some() && paths.gh_shim().exists() {
        shim_install::shell_coverage(paths)
    } else {
        Vec::new()
    };
    let shim_files_ok = paths.gh_shim().exists() && shim_install::profiles_ok(paths);
    let missing_shells: Vec<&str> = coverage
        .iter()
        .filter(|(_, ok)| *ok == Some(false))
        .map(|(s, _)| s.as_str())
        .collect();
    let mut shim_details: Vec<String> = coverage
        .iter()
        .map(|(s, ok)| {
            let what = match ok {
                Some(true) => "uses GitAssist",
                Some(false) => "goes straight to gh",
                None => "too slow to check right now",
            };
            format!("{s}: {what}")
        })
        .collect();
    shim_details.extend(state.profiles.iter().map(|p| format!("PATH set in {p}")));
    let (shim_state, shim_summary) = match (&real_gh, shim_files_ok) {
        (None, _) => (State3::NotNeeded, "GitHub CLI not installed".to_string()),
        (Some(_), true) if missing_shells.is_empty() => (
            State3::On,
            "PRs, comments, reviews and issues are checked".to_string(),
        ),
        (Some(_), true) => (
            State3::On,
            format!(
                "Checked in your shells, except {}",
                missing_shells.join(", ")
            ),
        ),
        (Some(_), false) if state.shim_installed || state.turned_on => (
            State3::Attention,
            "A shell profile lost GitAssist's PATH line".to_string(),
        ),
        (Some(_), false) => (State3::Off, "Not set up".to_string()),
    };
    if let Some(gh) = &real_gh {
        shim_details.push(format!("Real gh: {}", gh.display()));
    }
    items.push(item(
        "shim",
        "GitHub CLI",
        shim_state,
        shim_summary,
        shim_details,
    ));

    let auto = autostart::is_enabled(env);
    items.push(item(
        "autostart",
        "Start at login",
        if auto { State3::On } else { State3::Off },
        if auto {
            "Runs quietly in the menu bar"
        } else {
            "Off"
        },
        vec!["Checks run inside git and gh, so they work even when the app is closed.".into()],
    ));

    items.push(item(
        "repair",
        "Fixes itself",
        if cfg.auto_repair && state.turned_on {
            State3::On
        } else {
            State3::Off
        },
        if cfg.auto_repair && state.turned_on {
            "Fixes anything another tool undoes"
        } else {
            "Off"
        },
        vec!["Re-checks every 15 minutes and at login.".into()],
    ));

    let adapters: Vec<AdapterHealth> = adapters::all(env, &cfg)
        .into_iter()
        .map(|a| AdapterHealth {
            id: a.id.into(),
            name: a.name.into(),
            status: adapters::status(&a, paths),
            manual: a.manual.clone(),
            limits: a.limits.clone(),
            files: a
                .changes
                .iter()
                .map(|c| c.path().display().to_string())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        })
        .collect();

    let repos = hooked_repos(paths, env, &cfg, &state);

    let ok = items
        .iter()
        .filter(|i| i.id != "autostart" && i.id != "repair")
        .all(|i| i.ok)
        && adapters
            .iter()
            .all(|a| matches!(a.status, Status::Current | Status::NotDetected))
        && repos.iter().all(|r| r.connected);

    Report {
        ok,
        turned_on: state.turned_on || state.hooks_installed,
        enabled: cfg.enabled,
        github_user: cfg.ignore.github_user.clone(),
        items,
        adapters,
        repos,
        limits: vec![
            "Cloud agents (Copilot coding agent, Cursor cloud agents, Codex cloud) run on remote servers; nothing on this machine can intercept them.".into(),
            "Tools that call the GitHub API directly instead of `gh` (GitHub MCP servers, IDE pull-request panels) get your rules as instructions only.".into(),
        ],
    }
}
