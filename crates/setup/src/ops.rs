use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::Result;
use gitassist_core::Config;
use gitassist_gateway::gh::find_real_gh;
use gitassist_gateway::log::{self, Entry};
use gitassist_gateway::repo::output_with_timeout;
use gitassist_gateway::state::State;
use gitassist_gateway::Paths;
use serde::Serialize;

use crate::adapters::{self, Status};
use crate::{autostart, health, hooks_install, repos, shim_install, Env};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Hooks,
    Shim,
    Agents,
    Autostart,
}

impl Step {
    pub fn parse(s: &str) -> Option<Step> {
        match s {
            "hooks" => Some(Step::Hooks),
            "shim" => Some(Step::Shim),
            "agents" => Some(Step::Agents),
            "autostart" => Some(Step::Autostart),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Step::Hooks => "Git hooks",
            Step::Shim => "GitHub CLI gateway",
            Step::Agents => "Agents",
            Step::Autostart => "Start at login",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StepResult {
    pub step: Step,
    pub ok: bool,
    pub message: String,
}

fn log_setup(paths: &Paths, outcome: &str, msg: &str) {
    let mut e = Entry::new("setup", outcome);
    e.messages = vec![msg.to_string()];
    log::append(&paths.log_file, &e);
}

fn verify(step: Step, paths: &Paths, env: &Env) -> bool {
    match step {
        Step::Hooks => hooks_install::hooks_active(paths),
        Step::Shim => {
            find_real_gh(Some(&paths.bin_dir)).is_none() || shim_install::profiles_ok(paths)
        }
        Step::Agents => {
            let cfg = Config::load(&paths.config_file).unwrap_or_default();
            adapters::all(env, &cfg).iter().all(|a| {
                matches!(
                    adapters::status(a, paths),
                    Status::Current | Status::NotDetected
                )
            })
        }
        Step::Autostart => autostart::is_enabled(env),
    }
}

fn attempt(
    step: Step,
    paths: &Paths,
    env: &Env,
    cli: &Path,
    app: Option<&Path>,
    try_no: u32,
) -> Result<String> {
    // Later attempts start from a clean copy of the binary and scripts.
    if try_no > 1 {
        let _ = std::fs::remove_file(paths.cli_exe());
    }
    hooks_install::install_cli(paths, cli)?;
    match step {
        Step::Hooks => {
            hooks_install::install_hooks(paths)?;
            Ok("Every commit and push now goes through GitAssist.".into())
        }
        Step::Shim => {
            if find_real_gh(Some(&paths.bin_dir)).is_none() {
                return Ok("GitHub CLI isn't installed, so there is nothing to gate yet.".into());
            }
            if try_no > 1 {
                let _ = shim_install::uninstall_shim(paths);
            }
            let r = shim_install::install_shim(paths, env)?;
            anyhow::ensure!(!r.edited.is_empty(), "no shell profile could be updated");
            Ok("`gh` pull request and issue commands now go through GitAssist.".into())
        }
        Step::Agents => {
            let cfg = Config::load(&paths.config_file)?;
            let mut done = Vec::new();
            let mut failed = Vec::new();
            for a in adapters::all(env, &cfg).into_iter().filter(|a| a.detected) {
                match adapters::apply(&a, paths) {
                    Ok(()) => done.push(a.name),
                    Err(e) => failed.push(format!("{}: {e:#}", a.name)),
                }
            }
            anyhow::ensure!(failed.is_empty(), "{}", failed.join("; "));
            Ok(if done.is_empty() {
                "No coding agents found yet.".into()
            } else {
                format!("Set up {}.", done.join(", "))
            })
        }
        Step::Autostart => {
            let app =
                app.ok_or_else(|| anyhow::anyhow!("start at login needs the GitAssist app"))?;
            if try_no > 1 {
                let _ = autostart::disable(env);
            }
            autostart::enable(env, app)?;
            let mut s = State::load(&paths.state_file);
            s.autostart = true;
            s.save(&paths.state_file)?;
            Ok("GitAssist starts when you log in.".into())
        }
    }
}

/// Installs one step, checks it took effect, and retries from scratch (up to
/// three times) if it didn't.
pub fn install(
    step: Step,
    paths: &Paths,
    env: &Env,
    cli: &Path,
    app: Option<&Path>,
) -> Result<String> {
    let mut last_err = None;
    for try_no in 1..=3 {
        match attempt(step, paths, env, cli, app, try_no) {
            Ok(msg) if verify(step, paths, env) => {
                log_setup(paths, "fixed", &format!("{}: {msg}", step.label()));
                return Ok(msg);
            }
            Ok(_) => last_err = Some(anyhow::anyhow!("{} did not take effect", step.label())),
            Err(e) => last_err = Some(e),
        }
        std::thread::sleep(Duration::from_millis(250 * u64::from(try_no)));
    }
    let err = last_err.unwrap_or_else(|| anyhow::anyhow!("unknown error"));
    log_setup(paths, "error", &format!("{}: {err:#}", step.label()));
    Err(err)
}

/// The GitHub account the user's own `gh` is signed in as, if any.
pub fn detect_github_user(paths: &Paths) -> Option<String> {
    let gh = find_real_gh(Some(&paths.bin_dir))?;
    let mut cmd = Command::new(gh);
    cmd.args(["api", "user", "--jq", ".login"])
        .env("GH_PROMPT_DISABLED", "1");
    let out = output_with_timeout(cmd, Duration::from_secs(8))?;
    let login = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !login.is_empty() && !login.contains(char::is_whitespace))
        .then_some(login)
}

/// One switch: installs everything, configures every agent found, and keeps
/// it that way afterwards (see [`heal`]).
pub fn turn_on(paths: &Paths, env: &Env, cli: &Path, app: Option<&Path>) -> Vec<StepResult> {
    let mut cfg = Config::load(&paths.config_file).unwrap_or_default();
    cfg.enabled = true;
    if cfg.ignore.github_user.trim().is_empty() {
        if let Some(user) = detect_github_user(paths) {
            cfg.ignore.github_user = user;
        }
    }
    let _ = cfg.save_atomic(&paths.config_file);

    let mut state = State::load(&paths.state_file);
    state.turned_on = true;
    let _ = state.save(&paths.state_file);

    let mut steps = vec![Step::Hooks, Step::Shim, Step::Agents];
    if app.is_some() {
        steps.push(Step::Autostart);
    }
    steps
        .into_iter()
        .map(|step| match install(step, paths, env, cli, app) {
            Ok(message) => StepResult {
                step,
                ok: true,
                message,
            },
            Err(e) => StepResult {
                step,
                ok: false,
                message: format!("{e:#}"),
            },
        })
        .collect()
}

/// Rewrites everything derived from settings: agent instructions (including
/// agents installed since), the global ignore file.
pub fn refresh(paths: &Paths, env: &Env) -> Result<()> {
    let cfg = Config::load(&paths.config_file)?;
    let state = State::load(&paths.state_file);
    if state.hooks_installed {
        hooks_install::write_global_ignore(paths, &cfg)?;
    }
    for a in adapters::all(env, &cfg) {
        let configured = state.adapter_previous.contains_key(a.id);
        if a.detected
            && (configured || state.turned_on)
            && adapters::status(&a, paths) != Status::Current
        {
            adapters::apply(&a, paths)?;
        }
    }
    Ok(())
}

/// Backwards-compatible name used by the settings window after saving.
pub fn refresh_agents(paths: &Paths, env: &Env) -> Result<()> {
    refresh(paths, env)
}

/// Makes GitAssist's own records readable by the user only, including files
/// written by older versions.
pub fn lock_private_files(paths: &Paths) {
    use gitassist_gateway::secure;
    secure::lock_dir(&paths.data_dir);
    if let Some(dir) = paths.config_file.parent() {
        secure::lock_dir(dir);
    }
    for f in [
        paths.config_file.clone(),
        paths.state_file.clone(),
        paths.log_file.clone(),
        paths.data_dir.join("repos.json"),
        paths.data_dir.join("repos-index.json"),
    ] {
        if f.is_file() {
            secure::lock_file(&f);
        }
    }
}

/// Reinstalls whatever was installed before and has drifted.
pub fn repair(paths: &Paths, env: &Env, cli: &Path, app: Option<&Path>) -> Result<Vec<String>> {
    lock_private_files(paths);
    let state = State::load(&paths.state_file);
    let mut done = Vec::new();
    if (state.hooks_installed || state.turned_on) && !verify(Step::Hooks, paths, env) {
        install(Step::Hooks, paths, env, cli, app)?;
        done.push("git hooks".to_string());
    } else if state.hooks_installed {
        hooks_install::install_cli(paths, cli)?;
        hooks_install::write_hook_scripts(paths)?;
    }
    if (state.shim_installed || state.turned_on) && !verify(Step::Shim, paths, env) {
        install(Step::Shim, paths, env, cli, app)?;
        done.push("GitHub CLI gateway".into());
    }
    refresh(paths, env)?;
    if state.autostart
        && (!autostart::is_enabled(env) || app.is_some_and(|a| !autostart::is_current(env, a)))
    {
        if let Some(app) = app {
            install(Step::Autostart, paths, env, cli, Some(app))?;
            done.push("start at login".into());
        }
    }
    if state.autostart {
        if let Some(app) = app {
            autostart::ensure_running(env, app);
        }
    }
    // Connect every disconnected hooked repo the status screen would show —
    // not only ones we already recorded. Otherwise "Fix all" spins green while
    // the Overview still counts those repos as problems.
    let cfg = Config::load(&paths.config_file).unwrap_or_default();
    let state = State::load(&paths.state_file);
    for r in health::hooked_repos(paths, env, &cfg, &state) {
        if r.connected {
            continue;
        }
        let p = Path::new(&r.path);
        if !p.exists() {
            continue;
        }
        if repos::connect(p, paths).is_ok() {
            done.push(format!(
                "repo {}",
                p.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            ));
        }
    }
    // Re-apply any agent that drifted or was never applied while GitAssist is on.
    if state.turned_on {
        for a in adapters::all(env, &cfg).into_iter().filter(|a| a.detected) {
            if !matches!(
                adapters::status(&a, paths),
                Status::Current | Status::NotDetected
            ) && adapters::apply(&a, paths).is_ok()
            {
                done.push(format!("{} rules", a.name));
            }
        }
    }
    Ok(done)
}

/// Called by the background monitor: if the user turned GitAssist on and
/// something drifted, put it back. Returns what was repaired.
pub fn heal(paths: &Paths, env: &Env, cli: &Path, app: Option<&Path>) -> Vec<String> {
    lock_private_files(paths);
    let cfg = Config::load(&paths.config_file).unwrap_or_default();
    let state = State::load(&paths.state_file);
    if !cfg.auto_repair || !state.turned_on {
        return Vec::new();
    }
    // Start at login doesn't count towards "protected", but it is still repaired.
    let login_item_lost = state.autostart
        && (!autostart::is_enabled(env) || app.is_some_and(|a| !autostart::is_current(env, a)));
    if health::check_with(paths, env, false).ok && !login_item_lost {
        let _ = refresh(paths, env);
        return Vec::new();
    }
    match repair(paths, env, cli, app) {
        Ok(done) => {
            if !done.is_empty() {
                log_setup(
                    paths,
                    "fixed",
                    &format!("Self-repair restored: {}.", done.join(", ")),
                );
            }
            done
        }
        Err(e) => {
            log_setup(
                paths,
                "error",
                &format!("Self-repair failed, will retry: {e:#}"),
            );
            Vec::new()
        }
    }
}

pub fn uninstall_all(paths: &Paths, env: &Env) -> Result<()> {
    let cfg = Config::load(&paths.config_file).unwrap_or_default();
    for a in adapters::all(env, &cfg) {
        adapters::remove(&a, paths)?;
    }
    for r in State::load(&paths.state_file).connected_repos {
        let _ = repos::disconnect(Path::new(&r), paths);
    }
    let seen: std::collections::BTreeSet<String> = log::read_recent(&paths.log_file, 5000)
        .into_iter()
        .map(|e| e.repo)
        .filter(|r| !r.is_empty())
        .collect();
    for r in seen {
        repos::remove_repo_rule(Path::new(&r));
    }
    shim_install::uninstall_shim(paths)?;
    hooks_install::uninstall_hooks(paths)?;
    autostart::disable(env)?;
    let mut s = State::load(&paths.state_file);
    s.autostart = false;
    s.turned_on = false;
    s.save(&paths.state_file)?;
    log_setup(
        paths,
        "clean",
        "GitAssist removed; previous settings restored.",
    );
    Ok(())
}
