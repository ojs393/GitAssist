use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use gitassist_gateway::state::State;
use gitassist_gateway::Paths;

use crate::blocks::{self, SHELL};
use crate::hooks_install::portable;
use crate::Env;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Shell {
    Posix,
    Fish,
    Pwsh,
}

fn profile_candidates(env: &Env) -> Vec<(PathBuf, Shell)> {
    let h = &env.home;
    // .zprofile/.bash_profile/.profile cover login shells (`zsh -lc`, used by
    // agents to run commands); the rc files cover interactive terminals.
    let mut v = vec![
        (h.join(".zprofile"), Shell::Posix),
        (h.join(".zshrc"), Shell::Posix),
        (h.join(".bashrc"), Shell::Posix),
        (h.join(".bash_profile"), Shell::Posix),
        (h.join(".profile"), Shell::Posix),
        (h.join(".config/fish/config.fish"), Shell::Fish),
    ];
    if cfg!(windows) {
        v.push((
            h.join("Documents/PowerShell/Microsoft.PowerShell_profile.ps1"),
            Shell::Pwsh,
        ));
        v.push((
            h.join("Documents/WindowsPowerShell/Microsoft.PowerShell_profile.ps1"),
            Shell::Pwsh,
        ));
    }
    v
}

/// Git Bash / MSYS spell `C:\x` as `/c/x`; a drive colon would split PATH.
fn posix_path(bin: &Path) -> String {
    let p = portable(bin);
    let b = p.as_bytes();
    if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
        return format!("/{}{}", (b[0] as char).to_ascii_lowercase(), &p[2..]);
    }
    p
}

fn path_line(shell: Shell, bin: &Path) -> String {
    let p = bin.to_string_lossy();
    match shell {
        Shell::Posix => format!("export PATH=\"{}:$PATH\"", posix_path(bin)),
        Shell::Fish => format!("set -gx PATH \"{}\" $PATH", posix_path(bin)),
        Shell::Pwsh => format!("$env:Path = \"{p};\" + $env:Path"),
    }
}

/// Profiles to edit: those that exist, or the login shell's default rc file if none do.
fn targets(env: &Env) -> Vec<(PathBuf, Shell)> {
    let existing: Vec<_> = profile_candidates(env)
        .into_iter()
        .filter(|(p, _)| p.is_file())
        .collect();
    if !existing.is_empty() {
        return existing;
    }
    let shell = std::env::var("SHELL").unwrap_or_default();
    if shell.ends_with("zsh") {
        vec![
            (env.home.join(".zprofile"), Shell::Posix),
            (env.home.join(".zshrc"), Shell::Posix),
        ]
    } else if shell.ends_with("fish") {
        vec![(env.home.join(".config/fish/config.fish"), Shell::Fish)]
    } else if cfg!(windows) {
        vec![(
            env.home
                .join("Documents/PowerShell/Microsoft.PowerShell_profile.ps1"),
            Shell::Pwsh,
        )]
    } else {
        vec![(env.home.join(".bashrc"), Shell::Posix)]
    }
}

pub fn install_shim_binary(paths: &Paths) -> Result<()> {
    fs::create_dir_all(&paths.bin_dir)?;
    let shim = paths.gh_shim();
    let _ = fs::remove_file(&shim);
    #[cfg(unix)]
    std::os::unix::fs::symlink(gitassist_gateway::paths::exe_name("gitassist"), &shim)?;
    #[cfg(not(unix))]
    fs::copy(paths.cli_exe(), &shim)?;
    Ok(())
}

pub struct ShimReport {
    pub edited: Vec<String>,
    pub failed: Vec<(String, String)>,
}

/// Puts GitAssist's bin dir first on PATH via a marked block at the end of
/// each shell profile, so it wins over earlier PATH edits.
pub fn install_shim(paths: &Paths, env: &Env) -> Result<ShimReport> {
    install_shim_binary(paths)?;
    let mut state = State::load(&paths.state_file);
    let mut report = ShimReport {
        edited: Vec::new(),
        failed: Vec::new(),
    };
    for (file, shell) in targets(env) {
        let name = file.to_string_lossy().into_owned();
        let line = path_line(shell, &paths.bin_dir);
        let content =
            format!("# Lets GitAssist check `gh` pull requests, comments and issues.\n{line}");
        let text = fs::read_to_string(&file).unwrap_or_default();
        let new = blocks::remove(&text, SHELL);
        let new = blocks::upsert(&new, SHELL, &content);
        let result = (|| -> Result<()> {
            crate::backup::backup_file(paths, &file)?;
            if let Some(dir) = file.parent() {
                fs::create_dir_all(dir)?;
            }
            fs::write(&file, new)?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                if !state.profiles.contains(&name) {
                    state.profiles.push(name.clone());
                }
                report.edited.push(name);
            }
            Err(e) => report.failed.push((name, format!("{e:#}"))),
        }
    }
    if cfg!(windows) && set_windows_user_path(&paths.bin_dir, true).is_ok() {
        state.windows_user_path = true;
    }
    state.shim_installed = true;
    state.save(&paths.state_file)?;
    Ok(report)
}

pub fn uninstall_shim(paths: &Paths) -> Result<()> {
    let mut state = State::load(&paths.state_file);
    for p in state.profiles.drain(..) {
        let _ = blocks::remove_file_block(Path::new(&p), SHELL, false);
    }
    if state.windows_user_path {
        let _ = set_windows_user_path(&paths.bin_dir, false);
        state.windows_user_path = false;
    }
    let _ = fs::remove_file(paths.gh_shim());
    state.shim_installed = false;
    state.save(&paths.state_file)
}

/// What `gh` resolves to in each kind of shell an agent might use. This is
/// the real test of the gateway, not whether a profile file was edited.
pub fn shell_coverage(paths: &Paths) -> Vec<(String, Option<bool>)> {
    if cfg!(windows) {
        return Vec::new();
    }
    let shim = paths.gh_shim();
    let mut out = Vec::new();
    let user_shell = std::env::var("SHELL").unwrap_or_default();
    let mut shells: Vec<(&str, &str)> = vec![("zsh", "-lc"), ("zsh", "-ic"), ("bash", "-lc")];
    if user_shell.ends_with("fish") {
        shells.push(("fish", "-lc"));
    }
    // Probed in parallel: an interactive zsh alone can take a second or more.
    let handles: Vec<_> = shells
        .into_iter()
        .filter_map(|(sh, flag)| {
            let bin = ["/bin", "/usr/bin", "/opt/homebrew/bin", "/usr/local/bin"]
                .iter()
                .map(|d| Path::new(d).join(sh))
                .find(|p| p.is_file())?;
            let shim = shim.clone();
            Some(std::thread::spawn(move || {
                let mut cmd = Command::new(bin);
                cmd.args([flag, "command -v gh"]);
                let found = gitassist_gateway::repo::output_with_timeout(
                    cmd,
                    std::time::Duration::from_secs(6),
                )
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .last()
                        .unwrap_or("")
                        .trim()
                        .to_string()
                });
                let label = if flag == "-lc" {
                    format!("{sh} (login)")
                } else {
                    format!("{sh} (terminal)")
                };
                // A shell that didn't start in time (a busy machine) is unknown, not a failure.
                (label, found.map(|f| Path::new(&f) == shim))
            }))
        })
        .collect();
    for h in handles {
        if let Ok(r) = h.join() {
            out.push(r);
        }
    }
    out
}

pub fn profiles_ok(paths: &Paths) -> bool {
    let state = State::load(&paths.state_file);
    !state.profiles.is_empty()
        && state
            .profiles
            .iter()
            .all(|p| fs::read_to_string(p).is_ok_and(|t| blocks::contains(&t, SHELL)))
}

/// Adds or removes the bin dir at the front of the per-user PATH (HKCU) on Windows.
fn set_windows_user_path(bin: &Path, add: bool) -> Result<()> {
    let dir = bin.to_string_lossy().replace('\'', "''");
    let script = if add {
        format!(
            "$d='{dir}'; $p=[Environment]::GetEnvironmentVariable('Path','User'); \
             $parts=@($p -split ';' | Where-Object {{ $_ -and $_ -ne $d }}); \
             [Environment]::SetEnvironmentVariable('Path', (@($d) + $parts) -join ';', 'User')"
        )
    } else {
        format!(
            "$d='{dir}'; $p=[Environment]::GetEnvironmentVariable('Path','User'); \
             $parts=@($p -split ';' | Where-Object {{ $_ -and $_ -ne $d }}); \
             [Environment]::SetEnvironmentVariable('Path', $parts -join ';', 'User')"
        )
    };
    let status = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()?;
    anyhow::ensure!(status.success(), "powershell exited with {status}");
    Ok(())
}
