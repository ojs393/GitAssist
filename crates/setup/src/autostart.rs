use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::Result;

use crate::Env;

pub const LABEL: &str = "com.gitassist.desktop";

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The `.app` bundle an executable lives in, if any.
pub fn app_bundle(exe: &Path) -> Option<PathBuf> {
    exe.ancestors()
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .map(Path::to_path_buf)
}

/// Starts GitAssist through Launch Services (`open`), exactly like a click.
/// Started any other way, macOS doesn't treat it as a running app, so a later
/// click on the app fails (error -600) and its window never comes forward.
pub fn launch_agent_plist(app: &Path) -> String {
    let args = match app_bundle(app) {
        Some(bundle) => format!(
            "<string>/usr/bin/open</string><string>-g</string><string>-a</string><string>{}</string><string>--args</string><string>--background</string>",
            xml_escape(&bundle.to_string_lossy())
        ),
        None => format!("<string>{}</string><string>--background</string>", xml_escape(&app.to_string_lossy())),
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>{args}</array>
  <key>RunAtLoad</key><true/>
  <key>AssociatedBundleIdentifiers</key><string>{LABEL}</string>
</dict>
</plist>
"#
    )
}

pub fn desktop_entry(app: &Path) -> String {
    let exe = app.to_string_lossy().replace('"', "\\\"");
    format!(
        "[Desktop Entry]\nType=Application\nName=GitAssist\nComment=Keeps GitAssist's commit and PR rules active\nExec=\"{exe}\" --background\nTerminal=false\nX-GNOME-Autostart-enabled=true\nNoDisplay=true\n"
    )
}

pub fn windows_run_value(app: &Path) -> String {
    format!("\"{}\" --background", app.to_string_lossy())
}

fn mac_plist(env: &Env) -> PathBuf {
    env.home
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

fn linux_desktop(env: &Env) -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| env.home.join(".config"));
    base.join("autostart/gitassist.desktop")
}

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

fn uid() -> Option<String> {
    let out = Command::new("id").arg("-u").output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn launchctl(args: &[&str]) -> bool {
    Command::new("launchctl")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn mac_loaded() -> bool {
    uid().is_some_and(|u| launchctl(&["print", &format!("gui/{u}/{LABEL}")]))
}

/// Starts the background monitor now (Windows/Linux); single-instance makes repeats harmless.
fn start_now(app: &Path) {
    let _ = Command::new(app)
        .arg("--background")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

pub fn enable(env: &Env, app: &Path) -> Result<()> {
    if cfg!(target_os = "macos") {
        let plist = mac_plist(env);
        fs::create_dir_all(plist.parent().unwrap_or(Path::new(".")))?;
        fs::write(&plist, launch_agent_plist(app))?;
        let uid = uid().ok_or_else(|| anyhow::anyhow!("could not read the user id"))?;
        let domain = format!("gui/{uid}");
        let service = format!("{domain}/{LABEL}");
        // Reload so a changed app path takes effect; launchd can need a moment after bootout.
        launchctl(&["bootout", &service]);
        let plist_s = plist.to_string_lossy().into_owned();
        for attempt in 0..5 {
            if launchctl(&["bootstrap", &domain, &plist_s]) || mac_loaded() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(300 * (attempt + 1)));
        }
        // Start it now unless it's already running: `open` on a running app
        // would bring its window forward, which self-repair must not do.
        let running = Command::new("pgrep")
            .args(["-f", &app.to_string_lossy()])
            .stdout(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if !running {
            launchctl(&["kickstart", &service]);
        }
        anyhow::ensure!(mac_loaded(), "launchd did not load {LABEL}");
    } else if cfg!(windows) {
        let status = Command::new("reg")
            .args([
                "add",
                RUN_KEY,
                "/v",
                "GitAssist",
                "/t",
                "REG_SZ",
                "/d",
                &windows_run_value(app),
                "/f",
            ])
            .stdout(Stdio::null())
            .status()?;
        anyhow::ensure!(status.success(), "reg add failed");
        start_now(app);
    } else {
        let file = linux_desktop(env);
        fs::create_dir_all(file.parent().unwrap_or(Path::new(".")))?;
        fs::write(file, desktop_entry(app))?;
        start_now(app);
    }
    Ok(())
}

pub fn disable(env: &Env) -> Result<()> {
    if cfg!(target_os = "macos") {
        if let Some(uid) = uid() {
            let _ = Command::new("launchctl")
                .args(["bootout", &format!("gui/{uid}/{LABEL}")])
                .stderr(Stdio::null())
                .status();
        }
        let _ = fs::remove_file(mac_plist(env));
    } else if cfg!(windows) {
        let _ = Command::new("reg")
            .args(["delete", RUN_KEY, "/v", "GitAssist", "/f"])
            .stdout(Stdio::null())
            .status();
    } else {
        let _ = fs::remove_file(linux_desktop(env));
    }
    Ok(())
}

pub fn is_running(app: &Path) -> bool {
    if cfg!(windows) {
        return true;
    }
    Command::new("pgrep")
        .args(["-f", &app.to_string_lossy()])
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Starts the background app if the login item is on but the app isn't running.
pub fn ensure_running(env: &Env, app: &Path) {
    if !is_running(app) && is_enabled(env) {
        if cfg!(target_os = "macos") {
            if let Some(u) = uid() {
                launchctl(&["kickstart", &format!("gui/{u}/{LABEL}")]);
            }
        } else {
            start_now(app);
        }
    }
}

/// The login item exists and starts this app the current way (macOS).
pub fn is_current(env: &Env, app: &Path) -> bool {
    if !cfg!(target_os = "macos") {
        return is_enabled(env);
    }
    fs::read_to_string(mac_plist(env)).is_ok_and(|t| t == launch_agent_plist(app)) && mac_loaded()
}

pub fn is_enabled(env: &Env) -> bool {
    if cfg!(target_os = "macos") {
        mac_plist(env).is_file() && mac_loaded()
    } else if cfg!(windows) {
        Command::new("reg")
            .args(["query", RUN_KEY, "/v", "GitAssist"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    } else {
        linux_desktop(env).is_file()
    }
}
