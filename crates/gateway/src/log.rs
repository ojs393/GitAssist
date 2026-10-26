use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const MAX_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Entry {
    pub ts: u64,
    /// `hook:<name>` or `gh`.
    pub source: String,
    pub tab: String,
    /// clean | fixed | warned | blocked | passthrough | bypass | error
    pub outcome: String,
    pub rules: Vec<String>,
    pub messages: Vec<String>,
    pub repo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

impl Entry {
    pub fn new(source: impl Into<String>, outcome: impl Into<String>) -> Self {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            ts,
            source: source.into(),
            outcome: outcome.into(),
            ..Default::default()
        }
    }
}

/// Appends one JSON line. Logging must never break a commit, so all errors are ignored.
pub fn append(path: &Path, entry: &Entry) {
    let _ = (|| -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
            crate::secure::lock_dir(dir);
        }
        if fs::metadata(path)
            .map(|m| m.len() > MAX_BYTES)
            .unwrap_or(false)
        {
            let one = path.with_extension("log.1");
            let two = path.with_extension("log.2");
            let _ = fs::rename(&one, &two);
            fs::rename(path, &one)?;
        }
        let mut f = OpenOptions::new().create(true).append(true).open(path)?;
        let line = serde_json::to_string(entry).map_err(std::io::Error::other)?;
        crate::secure::lock_file(path);
        writeln!(f, "{line}")
    })();
}

/// Most recent entries first.
pub fn read_recent(path: &Path, limit: usize) -> Vec<Entry> {
    let Ok(s) = fs::read_to_string(path) else {
        return Vec::new();
    };
    s.lines()
        .rev()
        .filter_map(|l| serde_json::from_str(l).ok())
        .take(limit)
        .collect()
}
