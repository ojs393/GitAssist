use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use gitassist_gateway::Paths;

/// Copies a file into `backups/<unix-time>/` before GitAssist edits it. These
/// are a safety net; normal uninstall restores from recorded state instead.
pub fn backup_file(paths: &Paths, file: &Path) -> Result<()> {
    if !file.is_file() {
        return Ok(());
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dir = paths.backups_dir.join(stamp.to_string());
    fs::create_dir_all(&dir)?;
    let name: String = file
        .to_string_lossy()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    fs::copy(file, dir.join(name))?;
    crate::storage::prune_backups(paths);
    Ok(())
}
