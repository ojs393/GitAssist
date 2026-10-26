use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::git;
use crate::paths::Paths;
use crate::state::State;

fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

fn is_runnable(p: &Path) -> bool {
    let Ok(meta) = p.metadata() else { return false };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Hook scripts other tools installed that git would have run without GitAssist:
/// the previous global hooks dir, then the repository's own `.git/hooks`.
pub fn chained_hooks(name: &str, paths: &Paths) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(prev) = State::load(&paths.state_file).previous_global_hooks_path {
        dirs.push(expand_home(&prev));
    }
    if let Some(common) = git::common_dir() {
        dirs.push(common.join("hooks"));
    }
    let mut out = Vec::new();
    for d in dirs {
        if same_dir(&d, &paths.hooks_dir) {
            continue;
        }
        let candidate = d.join(name);
        if is_runnable(&candidate) && !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}

fn expand_home(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = directories::BaseDirs::new() {
            return home.home_dir().join(rest);
        }
    }
    PathBuf::from(p)
}

fn hook_command(path: &Path) -> Command {
    if cfg!(windows) {
        // Git for Windows runs hooks through its bundled sh, which is on PATH inside hooks.
        let mut c = Command::new("sh");
        c.arg(path);
        c
    } else {
        Command::new(path)
    }
}

/// Runs the chained hooks in order; returns the first non-zero exit code.
pub fn run_chain(name: &str, args: &[String], stdin: Option<&[u8]>, paths: &Paths) -> i32 {
    for hook in chained_hooks(name, paths) {
        let mut cmd = hook_command(&hook);
        cmd.args(args).stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::inherit()
        });
        let Ok(mut child) = cmd.spawn() else {
            eprintln!("GitAssist: could not run chained hook {}", hook.display());
            return 1;
        };
        if let (Some(data), Some(mut pipe)) = (stdin, child.stdin.take()) {
            let _ = pipe.write_all(data);
        }
        match child.wait() {
            Ok(s) if s.success() => {}
            Ok(s) => return s.code().unwrap_or(1),
            Err(_) => return 1,
        }
    }
    0
}
