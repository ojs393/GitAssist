#![cfg(unix)]

mod common;

use std::fs;

use common::{Sandbox, CLAUDE_FOOTER};

fn setup() -> Sandbox {
    let s = Sandbox::new();
    s.install_hooks();
    s.init_repo();
    s
}

#[test]
fn agent_trailers_are_stripped_from_commits() {
    let s = setup();
    s.change("b.txt");
    s.git_ok(&["commit", "-q", "-m", &format!("feat: add b{CLAUDE_FOOTER}")]);
    assert_eq!(s.last_message(), "feat: add b");
    assert!(s.log().contains("\"outcome\":\"fixed\""));
}

#[test]
fn cursor_trailer_added_via_git_trailer_flag_is_stripped() {
    let s = setup();
    s.change("b.txt");
    s.git_ok(&[
        "commit",
        "-q",
        "-m",
        "fix: b",
        "--trailer",
        "Co-authored-by: Cursor <cursoragent@cursor.com>",
    ]);
    assert_eq!(s.last_message(), "fix: b");
}

#[test]
fn no_verify_still_strips() {
    let s = setup();
    s.change("b.txt");
    s.git_ok(&[
        "commit",
        "-q",
        "--no-verify",
        "-m",
        &format!("fix: b{CLAUDE_FOOTER}"),
    ]);
    assert_eq!(s.last_message(), "fix: b");
}

#[test]
fn editor_commit_with_comments_is_cleaned() {
    let s = setup();
    s.change("b.txt");
    let o = s
        .cmd("git")
        .args([
            "commit",
            "-q",
            "-e",
            "-m",
            "fix: edited\n\nCo-authored-by: Claude <c@x>",
        ])
        .env("GIT_EDITOR", "true")
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(s.last_message(), "fix: edited");
}

#[test]
fn blocking_rule_rejects_commit_with_reason() {
    let s = setup();
    s.write_config("[tabs.commit]\nsubject_max = { value = 20, mode = \"block\" }\n");
    s.change("b.txt");
    let o = s.git(&["commit", "-q", "-m", "feat: this subject is far too long"]);
    assert!(!o.status.success());
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(
        err.contains("GitAssist [commit] subject is 34 chars; limit is 20."),
        "{err}"
    );
    assert_eq!(s.last_message(), "chore: initial");
}

#[test]
fn bypass_env_skips_rules_and_is_logged() {
    let s = setup();
    s.write_config("[tabs.commit]\nsubject_max = { value = 5, mode = \"block\" }\n");
    s.change("b.txt");
    let o = s
        .cmd("git")
        .args(["commit", "-q", "-m", "long subject"])
        .env("GITASSIST_BYPASS", "1")
        .output()
        .unwrap();
    assert!(o.status.success());
    assert!(s.log().contains("\"outcome\":\"bypass\""));
}

#[test]
fn docs_only_commit_uses_docs_tab() {
    let s = setup();
    s.write_config("[tabs.docs_commit]\nsubject_max = { value = 10, mode = \"block\" }\n");
    s.change("docs/guide.md");
    assert!(!s
        .git(&["commit", "-q", "-m", "docs: a long docs subject"])
        .status
        .success());
    s.change("src/main.rs");
    assert!(s
        .git(&["commit", "-q", "-m", "docs: a long docs subject"])
        .status
        .success());
}

#[test]
fn merge_commit_uses_merge_tab() {
    let s = setup();
    s.git_ok(&["checkout", "-q", "-b", "feature"]);
    s.change("f.txt");
    s.git_ok(&["commit", "-q", "-m", "feat: f"]);
    s.git_ok(&["checkout", "-q", "main"]);
    s.change("m.txt");
    s.git_ok(&["commit", "-q", "-m", "fix: m"]);
    s.git_ok(&["merge", "-q", "--no-edit", "feature"]);
    let log = s.log();
    let last = log
        .lines()
        .rfind(|l| l.contains("hook:commit-msg"))
        .unwrap();
    assert!(last.contains("\"tab\":\"merge\""), "{last}");
}

#[test]
fn repository_hooks_still_run() {
    let s = setup();
    let marker = s.root.join("repo-hook-ran");
    let hook = s.repo.join(".git/hooks/commit-msg");
    fs::write(&hook, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    s.change("b.txt");
    s.git_ok(&["commit", "-q", "-m", "fix: b"]);
    assert!(marker.exists());
}

#[test]
fn corrupt_config_fails_open() {
    let s = setup();
    s.write_config("this is = = not toml");
    s.change("b.txt");
    let msg = format!("fix: b{CLAUDE_FOOTER}");
    s.git_ok(&["commit", "-q", "-m", &msg]);
    assert!(
        s.last_message().contains("Co-Authored-By"),
        "left unchanged"
    );
    assert!(s.log().contains("\"outcome\":\"error\""));
}

#[test]
fn trailers_are_added_from_settings() {
    let s = setup();
    s.write_config("[global.trailers]\nadd = [\"Signed-off-by: Test User <test@example.com>\"]\n");
    s.change("b.txt");
    s.git_ok(&["commit", "-q", "-m", &format!("fix: b{CLAUDE_FOOTER}")]);
    assert_eq!(
        s.last_message(),
        "fix: b\n\nSigned-off-by: Test User <test@example.com>"
    );
}

#[test]
fn author_is_enforced_when_enabled() {
    let s = setup();
    s.write_config(
        "[global.identity]\nauthor_name = \"Me Myself\"\nauthor_email = \"me@example.com\"\nenforce_author = true\n",
    );
    s.change("b.txt");
    let o = s
        .cmd("git")
        .args(["commit", "-q", "-m", "fix: b"])
        .env("GIT_AUTHOR_NAME", "Some Agent (aider)")
        .env("GIT_AUTHOR_EMAIL", "agent@example.com")
        .output()
        .unwrap();
    assert!(o.status.success());
    assert_eq!(
        s.git_ok(&["log", "-1", "--format=%an <%ae>"]).trim(),
        "Me Myself <me@example.com>"
    );
    assert_eq!(
        s.git_ok(&["rev-list", "--count", "HEAD"]).trim(),
        "2",
        "amended, not a new commit"
    );
}

#[test]
fn pre_push_blocks_commits_that_break_rules() {
    let s = setup();
    let remote = s.root.join("remote.git");
    s.git_ok(&["init", "-q", "--bare", remote.to_str().unwrap()]);
    s.git_ok(&["remote", "add", "origin", remote.to_str().unwrap()]);
    s.git_ok(&["push", "-q", "origin", "main"]);

    s.change("b.txt");
    s.cmd("git")
        .args(["commit", "-q", "-m", "a subject that is long"])
        .env("GITASSIST_BYPASS", "1")
        .output()
        .unwrap();
    s.write_config("[tabs.commit]\nsubject_max = { value = 10, mode = \"block\" }\n");
    let o = s.git(&["push", "-q", "origin", "main"]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("push blocked"));
}

#[test]
fn uninstall_restores_previous_global_hooks_path() {
    let s = Sandbox::new();
    s.git_ok(&[
        "config",
        "--global",
        "core.hooksPath",
        "/some/previous/hooks",
    ]);
    s.install_hooks();
    assert_eq!(
        s.git_ok(&["config", "--global", "core.hooksPath"]).trim(),
        s.ga.join("hooks").to_str().unwrap()
    );
    assert!(s.gitassist(&["uninstall"]).status.success());
    assert_eq!(
        s.git_ok(&["config", "--global", "core.hooksPath"]).trim(),
        "/some/previous/hooks"
    );
}

#[test]
fn status_reports_drift() {
    let s = setup();
    let o = s.gitassist(&["status", "--json"]);
    let report: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    let hooks = report["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == "hooks")
        .unwrap();
    assert_eq!(hooks["ok"], true);
    s.git_ok(&["config", "--global", "core.hooksPath", "/elsewhere"]);
    let o = s.gitassist(&["status", "--json"]);
    let report: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    let hooks = report["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == "hooks")
        .unwrap();
    assert_eq!(hooks["ok"], false);
    assert!(s.gitassist(&["repair"]).status.success());
    assert_eq!(
        s.git_ok(&["config", "--global", "core.hooksPath"]).trim(),
        s.ga.join("hooks").to_str().unwrap()
    );
}

#[test]
fn connect_repo_with_local_hooks_path() {
    let s = setup();
    fs::create_dir_all(s.repo.join(".husky/_")).unwrap();
    s.git_ok(&["config", "--local", "core.hooksPath", ".husky/_"]);
    s.change("b.txt");
    s.git_ok(&["commit", "-q", "-m", &format!("fix: before{CLAUDE_FOOTER}")]);
    assert!(
        s.last_message().contains("Co-Authored-By"),
        "local hooksPath bypasses global hooks"
    );

    assert!(s
        .gitassist(&["connect", s.repo.to_str().unwrap()])
        .status
        .success());
    s.change("c.txt");
    s.git_ok(&["commit", "-q", "-m", &format!("fix: after{CLAUDE_FOOTER}")]);
    assert_eq!(s.last_message(), "fix: after");
}
