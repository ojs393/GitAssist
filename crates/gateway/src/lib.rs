pub mod chain;
pub mod failopen;
pub mod gh;
pub mod git;
pub mod hooks;
pub mod jobs;
pub mod log;
pub mod paths;
pub mod publish;
pub mod refname;
pub mod registry;
pub mod repo;
pub mod secure;
pub mod state;
pub mod targets;

pub use paths::Paths;

/// Hooks GitAssist acts on. Every other client-side hook is a pass-through
/// so repositories' own hooks keep running under the global hooks path.
pub const ACTIVE_HOOKS: [&str; 6] = [
    "pre-commit",
    "post-checkout",
    "prepare-commit-msg",
    "commit-msg",
    "post-commit",
    "pre-push",
];

pub const PASSTHROUGH_HOOKS: [&str; 10] = [
    "applypatch-msg",
    "pre-applypatch",
    "post-applypatch",
    "pre-merge-commit",
    "pre-rebase",
    "post-merge",
    "post-rewrite",
    "pre-auto-gc",
    "push-to-checkout",
    "reference-transaction",
];

pub fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| !v.is_empty() && v != "0" && v.to_lowercase() != "false")
}

pub const BYPASS_ENV: &str = "GITASSIST_BYPASS";
pub const INTERNAL_ENV: &str = "GITASSIST_INTERNAL";
