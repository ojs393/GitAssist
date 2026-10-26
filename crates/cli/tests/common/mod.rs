#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// An isolated machine: its own HOME, global git config and GitAssist dir.
/// Nothing here touches the real user's settings.
pub struct Sandbox {
    _tmp: tempfile::TempDir,
    pub root: PathBuf,
    pub home: PathBuf,
    pub ga: PathBuf,
    pub gitconfig: PathBuf,
    pub fake_gh_dir: PathBuf,
    pub repo: PathBuf,
}

pub const CLAUDE_FOOTER: &str =
    "\n\n🤖 Generated with [Claude Code](https://claude.com/claude-code)\n\nCo-Authored-By: Claude <noreply@anthropic.com>";

impl Sandbox {
    pub fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let home = root.join("home");
        // A space in the path, like the real "Application Support" folder.
        let ga = root.join("Git Assist Home");
        let fake_gh_dir = root.join("fakegh");
        let repo = root.join("repo");
        for d in [&home, &ga, &fake_gh_dir, &repo] {
            fs::create_dir_all(d).unwrap();
        }
        let gitconfig = root.join("gitconfig");
        fs::write(
            &gitconfig,
            "[user]\n\tname = Test User\n\temail = test@example.com\n[init]\n\tdefaultBranch = main\n[commit]\n\tgpgsign = false\n",
        )
        .unwrap();
        let s = Self {
            _tmp: tmp,
            root,
            home,
            ga,
            gitconfig,
            fake_gh_dir,
            repo,
        };
        s.write_fake_gh();
        s
    }

    pub fn path_var(&self) -> String {
        let orig = std::env::var("PATH").unwrap_or_default();
        let sep = if cfg!(windows) { ";" } else { ":" };
        format!(
            "{}{sep}{}{sep}{orig}",
            self.ga.join("bin").display(),
            self.fake_gh_dir.display()
        )
    }

    pub fn cmd(&self, program: impl AsRef<std::ffi::OsStr>) -> Command {
        let mut c = Command::new(program);
        c.env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("GITASSIST_HOME", &self.ga)
            .env("GIT_CONFIG_GLOBAL", &self.gitconfig)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("PATH", self.path_var())
            .env("FAKE_GH_LOG", self.root.join("gh.log"))
            .env("SHELL", "/bin/zsh")
            .env_remove("GITASSIST_BYPASS")
            .env_remove("GIT_DIR")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("CLAUDE_CONFIG_DIR")
            .env_remove("CODEX_HOME")
            .env_remove("COPILOT_HOME")
            .current_dir(&self.repo);
        c
    }

    pub fn gitassist(&self, args: &[&str]) -> Output {
        self.cmd(env!("CARGO_BIN_EXE_gitassist"))
            .args(args)
            .output()
            .unwrap()
    }

    pub fn git(&self, args: &[&str]) -> Output {
        self.cmd("git").args(args).output().unwrap()
    }

    pub fn git_ok(&self, args: &[&str]) -> String {
        let o = self.git(args);
        assert!(
            o.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        String::from_utf8_lossy(&o.stdout).into_owned()
    }

    pub fn install_hooks(&self) {
        let o = self.gitassist(&["install", "hooks"]);
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    }

    pub fn init_repo(&self) {
        self.git_ok(&["init", "-q"]);
        fs::write(self.repo.join("a.txt"), "a\n").unwrap();
        self.git_ok(&["add", "."]);
        self.git_ok(&["commit", "-q", "-m", "chore: initial"]);
    }

    pub fn set_github_remote(&self, slug: &str) {
        self.git_ok(&[
            "remote",
            "add",
            "origin",
            &format!("https://github.com/{slug}.git"),
        ]);
    }

    pub fn committed_files(&self) -> String {
        self.git_ok(&["show", "--name-only", "--format=", "HEAD"])
    }

    pub fn write_config(&self, toml: &str) {
        fs::write(self.ga.join("config.toml"), toml).unwrap();
    }

    pub fn change(&self, file: &str) {
        let p = self.repo.join(file);
        if let Some(d) = p.parent() {
            fs::create_dir_all(d).unwrap();
        }
        let prev = fs::read_to_string(&p).unwrap_or_default();
        fs::write(&p, format!("{prev}x\n")).unwrap();
        self.git_ok(&["add", "."]);
    }

    pub fn last_message(&self) -> String {
        self.git_ok(&["log", "-1", "--format=%B"])
            .trim_end()
            .to_string()
    }

    pub fn log(&self) -> String {
        fs::read_to_string(self.ga.join("activity.log")).unwrap_or_default()
    }

    fn write_fake_gh(&self) {
        let script = r#"#!/bin/sh
if [ "$1" = "pr" ] && [ "$2" = "view" ]; then
  echo "old body"
  exit 0
fi
if [ "$1" = "repo" ] && [ "$2" = "view" ]; then
  echo "${FAKE_GH_VISIBILITY:-PUBLIC}"
  exit 0
fi
if [ "$1" = "api" ] && [ "$2" = "user" ]; then
  echo "tester"
  exit 0
fi
printf '%s\n' "$@" > "$FAKE_GH_LOG"
while [ $# -gt 0 ]; do
  if [ "$1" = "--body-file" ]; then cat "$2" > "$FAKE_GH_LOG.body"; fi
  shift
done
exit "${FAKE_GH_EXIT:-0}"
"#;
        let p = self.fake_gh_dir.join("gh");
        fs::write(&p, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    pub fn gh(&self, args: &[&str]) -> Output {
        self.cmd(self.ga.join("bin").join("gh"))
            .args(args)
            .output()
            .unwrap()
    }

    pub fn gh_log(&self) -> (String, String) {
        (
            fs::read_to_string(self.root.join("gh.log")).unwrap_or_default(),
            fs::read_to_string(self.root.join("gh.log.body")).unwrap_or_default(),
        )
    }
}

pub fn exists(p: &Path) -> bool {
    p.exists()
}
