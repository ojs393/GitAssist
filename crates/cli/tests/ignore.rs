#![cfg(unix)]

mod common;

use std::fs;

use common::Sandbox;

fn setup(github_user: &str) -> Sandbox {
    let s = Sandbox::new();
    s.install_hooks();
    s.init_repo();
    s.write_config(&format!("[ignore]\ngithub_user = \"{github_user}\"\n"));
    s
}

fn stage(s: &Sandbox, files: &[&str]) {
    for f in files {
        let p = s.repo.join(f);
        if let Some(d) = p.parent() {
            fs::create_dir_all(d).unwrap();
        }
        fs::write(p, "x\n").unwrap();
    }
    s.git_ok(&["add", "-f", "."]);
}

#[test]
fn secrets_are_left_out_of_public_repo_commits() {
    let s = setup("me");
    s.set_github_remote("me/app");
    stage(
        &s,
        &[".env", ".env.example", "src/main.rs", "certs/dev.pem"],
    );
    let o = s.git(&["commit", "-q", "-m", "feat: add app"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let files = s.committed_files();
    assert!(
        files.contains("src/main.rs") && files.contains(".env.example"),
        "{files}"
    );
    assert!(
        !files.contains(".env\n") && !files.contains("dev.pem"),
        "{files}"
    );
    assert!(s.repo.join(".env").exists(), "file stays on disk");
    let exclude = fs::read_to_string(s.repo.join(".git/info/exclude")).unwrap();
    assert!(exclude.contains("# >>> gitassist ignore >>>") && exclude.contains(".env"));
    assert!(String::from_utf8_lossy(&o.stderr).contains("personal public repo"));
}

#[test]
fn private_repos_use_the_private_list() {
    let s = setup("me");
    s.set_github_remote("me/app");
    stage(&s, &[".env"]);
    let o = s
        .cmd("git")
        .args(["commit", "-q", "-m", "chore: env"])
        .env("FAKE_GH_VISIBILITY", "PRIVATE")
        .output()
        .unwrap();
    assert!(o.status.success());
    assert!(
        s.committed_files().contains(".env"),
        "private repos keep .env unless you list it"
    );
}

#[test]
fn organization_repos_get_the_organization_list() {
    let s = setup("me");
    s.set_github_remote("acme/tool");
    stage(&s, &["CLAUDE.local.md", "README.md"]);
    let o = s
        .cmd("git")
        .args(["commit", "-q", "-m", "docs: notes"])
        .env("FAKE_GH_VISIBILITY", "PRIVATE")
        .output()
        .unwrap();
    assert!(o.status.success());
    let files = s.committed_files();
    assert!(
        files.contains("README.md") && !files.contains("CLAUDE.local.md"),
        "{files}"
    );
}

#[test]
fn local_repos_only_get_the_every_repo_list() {
    let s = setup("me");
    stage(&s, &[".DS_Store", "sub/.DS_Store", ".env", "a.rs"]);
    s.git_ok(&["commit", "-q", "-m", "feat: local"]);
    let files = s.committed_files();
    assert!(
        !files.contains(".DS_Store") && files.contains(".env") && files.contains("a.rs"),
        "{files}"
    );
}

#[test]
fn block_mode_stops_the_commit() {
    let s = setup("me");
    s.write_config("[ignore]\ngithub_user = \"me\"\nmode = \"block\"\n");
    s.set_github_remote("me/app");
    stage(&s, &[".env", "a.rs"]);
    let o = s.git(&["commit", "-q", "-m", "feat: x"]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains(".env must not be committed"));
}

#[test]
fn already_tracked_files_stay_tracked_but_new_changes_are_held_back() {
    let s = setup("me");
    s.set_github_remote("me/app");
    stage(&s, &[".env"]);
    s.cmd("git")
        .args(["commit", "-q", "-m", "chore: old env"])
        .env("GITASSIST_BYPASS", "1")
        .output()
        .unwrap();
    fs::write(s.repo.join(".env"), "SECRET=2\n").unwrap();
    stage(&s, &["b.rs"]);
    s.git_ok(&["add", "-f", ".env"]);
    s.git_ok(&["commit", "-q", "-m", "feat: b"]);
    assert!(
        s.git_ok(&["ls-files"]).contains(".env"),
        "not deleted from the repo"
    );
    assert!(!s.committed_files().contains(".env"));
    assert_eq!(s.git_ok(&["show", "HEAD:.env"]), "x\n");
}

#[test]
fn unknown_visibility_is_treated_as_public() {
    let s = setup("me");
    s.write_config("[ignore]\ngithub_user = \"me\"\nlookup_visibility = false\n");
    s.set_github_remote("me/app");
    stage(&s, &[".env", "a.rs"]);
    s.git_ok(&["commit", "-q", "-m", "feat: a"]);
    assert!(!s.committed_files().contains(".env"));
}

#[test]
fn visibility_is_cached_after_first_lookup() {
    let s = setup("me");
    s.set_github_remote("me/app");
    stage(&s, &["a.rs"]);
    s.git_ok(&["commit", "-q", "-m", "feat: a"]);
    let cache = fs::read_to_string(s.ga.join("repos.json")).unwrap();
    assert!(
        cache.contains("github.com/me/app") && cache.contains("public"),
        "{cache}"
    );
}

#[test]
fn push_is_blocked_when_history_contains_an_ignored_file() {
    let s = setup("me");
    let bare = s.root.join("remote.git");
    s.git_ok(&["init", "-q", "--bare", bare.to_str().unwrap()]);
    s.set_github_remote("me/app");
    s.git_ok(&["config", "remote.origin.pushurl", bare.to_str().unwrap()]);
    stage(&s, &[".env"]);
    s.cmd("git")
        .args(["commit", "-q", "-m", "chore: env"])
        .env("GITASSIST_BYPASS", "1")
        .output()
        .unwrap();
    let o = s.git(&["push", "-q", "origin", "main"]);
    assert!(!o.status.success());
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(
        err.contains("push blocked") && err.contains(".env"),
        "{err}"
    );
}

#[test]
fn ignore_can_be_turned_off() {
    let s = setup("me");
    s.write_config("[ignore]\nenabled = false\n");
    s.set_github_remote("me/app");
    stage(&s, &[".env"]);
    s.git_ok(&["commit", "-q", "-m", "chore: env"]);
    assert!(s.committed_files().contains(".env"));
}

#[test]
fn global_excludes_file_is_set_and_restored() {
    let s = Sandbox::new();
    s.install_hooks();
    let path = s.git_ok(&["config", "--global", "core.excludesFile"]);
    assert!(path.trim().ends_with("global-ignore"), "{path}");
    assert!(fs::read_to_string(path.trim())
        .unwrap()
        .contains(".DS_Store"));
    s.gitassist(&["uninstall"]);
    assert!(!s
        .git(&["config", "--global", "core.excludesFile"])
        .status
        .success());
}

#[test]
fn users_own_excludes_file_is_never_replaced() {
    let s = Sandbox::new();
    s.git_ok(&["config", "--global", "core.excludesFile", "/my/own/ignore"]);
    s.install_hooks();
    s.gitassist(&["uninstall"]);
    assert_eq!(
        s.git_ok(&["config", "--global", "core.excludesFile"])
            .trim(),
        "/my/own/ignore"
    );
}
