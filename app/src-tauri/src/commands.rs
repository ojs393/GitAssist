//! Every command is `async` so Tauri runs it off the UI thread; health checks
//! spawn git and shells and must never freeze the window.

use std::path::{Path, PathBuf};

use gitassist_core::instructions::render_agent_instructions;
use gitassist_core::scope::{
    parse_remote, patterns_for, scope_for, scope_label, Matcher, Visibility,
};
use gitassist_core::{effective, process, Config, Input, Outcome, Tab, TabConfig};
use gitassist_gateway::log::{self, Entry};
use gitassist_gateway::paths::exe_name;
use gitassist_gateway::Paths;
use gitassist_setup::adapters::{self, Adapter};
use gitassist_setup::health::{self, Report};
use gitassist_setup::ops::{self, Step, StepResult};
use gitassist_setup::repos::{self, RepoStatus};
use gitassist_setup::Env;
use serde::Serialize;

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn ctx() -> CmdResult<(Paths, Env)> {
    Ok((Paths::resolve().map_err(err)?, Env::detect().map_err(err)?))
}

/// The `gitassist` command ships next to the app executable (Tauri sidecar).
pub fn bundled_cli() -> CmdResult<PathBuf> {
    let exe = std::env::current_exe().map_err(err)?;
    let cli = exe
        .parent()
        .map(|d| d.join(exe_name("gitassist")))
        .ok_or("app has no parent directory")?;
    if cli.is_file() {
        Ok(cli)
    } else {
        Err("the bundled gitassist command is missing; reinstall GitAssist".into())
    }
}

pub fn app_exe() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(err)?
}

#[derive(Serialize)]
pub struct TabInfo {
    id: &'static str,
    label: &'static str,
    commit_message: bool,
    has_title: bool,
    defaults: TabConfig,
}

#[derive(Serialize)]
pub struct AppState {
    config: Config,
    tabs: Vec<TabInfo>,
}

#[tauri::command]
pub async fn get_state() -> CmdResult<AppState> {
    blocking(|| {
        let (paths, env) = ctx()?;
        // Opening the window also sets up agents installed since the last check.
        let _ = ops::refresh(&paths, &env);
        let config = Config::load(&paths.config_file).map_err(err)?;
        let tabs = Tab::ALL
            .into_iter()
            .map(|t| TabInfo {
                id: t.as_str(),
                label: t.label(),
                commit_message: t.is_commit_message(),
                has_title: t.has_title(),
                defaults: gitassist_core::effective::builtin(t),
            })
            .collect();
        Ok(AppState { config, tabs })
    })
    .await
}

#[tauri::command]
pub async fn save_config(config: Config) -> CmdResult<()> {
    blocking(move || {
        let (paths, env) = ctx()?;
        config.save_atomic(&paths.config_file).map_err(err)?;
        ops::refresh(&paths, &env).map_err(err)
    })
    .await
}

/// Uses the unsaved settings from the window, so the preview updates as you type.
#[tauri::command]
pub async fn preview(
    config: Config,
    tab: String,
    title: Option<String>,
    body: String,
) -> CmdResult<Outcome> {
    let tab = Tab::parse(&tab).ok_or("unknown tab")?;
    let rules = effective(&config, tab, None);
    let title = if tab.is_commit_message() || !tab.has_title() {
        None
    } else {
        title
    };
    Ok(process(
        &Input {
            title: title.as_deref(),
            body: Some(&body),
            ..Default::default()
        },
        &rules,
    ))
}

#[derive(Serialize)]
pub struct IgnorePreview {
    scope: String,
    patterns: Vec<String>,
    files: Vec<(String, bool)>,
}

#[tauri::command]
pub async fn ignore_preview(
    config: Config,
    remote: String,
    visibility: String,
    files: Vec<String>,
) -> CmdResult<IgnorePreview> {
    let remote = parse_remote(&remote);
    let vis = match visibility.as_str() {
        "public" => Visibility::Public,
        "private" => Visibility::Private,
        _ => Visibility::Unknown,
    };
    let scope = scope_for(remote.as_ref(), &config.ignore.github_user, vis);
    let patterns = patterns_for(&config.ignore, &scope);
    let m = Matcher::new(&patterns);
    Ok(IgnorePreview {
        scope: scope_label(&scope),
        files: files
            .into_iter()
            .filter(|f| !f.trim().is_empty())
            .map(|f| (f.clone(), m.is_ignored(&f)))
            .collect(),
        patterns,
    })
}

#[tauri::command]
pub async fn detect_github_user() -> CmdResult<Option<String>> {
    blocking(|| {
        let (paths, _) = ctx()?;
        Ok(ops::detect_github_user(&paths))
    })
    .await
}

#[tauri::command]
pub async fn instructions(config: Config) -> String {
    render_agent_instructions(&config)
}

#[tauri::command]
pub async fn health(probe: Option<bool>) -> CmdResult<Report> {
    blocking(move || {
        let (paths, env) = ctx()?;
        Ok(health::check_with(&paths, &env, probe.unwrap_or(true)))
    })
    .await
}

#[tauri::command]
pub async fn turn_on() -> CmdResult<Vec<StepResult>> {
    blocking(|| {
        let (paths, env) = ctx()?;
        Ok(ops::turn_on(
            &paths,
            &env,
            &bundled_cli()?,
            app_exe().as_deref(),
        ))
    })
    .await
}

#[tauri::command]
pub async fn install_step(step: String) -> CmdResult<String> {
    blocking(move || {
        let (paths, env) = ctx()?;
        let step = Step::parse(&step).ok_or("unknown step")?;
        ops::install(step, &paths, &env, &bundled_cli()?, app_exe().as_deref()).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn disable_autostart() -> CmdResult<()> {
    blocking(|| {
        let (paths, env) = ctx()?;
        gitassist_setup::autostart::disable(&env).map_err(err)?;
        let mut s = gitassist_gateway::state::State::load(&paths.state_file);
        s.autostart = false;
        s.save(&paths.state_file).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn repair() -> CmdResult<Vec<String>> {
    blocking(|| {
        let (paths, env) = ctx()?;
        ops::repair(&paths, &env, &bundled_cli()?, app_exe().as_deref()).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn uninstall_all() -> CmdResult<()> {
    blocking(|| {
        let (paths, env) = ctx()?;
        ops::uninstall_all(&paths, &env).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn adapter_plan(id: String) -> CmdResult<Adapter> {
    blocking(move || {
        let (paths, env) = ctx()?;
        let cfg = Config::load(&paths.config_file).map_err(err)?;
        adapters::find(&env, &cfg, &id).ok_or_else(|| "unknown agent".into())
    })
    .await
}

/// Applies and then re-reads the status, so the button reflects what is really on disk.
#[tauri::command]
pub async fn apply_adapter(id: String) -> CmdResult<adapters::Status> {
    blocking(move || {
        let (paths, env) = ctx()?;
        let cfg = Config::load(&paths.config_file).map_err(err)?;
        let a = adapters::find(&env, &cfg, &id).ok_or("unknown agent")?;
        adapters::apply(&a, &paths).map_err(err)?;
        let status = adapters::status(&a, &paths);
        if status == adapters::Status::Current {
            Ok(status)
        } else {
            Err(format!(
                "{} settings did not stick; check the files are writable",
                a.name
            ))
        }
    })
    .await
}

#[tauri::command]
pub async fn remove_adapter(id: String) -> CmdResult<adapters::Status> {
    blocking(move || {
        let (paths, env) = ctx()?;
        let cfg = Config::load(&paths.config_file).map_err(err)?;
        let a = adapters::find(&env, &cfg, &id).ok_or("unknown agent")?;
        adapters::remove(&a, &paths).map_err(err)?;
        Ok(adapters::status(&a, &paths))
    })
    .await
}

#[tauri::command]
pub async fn activity(limit: usize) -> CmdResult<Vec<Entry>> {
    blocking(move || {
        let (paths, _) = ctx()?;
        Ok(log::read_recent(&paths.log_file, limit.min(2000)))
    })
    .await
}

#[tauri::command]
pub async fn clear_activity() -> CmdResult<()> {
    blocking(|| {
        let (paths, _) = ctx()?;
        for f in [
            paths.log_file.clone(),
            paths.log_file.with_extension("log.1"),
            paths.log_file.with_extension("log.2"),
        ] {
            let _ = std::fs::remove_file(f);
        }
        Ok(())
    })
    .await
}

fn expand(p: &str) -> PathBuf {
    let p = p.trim();
    match (p.strip_prefix("~/"), directories::BaseDirs::new()) {
        (Some(rest), Some(b)) => b.home_dir().join(rest),
        _ => PathBuf::from(p),
    }
}

#[tauri::command]
pub async fn scan_repos(root: String) -> CmdResult<Vec<RepoStatus>> {
    blocking(move || {
        let root = expand(&root);
        if !root.is_dir() {
            return Err("That folder doesn't exist".into());
        }
        Ok(repos::scan(&root, 4)
            .iter()
            .map(|r| repos::status(r))
            .filter(|r| r.local_hooks_path.is_some())
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn connect_repo(path: String) -> CmdResult<()> {
    blocking(move || {
        let (paths, _) = ctx()?;
        repos::connect(Path::new(&path), &paths).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn disconnect_repo(path: String) -> CmdResult<()> {
    blocking(move || {
        let (paths, _) = ctx()?;
        repos::disconnect(Path::new(&path), &paths).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn storage() -> CmdResult<gitassist_setup::storage::Usage> {
    blocking(|| {
        let (paths, _) = ctx()?;
        Ok(gitassist_setup::storage::usage(&paths))
    })
    .await
}

#[tauri::command]
pub async fn clear_storage(what: String) -> CmdResult<gitassist_setup::storage::Usage> {
    blocking(move || {
        let (paths, _) = ctx()?;
        gitassist_setup::storage::clear(&paths, &what).map_err(err)?;
        Ok(gitassist_setup::storage::usage(&paths))
    })
    .await
}

#[derive(Serialize)]
pub struct RepoView {
    #[serde(flatten)]
    record: gitassist_gateway::registry::RepoRecord,
    name: String,
    host: Option<String>,
    slug: Option<String>,
    owner: gitassist_core::scope::Owner,
    visibility: Visibility,
    connected: bool,
}

fn repo_views(paths: &Paths, env: &Env) -> Vec<RepoView> {
    let cfg = Config::load(&paths.config_file).unwrap_or_default();
    let mut map = gitassist_gateway::registry::load(paths);
    gitassist_gateway::registry::prune(&mut map, &env.home, repos::in_protected_folder);
    let mut out: Vec<RepoView> = map
        .into_values()
        .map(|record| {
            let primary = record
                .remotes
                .iter()
                .find(|r| r.name == "origin")
                .or(record.remotes.first())
                .and_then(|r| parse_remote(&r.url));
            let vis = primary
                .as_ref()
                .map(|r| gitassist_gateway::repo::cached_visibility(paths, r))
                .unwrap_or(Visibility::Unknown);
            let scope = scope_for(primary.as_ref(), &cfg.ignore.github_user, vis);
            let connected = record.local_hooks_path.is_some()
                && !repos::in_protected_folder(Path::new(&record.path), &env.home)
                && repos::status(Path::new(&record.path)).connected;
            RepoView {
                name: Path::new(&record.path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                host: primary.as_ref().map(|r| r.host.clone()),
                slug: primary.as_ref().map(|r| r.slug()),
                owner: scope.owner,
                visibility: scope.visibility,
                connected,
                record,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        b.record
            .last_commit
            .max(b.record.last_push)
            .cmp(&a.record.last_commit.max(a.record.last_push))
    });
    out
}

#[tauri::command]
pub async fn repos_list() -> CmdResult<Vec<RepoView>> {
    blocking(|| {
        let (paths, env) = ctx()?;
        Ok(repo_views(&paths, &env))
    })
    .await
}

/// Finds repos in `root` (or your home folder's usual places) and returns the full list.
#[tauri::command]
pub async fn repos_discover(root: Option<String>) -> CmdResult<Vec<RepoView>> {
    blocking(move || {
        let (paths, env) = ctx()?;
        let roots = match root.as_deref().map(str::trim).filter(|r| !r.is_empty()) {
            Some(r) => {
                let p = expand(r);
                if !p.is_dir() {
                    return Err("That folder doesn't exist".into());
                }
                vec![p]
            }
            None => repos::default_roots(&env.home),
        };
        repos::discover(&paths, &roots, 4);
        Ok(repo_views(&paths, &env))
    })
    .await
}
