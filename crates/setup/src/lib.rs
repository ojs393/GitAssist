pub mod adapters;
pub mod autostart;
pub mod backup;
pub mod blocks;
pub mod envpath;
pub mod health;
pub mod hooks_install;
pub mod jsonc;
pub mod ops;
pub mod repos;
pub mod shim_install;
pub mod storage;

use std::path::PathBuf;

/// User-level locations, resolved at runtime. Tests point these at a temp dir
/// by setting `HOME` (and `APPDATA` on Windows).
#[derive(Debug, Clone)]
pub struct Env {
    pub home: PathBuf,
    /// OS application config dir: `~/Library/Application Support`, `~/.config`, `%APPDATA%`.
    pub app_config: PathBuf,
}

impl Env {
    pub fn detect() -> anyhow::Result<Self> {
        let base =
            directories::BaseDirs::new().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
        Ok(Self {
            home: base.home_dir().to_path_buf(),
            app_config: base.config_dir().to_path_buf(),
        })
    }

    /// `name` resolves to an executable on PATH.
    pub fn on_path(name: &str) -> bool {
        let Some(path) = std::env::var_os("PATH") else {
            return false;
        };
        let exe = gitassist_gateway::paths::exe_name(name);
        std::env::split_paths(&path).any(|d| d.join(&exe).is_file())
    }
}
