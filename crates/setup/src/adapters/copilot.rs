use gitassist_core::instructions::{render_agent_instructions, render_tab_instructions};
use gitassist_core::{Config, Tab};
use serde_json::json;

use super::{env_dir, json, Adapter, Change};
use crate::Env;

// Verified against:
//   https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-custom-instructions
//     ($HOME/.copilot/copilot-instructions.md, COPILOT_HOME; also read by VS Code agent mode)
//   https://code.visualstudio.com/docs/agent-customization/custom-instructions
//     (`github.copilot.chat.commitMessageGeneration.instructions`,
//      `github.copilot.chat.pullRequestDescriptionGeneration.instructions`: arrays of {text})
pub fn adapter(env: &Env, cfg: &Config) -> Adapter {
    let dir = env_dir("COPILOT_HOME", env.home.join(".copilot"));
    let vscode = env.app_config.join("Code/User/settings.json");
    let changes = vec![
        Change::Block {
            path: dir.join("copilot-instructions.md"),
            content: render_agent_instructions(cfg),
        },
        json(
            vscode.clone(),
            &["github.copilot.chat.commitMessageGeneration.instructions"],
            json!([{ "text": render_tab_instructions(cfg, Tab::Commit) }]),
            true,
        ),
        json(
            vscode.clone(),
            &["github.copilot.chat.pullRequestDescriptionGeneration.instructions"],
            json!([{ "text": render_tab_instructions(cfg, Tab::PrCreate) }]),
            true,
        ),
    ];
    Adapter {
        id: "copilot",
        name: "GitHub Copilot",
        docs: vec![
            "https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-custom-instructions",
            "https://code.visualstudio.com/docs/agent-customization/custom-instructions",
        ],
        detected: dir.is_dir() || vscode.is_file() || Env::on_path("copilot"),
        changes,
        manual: vec![],
        limits: vec![
            "Copilot coding agent runs on GitHub's servers; local hooks cannot see those commits or PRs.".into(),
            "VS Code's GitHub PR extension calls the GitHub API directly, not `gh`, so only instructions apply there.".into(),
        ],
    }
}
