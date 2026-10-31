//! Shared publish pipeline for CLI, UI, and hooks.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use gitassist_core::config::Mode;
use gitassist_core::scope::Matcher;
use gitassist_core::{effective, process, Config, Input, Tab};
use serde::{Deserialize, Serialize};

use crate::git;
use crate::log::{self, Entry};
use crate::paths::Paths;
use crate::targets::{
    self, effective_config, find_by_label, for_push_all, PublishTarget, RepoTargets, TargetChecks,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetResult {
    pub label: String,
    pub remote: String,
    pub ok: bool,
    pub message: String,
    pub pushed_sha: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishReport {
    pub ok: bool,
    pub results: Vec<TargetResult>,
    /// Remotes that received a push before a later failure (drift).
    pub drift: Vec<String>,
}

pub enum Selection<'a> {
    Label(&'a str),
    All,
    Remotes(Vec<&'a str>),
}

fn resolve<'a>(data: &'a RepoTargets, sel: Selection<'a>) -> Result<Vec<&'a PublishTarget>> {
    match sel {
        Selection::All => {
            let v = for_push_all(data);
            if v.is_empty() {
                bail!("no enabled targets marked include-in-push-all");
            }
            Ok(v)
        }
        Selection::Label(label) => {
            let t = find_by_label(data, label)
                .with_context(|| format!("no enabled target labeled `{label}`"))?;
            Ok(vec![t])
        }
        Selection::Remotes(names) => {
            let mut out = Vec::new();
            for n in names {
                if let Some(t) = targets::find_by_remote(data, n) {
                    out.push(t);
                }
            }
            if out.is_empty() {
                bail!("no configured targets match those remotes");
            }
            Ok(out)
        }
    }
}

/// Run a target's pre-push checks (used by the pipeline and the pre-push hook).
pub fn preflight_target_checks(repo: &Path, target: &PublishTarget) -> Result<()> {
    run_checks(repo, &target.checks)
}

/// Run checks for one target. Returns Err with message if blocked.
fn run_checks(repo: &Path, checks: &TargetChecks) -> Result<()> {
    if !checks.enabled || checks.commands.is_empty() {
        return Ok(());
    }
    let timeout = Duration::from_secs(checks.timeout_secs.max(1));
    for cmd in &checks.commands {
        if cmd.shell.trim().is_empty() {
            continue;
        }
        let name = if cmd.name.is_empty() {
            cmd.shell.clone()
        } else {
            cmd.name.clone()
        };
        let shell = cmd.shell.clone();
        let env_map = cmd.env.clone();
        let repo = repo.to_path_buf();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut c = if cfg!(windows) {
                let mut c = Command::new("cmd");
                c.arg("/C").arg(&shell);
                c
            } else {
                let mut c = Command::new("sh");
                c.arg("-c").arg(&shell);
                c
            };
            c.current_dir(&repo)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            for (k, v) in &env_map {
                c.env(k, v);
            }
            let _ = tx.send(c.output());
        });
        let out = match rx.recv_timeout(timeout) {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => bail!("check `{name}`: {e}"),
            Err(_) => {
                let msg = format!("check `{name}` timed out after {}s", checks.timeout_secs);
                if checks.on_fail == Mode::Warn {
                    eprintln!("GitAssist: {msg} (warn)");
                    continue;
                }
                bail!("{msg}");
            }
        };
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            let detail = if !stderr.trim().is_empty() {
                stderr.trim()
            } else {
                stdout.trim()
            };
            let msg = format!(
                "check `{name}` failed (exit {:?}): {detail}",
                out.status.code()
            );
            if checks.on_fail == Mode::Warn {
                eprintln!("GitAssist: {msg} (warn)");
                continue;
            }
            bail!("{msg}");
        }
    }
    Ok(())
}

fn preflight_messages(repo: &Path, cfg: &Config, remote: &str) -> Result<()> {
    // Compare local HEAD to remote-tracking if present; else just HEAD.
    let head = git::git_in(Some(repo), &["rev-parse", "HEAD"])?
        .trim()
        .to_string();
    let range_end = head.clone();
    let remote_sha = git::git_in(
        Some(repo),
        &[
            "rev-parse",
            "--verify",
            &format!("refs/remotes/{remote}/HEAD"),
        ],
    )
    .ok()
    .or_else(|| {
        let branch = git::git_in(Some(repo), &["rev-parse", "--abbrev-ref", "HEAD"]).ok()?;
        git::git_in(
            Some(repo),
            &[
                "rev-parse",
                "--verify",
                &format!("refs/remotes/{remote}/{}", branch.trim()),
            ],
        )
        .ok()
    });
    let range = match remote_sha {
        Some(ref r) => format!("{}..{}", r.trim(), range_end),
        None => range_end,
    };
    let log =
        git::git_in(Some(repo), &["log", "--format=%H%x00%B%x00", &range]).unwrap_or_default();
    if log.trim().is_empty() && remote_sha.is_some() {
        return Ok(()); // nothing new
    }
    // If no remote tracking, check the tip commit only.
    let commits: Vec<(String, String)> = if log.trim().is_empty() {
        let msg = git::git_in(Some(repo), &["log", "-1", "--format=%B"])?;
        vec![(head, msg)]
    } else {
        log.split('\0')
            .collect::<Vec<_>>()
            .chunks(2)
            .filter_map(|c| {
                if c.len() < 2 {
                    return None;
                }
                let sha = c[0].trim();
                if sha.is_empty() {
                    return None;
                }
                Some((sha.to_string(), c[1].to_string()))
            })
            .collect()
    };
    let mut blocked = Vec::new();
    for (sha, message) in commits {
        for tab in [Tab::Commit, Tab::DocsCommit, Tab::Merge] {
            let rules = effective(cfg, tab, None);
            if !rules.enabled {
                continue;
            }
            let out = process(
                &Input {
                    body: Some(&message),
                    ..Default::default()
                },
                &rules,
            );
            for f in out.findings.iter().filter(|f| f.mode == Mode::Block) {
                blocked.push(format!(
                    "  {}… {}\n    {}",
                    &sha[..sha.len().min(10)],
                    message.lines().next().unwrap_or(""),
                    f.message
                ));
            }
            break; // one tab classification is enough for preflight lite
        }
    }
    if !blocked.is_empty() {
        bail!(
            "commits break message rules for this target:\n{}",
            blocked.join("\n")
        );
    }
    Ok(())
}

fn preflight_branches(cfg: &Config, branch: &str) -> Result<()> {
    if let Some(f) = gitassist_core::branch::check_commit(&cfg.branches, branch) {
        if f.mode == Mode::Block {
            bail!("{}", f.message);
        }
    }
    Ok(())
}

/// Build or update a cleaned mirror tip for a filtered target.
pub fn build_mirror(repo: &Path, target: &PublishTarget, branch: &str) -> Result<String> {
    let mirror_ref = format!("refs/gitassist/mirror/{}/{}", target.label, branch);
    let head = git::git_in(Some(repo), &["rev-parse", "HEAD"])?
        .trim()
        .to_string();
    let matcher = Matcher::new(&target.publish_filter);

    // List files at HEAD and drop filtered ones into a temp index.
    let files = git::git_in(Some(repo), &["ls-tree", "-r", "--name-only", "HEAD"])?;
    let keep: Vec<&str> = files
        .lines()
        .filter(|p| !p.is_empty() && !matcher.is_ignored(p))
        .collect();
    if keep.is_empty() {
        bail!(
            "publish filter for `{}` would leave an empty tree",
            target.label
        );
    }

    let git_dir = git::git_in(Some(repo), &["rev-parse", "--git-dir"])?
        .trim()
        .to_string();
    let git_dir = Path::new(repo).join(&git_dir);
    let index = git_dir.join(format!(
        "gitassist-mirror-{}-{}.index",
        target.label.replace('/', "_"),
        branch.replace('/', "_")
    ));
    let _ = std::fs::remove_file(&index);

    // Private index so the user's index is untouched.
    let read = Command::new("git")
        .current_dir(repo)
        .env("GIT_INDEX_FILE", &index)
        .args(["read-tree", "HEAD"])
        .output()?;
    if !read.status.success() {
        let _ = std::fs::remove_file(&index);
        bail!(
            "read-tree failed: {}",
            String::from_utf8_lossy(&read.stderr)
        );
    }
    for path in files
        .lines()
        .filter(|p| !p.is_empty() && matcher.is_ignored(p))
    {
        let _ = Command::new("git")
            .current_dir(repo)
            .env("GIT_INDEX_FILE", &index)
            .args(["rm", "--cached", "-f", "--", path])
            .output();
    }
    let tree_out = Command::new("git")
        .current_dir(repo)
        .env("GIT_INDEX_FILE", &index)
        .args(["write-tree"])
        .output()?;
    let _ = std::fs::remove_file(&index);
    if !tree_out.status.success() {
        bail!(
            "write-tree failed: {}",
            String::from_utf8_lossy(&tree_out.stderr)
        );
    }
    let tree = String::from_utf8_lossy(&tree_out.stdout).trim().to_string();

    let parent = git::git_in(Some(repo), &["rev-parse", "-q", "--verify", &mirror_ref]).ok();
    let msg = format!("gitassist publish {} from {head}", target.label);
    let mut commit_args = vec!["commit-tree".to_string(), tree.clone(), "-m".into(), msg];
    if let Some(p) = parent {
        commit_args.push("-p".into());
        commit_args.push(p.trim().into());
    }
    let commit_refs: Vec<&str> = commit_args.iter().map(|s| s.as_str()).collect();
    let new_commit = git::git_in(Some(repo), &commit_refs)?.trim().to_string();
    git::git_in(Some(repo), &["update-ref", &mirror_ref, &new_commit])?;
    Ok(new_commit)
}

fn push_one(repo: &Path, target: &PublishTarget, branch: &str) -> Result<String> {
    let sha = if target.needs_mirror() {
        let tip = build_mirror(repo, target, branch)?;
        let spec = format!(
            "refs/gitassist/mirror/{}/{}:refs/heads/{}",
            target.label, branch, branch
        );
        git::git_in(Some(repo), &["push", &target.remote_name, &spec])?;
        tip
    } else {
        let head = git::git_in(Some(repo), &["rev-parse", "HEAD"])?
            .trim()
            .to_string();
        git::git_in(
            Some(repo),
            &[
                "push",
                &target.remote_name,
                &format!("HEAD:refs/heads/{branch}"),
            ],
        )?;
        head
    };
    Ok(sha)
}

/// Publish selected targets with all-or-nothing preflight.
pub fn publish(
    paths: &Paths,
    repo: &Path,
    cfg: &Config,
    sel: Selection<'_>,
) -> Result<PublishReport> {
    let data = targets::load(paths, repo);
    let chosen = resolve(&data, sel)?;
    let branch = git::git_in(Some(repo), &["rev-parse", "--abbrev-ref", "HEAD"])?
        .trim()
        .to_string();
    if branch == "HEAD" {
        bail!("detached HEAD: check out a branch before publishing");
    }

    // Preflight all first.
    let mut pref_errs = Vec::new();
    for t in &chosen {
        let eff = effective_config(cfg, t);
        if let Err(e) = preflight_branches(&eff, &branch) {
            pref_errs.push(format!("{}: {e}", t.label));
        }
        if let Err(e) = preflight_messages(repo, &eff, &t.remote_name) {
            pref_errs.push(format!("{}: {e}", t.label));
        }
        if let Err(e) = run_checks(repo, &t.checks) {
            pref_errs.push(format!("{}: {e}", t.label));
        }
        if t.needs_mirror() {
            // Dry-run filter emptiness
            let files = git::git_in(Some(repo), &["ls-tree", "-r", "--name-only", "HEAD"])
                .unwrap_or_default();
            let matcher = Matcher::new(&t.publish_filter);
            if !files
                .lines()
                .any(|p| !p.is_empty() && !matcher.is_ignored(p))
            {
                pref_errs.push(format!(
                    "{}: publish filter would leave an empty tree",
                    t.label
                ));
            }
        }
        // Ensure remote URL is set if configured.
        if !t.url.trim().is_empty() {
            let _ = targets::ensure_remotes(
                repo,
                &RepoTargets {
                    targets: vec![(*t).clone()],
                    ..Default::default()
                },
            );
        }
    }
    if !pref_errs.is_empty() {
        let mut report = PublishReport {
            ok: false,
            results: chosen
                .iter()
                .map(|t| TargetResult {
                    label: t.label.clone(),
                    remote: t.remote_name.clone(),
                    ok: false,
                    message: "preflight failed; nothing pushed".into(),
                    pushed_sha: None,
                })
                .collect(),
            drift: Vec::new(),
        };
        let msg = pref_errs.join("\n");
        for r in &mut report.results {
            r.message = msg.clone();
        }
        let mut e = Entry::new("publish", "blocked");
        e.repo = repo.to_string_lossy().into();
        e.messages = pref_errs;
        log::append(&paths.log_file, &e);
        return Ok(report);
    }

    let mut results = Vec::new();
    let mut drift = Vec::new();
    let mut ok_all = true;
    for t in &chosen {
        match push_one(repo, t, &branch) {
            Ok(sha) => {
                drift.push(t.remote_name.clone());
                results.push(TargetResult {
                    label: t.label.clone(),
                    remote: t.remote_name.clone(),
                    ok: true,
                    message: "pushed".into(),
                    pushed_sha: Some(sha),
                });
                crate::registry::touch(paths, |r| {
                    r.last_push = Some(crate::registry::now());
                    r.last_push_remote = Some(t.remote_name.clone());
                });
            }
            Err(err) => {
                ok_all = false;
                results.push(TargetResult {
                    label: t.label.clone(),
                    remote: t.remote_name.clone(),
                    ok: false,
                    message: format!("{err:#}"),
                    pushed_sha: None,
                });
                // Remaining targets not attempted.
                let failed_label = t.label.clone();
                for rest in chosen
                    .iter()
                    .skip_while(|x| x.label != failed_label)
                    .skip(1)
                {
                    results.push(TargetResult {
                        label: rest.label.clone(),
                        remote: rest.remote_name.clone(),
                        ok: false,
                        message: "skipped after earlier push failure (possible drift)".into(),
                        pushed_sha: None,
                    });
                }
                break;
            }
        }
    }

    // Drift = remotes that succeeded when overall failed.
    let drift = if ok_all {
        Vec::new()
    } else {
        results
            .iter()
            .filter(|r| r.ok)
            .map(|r| r.remote.clone())
            .collect()
    };

    let mut e = Entry::new("publish", if ok_all { "clean" } else { "error" });
    e.repo = repo.to_string_lossy().into();
    e.messages = results
        .iter()
        .map(|r| {
            format!(
                "{}@{}: {} — {}",
                r.label,
                r.remote,
                if r.ok { "ok" } else { "fail" },
                r.message
            )
        })
        .collect();
    if !drift.is_empty() {
        e.messages
            .push(format!("drift on remotes: {}", drift.join(", ")));
    }
    log::append(&paths.log_file, &e);

    Ok(PublishReport {
        ok: ok_all,
        results,
        drift,
    })
}

/// After a successful commit, push targets with AfterCommit policy (best-effort).
pub fn after_commit_publish(paths: &Paths, cfg: &Config) {
    let Some(top) = git::toplevel() else { return };
    let repo = PathBuf::from(&top);
    let data = targets::load(paths, &repo);
    let labels: Vec<String> = data
        .targets
        .iter()
        .filter(|t| t.enabled && matches!(t.push_policy, targets::PushPolicy::AfterCommit))
        .map(|t| t.label.clone())
        .collect();
    for label in labels {
        match publish(paths, &repo, cfg, Selection::Label(&label)) {
            Ok(r) if r.ok => {}
            Ok(r) => {
                let mut e = Entry::new("publish:after_commit", "warned");
                e.repo = top.clone();
                e.messages = r.results.into_iter().map(|x| x.message).collect();
                log::append(&paths.log_file, &e);
            }
            Err(err) => {
                let mut e = Entry::new("publish:after_commit", "error");
                e.repo = top.clone();
                e.messages = vec![format!("{err:#}")];
                log::append(&paths.log_file, &e);
            }
        }
    }
}
