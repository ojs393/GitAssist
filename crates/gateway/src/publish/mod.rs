//! Shared publish pipeline for CLI, UI, and hooks.

mod checks;
mod mirror;
mod preflight;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::git;
use crate::log::{self, Entry};
use crate::paths::Paths;
use crate::refname::{safe_branch_name, safe_ref_part, safe_remote_name};
use crate::targets::{
    self, effective_config, find_by_label, for_push_all, PublishTarget, RepoTargets,
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
    pub drift: Vec<String>,
}

pub enum Selection<'a> {
    Label(&'a str),
    All,
    Remotes(Vec<&'a str>),
}

fn active_publishes() -> &'static Mutex<HashSet<String>> {
    static LOCKS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    LOCKS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn lock_repo(repo: &Path) -> Result<String> {
    let key = repo
        .canonicalize()
        .unwrap_or_else(|_| repo.to_path_buf())
        .to_string_lossy()
        .into_owned();
    let mut set = active_publishes().lock().unwrap_or_else(|e| e.into_inner());
    if !set.insert(key.clone()) {
        bail!("a publish is already running for this repo; wait and retry");
    }
    Ok(key)
}

fn unlock_repo(key: &str) {
    if let Ok(mut set) = active_publishes().lock() {
        set.remove(key);
    }
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

pub fn preflight_target_checks(repo: &Path, target: &PublishTarget) -> Result<()> {
    checks::run(repo, &target.checks)
}

fn push_one(repo: &Path, target: &PublishTarget, branch: &str) -> Result<String> {
    let _ = safe_remote_name(&target.remote_name)?;
    let branch_part = safe_branch_name(branch)?;
    if target.needs_mirror() {
        let tip = mirror::build(repo, target, &branch_part)?;
        let label = safe_ref_part(&target.label, "label")?;
        let spec = format!("refs/gitassist/mirror/{label}/{branch_part}:refs/heads/{branch_part}");
        git::git_in(Some(repo), &["push", &target.remote_name, &spec])?;
        Ok(tip)
    } else {
        let head = git::git_in(Some(repo), &["rev-parse", "HEAD"])?
            .trim()
            .to_string();
        git::git_in(
            Some(repo),
            &[
                "push",
                &target.remote_name,
                &format!("HEAD:refs/heads/{branch_part}"),
            ],
        )?;
        Ok(head)
    }
}

pub fn publish(
    paths: &Paths,
    repo: &Path,
    cfg: &gitassist_core::Config,
    sel: Selection<'_>,
) -> Result<PublishReport> {
    let lock_key = lock_repo(repo)?;
    let result = publish_inner(paths, repo, cfg, sel);
    unlock_repo(&lock_key);
    result
}

fn publish_inner(
    paths: &Paths,
    repo: &Path,
    cfg: &gitassist_core::Config,
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

    let mut pref_errs = Vec::new();
    for t in &chosen {
        if let Err(e) = safe_ref_part(&t.label, "label") {
            pref_errs.push(format!("{}: {e}", t.label));
            continue;
        }
        if let Err(e) = safe_remote_name(&t.remote_name) {
            pref_errs.push(format!("{}: {e}", t.label));
            continue;
        }
        let eff = effective_config(cfg, t);
        if let Err(e) = preflight::branches(&eff, &branch) {
            pref_errs.push(format!("{}: {e}", t.label));
        }
        if let Err(e) = preflight::messages(repo, &eff, &t.remote_name) {
            pref_errs.push(format!("{}: {e}", t.label));
        }
        if let Err(e) = checks::run(repo, &t.checks) {
            pref_errs.push(format!("{}: {e}", t.label));
        }
        if t.needs_mirror() && !mirror::filter_leaves_files(repo, t) {
            pref_errs.push(format!(
                "{}: publish filter would leave an empty tree",
                t.label
            ));
        }
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
        let msg = pref_errs.join("\n");
        let results = chosen
            .iter()
            .map(|t| TargetResult {
                label: t.label.clone(),
                remote: t.remote_name.clone(),
                ok: false,
                message: msg.clone(),
                pushed_sha: None,
            })
            .collect();
        let mut e = Entry::new("publish", "blocked");
        e.repo = repo.to_string_lossy().into();
        e.messages = pref_errs;
        log::append(&paths.log_file, &e);
        return Ok(PublishReport {
            ok: false,
            results,
            drift: Vec::new(),
        });
    }

    let mut results = Vec::new();
    let mut ok_all = true;
    for t in &chosen {
        match push_one(repo, t, &branch) {
            Ok(sha) => {
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

pub fn after_commit_publish(paths: &Paths, cfg: &gitassist_core::Config) {
    let Some(top) = git::toplevel() else {
        return;
    };
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
