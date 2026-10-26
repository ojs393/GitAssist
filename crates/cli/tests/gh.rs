#![cfg(unix)]

mod common;

use common::Sandbox;

fn setup() -> Sandbox {
    let s = Sandbox::new();
    s.install_hooks();
    let o = s.gitassist(&["install", "shim"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    s.init_repo();
    s
}

#[test]
fn pr_body_is_rewritten_through_body_file() {
    let s = setup();
    let o = s.gh(&[
        "pr",
        "create",
        "--title",
        "Add  parser",
        "--body",
        "Adds it.\n\nCo-authored-by: Claude <c@x>",
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let (args, body) = s.gh_log();
    assert!(args.contains("--body-file"), "{args}");
    assert!(args.contains("--title\nAdd parser"), "{args}");
    assert!(!args.contains("--body\n"), "{args}");
    assert_eq!(body, "Adds it.");
}

#[test]
fn unrelated_commands_pass_through_exactly() {
    let s = setup();
    assert!(s.gh(&["release", "list", "--limit", "1"]).status.success());
    assert_eq!(s.gh_log().0, "release\nlist\n--limit\n1\n");
}

#[test]
fn fill_passes_through() {
    let s = setup();
    s.gh(&["pr", "create", "--fill"]);
    assert_eq!(s.gh_log().0, "pr\ncreate\n--fill\n");
}

#[test]
fn clean_text_passes_through_unchanged() {
    let s = setup();
    s.gh(&["issue", "comment", "3", "--body", "Looks good"]);
    assert_eq!(s.gh_log().0, "issue\ncomment\n3\n--body\nLooks good\n");
}

#[test]
fn blocked_pr_never_reaches_gh() {
    let s = setup();
    s.write_config("[tabs.pr_create]\nsubject_max = { value = 10, mode = \"block\" }\n");
    let o = s.gh(&[
        "pr",
        "create",
        "--title",
        "A very long title",
        "--body",
        "x",
    ]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("GitAssist [pr_create] title is 17 chars"));
    assert_eq!(s.gh_log().0, "");
}

#[test]
fn pr_update_growth_uses_current_body() {
    let s = setup();
    s.write_config("[tabs.pr_update]\ngrowth_max = { value = 5, mode = \"block\" }\n");
    let o = s.gh(&[
        "pr",
        "edit",
        "12",
        "--body",
        "old body and a lot of new text",
    ]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("grew by"));
}

#[test]
fn review_reply_rules_apply() {
    let s = setup();
    s.gh(&[
        "pr",
        "review",
        "-r",
        "-b",
        "Needs work\n\n🤖 Generated with [Claude Code](https://claude.com)",
    ]);
    let (args, body) = s.gh_log();
    assert!(args.starts_with("pr\nreview\n-r\n"), "{args}");
    assert_eq!(body, "Needs work");
}

#[test]
fn every_flag_spelling_is_understood() {
    let s = setup();
    let attribution = "Body\n\nCo-authored-by: Claude <c@x>";
    for args in [
        vec![
            "pr",
            "create",
            "--title=T",
            &format!("--body={attribution}"),
        ],
        vec!["pr", "create", "-tT", "-b", attribution],
        vec!["issue", "edit", "7", "-t", "T", "--body", attribution],
        vec![
            "pr",
            "merge",
            "3",
            "--squash",
            "--subject",
            "T",
            "-b",
            attribution,
        ],
        vec!["pr", "comment", "3", "-b", attribution, "--", "extra"],
    ] {
        let _ = std::fs::remove_file(s.root.join("gh.log.body"));
        let o = s.gh(&args.iter().map(|a| a.as_ref()).collect::<Vec<&str>>());
        assert!(
            o.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        let (logged, body) = s.gh_log();
        assert_eq!(body, "Body", "{args:?} -> {logged}");
        assert!(!logged.contains("Co-authored-by"), "{args:?} -> {logged}");
    }
    let (logged, _) = s.gh_log();
    assert!(
        logged.ends_with("--\nextra\n"),
        "text after -- is kept last: {logged}"
    );
}

#[test]
fn body_from_stdin_is_handled() {
    use std::io::Write;
    let s = setup();
    let mut child = s
        .cmd(s.ga.join("bin").join("gh"))
        .args(["pr", "create", "--title", "T", "--body-file", "-"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"From stdin\n\n\xf0\x9f\xa4\x96 Generated with Claude Code")
        .unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(s.gh_log().1, "From stdin");
}

#[test]
fn exit_codes_are_passed_through() {
    let s = setup();
    let o = s
        .cmd(s.ga.join("bin").join("gh"))
        .args(["release", "list"])
        .env("FAKE_GH_EXIT", "3")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(3));
    let o = s
        .cmd(s.ga.join("bin").join("gh"))
        .args([
            "pr",
            "create",
            "-t",
            "T",
            "-b",
            "Co-authored-by: a <b>\n\nx",
        ])
        .env("FAKE_GH_EXIT", "4")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(4));
}

#[test]
fn missing_real_gh_is_reported_not_looped() {
    let s = setup();
    let o = s
        .cmd(s.ga.join("bin").join("gh"))
        .arg("--version")
        .env("PATH", s.ga.join("bin"))
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(127));
    assert!(String::from_utf8_lossy(&o.stderr).contains("real `gh` was not found"));
}

#[test]
fn gh_activity_is_logged() {
    let s = setup();
    s.gh(&[
        "pr",
        "create",
        "-t",
        "T",
        "-b",
        "x\n\nCo-authored-by: a <b>",
    ]);
    let log = s.log();
    let line = log
        .lines()
        .rfind(|l| l.contains("\"source\":\"gh\""))
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(v["tab"], "pr_create");
    assert_eq!(v["outcome"], "fixed");
    assert!(
        v.get("body").is_none(),
        "message text is not logged by default"
    );
}

#[test]
fn shim_profile_block_installed_once() {
    let s = setup();
    s.gitassist(&["install", "shim"]);
    let zshrc = std::fs::read_to_string(s.home.join(".zshrc")).unwrap();
    assert_eq!(zshrc.matches("# >>> gitassist >>>").count(), 1);
    s.gitassist(&["uninstall"]);
    let zshrc = std::fs::read_to_string(s.home.join(".zshrc")).unwrap();
    assert!(!zshrc.contains("gitassist"));
}
