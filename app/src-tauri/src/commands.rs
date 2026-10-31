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
use gitassist_gateway::publish::{self, PublishReport, Selection};
use gitassist_gateway::targets::{self, RepoTargets};
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
pub async fn uninstall_step(step: String) -> CmdResult<()> {
    blocking(move || {
        let (paths, env) = ctx()?;
        let step = Step::parse(&step).ok_or("unknown step")?;
        ops::uninstall_step(step, &paths, &env).map_err(err)
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
    /// GitHub login for avatar (`https://github.com/{login}.png`), if host is github.com.
    avatar_login: Option<String>,
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
            let avatar_login = primary.as_ref().and_then(|r| {
                if r.host == "github.com" {
                    Some(r.owner.clone())
                } else {
                    None
                }
            });
            RepoView {
                name: Path::new(&record.path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                host: primary.as_ref().map(|r| r.host.clone()),
                slug: primary.as_ref().map(|r| r.slug()),
                avatar_login,
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

#[tauri::command]
pub async fn targets_get(path: String) -> CmdResult<RepoTargets> {
    blocking(move || {
        let (paths, _) = ctx()?;
        let repo = expand(&path);
        Ok(targets::load(&paths, &repo))
    })
    .await
}

#[tauri::command]
pub async fn targets_save(path: String, data: RepoTargets) -> CmdResult<Vec<String>> {
    blocking(move || {
        let (paths, _) = ctx()?;
        let repo = expand(&path);
        targets::save(&paths, &repo, &data).map_err(err)?;
        let notes = targets::ensure_remotes(&repo, &data).map_err(err)?;
        // Refresh Cursor/repo rule so agents see the new targets.
        if let Ok(cfg) = Config::load(&paths.config_file) {
            let extra = targets::render_targets_section(&data);
            let file = repo.join(gitassist_core::instructions::REPO_RULE_PATH);
            if cfg.repo_rule_files {
                let body = gitassist_core::instructions::render_repo_rule_with_extra(&cfg, &extra);
                if let Some(d) = file.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                let _ = std::fs::write(&file, body);
            }
        }
        Ok(notes)
    })
    .await
}

#[tauri::command]
pub async fn targets_push(path: String, which: String) -> CmdResult<PublishReport> {
    blocking(move || {
        let (paths, _) = ctx()?;
        let repo = expand(&path);
        let cfg = Config::load(&paths.config_file).map_err(err)?;
        let report = if which.trim() == "--all" || which.trim() == "all" {
            publish::publish(&paths, &repo, &cfg, Selection::All).map_err(err)?
        } else {
            publish::publish(&paths, &repo, &cfg, Selection::Label(which.trim())).map_err(err)?
        };
        Ok(report)
    })
    .await
}

#[tauri::command]
pub async fn repo_work(path: String) -> CmdResult<gitassist_gateway::work::WorkStatus> {
    blocking(move || {
        let repo = expand(&path);
        gitassist_gateway::work::status(&repo).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn repos_work_summaries(paths_in: Vec<String>) -> CmdResult<Vec<(String, gitassist_gateway::work::WorkStatus)>> {
    blocking(move || {
        let mut out = Vec::with_capacity(paths_in.len());
        for p in paths_in {
            let repo = expand(&p);
            let mut w = gitassist_gateway::work::summary(&repo);
            w.files.clear();
            out.push((p, w));
        }
        Ok(out)
    })
    .await
}

#[tauri::command]
pub async fn overrides_get(path: String) -> CmdResult<gitassist_gateway::overrides::RepoOverrides> {
    blocking(move || {
        let (paths, _) = ctx()?;
        Ok(gitassist_gateway::overrides::load(&paths, &expand(&path)))
    })
    .await
}

#[tauri::command]
pub async fn overrides_save(
    path: String,
    data: gitassist_gateway::overrides::RepoOverrides,
) -> CmdResult<()> {
    blocking(move || {
        let (paths, _) = ctx()?;
        gitassist_gateway::overrides::save(&paths, &expand(&path), &data).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn overrides_clear_tab(path: String, tab: String) -> CmdResult<()> {
    blocking(move || {
        let (paths, _) = ctx()?;
        gitassist_gateway::overrides::clear_tab(&paths, &expand(&path), &tab).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn jobs_list() -> CmdResult<Vec<gitassist_gateway::jobs::Job>> {
    blocking(|| {
        let (paths, _) = ctx()?;
        Ok(gitassist_gateway::jobs::list(&paths))
    })
    .await
}

#[tauri::command]
pub async fn jobs_cancel(id: String) -> CmdResult<gitassist_gateway::jobs::Job> {
    blocking(move || {
        let (paths, _) = ctx()?;
        gitassist_gateway::jobs::request_cancel(&paths, &id).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn jobs_resume(id: String) -> CmdResult<gitassist_gateway::jobs::Job> {
    let job = blocking(move || {
        let (paths, _) = ctx()?;
        let job = gitassist_gateway::jobs::get(&paths, &id).ok_or("job not found")?;
        if job.kind != gitassist_gateway::jobs::JobKind::Publish {
            return Err("only publish jobs can resume here".into());
        }
        Ok(job)
    })
    .await?;
    let jid = job.id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Ok((paths, _)) = ctx() else { return };
        let Ok(cfg) = Config::load(&paths.config_file) else { return };
        let _ = gitassist_gateway::jobs::resume(&paths, &cfg, &jid);
    });
    Ok(job)
}

#[tauri::command]
pub async fn job_start_publish(path: String, which: String) -> CmdResult<gitassist_gateway::jobs::Job> {
    let job = blocking(move || {
        let (paths, _) = ctx()?;
        let repo = expand(&path);
        gitassist_gateway::jobs::start_publish(&paths, &repo, &which).map_err(err)
    })
    .await?;
    let jid = job.id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Ok((paths, _)) = ctx() else { return };
        let Ok(cfg) = Config::load(&paths.config_file) else { return };
        let _ = gitassist_gateway::jobs::run_publish(&paths, &cfg, &jid);
    });
    Ok(job)
}

/// Fix-all as a background job. Steps come from the UI health snapshot.
#[derive(Clone, serde::Deserialize)]
pub struct FixAllPlan {
    pub connect: Vec<String>,
    pub adapters: Vec<String>,
    pub installs: Vec<String>,
}

fn step_done(paths: &Paths, job_id: &str, step_id: &str) -> bool {
    gitassist_gateway::jobs::get(paths, job_id)
        .and_then(|j| j.steps.into_iter().find(|s| s.id == step_id))
        .is_some_and(|s| s.status == gitassist_gateway::jobs::StepStatus::Done)
}

#[tauri::command]
pub async fn job_start_fix_all(plan: FixAllPlan) -> CmdResult<gitassist_gateway::jobs::Job> {
    let plan_run = plan.clone();
    let job = blocking(move || {
        let (paths, _) = ctx()?;
        let mut steps = Vec::new();
        for p in &plan.connect {
            let name = Path::new(p)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.clone());
            steps.push((format!("connect:{p}"), format!("Connect {name}")));
        }
        for id in &plan.adapters {
            steps.push((format!("adapter:{id}"), format!("Apply {id}")));
        }
        for id in &plan.installs {
            steps.push((format!("install:{id}"), format!("Fix {id}")));
        }
        steps.push(("repair".into(), "Repair".into()));
        gitassist_gateway::jobs::start_fix_all(&paths, steps).map_err(err)
    })
    .await?;

    let jid = job.id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Ok((paths, env)) = ctx() else { return };
        let cli = bundled_cli().ok();
        let app = app_exe();
        let dummy = PathBuf::from("gitassist");
        let cli_path = cli.as_deref().unwrap_or(dummy.as_path());

        for p in &plan_run.connect {
            if gitassist_gateway::jobs::is_cancelled(&jid) {
                let _ = gitassist_gateway::jobs::request_cancel(&paths, &jid);
                return;
            }
            let sid = format!("connect:{p}");
            if step_done(&paths, &jid, &sid) {
                continue;
            }
            let _ = gitassist_gateway::jobs::set_step(
                &paths,
                &jid,
                &sid,
                gitassist_gateway::jobs::StepStatus::Running,
                "connecting…",
            );
            match repos::connect(Path::new(p), &paths) {
                Ok(()) => {
                    let _ = gitassist_gateway::jobs::set_step(
                        &paths,
                        &jid,
                        &sid,
                        gitassist_gateway::jobs::StepStatus::Done,
                        "connected",
                    );
                }
                Err(e) => {
                    let _ = gitassist_gateway::jobs::set_step(
                        &paths,
                        &jid,
                        &sid,
                        gitassist_gateway::jobs::StepStatus::Failed,
                        e.to_string(),
                    );
                }
            }
        }

        let Ok(cfg) = Config::load(&paths.config_file) else {
            let _ = gitassist_gateway::jobs::finish_job(
                &paths,
                &jid,
                gitassist_gateway::jobs::JobStatus::Failed,
                Some("config load failed".into()),
            );
            return;
        };

        for id in &plan_run.adapters {
            if gitassist_gateway::jobs::is_cancelled(&jid) {
                let _ = gitassist_gateway::jobs::request_cancel(&paths, &jid);
                return;
            }
            let sid = format!("adapter:{id}");
            if step_done(&paths, &jid, &sid) {
                continue;
            }
            let _ = gitassist_gateway::jobs::set_step(
                &paths,
                &jid,
                &sid,
                gitassist_gateway::jobs::StepStatus::Running,
                "applying…",
            );
            match adapters::find(&env, &cfg, id) {
                Some(a) => match adapters::apply(&a, &paths) {
                    Ok(()) => {
                        let _ = gitassist_gateway::jobs::set_step(
                            &paths,
                            &jid,
                            &sid,
                            gitassist_gateway::jobs::StepStatus::Done,
                            "applied",
                        );
                    }
                    Err(e) => {
                        let _ = gitassist_gateway::jobs::set_step(
                            &paths,
                            &jid,
                            &sid,
                            gitassist_gateway::jobs::StepStatus::Failed,
                            e.to_string(),
                        );
                    }
                },
                None => {
                    let _ = gitassist_gateway::jobs::set_step(
                        &paths,
                        &jid,
                        &sid,
                        gitassist_gateway::jobs::StepStatus::Failed,
                        "unknown agent",
                    );
                }
            }
        }

        for id in &plan_run.installs {
            if gitassist_gateway::jobs::is_cancelled(&jid) {
                let _ = gitassist_gateway::jobs::request_cancel(&paths, &jid);
                return;
            }
            let sid = format!("install:{id}");
            if step_done(&paths, &jid, &sid) {
                continue;
            }
            let _ = gitassist_gateway::jobs::set_step(
                &paths,
                &jid,
                &sid,
                gitassist_gateway::jobs::StepStatus::Running,
                "installing…",
            );
            let Some(step) = Step::parse(id) else {
                let _ = gitassist_gateway::jobs::set_step(
                    &paths,
                    &jid,
                    &sid,
                    gitassist_gateway::jobs::StepStatus::Failed,
                    "unknown step",
                );
                continue;
            };
            match ops::install(step, &paths, &env, cli_path, app.as_deref()) {
                Ok(msg) => {
                    let _ = gitassist_gateway::jobs::set_step(
                        &paths,
                        &jid,
                        &sid,
                        gitassist_gateway::jobs::StepStatus::Done,
                        msg,
                    );
                }
                Err(e) => {
                    let _ = gitassist_gateway::jobs::set_step(
                        &paths,
                        &jid,
                        &sid,
                        gitassist_gateway::jobs::StepStatus::Failed,
                        e.to_string(),
                    );
                }
            }
        }

        if gitassist_gateway::jobs::is_cancelled(&jid) {
            let _ = gitassist_gateway::jobs::request_cancel(&paths, &jid);
            return;
        }
        if !step_done(&paths, &jid, "repair") {
            let _ = gitassist_gateway::jobs::set_step(
                &paths,
                &jid,
                "repair",
                gitassist_gateway::jobs::StepStatus::Running,
                "repairing…",
            );
            match ops::repair(&paths, &env, cli_path, app.as_deref()) {
                Ok(notes) => {
                    let _ = gitassist_gateway::jobs::set_step(
                        &paths,
                        &jid,
                        "repair",
                        gitassist_gateway::jobs::StepStatus::Done,
                        notes.join("; "),
                    );
                }
                Err(e) => {
                    let _ = gitassist_gateway::jobs::set_step(
                        &paths,
                        &jid,
                        "repair",
                        gitassist_gateway::jobs::StepStatus::Failed,
                        e.to_string(),
                    );
                    let _ = gitassist_gateway::jobs::finish_job(
                        &paths,
                        &jid,
                        gitassist_gateway::jobs::JobStatus::Failed,
                        Some(e.to_string()),
                    );
                    return;
                }
            }
        }
        let failed = gitassist_gateway::jobs::get(&paths, &jid)
            .map(|j| {
                j.steps
                    .iter()
                    .any(|s| s.status == gitassist_gateway::jobs::StepStatus::Failed)
            })
            .unwrap_or(false);
        let _ = gitassist_gateway::jobs::finish_job(
            &paths,
            &jid,
            if failed {
                gitassist_gateway::jobs::JobStatus::Failed
            } else {
                gitassist_gateway::jobs::JobStatus::Done
            },
            None,
        );
    });
    Ok(job)
}

#[tauri::command]
pub async fn job_start_apply(id: String) -> CmdResult<gitassist_gateway::jobs::Job> {
    let id2 = id.clone();
    let job = blocking(move || {
        let (paths, env) = ctx()?;
        let cfg = Config::load(&paths.config_file).map_err(err)?;
        let a = adapters::find(&env, &cfg, &id).ok_or("unknown agent")?;
        gitassist_gateway::jobs::start_apply_adapter(&paths, &a.id, &a.name).map_err(err)
    })
    .await?;
    let jid = job.id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Ok((paths, env)) = ctx() else { return };
        let Ok(cfg) = Config::load(&paths.config_file) else { return };
        let sid = id2.clone();
        let _ = gitassist_gateway::jobs::set_step(
            &paths,
            &jid,
            &sid,
            gitassist_gateway::jobs::StepStatus::Running,
            "applying…",
        );
        match adapters::find(&env, &cfg, &id2) {
            Some(a) => match adapters::apply(&a, &paths) {
                Ok(()) => {
                    let _ = gitassist_gateway::jobs::set_step(
                        &paths,
                        &jid,
                        &sid,
                        gitassist_gateway::jobs::StepStatus::Done,
                        "applied",
                    );
                    let _ = gitassist_gateway::jobs::finish_job(
                        &paths,
                        &jid,
                        gitassist_gateway::jobs::JobStatus::Done,
                        None,
                    );
                }
                Err(e) => {
                    let _ = gitassist_gateway::jobs::set_step(
                        &paths,
                        &jid,
                        &sid,
                        gitassist_gateway::jobs::StepStatus::Failed,
                        e.to_string(),
                    );
                    let _ = gitassist_gateway::jobs::finish_job(
                        &paths,
                        &jid,
                        gitassist_gateway::jobs::JobStatus::Failed,
                        Some(e.to_string()),
                    );
                }
            },
            None => {
                let _ = gitassist_gateway::jobs::finish_job(
                    &paths,
                    &jid,
                    gitassist_gateway::jobs::JobStatus::Failed,
                    Some("unknown agent".into()),
                );
            }
        }
    });
    Ok(job)
}
