use crate::config::{BranchConfig, Mode};

#[derive(Debug, Clone, PartialEq)]
pub struct BranchFinding {
    pub mode: Mode,
    pub message: String,
}

fn unfixable(m: Mode) -> Mode {
    if m == Mode::Fix {
        Mode::Block
    } else {
        m
    }
}

pub fn is_protected(cfg: &BranchConfig, branch: &str) -> bool {
    cfg.protected.iter().any(|p| p.trim() == branch)
}

/// A commit made directly on a protected branch.
pub fn check_commit(cfg: &BranchConfig, branch: &str) -> Option<BranchFinding> {
    (cfg.enabled && is_protected(cfg, branch)).then(|| BranchFinding {
        mode: unfixable(cfg.protect_mode),
        message: format!(
            "GitAssist [branch] {branch} is protected. Work on a branch instead: git switch -c {}/<short-topic>",
            cfg.prefixes.first().map(String::as_str).unwrap_or("feat")
        ),
    })
}

/// Whether a branch name follows the naming rule: `<prefix>/<words>`.
pub fn name_ok(cfg: &BranchConfig, branch: &str) -> bool {
    if cfg.prefixes.is_empty() || branch == cfg.base || is_protected(cfg, branch) {
        return true;
    }
    let Some((prefix, rest)) = branch.split_once('/') else {
        return false;
    };
    cfg.prefixes.iter().any(|p| p.trim() == prefix)
        && !rest.is_empty()
        && rest
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
}

/// Checks one pushed ref (`refs/heads/<name>`); `new` is true when the branch does not exist on the remote yet.
pub fn check_push(cfg: &BranchConfig, remote_ref: &str, new: bool) -> Vec<BranchFinding> {
    let mut out = Vec::new();
    let Some(branch) = remote_ref.strip_prefix("refs/heads/") else {
        return out;
    };
    if !cfg.enabled {
        return out;
    }
    if is_protected(cfg, branch) && !new {
        out.push(BranchFinding {
            mode: unfixable(cfg.protect_mode),
            message: format!("GitAssist [branch] pushing straight to {branch}, which is protected. Push a branch and open a pull request."),
        });
    }
    if new && !name_ok(cfg, branch) {
        out.push(BranchFinding {
            mode: unfixable(cfg.naming_mode),
            message: format!(
                "GitAssist [branch] {branch} doesn't follow the naming rule ({}/<short-topic>).",
                cfg.prefixes.join("|")
            ),
        });
    }
    out
}

/// Agent instructions for the branch workflow.
pub fn instructions(cfg: &BranchConfig) -> String {
    if !cfg.enabled {
        return String::new();
    }
    let mut lines = vec!["### Branches and ideas".to_string()];
    if !cfg.base.trim().is_empty() {
        lines.push(format!(
            "- Start new work from `{}` (pull it first).",
            cfg.base.trim()
        ));
    }
    if !cfg.protected.is_empty() {
        lines.push(format!(
            "- Never commit or push straight to {}; make a branch and open a pull request.",
            cfg.protected
                .iter()
                .map(|b| format!("`{b}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !cfg.prefixes.is_empty() {
        lines.push(format!(
            "- Name branches `<type>/<short-topic>` where type is one of: {}.",
            cfg.prefixes.join(", ")
        ));
    }
    if !cfg.ideas.trim().is_empty() {
        lines.push(format!("- New ideas: {}", cfg.ideas.trim()));
    }
    lines.join("\n")
}
