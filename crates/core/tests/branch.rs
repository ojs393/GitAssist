use gitassist_core::branch::{check_commit, check_push, instructions, name_ok};
use gitassist_core::config::{BranchConfig, Mode};

fn team() -> BranchConfig {
    BranchConfig {
        prefixes: vec!["feat".into(), "fix".into()],
        ..Default::default()
    }
}

#[test]
fn commits_to_protected_branches_are_flagged() {
    let cfg = BranchConfig::default();
    let f = check_commit(&cfg, "main").unwrap();
    assert_eq!(
        f.mode,
        Mode::Warn,
        "warns by default, never blocks out of the box"
    );
    assert!(f.message.contains("git switch -c feat/"));
    assert!(check_commit(&cfg, "feat/login").is_none());
    let off = BranchConfig {
        enabled: false,
        ..Default::default()
    };
    assert!(check_commit(&off, "main").is_none());
    let fix_mode = BranchConfig {
        protect_mode: Mode::Fix,
        ..Default::default()
    };
    assert_eq!(
        check_commit(&fix_mode, "main").unwrap().mode,
        Mode::Block,
        "can't be fixed, so it blocks"
    );
}

#[test]
fn branch_names_follow_prefixes() {
    let cfg = team();
    assert!(name_ok(&cfg, "feat/login-page"));
    assert!(name_ok(&cfg, "fix/issue-12/retry"));
    assert!(
        name_ok(&cfg, "main"),
        "protected and base branches are always fine"
    );
    assert!(!name_ok(&cfg, "login-page"));
    assert!(!name_ok(&cfg, "chore/x"));
    assert!(!name_ok(&cfg, "feat/"));
    assert!(!name_ok(&cfg, "feat/has space"));
    assert!(
        name_ok(&BranchConfig::default(), "anything-goes"),
        "no prefixes means any name"
    );
}

#[test]
fn pushes_are_checked_for_protection_and_names() {
    let cfg = team();
    assert_eq!(
        check_push(&cfg, "refs/heads/main", false).len(),
        1,
        "push straight to main"
    );
    assert!(
        check_push(&cfg, "refs/heads/main", true).is_empty(),
        "creating main in a new repo is fine"
    );
    assert_eq!(
        check_push(&cfg, "refs/heads/random", true).len(),
        1,
        "new badly named branch"
    );
    assert!(
        check_push(&cfg, "refs/heads/random", false).is_empty(),
        "existing branches are left alone"
    );
    assert!(
        check_push(&cfg, "refs/tags/v1.0", true).is_empty(),
        "tags are not branches"
    );
}

#[test]
fn instructions_describe_the_workflow() {
    let text = instructions(&team());
    assert!(text.contains("Start new work from `main`"));
    assert!(text.contains("`main`, `master`"));
    assert!(text.contains("feat, fix"));
    assert!(text.contains("New ideas:"));
    assert!(instructions(&BranchConfig {
        enabled: false,
        ..Default::default()
    })
    .is_empty());
}
