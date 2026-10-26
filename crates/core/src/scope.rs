use std::path::Path;

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use serde::{Deserialize, Serialize};

use crate::config::IgnoreConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Owner {
    Personal,
    Organization,
    /// No GitHub remote, or the user's GitHub account is not set.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Public,
    Private,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemoteRepo {
    pub host: String,
    pub owner: String,
    pub name: String,
}

impl RemoteRepo {
    pub fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Scope {
    pub owner: Owner,
    pub visibility: Visibility,
    pub repo: Option<String>,
}

/// Parses `https://host/owner/repo(.git)`, `git@host:owner/repo.git` and
/// `ssh://git@host[:port]/owner/repo`.
pub fn parse_remote(url: &str) -> Option<RemoteRepo> {
    let url = url.trim();
    let (host, path) = if let Some(rest) = url.split_once("://").map(|(_, r)| r) {
        let rest = rest.rsplit_once('@').map(|(_, r)| r).unwrap_or(rest);
        let (host, path) = rest.split_once('/')?;
        (host.split(':').next()?.to_string(), path.to_string())
    } else {
        let rest = url.rsplit_once('@').map(|(_, r)| r).unwrap_or(url);
        let (host, path) = rest.split_once(':')?;
        (host.to_string(), path.to_string())
    };
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut parts = path.split('/').filter(|p| !p.is_empty());
    let owner = parts.next()?.to_string();
    let name = parts.next()?.to_string();
    if parts.next().is_some() || host.is_empty() {
        return None;
    }
    Some(RemoteRepo {
        host: host.to_lowercase(),
        owner,
        name,
    })
}

pub fn scope_for(remote: Option<&RemoteRepo>, github_user: &str, visibility: Visibility) -> Scope {
    let owner = match remote {
        None => Owner::Unknown,
        Some(_) if github_user.trim().is_empty() => Owner::Unknown,
        Some(r) if r.owner.eq_ignore_ascii_case(github_user.trim()) => Owner::Personal,
        Some(_) => Owner::Organization,
    };
    Scope {
        owner,
        visibility: if remote.is_some() {
            visibility
        } else {
            Visibility::Unknown
        },
        repo: remote.map(RemoteRepo::slug),
    }
}

/// Which lists apply. A repo with a remote whose visibility is unknown gets the
/// public list too: assuming public is the safe mistake.
pub fn patterns_for(cfg: &IgnoreConfig, scope: &Scope) -> Vec<String> {
    let mut out = cfg.every_repo.clone();
    match scope.owner {
        Owner::Personal => out.extend(cfg.personal.iter().cloned()),
        Owner::Organization => out.extend(cfg.organization.iter().cloned()),
        Owner::Unknown => {}
    }
    match (scope.visibility, scope.repo.is_some()) {
        (Visibility::Private, _) => out.extend(cfg.private.iter().cloned()),
        (Visibility::Public, _) | (Visibility::Unknown, true) => {
            out.extend(cfg.public.iter().cloned())
        }
        (Visibility::Unknown, false) => {}
    }
    out.retain(|p| !p.trim().is_empty() && !p.trim_start().starts_with('#'));
    out
}

pub fn scope_label(scope: &Scope) -> String {
    let owner = match scope.owner {
        Owner::Personal => "personal",
        Owner::Organization => "organization",
        Owner::Unknown => "local",
    };
    let vis = match scope.visibility {
        Visibility::Public => " public",
        Visibility::Private => " private",
        Visibility::Unknown if scope.repo.is_some() => " (visibility unknown, treated as public)",
        Visibility::Unknown => "",
    };
    format!("{owner}{vis} repo")
}

pub struct Matcher {
    inner: Gitignore,
}

const ROOT: &str = "/gitassist-root";

impl Matcher {
    /// Invalid patterns are skipped rather than failing the whole list.
    pub fn new(patterns: &[String]) -> Self {
        let mut b = GitignoreBuilder::new(ROOT);
        for p in patterns {
            let _ = b.add_line(None, p.trim());
        }
        Self {
            inner: b.build().unwrap_or_else(|_| Gitignore::empty()),
        }
    }

    pub fn is_ignored(&self, rel: &str) -> bool {
        let rel = rel.trim_start_matches("./").replace('\\', "/");
        if rel.is_empty() {
            return false;
        }
        self.inner
            .matched_path_or_any_parents(Path::new(ROOT).join(rel), false)
            .is_ignore()
    }

    pub fn filter<'a>(&self, paths: &'a [String]) -> Vec<&'a String> {
        paths.iter().filter(|p| self.is_ignored(p)).collect()
    }
}
