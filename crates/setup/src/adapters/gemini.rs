use gitassist_core::instructions::render_agent_instructions;
use gitassist_core::Config;

use super::{Adapter, Change};
use crate::Env;

// Verified against:
//   https://geminicli.com/docs/cli/gemini-md/ (global context file ~/.gemini/GEMINI.md)
//   https://google-gemini.github.io/gemini-cli/docs/get-started/configuration.html
//   No documented co-author setting; the hooks strip any trailer it adds.
pub fn adapter(env: &Env, cfg: &Config) -> Adapter {
    let dir = env.home.join(".gemini");
    Adapter {
        id: "gemini",
        name: "Gemini CLI",
        docs: vec!["https://geminicli.com/docs/cli/gemini-md/"],
        detected: dir.is_dir() || Env::on_path("gemini"),
        changes: vec![Change::Block {
            path: dir.join("GEMINI.md"),
            content: render_agent_instructions(cfg),
        }],
        manual: vec![],
        limits: vec![],
    }
}
