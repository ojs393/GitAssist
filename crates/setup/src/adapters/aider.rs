use gitassist_core::instructions::render_tab_instructions;
use gitassist_core::{Config, Tab};

use super::{strips_attribution, Adapter, Change};
use crate::Env;

// Verified against:
//   https://aider.chat/docs/config/aider_conf.html (~/.aider.conf.yml)
//   https://aider.chat/docs/config/options.html (attribute-co-authored-by, attribute-author,
//   attribute-committer, commit-prompt)
pub fn adapter(env: &Env, cfg: &Config) -> Adapter {
    let file = env.home.join(".aider.conf.yml");
    let prompt = format!(
        "You are an expert software engineer who writes git commit messages for the provided diffs. Reply with only the commit message, no explanations or quotes.\n{}",
        render_tab_instructions(cfg, Tab::Commit)
    );
    let yaml = |key: &str, value: String| Change::Yaml {
        path: file.clone(),
        key: key.into(),
        value,
    };
    let mut changes = vec![yaml(
        "commit-prompt",
        serde_json::to_string(&prompt).unwrap_or_default(),
    )];
    if strips_attribution(cfg) {
        changes.push(yaml("attribute-co-authored-by", "false".into()));
        changes.push(yaml("attribute-author", "false".into()));
        changes.push(yaml("attribute-committer", "false".into()));
    }
    Adapter {
        id: "aider",
        name: "Aider",
        docs: vec!["https://aider.chat/docs/config/options.html"],
        detected: file.is_file() || Env::on_path("aider"),
        changes,
        manual: vec![],
        limits: vec![],
    }
}
