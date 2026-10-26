use std::path::Path;

use gitassist_setup::autostart::{desktop_entry, launch_agent_plist, windows_run_value, LABEL};

#[test]
fn plist_starts_the_app_like_a_click() {
    let p = launch_agent_plist(Path::new(
        "/Applications/Git & Assist.app/Contents/MacOS/gitassist-app",
    ));
    assert!(p.contains(&format!("<string>{LABEL}</string>")));
    assert!(p.contains("<string>/usr/bin/open</string><string>-g</string><string>-a</string><string>/Applications/Git &amp; Assist.app</string>"));
    assert!(p.contains("<string>--args</string><string>--background</string>"));
    assert!(p.contains("<key>RunAtLoad</key><true/>"));
}

#[test]
fn plist_runs_a_bare_binary_directly() {
    let p = launch_agent_plist(Path::new("/opt/gitassist/gitassist-app"));
    assert!(
        p.contains("<string>/opt/gitassist/gitassist-app</string><string>--background</string>")
    );
}

#[test]
fn desktop_entry_quotes_exec() {
    let d = desktop_entry(Path::new("/opt/Git Assist/gitassist-app"));
    assert!(d.contains("Exec=\"/opt/Git Assist/gitassist-app\" --background"));
}

#[test]
fn windows_run_value_quotes_path() {
    assert_eq!(
        windows_run_value(Path::new(r"C:\Program Files\GitAssist\GitAssist.exe")),
        "\"C:\\Program Files\\GitAssist\\GitAssist.exe\" --background"
    );
}
