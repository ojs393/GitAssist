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
fn commit_from_message_file() {
    let s = setup();
    s.change("b.txt");
    let f = s.root.join("msg.txt");
    fs::write(&f, format!("fix: from file{CLAUDE_FOOTER}\n")).unwrap();
    s.git_ok(&["commit", "-q", "-F", f.to_str().unwrap()]);
    assert_eq!(s.last_message(), "fix: from file");
}

#[test]
fn amend_cleans_the_amended_message() {
    let s = setup();
    s.change("b.txt");
    s.git_ok(&["commit", "-q", "-m", "fix: first"]);
    s.git_ok(&[
        "commit",
        "-q",
        "--amend",
        "-m",
        &format!("fix: amended{CLAUDE_FOOTER}"),
    ]);
    assert_eq!(s.last_message(), "fix: amended");
    assert_eq!(s.git_ok(&["rev-list", "--count", "HEAD"]).trim(), "2");
}

#[test]
fn cherry_pick_and_revert_keep_working() {
    let s = setup();
    s.git_ok(&["checkout", "-q", "-b", "side"]);
    s.change("side.txt");
    s.git_ok(&["commit", "-q", "-m", &format!("feat: side{CLAUDE_FOOTER}")]);
    let sha = s.git_ok(&["rev-parse", "HEAD"]).trim().to_string();
    s.git_ok(&["checkout", "-q", "main"]);
    s.git_ok(&["cherry-pick", &sha]);
    assert_eq!(s.last_message(), "feat: side");
    s.git_ok(&["revert", "--no-edit", "HEAD"]);
    assert!(s.last_message().starts_with("Revert \"feat: side\""));
}

#[test]
fn rebase_rewords_go_through_the_hooks() {
    let s = setup();
    s.change("a2.txt");
    s.cmd("git")
        .args(["commit", "-q", "-m", &format!("fix: old{CLAUDE_FOOTER}")])
        .env("GITASSIST_BYPASS", "1")
        .output()
        .unwrap();
    assert!(s.last_message().contains("Co-Authored-By"));
    // Reword the last commit non-interactively: the hooks clean it up.
    let o = s
        .cmd("git")
        .args(["rebase", "-q", "-i", "HEAD~1"])
        .env("GIT_SEQUENCE_EDITOR", "sed -i.bak 's/^pick/reword/'")
        .env("GIT_EDITOR", "true")
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(s.last_message(), "fix: old");
}

#[test]
fn worktrees_are_covered() {
    let s = setup();
    let wt = s.root.join("wt");
    s.git_ok(&[
        "worktree",
        "add",
        "-q",
        wt.to_str().unwrap(),
        "-b",
        "wt-branch",
    ]);
    fs::write(wt.join("w.txt"), "w\n").unwrap();
    let run = |args: &[&str]| s.cmd("git").current_dir(&wt).args(args).output().unwrap();
    assert!(run(&["add", "."]).status.success());
    assert!(run(&[
        "commit",
        "-q",
        "-m",
        &format!("feat: in worktree{CLAUDE_FOOTER}")
    ])
    .status
    .success());
    let msg = String::from_utf8(run(&["log", "-1", "--format=%B"]).stdout).unwrap();
    assert_eq!(msg.trim_end(), "feat: in worktree");
}

#[test]
fn commit_template_comments_do_not_leak() {
    let s = setup();
    let tpl = s.root.join("tpl.txt");
    fs::write(&tpl, "# Write why, not what\n").unwrap();
    s.git_ok(&["config", "commit.template", tpl.to_str().unwrap()]);
    s.change("b.txt");
    let o = s
        .cmd("git")
        .args(["commit", "-q", "-e", "-m", "fix: with template"])
        .env("GIT_EDITOR", "true")
        .output()
        .unwrap();
    assert!(o.status.success());
    assert_eq!(s.last_message(), "fix: with template");
}

#[test]
fn unicode_and_windows_line_endings() {
    let s = setup();
    s.change("b.txt");
    s.git_ok(&[
        "commit",
        "-q",
        "-m",
        "fix: ünïcödé 漢字 🎉\r\n\r\nBody line\r\nCo-authored-by: Claude <c@x>\r\n",
    ]);
    assert_eq!(s.last_message(), "fix: ünïcödé 漢字 🎉\n\nBody line");
}

#[test]
fn custom_comment_char_is_respected() {
    let s = setup();
    s.git_ok(&["config", "core.commentChar", ";"]);
    s.change("b.txt");
    let o = s
        .cmd("git")
        .args([
            "commit",
            "-q",
            "-e",
            "-m",
            "fix: semi\n\n# not a comment here\nCo-authored-by: Claude <c@x>",
        ])
        .env("GIT_EDITOR", "true")
        .output()
        .unwrap();
    assert!(o.status.success());
    assert_eq!(s.last_message(), "fix: semi\n\n# not a comment here");
}

#[test]
fn empty_message_still_aborts_normally() {
    let s = setup();
    s.change("b.txt");
    let o = s.git(&["commit", "-q", "-m", ""]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("empty commit message"));
}

#[test]
fn message_that_is_only_attribution_becomes_empty_and_aborts() {
    let s = setup();
    s.change("b.txt");
    let o = s.git(&["commit", "-q", "-m", "Co-authored-by: Claude <c@x>"]);
    assert!(
        !o.status.success(),
        "git refuses the now-empty message rather than committing junk"
    );
}

#[test]
fn huge_message_stays_fast() {
    let s = setup();
    s.change("b.txt");
    let body =
        "a line of text that goes past the wrap width for sure, yes it does indeed\n".repeat(1500);
    // Passed as a file: Linux caps a single argument at 128 KiB.
    let file = s.root.join("huge.txt");
    fs::write(&file, format!("fix: huge\n\n{body}")).unwrap();
    let start = std::time::Instant::now();
    s.git_ok(&["commit", "-q", "-F", file.to_str().unwrap()]);
    let with_hooks = start.elapsed();
    // Compared with a plain commit on the same machine, so a busy CI box can't fail it.
    s.change("c.txt");
    let start = std::time::Instant::now();
    s.git_ok(&[
        "-c",
        "core.hooksPath=/dev/null",
        "commit",
        "-q",
        "-F",
        file.to_str().unwrap(),
    ]);
    let plain = start.elapsed();
    assert!(
        with_hooks < plain * 20 + std::time::Duration::from_secs(2),
        "{with_hooks:?} vs plain {plain:?}"
    );
    assert!(s.last_message().starts_with("fix: huge"));
    let wrapped = s.git_ok(&["log", "-2", "--skip=1", "--format=%B"]);
    assert!(
        wrapped.lines().all(|l| l.chars().count() <= 72),
        "processed, not skipped by the time limit"
    );
}

#[test]
fn parallel_commits_in_many_repos() {
    let sandboxes: Vec<Sandbox> = (0..4).map(|_| setup()).collect();
    std::thread::scope(|scope| {
        for s in &sandboxes {
            scope.spawn(move || {
                for i in 0..5 {
                    s.change(&format!("f{i}.txt"));
                    s.git_ok(&["commit", "-q", "-m", &format!("fix: {i}{CLAUDE_FOOTER}")]);
                }
            });
        }
    });
    for s in &sandboxes {
        assert_eq!(s.last_message(), "fix: 4");
        assert_eq!(s.git_ok(&["rev-list", "--count", "HEAD"]).trim(), "6");
    }
}

#[test]
fn invalid_regex_and_unknown_values_fail_open() {
    let s = setup();
    s.write_config(
        "[global.trailers]\nstrip_patterns = [\"(unclosed\", \"^Co-authored-by:.*$\"]\n",
    );
    s.change("b.txt");
    s.git_ok(&["commit", "-q", "-m", "fix: ok\n\nCo-authored-by: X <x@x>"]);
    assert_eq!(
        s.last_message(),
        "fix: ok",
        "bad pattern skipped, good one applied"
    );

    s.write_config("[tabs.commit]\nsubject_max = { value = 10, mode = \"explode\" }\n");
    s.change("c.txt");
    s.git_ok(&["commit", "-q", "-m", "fix: a subject longer than ten"]);
    assert!(
        s.log().contains("\"outcome\":\"error\""),
        "bad config is logged, commit goes through"
    );
}

#[test]
fn project_override_file_is_ignored_unless_allowed() {
    let s = setup();
    fs::write(
        s.repo.join(".gitassist.toml"),
        "[tabs.commit]\nsubject_max = { value = 5, mode = \"block\" }\n",
    )
    .unwrap();
    s.change("b.txt");
    s.git_ok(&["commit", "-q", "-m", "fix: allowed by default"]);
    s.write_config("allow_project_overrides = true\n");
    s.change("c.txt");
    assert!(!s
        .git(&["commit", "-q", "-m", "fix: now blocked"])
        .status
        .success());
}

#[test]
fn total_limit_of_500_blocks_long_commits() {
    let s = setup();
    s.write_config("[tabs.commit]\ntotal_max = { value = 500, mode = \"block\" }\n");
    s.change("b.txt");
    let o = s.git(&[
        "commit",
        "-q",
        "-m",
        &format!("fix: long\n\n{}", "word ".repeat(120)),
    ]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("limit is 500"));
}
