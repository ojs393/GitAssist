use std::path::PathBuf;

use anyhow::{Context, Result};

/// Every location GitAssist reads or writes, resolved from OS conventions at
/// runtime. `GITASSIST_HOME` puts everything under one directory instead.
#[derive(Debug, Clone)]
pub struct Paths {
    pub config_file: PathBuf,
    pub data_dir: PathBuf,
    pub hooks_dir: PathBuf,
    pub bin_dir: PathBuf,
    pub log_file: PathBuf,
    pub state_file: PathBuf,
    pub backups_dir: PathBuf,
    pub tmp_dir: PathBuf,
}

impl Paths {
    pub fn resolve() -> Result<Self> {
        if let Some(home) = std::env::var_os("GITASSIST_HOME").filter(|v| !v.is_empty()) {
            let base = PathBuf::from(home);
            return Ok(Self::from_dirs(base.clone(), base));
        }
        let dirs = directories::ProjectDirs::from("", "", "GitAssist")
            .context("could not determine the user's home directory")?;
        Ok(Self::from_dirs(
            dirs.config_dir().to_path_buf(),
            dirs.data_dir().to_path_buf(),
        ))
    }

    pub fn from_dirs(config_dir: PathBuf, data_dir: PathBuf) -> Self {
        Self {
            config_file: config_dir.join("config.toml"),
            hooks_dir: data_dir.join("hooks"),
            bin_dir: data_dir.join("bin"),
            log_file: data_dir.join("activity.log"),
            state_file: data_dir.join("state.json"),
            backups_dir: data_dir.join("backups"),
            tmp_dir: data_dir.join("tmp"),
            data_dir,
        }
    }

    pub fn cli_exe(&self) -> PathBuf {
        self.bin_dir.join(exe_name("gitassist"))
    }

    pub fn gh_shim(&self) -> PathBuf {
        self.bin_dir.join(exe_name("gh"))
    }
}

pub fn exe_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_string()
    }
}
