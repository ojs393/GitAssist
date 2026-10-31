mod common;

use std::fs;

use common::Sandbox;
use gitassist_gateway::targets::{
    self, PublishTarget, PushPolicy, RepoTargets, TargetChecks, TargetRole, CheckCommand,
};
use gitassist_gateway::Paths;

#[test]
fn push_all_runs_checks_and_respects_filter_mirror() {
    let s = Sandbox::new();
    s.install_hooks();
    s.init_repo();

    let bare_personal = s.root.join("personal.git");
    let bare_public = s.root.join("public.git");
    s.cmd("git")
        .args(["init", "--bare", bare_personal.to_str().unwrap()])
        .status()
        .unwrap();
    s.cmd("git")
        .args(["init", "--bare", bare_public.to_str().unwrap()])
        .status()
        .unwrap();

    fs::write(s.repo.join("secret.env"), "TOKEN=1\n").unwrap();
    fs::write(s.repo.join("readme.md"), "hello\n").unwrap();
    s.git_ok(&["add", "."]);
    s.git_ok(&["commit", "-m", "add files"]);

    let paths = Paths::from_dirs(s.ga.clone(), s.ga.clone());
    let data = RepoTargets {
        repo_path: s.repo.to_string_lossy().into(),
        default_target: Some("personal".into()),
        targets: vec![
            PublishTarget {
                id: "1".into(),
                label: "personal".into(),
                role: TargetRole::Personal,
                remote_name: "personal".into(),
                url: bare_personal.to_string_lossy().into(),
                enabled: true,
                push_policy: PushPolicy::Manual,
                include_in_push_all: true,
                publish_filter: vec![],
                use_mirror: false,
                tabs: None,
                branches: None,
                checks: TargetChecks {
                    enabled: true,
                    on_fail: gitassist_core::Mode::Block,
                    timeout_secs: 30,
                    commands: vec![CheckCommand {
                        name: "ok".into(),
                        shell: "true".into(),
                        env: Default::default(),
                    }],
                },
            },
            PublishTarget {
                id: "2".into(),
                label: "public".into(),
                role: TargetRole::Public,
                remote_name: "public".into(),
                url: bare_public.to_string_lossy().into(),
                enabled: true,
                push_policy: PushPolicy::Manual,
                include_in_push_all: true,
                publish_filter: vec!["secret.env".into(), "*.env".into()],
                use_mirror: true,
                tabs: None,
                branches: None,
                checks: TargetChecks::default(),
            },
        ],
    };
    targets::save(&paths, &s.repo, &data).unwrap();
    targets::ensure_remotes(&s.repo, &data).unwrap();

    let out = s.gitassist(&["push", "--all"]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "push --all failed: {combined}");

    // Personal bare should have secret.env; public should not.
    let personal_tree = s
        .cmd("git")
        .args([
            "--git-dir",
            bare_personal.to_str().unwrap(),
            "ls-tree",
            "-r",
            "--name-only",
            "HEAD",
        ])
        .output()
        .unwrap();
    let personal_files = String::from_utf8_lossy(&personal_tree.stdout);
    assert!(
        personal_files.contains("secret.env"),
        "personal missing secret: {personal_files}"
    );

    let public_tree = s
        .cmd("git")
        .args([
            "--git-dir",
            bare_public.to_str().unwrap(),
            "ls-tree",
            "-r",
            "--name-only",
            "HEAD",
        ])
        .output()
        .unwrap();
    let public_files = String::from_utf8_lossy(&public_tree.stdout);
    assert!(
        !public_files.contains("secret.env"),
        "public still has secret: {public_files}"
    );
    assert!(public_files.contains("readme.md"), "public missing readme");
}

#[test]
fn failing_check_blocks_all_pushes() {
    let s = Sandbox::new();
    s.install_hooks();
    s.init_repo();

    let bare = s.root.join("remote.git");
    s.cmd("git")
        .args(["init", "--bare", bare.to_str().unwrap()])
        .status()
        .unwrap();

    let paths = Paths::from_dirs(s.ga.clone(), s.ga.clone());
    let data = RepoTargets {
        repo_path: s.repo.to_string_lossy().into(),
        default_target: None,
        targets: vec![PublishTarget {
            id: "1".into(),
            label: "personal".into(),
            role: TargetRole::Personal,
            remote_name: "origin".into(),
            url: bare.to_string_lossy().into(),
            enabled: true,
            push_policy: PushPolicy::Manual,
            include_in_push_all: true,
            publish_filter: vec![],
            use_mirror: false,
            tabs: None,
            branches: None,
            checks: TargetChecks {
                enabled: true,
                on_fail: gitassist_core::Mode::Block,
                timeout_secs: 30,
                commands: vec![CheckCommand {
                    name: "fail".into(),
                    shell: "false".into(),
                    env: Default::default(),
                }],
            },
        }],
    };
    targets::save(&paths, &s.repo, &data).unwrap();
    targets::ensure_remotes(&s.repo, &data).unwrap();

    let out = s.gitassist(&["push", "personal"]);
    assert!(!out.status.success(), "push should fail when check fails");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("check") || combined.contains("fail") || combined.contains("preflight"),
        "unexpected output: {combined}"
    );
}
