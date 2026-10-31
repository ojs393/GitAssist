use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};
use gitassist_core::scope::Matcher;

use crate::git;
use crate::targets::PublishTarget;

use crate::refname::{safe_branch_name, safe_ref_part};

pub fn build(repo: &Path, target: &PublishTarget, branch: &str) -> Result<String> {
    let label = safe_ref_part(&target.label, "label")?;
    let branch_part = safe_branch_name(branch)?;
    let mirror_ref = format!("refs/gitassist/mirror/{label}/{branch_part}");
    let head = git::git_in(Some(repo), &["rev-parse", "HEAD"])?
        .trim()
        .to_string();
    let matcher = Matcher::new(&target.publish_filter);
    let files = git::git_in(Some(repo), &["ls-tree", "-r", "--name-only", "HEAD"])?;
    if !files
        .lines()
        .any(|p| !p.is_empty() && !matcher.is_ignored(p))
    {
        bail!("publish filter for `{label}` would leave an empty tree");
    }

    let git_dir = git::git_in(Some(repo), &["rev-parse", "--git-dir"])?
        .trim()
        .to_string();
    let git_dir = Path::new(repo).join(&git_dir);
    let index_key = format!(
        "gitassist-mirror-{}-{}.index",
        label,
        branch_part.replace('/', "_")
    );
    let index = git_dir.join(index_key);
    let _ = std::fs::remove_file(&index);

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
    let msg = format!("gitassist publish {label} from {head}");
    let mut commit_args = vec!["commit-tree".to_string(), tree, "-m".into(), msg];
    if let Some(p) = parent {
        commit_args.push("-p".into());
        commit_args.push(p.trim().into());
    }
    let commit_refs: Vec<&str> = commit_args.iter().map(String::as_str).collect();
    let new_commit = git::git_in(Some(repo), &commit_refs)?.trim().to_string();
    git::git_in(Some(repo), &["update-ref", &mirror_ref, &new_commit])?;
    Ok(new_commit)
}

pub fn filter_leaves_files(repo: &Path, target: &PublishTarget) -> bool {
    let files =
        git::git_in(Some(repo), &["ls-tree", "-r", "--name-only", "HEAD"]).unwrap_or_default();
    let matcher = Matcher::new(&target.publish_filter);
    files
        .lines()
        .any(|p| !p.is_empty() && !matcher.is_ignored(p))
}
