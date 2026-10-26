use crate::config::{Config, Conventional, Detail, Mode, Style, Tone, Voice};
use crate::effective::{effective, EffectiveRules};
use crate::tab::Tab;

fn style_line(s: &Style) -> String {
    let tone = match s.tone {
        Tone::Neutral => "neutral and factual",
        Tone::Friendly => "friendly and approachable",
        Tone::Formal => "formal and precise",
        Tone::Terse => "terse; no filler words",
    };
    let voice = match s.voice {
        Voice::Imperative => "imperative mood (\"Add\", \"Fix\", not \"Added\")",
        Voice::Past => "past tense (\"Added\", \"Fixed\")",
        Voice::We => "first-person plural (\"We add\", \"We fix\")",
    };
    let detail = match s.detail {
        Detail::Low => {
            "low technical detail: say what changed and why, skip implementation specifics"
        }
        Detail::Medium => {
            "medium technical detail: key implementation choices, no line-by-line narration"
        }
        Detail::High => {
            "high technical detail: name the functions, files, data flow and trade-offs involved"
        }
    };
    let mut line = format!("Tone: {tone}. Voice: {voice}. Detail: {detail}.");
    if !s.notes.trim().is_empty() {
        line.push_str(&format!(" Also: {}", s.notes.trim()));
    }
    line
}

fn severity(mode: Mode) -> &'static str {
    match mode {
        Mode::Block => "required",
        Mode::Warn => "preferred",
        Mode::Fix => "auto-applied",
    }
}

fn tab_section(r: &EffectiveRules) -> String {
    let mut lines = vec![
        format!("### {}", r.tab.label()),
        format!("- {}", style_line(&r.style)),
    ];
    let noun = if r.tab.is_commit_message() {
        "Subject line"
    } else {
        "Title"
    };
    if let Some(m) = &r.subject_max {
        lines.push(format!(
            "- {noun} at most {} characters ({}).",
            m.value,
            severity(m.mode)
        ));
    }
    if let Some(m) = &r.total_max {
        lines.push(format!(
            "- Whole message (title and body together) at most {} characters ({}).",
            m.value,
            severity(m.mode)
        ));
    }
    if let Some(m) = &r.body_max {
        lines.push(format!(
            "- Body at most {} characters ({}).",
            m.value,
            severity(m.mode)
        ));
    }
    if let Some(w) = &r.wrap {
        lines.push(format!("- Wrap body lines at {} characters.", w.value));
    }
    match r.conventional.value {
        Conventional::Required => lines.push(format!(
            "- {noun} must start with a conventional prefix such as `feat:`, `fix(scope):` ({}).",
            severity(r.conventional.mode)
        )),
        Conventional::Forbidden => lines.push(format!(
            "- {noun} must not use a conventional `type:` prefix."
        )),
        Conventional::Off => {}
    }
    if let Some(m) = &r.growth_max {
        lines.push(format!(
            "- When updating an existing PR description, add at most {} characters; condense instead of appending.",
            m.value
        ));
    }
    if !r.banned_phrases.value.is_empty() {
        lines.push(format!(
            "- Never use these phrases: {}.",
            r.banned_phrases.value.join(", ")
        ));
    }
    if !r.template.trim().is_empty() {
        lines.push("- Use exactly this layout:".into());
        lines.push(format!("```\n{}\n```", r.template.trim_end()));
    }
    if r.managed_table {
        lines.push(
            "- Leave the block between `<!-- gitassist:begin -->` and `<!-- gitassist:end -->` alone; GitAssist maintains it."
                .into(),
        );
    }
    lines.join("\n")
}

/// Instructions for a single tab, for tools that take per-task instructions
/// (e.g. VS Code's commit-message and PR-description generation settings).
pub fn render_tab_instructions(cfg: &Config, tab: Tab) -> String {
    let r = effective(cfg, tab, None);
    let mut s = tab_section(&r)
        .lines()
        .skip(1)
        .collect::<Vec<_>>()
        .join("\n");
    if cfg.global.trailers.strip_mode != Mode::Warn {
        s.push_str("\n- Never add Co-authored-by trailers or tool/agent signature lines.");
    }
    s
}

/// Markdown instructions describing the user's rules, for agent instruction
/// files. Markers are added by the writer, not here.
pub fn render_agent_instructions(cfg: &Config) -> String {
    let mut out = vec![
        "## Git and GitHub writing rules (managed by GitAssist)".to_string(),
        String::new(),
        "GitAssist checks every commit, push, pull request, comment, review and issue made from this machine."
            .to_string(),
    ];
    if cfg.global.trailers.strip_mode == Mode::Fix || cfg.global.trailers.strip_mode == Mode::Block
    {
        out.push(
            "- Never add `Co-authored-by:` trailers, \"Generated with ...\" lines, or any tool/agent signature to commits, PRs, comments, reviews or issues."
                .into(),
        );
    }
    out.push(
        "- Never set `GITASSIST_BYPASS` or use `--no-verify` to get around GitAssist. If a commit or `gh` command is blocked, read the reason, fix the text and retry."
            .into(),
    );
    out.push(
        "- Use `gh` for pull requests, comments, reviews and issues so the rules can be applied."
            .into(),
    );
    out.push(String::new());
    let enabled: Vec<EffectiveRules> = Tab::ALL
        .into_iter()
        .map(|t| effective(cfg, t, None))
        .filter(|r| r.enabled)
        .collect();
    for r in &enabled {
        out.push(tab_section(r));
        out.push(String::new());
    }
    let branches = crate::branch::instructions(&cfg.branches);
    if cfg.enabled && !branches.is_empty() {
        out.push(branches);
    }
    out.join("\n").trim_end().to_string()
}

pub const REPO_RULE_PATH: &str = ".cursor/rules/gitassist.mdc";
const REPO_RULE_HEAD: &str = "---\ndescription: Git and GitHub writing rules from GitAssist (kept out of commits)\nalwaysApply: true\n---\n\n";

/// Contents of the per-repo rule file for agents that only read project rules.
pub fn render_repo_rule(cfg: &Config) -> String {
    format!("{REPO_RULE_HEAD}{}\n", render_agent_instructions(cfg))
}

pub fn is_repo_rule(text: &str) -> bool {
    text.starts_with(REPO_RULE_HEAD)
}
