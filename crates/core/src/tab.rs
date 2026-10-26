use globset::{GlobBuilder, GlobSetBuilder};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tab {
    Commit,
    DocsCommit,
    Merge,
    PrCreate,
    PrUpdate,
    PrComment,
    Review,
    Issue,
}

impl Tab {
    pub const ALL: [Tab; 8] = [
        Tab::Commit,
        Tab::DocsCommit,
        Tab::Merge,
        Tab::PrCreate,
        Tab::PrUpdate,
        Tab::PrComment,
        Tab::Review,
        Tab::Issue,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Tab::Commit => "commit",
            Tab::DocsCommit => "docs_commit",
            Tab::Merge => "merge",
            Tab::PrCreate => "pr_create",
            Tab::PrUpdate => "pr_update",
            Tab::PrComment => "pr_comment",
            Tab::Review => "review",
            Tab::Issue => "issue",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Commit => "Commit messages",
            Tab::DocsCommit => "Docs commits",
            Tab::Merge => "Merge & squash commits",
            Tab::PrCreate => "New pull requests",
            Tab::PrUpdate => "Pull request updates",
            Tab::PrComment => "PR comments",
            Tab::Review => "Code reviews",
            Tab::Issue => "Issues",
        }
    }

    pub fn parse(s: &str) -> Option<Tab> {
        Tab::ALL.into_iter().find(|t| t.as_str() == s)
    }

    /// Git commit messages (subject line + body) as opposed to a GitHub title + markdown body.
    pub fn is_commit_message(self) -> bool {
        matches!(self, Tab::Commit | Tab::DocsCommit | Tab::Merge)
    }

    pub fn has_title(self) -> bool {
        !matches!(self, Tab::PrComment | Tab::Review)
    }
}

/// `source` is the second argument git passes to `prepare-commit-msg`
/// (`message`, `template`, `merge`, `squash`, `commit`), or the equivalent
/// derived from repository state in `commit-msg`.
pub fn classify_commit(source: Option<&str>, staged: &[String], docs_patterns: &[String]) -> Tab {
    if matches!(source, Some("merge") | Some("squash")) {
        return Tab::Merge;
    }
    if !staged.is_empty() && all_match(staged, docs_patterns) {
        return Tab::DocsCommit;
    }
    Tab::Commit
}

fn all_match(paths: &[String], patterns: &[String]) -> bool {
    let mut builder = GlobSetBuilder::new();
    let mut any = false;
    for p in patterns {
        if let Ok(g) = GlobBuilder::new(p).case_insensitive(true).build() {
            builder.add(g);
            any = true;
        }
    }
    if !any {
        return false;
    }
    let Ok(set) = builder.build() else {
        return false;
    };
    paths.iter().all(|path| {
        let file_name = path.rsplit('/').next().unwrap_or(path);
        set.is_match(path) || set.is_match(file_name)
    })
}

/// Classifies arguments passed to `gh` (without the program name).
/// Returns `None` for commands GitAssist does not gate.
pub fn classify_gh(args: &[String]) -> Option<Tab> {
    let positional: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|a| !a.starts_with('-'))
        .take(2)
        .collect();
    let has = |flags: &[&str]| args.iter().any(|a| flags.contains(&a.as_str()));
    match positional.as_slice() {
        ["pr", "create"] => Some(Tab::PrCreate),
        ["pr", "edit"] => Some(Tab::PrUpdate),
        ["pr", "comment"] => Some(Tab::PrComment),
        ["pr", "review"] => {
            if has(&["--approve", "-a", "--request-changes", "-r"]) {
                Some(Tab::Review)
            } else {
                Some(Tab::PrComment)
            }
        }
        ["pr", "merge"] => Some(Tab::Merge),
        ["issue", "create" | "edit" | "comment"] => Some(Tab::Issue),
        _ => None,
    }
}
