//! What GitAssist keeps on disk, and clearing it.

use std::fs;
use std::path::Path;

use anyhow::Result;
use gitassist_gateway::Paths;
use serde::Serialize;

pub const KEEP_BACKUPS: usize = 20;

#[derive(Debug, Clone, Serialize)]
pub struct Usage {
    pub logs: u64,
    pub backups: u64,
    pub backup_sets: usize,
    pub cache: u64,
}

fn size(p: &Path) -> u64 {
    match fs::metadata(p) {
        Ok(m) if m.is_file() => m.len(),
        Ok(m) if m.is_dir() => fs::read_dir(p)
            .map(|it| it.flatten().map(|e| size(&e.path())).sum())
            .unwrap_or(0),
        _ => 0,
    }
}

fn log_files(paths: &Paths) -> [std::path::PathBuf; 3] {
    [
        paths.log_file.clone(),
        paths.log_file.with_extension("log.1"),
        paths.log_file.with_extension("log.2"),
    ]
}

pub fn usage(paths: &Paths) -> Usage {
    Usage {
        logs: log_files(paths).iter().map(|p| size(p)).sum(),
        backups: size(&paths.backups_dir),
        backup_sets: fs::read_dir(&paths.backups_dir)
            .map(|it| it.count())
            .unwrap_or(0),
        cache: size(&paths.data_dir.join("repos.json")) + size(&paths.tmp_dir),
    }
}

/// `what` is logs, backups, cache or all. Settings and install state are never touched.
pub fn clear(paths: &Paths, what: &str) -> Result<()> {
    let all = what == "all";
    if all || what == "logs" {
        for f in log_files(paths) {
            let _ = fs::remove_file(f);
        }
    }
    if all || what == "backups" {
        let _ = fs::remove_dir_all(&paths.backups_dir);
    }
    if all || what == "cache" {
        let _ = fs::remove_file(paths.data_dir.join("repos.json"));
        let _ = fs::remove_dir_all(&paths.tmp_dir);
    }
    anyhow::ensure!(
        all || ["logs", "backups", "cache"].contains(&what),
        "unknown storage kind {what}"
    );
    Ok(())
}

/// Keeps the newest backup sets (folder names are unix timestamps).
pub fn prune_backups(paths: &Paths) {
    let Ok(it) = fs::read_dir(&paths.backups_dir) else {
        return;
    };
    let mut dirs: Vec<(u64, std::path::PathBuf)> = it
        .flatten()
        .filter_map(|e| Some((e.file_name().to_string_lossy().parse().ok()?, e.path())))
        .collect();
    dirs.sort();
    let extra = dirs.len().saturating_sub(KEEP_BACKUPS);
    for (_, d) in dirs.into_iter().take(extra) {
        let _ = fs::remove_dir_all(d);
    }
}
