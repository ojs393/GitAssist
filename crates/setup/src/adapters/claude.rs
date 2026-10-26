use gitassist_core::instructions::render_agent_instructions;
use gitassist_core::Config;
use serde_json::json;

use super::{env_dir, json, strips_attribution, Adapter, Change};
use crate::Env;

// Verified against:
//   https://code.claude.com/docs/en/settings        (~/.claude/settings.json, CLAUDE_CONFIG_DIR)
//   https://code.claude.com/docs/en/settings-reference (`attribution.commit` / `attribution.pr`;
//   empty strings hide attribution; supersedes deprecated `includeCoAuthoredBy`)
//   https://code.claude.com/docs/en/memory          (~/.claude/CLAUDE.md user memory)
pub fn adapter(env: &Env, cfg: &Config) -> Adapter {
    let dir = env_dir("CLAUDE_CONFIG_DIR", env.home.join(".claude"));
    let settings = dir.join("settings.json");
    let mut changes = vec![Change::Block {
        path: dir.join("CLAUDE.md"),
        content: render_agent_instructions(cfg),
    }];
    if strips_attribution(cfg) {
        changes.push(json(
            settings.clone(),
            &["attribution", "commit"],
            json!(""),
            false,
        ));
        changes.push(json(settings, &["attribution", "pr"], json!(""), false));
    }
    Adapter {
        id: "claude",
        name: "Claude Code",
        docs: vec![
            "https://code.claude.com/docs/en/settings-reference",
            "https://code.claude.com/docs/en/memory",
        ],
        detected: dir.is_dir() || Env::on_path("claude"),
        changes,
        manual: vec![],
        limits: vec![
            "Claude Code on the web runs remotely; local hooks cannot see those commits.".into(),
        ],
    }
}
