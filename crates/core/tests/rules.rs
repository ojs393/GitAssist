use gitassist_core::config::{Config, Conventional, Mode, RuleSetting};
use gitassist_core::effective::effective;
use gitassist_core::instructions::render_agent_instructions;
use gitassist_core::rules::{process, Input};
use gitassist_core::tab::Tab;

fn commit(cfg: &Config, msg: &str) -> gitassist_core::Outcome {
    process(
        &Input {
            body: Some(msg),
            ..Default::default()
        },
        &effective(cfg, Tab::Commit, None),
    )
}

#[test]
fn agent_message_is_cleaned_without_findings_that_block() {
    let msg = "feat: add parser\n\nAdds the parser.\n\n🤖 Generated with [Claude Code](https://claude.com/claude-code)\n\nCo-Authored-By: Claude <noreply@anthropic.com>";
    let out = commit(&Config::default(), msg);
    assert_eq!(
        out.body.as_deref(),
        Some("feat: add parser\n\nAdds the parser.")
    );
    assert!(out.changed);
    assert!(!out.blocked());
    assert_eq!(out.status(), "fixed");
}

#[test]
fn long_subject_blocks_with_exact_message() {
    let mut cfg = Config::default();
    cfg.tabs.commit.subject_max = Some(RuleSetting::new(72, Mode::Block));
    let out = commit(&cfg, &"x".repeat(91));
    assert!(out.blocked());
    assert_eq!(
        out.findings[0].message,
        "GitAssist [commit] subject is 91 chars; limit is 72. Shorten the subject and retry."
    );
}

#[test]
fn warn_mode_does_not_block() {
    let out = commit(&Config::default(), &"x".repeat(91));
    assert!(!out.blocked());
    assert_eq!(out.status(), "warned");
}

#[test]
fn unfixable_rule_in_fix_mode_blocks() {
    let mut cfg = Config::default();
    cfg.tabs.commit.subject_max = Some(RuleSetting::new(10, Mode::Fix));
    assert!(commit(&cfg, "this subject is long").blocked());
}

#[test]
fn conventional_required_and_forbidden() {
    let mut cfg = Config::default();
    cfg.tabs.commit.conventional = Some(RuleSetting::new(Conventional::Required, Mode::Block));
    assert!(commit(&cfg, "add parser").blocked());
    assert!(!commit(&cfg, "feat(core): add parser").blocked());
    cfg.tabs.commit.conventional = Some(RuleSetting::new(Conventional::Forbidden, Mode::Block));
    assert!(commit(&cfg, "feat: add parser").blocked());
}

#[test]
fn banned_phrase_blocks_case_insensitively() {
    let mut cfg = Config::default();
    cfg.tabs.commit.banned_phrases = Some(RuleSetting::new(vec!["as an AI".into()], Mode::Block));
    assert!(commit(&cfg, "fix: x\n\nAs an AI, I fixed it").blocked());
}

#[test]
fn body_max_fix_truncates_body_not_subject() {
    let mut cfg = Config::default();
    cfg.tabs.commit.body_max = Some(RuleSetting::new(10, Mode::Fix));
    cfg.tabs.commit.wrap = Some(RuleSetting::new(0, Mode::Fix));
    let out = commit(&cfg, "fix: subject\n\n0123456789abcdef");
    assert_eq!(out.body.as_deref(), Some("fix: subject\n\n012345678…"));
}

#[test]
fn wrap_fix_wraps_body_only() {
    let subject = "fix: a subject that is longer than twenty chars";
    let out = {
        let mut cfg = Config::default();
        cfg.tabs.commit.wrap = Some(RuleSetting::new(20, Mode::Fix));
        commit(
            &cfg,
            &format!("{subject}\n\none two three four five six seven"),
        )
    };
    let body = out.body.unwrap();
    assert!(body.starts_with(subject));
    assert!(body.lines().skip(2).all(|l| l.len() <= 20), "{body}");
}

#[test]
fn trailers_added_after_strip() {
    let mut cfg = Config::default();
    cfg.global.trailers.add = vec!["Co-authored-by: Me <me@x>".into()];
    let out = commit(
        &cfg,
        "fix: x\n\nCo-authored-by: Claude <noreply@anthropic.com>",
    );
    assert_eq!(
        out.body.as_deref(),
        Some("fix: x\n\nCo-authored-by: Me <me@x>")
    );
}

#[test]
fn strip_block_mode_blocks_instead_of_fixing() {
    let mut cfg = Config::default();
    cfg.global.trailers.strip_mode = Mode::Block;
    let out = commit(&cfg, "fix: x\n\nCo-authored-by: Claude <c@x>");
    assert!(out.blocked());
    assert!(out.body.unwrap().contains("Co-authored-by"));
}

#[test]
fn growth_max_uses_previous_body() {
    let mut cfg = Config::default();
    cfg.tabs.pr_update.growth_max = Some(RuleSetting::new(5, Mode::Block));
    let rules = effective(&cfg, Tab::PrUpdate, None);
    let input = Input {
        title: None,
        body: Some("old text plus a lot more"),
        previous_body: Some("old text"),
        ..Default::default()
    };
    assert!(process(&input, &rules).blocked());
}

#[test]
fn pr_title_rules_and_managed_block() {
    let mut cfg = Config::default();
    cfg.tabs.pr_create.managed_table = Some(true);
    cfg.tabs.pr_create.subject_max = Some(RuleSetting::new(10, Mode::Block));
    let rules = effective(&cfg, Tab::PrCreate, None);
    let input = Input {
        title: Some("A long pull request title"),
        body: Some("Summary"),
        managed_block: Some("| t |"),
        ..Default::default()
    };
    let out = process(&input, &rules);
    assert!(out.blocked());
    assert!(out
        .body
        .unwrap()
        .contains("<!-- gitassist:begin -->\n| t |"));
}

#[test]
fn disabled_config_changes_nothing() {
    let cfg = Config {
        enabled: false,
        ..Default::default()
    };
    let msg = "x\n\nCo-authored-by: Claude <c@x>";
    let out = commit(&cfg, msg);
    assert!(!out.changed);
    assert!(out.findings.is_empty());
}

#[test]
fn instructions_mention_rules() {
    let mut cfg = Config::default();
    cfg.tabs.commit.conventional = Some(RuleSetting::new(Conventional::Required, Mode::Block));
    let text = render_agent_instructions(&cfg);
    assert!(text.contains("Never add `Co-authored-by:`"));
    assert!(text.contains("### Commit messages"));
    assert!(text.contains("conventional prefix"));
    assert!(text.contains("GITASSIST_BYPASS"));
}

#[test]
fn large_message_is_fast() {
    let line =
        "\nline of text that is reasonably long and needs wrapping at some point, yes indeed";
    let msg = format!("fix: x\n\n{}", line.repeat(2000));
    let start = std::time::Instant::now();
    let _ = commit(&Config::default(), &msg);
    assert!(
        start.elapsed().as_millis() < 500,
        "took {:?}",
        start.elapsed()
    );
}

#[test]
fn total_max_counts_subject_and_body() {
    let mut cfg = Config::default();
    cfg.tabs.commit.total_max = Some(RuleSetting::new(500, Mode::Block));
    let long = format!("fix: subject\n\n{}", "word ".repeat(120));
    let out = commit(&cfg, &long);
    assert!(out.blocked());
    assert!(out
        .findings
        .iter()
        .any(|f| f.message.contains("limit is 500")));
    assert!(!commit(&cfg, "fix: short\n\nfine").blocked());
}

#[test]
fn total_max_fix_keeps_subject_and_added_lines_within_limit() {
    let mut cfg = Config::default();
    cfg.tabs.commit.total_max = Some(RuleSetting::new(80, Mode::Fix));
    cfg.global.trailers.add = vec!["Signed-off-by: Me <me@x>".into()];
    let out = commit(
        &cfg,
        &format!("fix: keep this subject\n\n{}", "a b ".repeat(60)),
    );
    let body = out.body.unwrap();
    assert!(body.starts_with("fix: keep this subject\n\n"));
    assert!(body.ends_with("Signed-off-by: Me <me@x>"));
    assert!(
        body.chars().count() <= 80,
        "{} chars: {body}",
        body.chars().count()
    );
}
