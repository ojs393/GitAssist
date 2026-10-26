use gitassist_core::config::{Config, Mode, RuleSetting, TabConfig};
use gitassist_core::effective::effective;
use gitassist_core::tab::{classify_commit, classify_gh, Tab};

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[test]
fn commit_classification() {
    let docs = s(&["*.md", "docs/**", "README*"]);
    assert_eq!(
        classify_commit(Some("merge"), &s(&["a.rs"]), &docs),
        Tab::Merge
    );
    assert_eq!(classify_commit(Some("squash"), &[], &docs), Tab::Merge);
    assert_eq!(
        classify_commit(None, &s(&["README", "docs/x/y.png", "a/b.md"]), &docs),
        Tab::DocsCommit
    );
    assert_eq!(
        classify_commit(None, &s(&["README.md", "src/main.rs"]), &docs),
        Tab::Commit
    );
    assert_eq!(classify_commit(Some("message"), &[], &docs), Tab::Commit);
}

#[test]
fn gh_classification() {
    let c = |v: &[&str]| classify_gh(&s(v));
    assert_eq!(c(&["pr", "create", "--title", "x"]), Some(Tab::PrCreate));
    assert_eq!(c(&["pr", "edit", "12", "--body", "x"]), Some(Tab::PrUpdate));
    assert_eq!(c(&["pr", "comment", "12", "-b", "x"]), Some(Tab::PrComment));
    assert_eq!(
        c(&["pr", "review", "--comment", "-b", "x"]),
        Some(Tab::PrComment)
    );
    assert_eq!(c(&["pr", "review", "--approve"]), Some(Tab::Review));
    assert_eq!(c(&["pr", "review", "-r", "-b", "no"]), Some(Tab::Review));
    assert_eq!(
        c(&["pr", "merge", "--squash", "--subject", "x"]),
        Some(Tab::Merge)
    );
    assert_eq!(c(&["issue", "create", "-t", "x"]), Some(Tab::Issue));
    assert_eq!(c(&["issue", "comment", "3", "-b", "x"]), Some(Tab::Issue));
    assert_eq!(c(&["repo", "view"]), None);
    assert_eq!(c(&["pr", "list"]), None);
}

#[test]
fn tab_override_beats_builtin() {
    let mut cfg = Config::default();
    cfg.tabs.commit.subject_max = Some(RuleSetting::new(50, Mode::Block));
    let r = effective(&cfg, Tab::Commit, None);
    assert_eq!(r.subject_max, Some(RuleSetting::new(50, Mode::Block)));
    assert_eq!(
        effective(&cfg, Tab::DocsCommit, None).subject_max,
        Some(RuleSetting::new(72, Mode::Warn))
    );
}

#[test]
fn builtin_defaults_never_block() {
    let cfg = Config::default();
    for tab in Tab::ALL {
        let r = effective(&cfg, tab, None);
        for m in [
            r.subject_max.map(|x| x.mode),
            r.body_max.map(|x| x.mode),
            r.growth_max.map(|x| x.mode),
        ]
        .into_iter()
        .flatten()
        {
            assert_ne!(m, Mode::Block, "{tab:?}");
        }
    }
}

#[test]
fn project_override_only_when_allowed_and_cannot_touch_trailers() {
    let mut cfg = Config::default();
    cfg.global.trailers.add = s(&["Signed-off-by: Me <me@x>"]);
    let project = TabConfig {
        subject_max: Some(RuleSetting::new(40, Mode::Block)),
        strip_trailers: Some(false),
        add_trailers: Some(s(&["Co-authored-by: Evil <e@x>"])),
        enabled: Some(false),
        ..Default::default()
    };
    let r = effective(&cfg, Tab::Commit, Some(&project));
    assert_eq!(r.subject_max.unwrap().value, 72, "ignored when not allowed");

    cfg.allow_project_overrides = true;
    let r = effective(&cfg, Tab::Commit, Some(&project));
    assert_eq!(r.subject_max.unwrap().value, 40);
    assert!(r.strip_trailers);
    assert!(r.enabled);
    assert_eq!(r.add_trailers, s(&["Signed-off-by: Me <me@x>"]));
}

#[test]
fn global_trailers_only_added_to_commit_tabs_by_default() {
    let mut cfg = Config::default();
    cfg.global.trailers.add = s(&["Signed-off-by: Me <me@x>"]);
    assert_eq!(effective(&cfg, Tab::Commit, None).add_trailers.len(), 1);
    assert!(effective(&cfg, Tab::PrComment, None)
        .add_trailers
        .is_empty());
}
