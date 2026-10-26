use gitassist_core::instructions::render_agent_instructions;
use gitassist_core::Config;

use super::{env_dir, Adapter, Change};
use crate::Env;

// Verified against:
//   https://learn.chatgpt.com/docs/agent-configuration/agents-md
//   Global scope is $CODEX_HOME (default ~/.codex): AGENTS.override.md if non-empty, else AGENTS.md.
//   Codex adds no commit attribution of its own.
pub fn adapter(env: &Env, cfg: &Config) -> Adapter {
    let dir = env_dir("CODEX_HOME", env.home.join(".codex"));
    let override_file = dir.join("AGENTS.override.md");
    let target = if std::fs::read_to_string(&override_file).is_ok_and(|t| !t.trim().is_empty()) {
        override_file
    } else {
        dir.join("AGENTS.md")
    };
    Adapter {
        id: "codex",
        name: "OpenAI Codex",
        docs: vec!["https://learn.chatgpt.com/docs/agent-configuration/agents-md"],
        detected: dir.is_dir() || Env::on_path("codex"),
        changes: vec![Change::Block {
            path: target,
            content: render_agent_instructions(cfg),
        }],
        manual: vec![],
        limits: vec![
            "Codex cloud tasks run remotely; local hooks cannot see those commits.".into(),
        ],
    }
}
