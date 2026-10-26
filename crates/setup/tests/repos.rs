use std::path::Path;

use gitassist_setup::repos::in_protected_folder;

#[test]
fn background_checks_stay_out_of_privacy_protected_folders() {
    let home = Path::new("/home/me");
    for p in [
        "Desktop/app",
        "Documents/x/y",
        "Downloads/repo",
        "Library/Mobile Documents/com~apple~CloudDocs/r",
    ] {
        assert!(in_protected_folder(&home.join(p), home), "{p}");
    }
    for p in ["code/app", "repos/x", "Desktop-old/app"] {
        assert!(!in_protected_folder(&home.join(p), home), "{p}");
    }
}

#[test]
fn repo_details_are_read_without_running_git() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("proj");
    let git = repo.join(".git");
    std::fs::create_dir_all(git.join("logs")).unwrap();
    std::fs::create_dir_all(git.join("refs/remotes/origin")).unwrap();
    std::fs::write(git.join("HEAD"), "ref: refs/heads/feat/x\n").unwrap();
    std::fs::write(git.join("logs/HEAD"), "x\n").unwrap();
    std::fs::write(git.join("refs/remotes/origin/main"), "abc\n").unwrap();
    std::fs::write(
        git.join("config"),
        "[core]\n\thooksPath = .husky/_\n[remote \"origin\"]\n\turl = git@github.com:acme/proj.git\n[remote \"fork\"]\n\turl = https://github.com/me/proj\n",
    )
    .unwrap();
    let r = gitassist_setup::repos::read_record(&repo).unwrap();
    assert_eq!(r.branch.as_deref(), Some("feat/x"));
    assert_eq!(r.remotes.len(), 2);
    assert_eq!(r.remotes[0].url, "git@github.com:acme/proj.git");
    assert_eq!(r.local_hooks_path.as_deref(), Some(".husky/_"));
    assert!(r.last_commit.is_some() && r.last_sync.is_some());
}
