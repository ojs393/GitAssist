//! Apps started from the Dock, Finder, Explorer or at login get a minimal
//! PATH (on macOS just /usr/bin:/bin:/usr/sbin:/sbin), so `gh`, `claude` or
//! Homebrew's git look missing. This rebuilds PATH the way a terminal sees it.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use gitassist_gateway::repo::output_with_timeout;

const MARK: &str = "__GITASSIST_PATH__";

fn login_shell_path() -> Option<String> {
    if cfg!(windows) {
        return None;
    }
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/bin/zsh".into());
    let mut cmd = Command::new(shell);
    cmd.args(["-ilc", &format!("printf '{MARK}%s{MARK}' \"$PATH\"")]);
    let out = output_with_timeout(cmd, Duration::from_secs(4))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let start = text.find(MARK)? + MARK.len();
    let end = text[start..].find(MARK)? + start;
    Some(text[start..end].to_string())
}

fn well_known(home: &std::path::Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            if let Some(base) = std::env::var_os(var).map(PathBuf::from) {
                v.push(base.join("GitHub CLI"));
                v.push(base.join("Git").join("cmd"));
                v.push(base.join("Programs").join("Git").join("cmd"));
            }
        }
        v.push(home.join("scoop").join("shims"));
        v.push(home.join(".local").join("bin"));
    } else {
        for p in [
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/home/linuxbrew/.linuxbrew/bin",
            "/snap/bin",
            "/usr/bin",
            "/bin",
        ] {
            v.push(PathBuf::from(p));
        }
        v.push(home.join(".local/bin"));
        v.push(home.join(".cargo/bin"));
        v.push(home.join("bin"));
        v.push(home.join(".npm-global/bin"));
        v.push(home.join(".bun/bin"));
    }
    v
}

/// Current PATH, then the login shell's, then common install locations; deduplicated.
pub fn augmented_path(home: &std::path::Path, include_login_shell: bool) -> std::ffi::OsString {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut push = |p: PathBuf| {
        if !p.as_os_str().is_empty() && !dirs.contains(&p) {
            dirs.push(p);
        }
    };
    if let Some(cur) = std::env::var_os("PATH") {
        std::env::split_paths(&cur).for_each(&mut push);
    }
    if include_login_shell {
        if let Some(login) = login_shell_path() {
            std::env::split_paths(&login).for_each(&mut push);
        }
    }
    well_known(home)
        .into_iter()
        .filter(|p| p.is_dir())
        .for_each(&mut push);
    std::env::join_paths(dirs).unwrap_or_default()
}

/// Call once at startup, before other threads read the environment.
pub fn fix_process_path(home: &std::path::Path) {
    let p = augmented_path(home, true);
    if !p.is_empty() {
        std::env::set_var("PATH", p);
    }
}
