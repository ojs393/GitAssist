use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use gitassist_gateway::state::State;
use gitassist_gateway::{git, Paths, ACTIVE_HOOKS, PASSTHROUGH_HOOKS};

/// Paths inside hook scripts and git config use forward slashes so Git for
/// Windows' sh can read them.
pub fn portable(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

pub fn hook_script(cli: &Path, name: &str) -> String {
    format!("#!/bin/sh\n# Installed by GitAssist. Do not edit; use the GitAssist app.\nexec {} hook {name} \"$@\"\n", sh_quote(&portable(cli)))
}

fn make_executable(p: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(p, fs::Permissions::from_mode(0o755))?;
    }
    let _ = p;
    Ok(())
}

/// Copies the running `gitassist` binary into GitAssist's own bin dir so hooks
/// keep working wherever the app itself is moved.
pub fn install_cli(paths: &Paths, source: &Path) -> Result<()> {
    fs::create_dir_all(&paths.bin_dir)?;
    let dest = paths.cli_exe();
    let same = match (source.canonicalize(), dest.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if !same {
        let tmp = dest.with_extension("new");
        fs::copy(source, &tmp).with_context(|| format!("copying {}", source.display()))?;
        make_executable(&tmp)?;
        fs::rename(&tmp, &dest)?;
    }
    Ok(())
}

pub fn write_hook_scripts(paths: &Paths) -> Result<()> {
    fs::create_dir_all(&paths.hooks_dir)?;
    let cli = paths.cli_exe();
    for name in ACTIVE_HOOKS.iter().chain(PASSTHROUGH_HOOKS.iter()) {
        let file = paths.hooks_dir.join(name);
        fs::write(&file, hook_script(&cli, name))?;
        make_executable(&file)?;
    }
    Ok(())
}

/// git's per-user config in the XDG location; read alongside ~/.gitconfig.
fn xdg_git_config() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|b| b.home_dir().join(".config")))?;
    Some(base.join("git").join("config"))
}

/// Sets a global git value. If ~/.gitconfig can't be written (read-only or
/// owned by another user), falls back to git's XDG config file.
fn set_global(key: &str, value: &str) -> Result<()> {
    if git::git(&["config", "--global", key, value]).is_ok()
        && git::global_config_get(key).as_deref() == Some(value)
    {
        return Ok(());
    }
    let file = xdg_git_config().context("no home directory")?;
    if let Some(d) = file.parent() {
        fs::create_dir_all(d)?;
    }
    git::git(&["config", "--file", &file.to_string_lossy(), key, value])?;
    anyhow::ensure!(
        git::global_config_get(key).as_deref() == Some(value),
        "git ignores {key} from {} because ~/.gitconfig sets it and cannot be edited",
        file.display()
    );
    Ok(())
}

fn unset_global(key: &str) {
    let _ = git::git(&["config", "--global", "--unset", key]);
    if let Some(f) = xdg_git_config().filter(|f| f.is_file()) {
        let _ = git::git(&["config", "--file", &f.to_string_lossy(), "--unset", key]);
    }
}

pub fn global_ignore_file(paths: &Paths) -> std::path::PathBuf {
    paths.data_dir.join("global-ignore")
}

/// Writes the "every repo" list to GitAssist's global excludes file.
pub fn write_global_ignore(paths: &Paths, cfg: &gitassist_core::Config) -> Result<()> {
    let body = format!(
        "# Managed by GitAssist (Ignore → Every repo)\n{}\n",
        cfg.ignore.every_repo.join("\n")
    );
    fs::create_dir_all(&paths.data_dir)?;
    fs::write(global_ignore_file(paths), body)?;
    Ok(())
}

pub fn install_hooks(paths: &Paths) -> Result<()> {
    write_hook_scripts(paths)?;
    let ours = portable(&paths.hooks_dir);
    let mut state = State::load(&paths.state_file);
    let current = git::global_config_get("core.hooksPath");
    if let Some(cur) = current.as_deref() {
        if cur != ours && state.previous_global_hooks_path.is_none() {
            state.previous_global_hooks_path = Some(cur.to_string());
        }
    }
    set_global("core.hooksPath", &ours)?;

    // Only take over core.excludesFile when the user has none of their own.
    let cfg = gitassist_core::Config::load(&paths.config_file).unwrap_or_default();
    write_global_ignore(paths, &cfg)?;
    let ignore_file = portable(&global_ignore_file(paths));
    match git::global_config_get("core.excludesFile") {
        None => {
            set_global("core.excludesFile", &ignore_file)?;
            state.excludes_file_set = true;
        }
        Some(v) if v == ignore_file => state.excludes_file_set = true,
        Some(_) => {}
    }
    state.hooks_installed = true;
    state.save(&paths.state_file)
}

pub fn hooks_active(paths: &Paths) -> bool {
    git::global_config_get("core.hooksPath").as_deref() == Some(portable(&paths.hooks_dir).as_str())
}

pub fn uninstall_hooks(paths: &Paths) -> Result<()> {
    let mut state = State::load(&paths.state_file);
    if hooks_active(paths) {
        match state.previous_global_hooks_path.take() {
            Some(prev) => set_global("core.hooksPath", &prev)?,
            None => unset_global("core.hooksPath"),
        }
    }
    if state.excludes_file_set
        && git::global_config_get("core.excludesFile").as_deref()
            == Some(portable(&global_ignore_file(paths)).as_str())
    {
        unset_global("core.excludesFile");
    }
    state.excludes_file_set = false;
    let _ = fs::remove_file(global_ignore_file(paths));
    let _ = fs::remove_dir_all(&paths.hooks_dir);
    state.hooks_installed = false;
    state.save(&paths.state_file)
}
