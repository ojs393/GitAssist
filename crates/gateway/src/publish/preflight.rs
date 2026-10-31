use std::path::Path;

use anyhow::{bail, Result};
use gitassist_core::config::Mode;
use gitassist_core::{effective, process, Config, Input, Tab};

use crate::git;

pub fn messages(repo: &Path, cfg: &Config, remote: &str) -> Result<()> {
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
        return Ok(());
    }
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
            break;
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

pub fn branches(cfg: &Config, branch: &str) -> Result<()> {
    if let Some(f) = gitassist_core::branch::check_commit(&cfg.branches, branch) {
        if f.mode == Mode::Block {
            bail!("{}", f.message);
        }
    }
    Ok(())
}
