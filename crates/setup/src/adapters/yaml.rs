//! Minimal line-based edits of top-level `key: value` scalars, so comments and
//! the rest of a hand-written YAML file are left exactly as they were.

use std::fs;
use std::path::Path;

use anyhow::Result;

fn key_of(line: &str) -> Option<&str> {
    if line.starts_with([' ', '\t', '#', '-']) {
        return None;
    }
    line.split_once(':').map(|(k, _)| k.trim())
}

pub fn get(text: &str, key: &str) -> Option<String> {
    text.lines()
        .find(|l| key_of(l) == Some(key))
        .and_then(|l| l.split_once(':'))
        .map(|(_, v)| v.trim().to_string())
}

pub fn set(text: &str, key: &str, value: Option<&str>) -> String {
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let pos = lines.iter().position(|l| key_of(l) == Some(key));
    match (pos, value) {
        (Some(i), Some(v)) => lines[i] = format!("{key}: {v}"),
        (Some(i), None) => {
            lines.remove(i);
        }
        (None, Some(v)) => lines.push(format!("{key}: {v}")),
        (None, None) => {}
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

pub fn get_file(path: &Path, key: &str) -> Option<String> {
    get(&fs::read_to_string(path).ok()?, key)
}

pub fn set_file(path: &Path, key: &str, value: Option<&str>) -> Result<()> {
    let text = fs::read_to_string(path).unwrap_or_default();
    let new = set(&text, key, value);
    if new != text {
        fs::write(path, new)?;
    }
    Ok(())
}
