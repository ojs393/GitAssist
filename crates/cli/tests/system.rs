#![cfg(unix)]

//! Turning GitAssist on, self-repair, agents appearing later, and the log.

mod common;

use std::fs;

use common::Sandbox;

fn json(s: &Sandbox) -> serde_json::Value {
    serde_json::from_slice(&s.gitassist(&["status", "--json"]).stdout).unwrap()
}

fn item<'a>(v: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    v["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == id)
        .unwrap()
}

fn adapter<'a>(v: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    v["adapters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == id)
        .unwrap()
}

#[test]
fn turn_on_sets_up_everything_in_one_go() {
    let s = Sandbox::new();
    fs::create_dir_all(s.home.join(".claude")).unwrap();
    fs::write(s.home.join(".zshrc"), "# mine\n").unwrap();
    let o = s.gitassist(&["on"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    let v = json(&s);
    assert_eq!(item(&v, "hooks")["state"], "on");
    assert_eq!(item(&v, "shim")["state"], "on");
    assert_eq!(item(&v, "repair")["state"], "on");
    assert_eq!(adapter(&v, "claude")["status"], "current");
    assert_eq!(
        v["github_user"], "tester",
        "account detected from the user's gh"
    );
    let claude = fs::read_to_string(s.home.join(".claude/settings.json")).unwrap();
    assert!(claude.contains("\"attribution\""));
}

#[test]
fn repair_connects_disconnected_hooked_repos() {
    let s = Sandbox::new();
    s.init_repo();
    s.gitassist(&["on"]);
    fs::create_dir_all(s.repo.join(".husky/_")).unwrap();
    s.git_ok(&["config", "--local", "core.hooksPath", ".husky/_"]);
    // Record the repo the same way activity/registry would, so status lists it.
    let mut cfg = gitassist_core::Config::load(&s.ga.join("config.toml")).unwrap();
    cfg.watched_repos = vec![s.repo.to_string_lossy().into_owned()];
    cfg.save_atomic(&s.ga.join("config.toml")).unwrap();

    let before = json(&s);
    let repos = before["repos"].as_array().unwrap();
    assert!(
        repos.iter().any(|r| r["connected"] == false),
        "expected a disconnected hooked repo: {before}"
    );

    let o = s.gitassist(&["repair"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    let after = json(&s);
    let repos = after["repos"].as_array().unwrap();
    assert!(
        repos.iter().all(|r| r["connected"] == true),
        "repair should connect hooked repos: {after}"
    );
}

#[test]
fn heal_restores_what_another_tool_undid() {
    let s = Sandbox::new();
    s.gitassist(&["on"]);
    s.git_ok(&["config", "--global", "core.hooksPath", "/somewhere/else"]);
    fs::remove_file(s.ga.join("hooks").join("commit-msg")).unwrap();
    assert_eq!(item(&json(&s), "hooks")["state"], "attention");
    let o = s.gitassist(&["heal"]);
    assert!(
        String::from_utf8_lossy(&o.stdout).contains("Repaired"),
        "{}",
        String::from_utf8_lossy(&o.stdout)
    );
    assert_eq!(item(&json(&s), "hooks")["state"], "on");
    assert!(s.log().contains("Self-repair restored"));
}

#[test]
fn agents_installed_later_are_configured_automatically() {
    let s = Sandbox::new();
    s.gitassist(&["on"]);
    // Aider is the one agent not on the test machine's PATH.
    assert_eq!(adapter(&json(&s), "aider")["status"], "not_detected");
    fs::write(s.home.join(".aider.conf.yml"), "model: x\n").unwrap();
    s.gitassist(&["heal"]);
    assert_eq!(adapter(&json(&s), "aider")["status"], "current");
    assert!(fs::read_to_string(s.home.join(".aider.conf.yml"))
        .unwrap()
        .contains("attribute-co-authored-by: false"));
}

#[test]
fn heal_does_nothing_unless_turned_on() {
    let s = Sandbox::new();
    s.install_hooks();
    s.git_ok(&["config", "--global", "core.hooksPath", "/elsewhere"]);
    s.gitassist(&["heal"]);
    assert_eq!(
        s.git_ok(&["config", "--global", "core.hooksPath"]).trim(),
        "/elsewhere"
    );
}

#[test]
fn uninstall_after_turn_on_leaves_no_trace() {
    let s = Sandbox::new();
    fs::create_dir_all(s.home.join(".claude")).unwrap();
    fs::write(s.home.join(".zshrc"), "# mine\n").unwrap();
    let before_zshrc = fs::read(s.home.join(".zshrc")).unwrap();
    let before_gitconfig = fs::read(&s.gitconfig).unwrap();
    s.gitassist(&["on"]);
    assert!(s.gitassist(&["uninstall"]).status.success());
    assert_eq!(fs::read(s.home.join(".zshrc")).unwrap(), before_zshrc);
    assert!(!s.home.join(".claude/settings.json").exists());
    assert!(!s.home.join(".claude/CLAUDE.md").exists());
    let after = String::from_utf8(fs::read(&s.gitconfig).unwrap()).unwrap();
    assert!(
        !after.contains("hooksPath") && !after.contains("excludesFile"),
        "{after}"
    );
    assert!(after.starts_with(std::str::from_utf8(&before_gitconfig).unwrap().trim_end()));
}

#[test]
fn gh_missing_is_not_an_error() {
    let s = Sandbox::new();
    fs::remove_file(s.fake_gh_dir.join("gh")).unwrap();
    // A PATH holding only git: CI machines often have a real gh in /usr/bin.
    let only_git = s.root.join("only-git");
    fs::create_dir_all(&only_git).unwrap();
    let git = String::from_utf8(
        std::process::Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    std::os::unix::fs::symlink(git.trim(), only_git.join("git")).unwrap();
    let path = only_git.to_str().unwrap().to_string();
    let o = s
        .cmd(env!("CARGO_BIN_EXE_gitassist"))
        .arg("on")
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    let v: serde_json::Value = serde_json::from_slice(
        &s.cmd(env!("CARGO_BIN_EXE_gitassist"))
            .args(["status", "--json"])
            .env("PATH", &path)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert_eq!(item(&v, "shim")["state"], "not_needed");
}

#[test]
fn log_rotates_instead_of_growing_forever() {
    let s = Sandbox::new();
    s.install_hooks();
    s.init_repo();
    fs::write(s.ga.join("activity.log"), "x".repeat(5 * 1024 * 1024 + 10)).unwrap();
    s.change("b.txt");
    s.git_ok(&["commit", "-q", "-m", "fix: rotate"]);
    assert!(s.ga.join("activity.log.1").exists());
    assert!(fs::metadata(s.ga.join("activity.log")).unwrap().len() < 10_000);
}

#[test]
fn every_log_line_is_valid_json_with_a_source() {
    let s = Sandbox::new();
    s.gitassist(&["on"]);
    s.init_repo();
    s.change("b.txt");
    s.git_ok(&["commit", "-q", "-m", "fix: logged\n\nCo-authored-by: a <b>"]);
    let log = s.log();
    assert!(log.lines().count() >= 3);
    for line in log.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(
            v["source"].is_string() && v["ts"].as_u64().unwrap() > 0,
            "{line}"
        );
    }
    assert!(log.contains("\"source\":\"setup\""));
    assert!(log.contains("\"source\":\"hook:commit-msg\""));
}

#[test]
fn repos_get_a_rule_file_that_is_never_committed() {
    let s = Sandbox::new();
    s.install_hooks();
    s.init_repo();
    let rule = s.repo.join(".cursor/rules/gitassist.mdc");
    assert!(rule.exists(), "written on the first commit");
    let text = fs::read_to_string(&rule).unwrap();
    assert!(
        text.starts_with("---\n")
            && text.contains("alwaysApply: true")
            && text.contains("managed by GitAssist")
    );
    assert_eq!(
        s.git_ok(&["status", "--porcelain"]),
        "",
        "excluded, so the repo stays clean"
    );
    s.git_ok(&["add", "-A"]);
    assert!(!s.git_ok(&["ls-files"]).contains("gitassist.mdc"));
}

#[test]
fn cloned_repos_get_the_rule_file_before_any_commit() {
    let s = Sandbox::new();
    s.install_hooks();
    s.init_repo();
    let clone = s.root.join("clone");
    s.git_ok(&[
        "clone",
        "-q",
        s.repo.to_str().unwrap(),
        clone.to_str().unwrap(),
    ]);
    assert!(clone.join(".cursor/rules/gitassist.mdc").exists());
    let status = s
        .cmd("git")
        .current_dir(&clone)
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert!(status.stdout.is_empty());
}

#[test]
fn rule_file_can_be_switched_off_and_is_removed_on_uninstall() {
    let s = Sandbox::new();
    s.install_hooks();
    s.init_repo();
    let rule = s.repo.join(".cursor/rules/gitassist.mdc");
    s.write_config("repo_rule_files = false\n");
    s.change("b.txt");
    s.git_ok(&["commit", "-q", "-m", "fix: b"]);
    assert!(!rule.exists());
    s.write_config("");
    s.change("c.txt");
    s.git_ok(&["commit", "-q", "-m", "fix: c"]);
    assert!(rule.exists());
    s.gitassist(&["uninstall"]);
    assert!(!rule.exists());
    assert!(!s.repo.join(".cursor").exists());
}

#[test]
fn a_users_own_file_at_that_path_is_left_alone() {
    let s = Sandbox::new();
    s.install_hooks();
    s.git_ok(&["init", "-q"]);
    let rule = s.repo.join(".cursor/rules/gitassist.mdc");
    fs::create_dir_all(rule.parent().unwrap()).unwrap();
    fs::write(&rule, "my own rule\n").unwrap();
    fs::write(s.repo.join("a.txt"), "a").unwrap();
    s.git_ok(&["add", "a.txt"]);
    s.git_ok(&["commit", "-q", "-m", "feat: a"]);
    assert_eq!(fs::read_to_string(&rule).unwrap(), "my own rule\n");
}

#[test]
fn protected_branch_commits_warn_by_default_and_block_when_strict() {
    let s = Sandbox::new();
    s.install_hooks();
    s.init_repo();
    s.change("b.txt");
    let o = s.git(&["commit", "-q", "-m", "fix: on main"]);
    assert!(o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("main is protected"));

    s.write_config("[branches]\nprotect_mode = \"block\"\n");
    s.change("c.txt");
    assert!(!s
        .git(&["commit", "-q", "-m", "fix: blocked"])
        .status
        .success());
    s.git_ok(&["switch", "-q", "-c", "feat/work"]);
    s.git_ok(&["commit", "-q", "-m", "fix: on a branch"]);
}

#[test]
fn push_to_protected_or_badly_named_branch_is_blocked_when_strict() {
    let s = Sandbox::new();
    s.install_hooks();
    s.init_repo();
    let bare = s.root.join("remote.git");
    s.git_ok(&["init", "-q", "--bare", bare.to_str().unwrap()]);
    s.git_ok(&["remote", "add", "origin", bare.to_str().unwrap()]);
    s.git_ok(&["push", "-q", "origin", "main"]);
    s.write_config(
        "[branches]\nprotect_mode = \"block\"\nnaming_mode = \"block\"\nprefixes = [\"feat\"]\n",
    );
    s.change("b.txt");
    s.cmd("git")
        .args(["commit", "-q", "-m", "fix: x"])
        .env("GITASSIST_BYPASS", "1")
        .output()
        .unwrap();
    let o = s.git(&["push", "-q", "origin", "main"]);
    assert!(!o.status.success() && String::from_utf8_lossy(&o.stderr).contains("protected"));
    s.git_ok(&["switch", "-q", "-c", "random-name"]);
    assert!(!s
        .git(&["push", "-q", "origin", "random-name"])
        .status
        .success());
    s.git_ok(&["switch", "-q", "-c", "feat/good"]);
    s.git_ok(&["push", "-q", "origin", "feat/good"]);
}

#[test]
fn storage_can_be_measured_and_cleared() {
    let s = Sandbox::new();
    s.install_hooks();
    s.init_repo();
    for i in 0..30 {
        let d = s.ga.join("backups").join(format!("{}", 1000 + i));
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("x"), "x").unwrap();
    }
    fs::write(s.home.join(".zshrc"), "# x\n").unwrap();
    s.gitassist(&["install", "shim"]);
    let sets = fs::read_dir(s.ga.join("backups")).unwrap().count();
    assert!(sets <= 20, "old backup sets are pruned, found {sets}");
}

fn registry(s: &Sandbox) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(s.ga.join("repos-index.json")).unwrap_or("{}".into()))
        .unwrap()
}

#[test]
fn repos_are_recorded_with_remotes_commits_and_pushes() {
    let s = Sandbox::new();
    s.install_hooks();
    s.init_repo();
    let bare = s.root.join("remote.git");
    s.git_ok(&["init", "-q", "--bare", bare.to_str().unwrap()]);
    s.git_ok(&["remote", "add", "origin", bare.to_str().unwrap()]);
    s.change("b.txt");
    s.git_ok(&["commit", "-q", "-m", "fix: b"]);
    s.git_ok(&["push", "-q", "origin", "main"]);
    let reg = registry(&s);
    let rec = &reg[s.repo.to_str().unwrap()];
    assert!(rec["last_commit"].as_u64().unwrap() > 0, "{reg}");
    assert!(rec["last_push"].as_u64().unwrap() > 0, "{reg}");
    assert_eq!(rec["last_push_remote"], "origin");
    assert_eq!(rec["branch"], "main");
    assert_eq!(rec["remotes"][0]["name"], "origin");
}

#[test]
fn gitassist_files_are_private_to_the_user() {
    use std::os::unix::fs::PermissionsExt;
    let s = Sandbox::new();
    s.gitassist(&["on"]);
    s.init_repo();
    for f in [
        "config.toml",
        "state.json",
        "activity.log",
        "repos-index.json",
    ] {
        let mode = fs::metadata(s.ga.join(f))
            .unwrap_or_else(|_| panic!("{f}"))
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "{f} is {mode:o}");
    }
    assert_eq!(
        fs::metadata(&s.ga).unwrap().permissions().mode() & 0o777,
        0o700
    );
}

#[test]
fn turning_on_pausing_resuming_off_and_on_again_all_work() {
    let s = Sandbox::new();
    fs::write(s.home.join(".zshrc"), "# mine\n").unwrap();
    let attributed = |s: &Sandbox, f: &str| {
        s.change(f);
        s.git_ok(&[
            "commit",
            "-q",
            "-m",
            "fix: x\n\nCo-authored-by: Claude <c@x>",
        ]);
        s.last_message().contains("Co-authored-by")
    };
    assert!(s.gitassist(&["on"]).status.success());
    s.init_repo();
    assert!(!attributed(&s, "a1"), "on: cleaned");

    let cfg = s.ga.join("config.toml");
    let text = fs::read_to_string(&cfg).unwrap();
    fs::write(&cfg, text.replace("enabled = true", "enabled = false")).unwrap();
    assert!(attributed(&s, "a2"), "paused: left alone");
    let text = fs::read_to_string(&cfg).unwrap();
    fs::write(&cfg, text.replacen("enabled = false", "enabled = true", 1)).unwrap();
    assert!(!attributed(&s, "a3"), "resumed: cleaned again");

    assert!(s.gitassist(&["uninstall"]).status.success());
    assert!(attributed(&s, "a4"), "off: git is back to normal");
    assert_eq!(
        fs::read_to_string(s.home.join(".zshrc")).unwrap(),
        "# mine\n"
    );

    assert!(s.gitassist(&["on"]).status.success());
    assert!(!attributed(&s, "a5"), "on again: cleaned");
}
