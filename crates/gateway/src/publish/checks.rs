use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{bail, Result};
use gitassist_core::config::Mode;

use crate::targets::TargetChecks;

const MAX_DETAIL: usize = 2000;
const MAX_TIMEOUT: u64 = 3600;

pub fn run(repo: &Path, checks: &TargetChecks) -> Result<()> {
    if !checks.enabled || checks.commands.is_empty() {
        return Ok(());
    }
    let secs = checks.timeout_secs.clamp(1, MAX_TIMEOUT);
    let timeout = Duration::from_secs(secs);
    let on_fail = match checks.on_fail {
        Mode::Warn => Mode::Warn,
        _ => Mode::Block,
    };

    for cmd in &checks.commands {
        if cmd.shell.trim().is_empty() {
            continue;
        }
        let name = if cmd.name.is_empty() {
            cmd.shell.clone()
        } else {
            cmd.name.clone()
        };
        let shell = cmd.shell.clone();
        let env_map = cmd.env.clone();
        let repo = repo.to_path_buf();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut c = if cfg!(windows) {
                let mut c = Command::new("cmd");
                c.arg("/C").arg(&shell);
                c
            } else {
                let mut c = Command::new("sh");
                c.arg("-c").arg(&shell);
                c
            };
            c.current_dir(&repo)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            for (k, v) in &env_map {
                c.env(k, v);
            }
            let _ = tx.send(c.output());
        });
        let out = match rx.recv_timeout(timeout) {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => bail!("check `{name}`: {e}"),
            Err(_) => {
                let msg = format!("check `{name}` timed out after {secs}s");
                if on_fail == Mode::Warn {
                    eprintln!("GitAssist: {msg} (warn)");
                    continue;
                }
                bail!("{msg}");
            }
        };
        if !out.status.success() {
            let detail = detail_text(&out.stderr, &out.stdout);
            let msg = format!(
                "check `{name}` failed (exit {:?}): {detail}",
                out.status.code()
            );
            if on_fail == Mode::Warn {
                eprintln!("GitAssist: {msg} (warn)");
                continue;
            }
            bail!("{msg}");
        }
    }
    Ok(())
}

fn detail_text(stderr: &[u8], stdout: &[u8]) -> String {
    let raw = {
        let e = String::from_utf8_lossy(stderr);
        if !e.trim().is_empty() {
            e.into_owned()
        } else {
            String::from_utf8_lossy(stdout).into_owned()
        }
    };
    let t = raw.trim();
    if t.chars().count() <= MAX_DETAIL {
        t.to_string()
    } else {
        format!("{}…", t.chars().take(MAX_DETAIL).collect::<String>())
    }
}
