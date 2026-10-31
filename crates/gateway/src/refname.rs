use anyhow::{bail, Result};

/// Label / remote name: no path separators.
pub fn safe_ref_part(s: &str, what: &str) -> Result<String> {
    let s = s.trim();
    if s.is_empty() {
        bail!("{what} is empty");
    }
    if s.len() > 64 {
        bail!("{what} is too long");
    }
    if s.contains("..") || s.starts_with('.') || s.ends_with('.') {
        bail!("{what} `{s}` is not allowed");
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        bail!("{what} `{s}` may only use letters, digits, -, _, .");
    }
    Ok(s.to_string())
}

pub fn safe_remote_name(s: &str) -> Result<String> {
    safe_ref_part(s, "remote name")
}

/// Branch names may include `/` (e.g. feat/x). Still reject traversal and odd chars.
pub fn safe_branch_name(s: &str) -> Result<String> {
    let s = s.trim();
    if s.is_empty() {
        bail!("branch is empty");
    }
    if s.len() > 200 {
        bail!("branch is too long");
    }
    if s.contains("..") || s.starts_with('/') || s.ends_with('/') || s.contains("//") {
        bail!("branch `{s}` is not allowed");
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
    {
        bail!("branch `{s}` has invalid characters");
    }
    Ok(s.to_string())
}
