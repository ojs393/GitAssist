use gitassist_core::config::IgnoreConfig;
use gitassist_core::scope::{parse_remote, patterns_for, scope_for, Matcher, Owner, Visibility};

fn slug(url: &str) -> Option<String> {
    parse_remote(url).map(|r| format!("{}|{}", r.host, r.slug()))
}

#[test]
fn parses_every_remote_url_shape() {
    for (url, want) in [
        ("https://github.com/octo/app.git", "github.com|octo/app"),
        ("https://github.com/octo/app", "github.com|octo/app"),
        ("https://github.com/octo/app/", "github.com|octo/app"),
        (
            "https://user:token@github.com/octo/app.git",
            "github.com|octo/app",
        ),
        ("git@github.com:octo/app.git", "github.com|octo/app"),
        ("ssh://git@github.com/octo/app.git", "github.com|octo/app"),
        (
            "ssh://git@github.com:22/octo/app.git",
            "github.com|octo/app",
        ),
        ("https://GitHub.com/Octo/App.git", "github.com|Octo/App"),
        (
            "git@ghe.corp.example:team/tool.git",
            "ghe.corp.example|team/tool",
        ),
    ] {
        assert_eq!(slug(url).as_deref(), Some(want), "{url}");
    }
}

#[test]
fn rejects_non_repo_urls() {
    for url in [
        "",
        "/tmp/repo",
        "../bare.git",
        "file:///tmp/repo.git",
        r"C:\repos\x",
        "https://github.com/onlyowner",
        "https://github.com/a/b/c",
    ] {
        assert_eq!(slug(url), None, "{url}");
    }
}

#[test]
fn owner_is_personal_only_for_the_users_account() {
    let r = parse_remote("git@github.com:Me/app.git");
    assert_eq!(
        scope_for(r.as_ref(), "me", Visibility::Public).owner,
        Owner::Personal
    );
    assert_eq!(
        scope_for(r.as_ref(), "someone", Visibility::Public).owner,
        Owner::Organization
    );
    assert_eq!(
        scope_for(r.as_ref(), "", Visibility::Public).owner,
        Owner::Unknown
    );
    let local = scope_for(None, "me", Visibility::Public);
    assert_eq!(
        (local.owner, local.visibility),
        (Owner::Unknown, Visibility::Unknown)
    );
}

fn cfg() -> IgnoreConfig {
    IgnoreConfig {
        every_repo: vec!["every".into()],
        personal: vec!["mine".into()],
        organization: vec!["org".into()],
        public: vec!["pub".into()],
        private: vec!["priv".into()],
        ..Default::default()
    }
}

#[test]
fn lists_combine_by_scope() {
    let r = parse_remote("git@github.com:me/app.git");
    let pats = |user: &str, v| patterns_for(&cfg(), &scope_for(r.as_ref(), user, v));
    assert_eq!(pats("me", Visibility::Public), ["every", "mine", "pub"]);
    assert_eq!(pats("me", Visibility::Private), ["every", "mine", "priv"]);
    assert_eq!(pats("other", Visibility::Private), ["every", "org", "priv"]);
    assert_eq!(
        pats("other", Visibility::Unknown),
        ["every", "org", "pub"],
        "unknown visibility is treated as public"
    );
    assert_eq!(
        patterns_for(&cfg(), &scope_for(None, "me", Visibility::Unknown)),
        ["every"],
        "local repos get the every-repo list only"
    );
}

#[test]
fn matcher_follows_gitignore_rules() {
    let m = Matcher::new(&IgnoreConfig::default().public);
    assert!(m.is_ignored(".env"));
    assert!(m.is_ignored("services/api/.env"));
    assert!(m.is_ignored(".env.production"));
    assert!(
        !m.is_ignored(".env.example"),
        "negation keeps the example file"
    );
    assert!(m.is_ignored("certs/server.pem"));
    assert!(m.is_ignored("secrets.yaml"));
    assert!(m.is_ignored(r"keys\id_rsa"), "windows separators");
    assert!(!m.is_ignored("src/main.rs"));
    assert!(!m.is_ignored("environment.md"));

    let dirs = Matcher::new(&[
        "/build".into(),
        ".idea/".into(),
        "# comment".into(),
        "[invalid".into(),
        "".into(),
    ]);
    assert!(dirs.is_ignored("build/out.js"));
    assert!(
        !dirs.is_ignored("src/build/out.js"),
        "leading slash anchors to the repo root"
    );
    assert!(dirs.is_ignored(".idea/workspace.xml"));
    assert!(!dirs.is_ignored("# comment"));
    assert!(!dirs.is_ignored(""));
}

#[test]
fn default_lists_cover_os_junk_everywhere() {
    let d = IgnoreConfig::default();
    let local = Matcher::new(&patterns_for(&d, &scope_for(None, "", Visibility::Unknown)));
    assert!(local.is_ignored(".DS_Store"));
    assert!(local.is_ignored("deep/dir/.DS_Store"));
    assert!(
        !local.is_ignored(".env"),
        "no secrets list without a remote"
    );
}
