use gitassist_core::Config;
use serde_json::json;

use super::{json, strips_attribution, Adapter};
use crate::Env;

// Verified against (and by running cursor-agent: it ignores ~/.cursor/rules but
// loads project rules in .cursor/rules/*.mdc, which the git hooks write per repo):
//   https://cursor.com/help/integrations/git — IDE attribution toggle lives in Cursor Settings
//   (Agent > Attribution; Git & PRs > Attribution from 3.11), stored in app state, not a file.
//   Cursor CLI: ~/.cursor/cli-config.json `attribution.attributeCommitsToAgent` /
//   `attribution.attributePRsToAgent`. Cursor adds its trailer before git hooks run, so the
//   commit-msg hook removes it even when a setting is ignored.
pub fn adapter(env: &Env, cfg: &Config) -> Adapter {
    let dir = env.home.join(".cursor");
    let cli_config = dir.join("cli-config.json");
    let mut changes = Vec::new();
    if strips_attribution(cfg) {
        changes.push(json(
            cli_config.clone(),
            &["attribution", "attributeCommitsToAgent"],
            json!(false),
            true,
        ));
        changes.push(json(
            cli_config,
            &["attribution", "attributePRsToAgent"],
            json!(false),
            true,
        ));
    }
    let mut manual = Vec::new();
    if strips_attribution(cfg) {
        manual.push(
            "Cursor Settings → Git & Pull Requests (older: Agent → Attribution): turn off Commit Attribution and PR Attribution."
                .to_string(),
        );
    }
    Adapter {
        id: "cursor",
        name: "Cursor",
        docs: vec!["https://cursor.com/help/integrations/git"],
        detected: dir.is_dir() || Env::on_path("cursor") || Env::on_path("cursor-agent"),
        changes,
        manual,
        limits: vec![
            "Cursor cloud agents commit on Cursor's servers; local hooks cannot see those commits."
                .into(),
            "Cursor reads no global rules file, so the rules go into each repo as .cursor/rules/gitassist.mdc, kept out of commits."
                .into(),
        ],
    }
}
