use std::fs;
use std::path::Path;

use gitassist_core::Config;
use gitassist_gateway::Paths;
use gitassist_setup::adapters::{self, Status};
use gitassist_setup::{blocks, jsonc, Env};

struct Fixture {
    _tmp: tempfile::TempDir,
    env: Env,
    paths: Paths,
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let app_config = tmp.path().join("appconfig");
    let ga = tmp.path().join("ga");
    for d in [&home, &app_config, &ga] {
        fs::create_dir_all(d).unwrap();
    }
    Fixture {
        env: Env { home, app_config },
        paths: Paths::from_dirs(ga.clone(), ga),
        _tmp: tmp,
    }
}

fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    fn walk(d: &Path, root: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        for e in fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, root, out);
            } else {
                out.push((
                    p.strip_prefix(root).unwrap().display().to_string(),
                    fs::read(&p).unwrap(),
                ));
            }
        }
    }
    walk(dir, dir, &mut out);
    out.sort();
    out
}

fn round_trip(id: &str, f: &Fixture) {
    let cfg = Config::default();
    let before = snapshot(&f.env.home);
    let before_cfg = snapshot(&f.env.app_config);
    let a = adapters::find(&f.env, &cfg, id).unwrap();
    assert!(a.detected, "{id} detected");
    assert_eq!(adapters::status(&a, &f.paths), Status::NotApplied, "{id}");
    adapters::apply(&a, &f.paths).unwrap();
    assert_eq!(adapters::status(&a, &f.paths), Status::Current, "{id}");
    adapters::apply(&a, &f.paths).unwrap();
    assert_eq!(
        adapters::status(&a, &f.paths),
        Status::Current,
        "{id} re-apply is idempotent"
    );
    adapters::remove(&a, &f.paths).unwrap();
    assert_eq!(
        snapshot(&f.env.home),
        before,
        "{id} restores home byte-for-byte"
    );
    assert_eq!(
        snapshot(&f.env.app_config),
        before_cfg,
        "{id} restores app config byte-for-byte"
    );
}

#[test]
fn claude_round_trip_preserves_existing_settings() {
    let f = fixture();
    let dir = f.env.home.join(".claude");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("settings.json"),
        "{\n  \"model\": \"opus\",\n  \"theme\": \"dark\"\n}\n",
    )
    .unwrap();
    fs::write(dir.join("CLAUDE.md"), "# My rules\n\nBe nice.\n").unwrap();

    let a = adapters::find(&f.env, &Config::default(), "claude").unwrap();
    adapters::apply(&a, &f.paths).unwrap();
    let settings = fs::read_to_string(dir.join("settings.json")).unwrap();
    assert!(settings.contains("\"model\": \"opus\""));
    assert_eq!(
        jsonc::get(&settings, &["attribution", "commit"]).unwrap(),
        Some(serde_json::json!(""))
    );
    let md = fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
    assert!(md.starts_with("# My rules\n\nBe nice.\n"));
    assert!(blocks::contains(&md, blocks::MD));
    adapters::remove(&a, &f.paths).unwrap();

    round_trip("claude", &f);
}

#[test]
fn claude_keeps_users_existing_attribution_value_on_remove() {
    let f = fixture();
    let dir = f.env.home.join(".claude");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("settings.json"),
        "{\n  \"attribution\": { \"commit\": \"custom\" }\n}\n",
    )
    .unwrap();
    round_trip("claude", &f);
}

#[test]
fn user_edits_after_apply_survive_removal() {
    let f = fixture();
    let dir = f.env.home.join(".claude");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("settings.json"), "{\n  \"model\": \"opus\"\n}\n").unwrap();
    let a = adapters::find(&f.env, &Config::default(), "claude").unwrap();
    adapters::apply(&a, &f.paths).unwrap();
    let edited = jsonc::set(
        &fs::read_to_string(dir.join("settings.json")).unwrap(),
        &["theme"],
        Some(&serde_json::json!("light")),
    )
    .unwrap();
    fs::write(dir.join("settings.json"), edited).unwrap();
    adapters::remove(&a, &f.paths).unwrap();
    let s = fs::read_to_string(dir.join("settings.json")).unwrap();
    assert_eq!(
        jsonc::get(&s, &["theme"]).unwrap(),
        Some(serde_json::json!("light"))
    );
    assert_eq!(jsonc::get(&s, &["attribution", "commit"]).unwrap(), None);
    assert_eq!(
        jsonc::get(&s, &["model"]).unwrap(),
        Some(serde_json::json!("opus"))
    );
}

#[test]
fn cursor_only_edits_existing_cli_config() {
    let f = fixture();
    fs::create_dir_all(f.env.home.join(".cursor")).unwrap();
    let a = adapters::find(&f.env, &Config::default(), "cursor").unwrap();
    adapters::apply(&a, &f.paths).unwrap();
    assert!(!f.env.home.join(".cursor/cli-config.json").exists());
    assert!(!a.manual.is_empty());
    adapters::remove(&a, &f.paths).unwrap();

    fs::write(
        f.env.home.join(".cursor/cli-config.json"),
        "{ \"version\": 1 }\n",
    )
    .unwrap();
    round_trip("cursor", &f);
}

#[test]
fn copilot_round_trip_keeps_jsonc_comments() {
    let f = fixture();
    fs::create_dir_all(f.env.home.join(".copilot")).unwrap();
    let vscode = f.env.app_config.join("Code/User");
    fs::create_dir_all(&vscode).unwrap();
    fs::write(
        vscode.join("settings.json"),
        "{\n  // my font\n  \"editor.fontSize\": 14,\n}\n",
    )
    .unwrap();
    let a = adapters::find(&f.env, &Config::default(), "copilot").unwrap();
    adapters::apply(&a, &f.paths).unwrap();
    let s = fs::read_to_string(vscode.join("settings.json")).unwrap();
    assert!(s.contains("// my font"), "{s}");
    assert!(
        s.contains("github.copilot.chat.commitMessageGeneration.instructions"),
        "{s}"
    );
    adapters::remove(&a, &f.paths).unwrap();
    round_trip("copilot", &f);
}

#[test]
fn codex_prefers_active_override_file() {
    let f = fixture();
    let dir = f.env.home.join(".codex");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("AGENTS.override.md"), "Override rules\n").unwrap();
    let a = adapters::find(&f.env, &Config::default(), "codex").unwrap();
    assert!(a.changes[0].path().ends_with("AGENTS.override.md"));
    round_trip("codex", &f);
}

#[test]
fn gemini_round_trip_creates_and_removes_file() {
    let f = fixture();
    fs::create_dir_all(f.env.home.join(".gemini")).unwrap();
    round_trip("gemini", &f);
}

#[test]
fn aider_round_trip_keeps_comments() {
    let f = fixture();
    fs::write(
        f.env.home.join(".aider.conf.yml"),
        "# my aider config\nmodel: sonnet\nattribute-author: true\n",
    )
    .unwrap();
    let a = adapters::find(&f.env, &Config::default(), "aider").unwrap();
    adapters::apply(&a, &f.paths).unwrap();
    let y = fs::read_to_string(f.env.home.join(".aider.conf.yml")).unwrap();
    assert!(
        y.contains("# my aider config\nmodel: sonnet\nattribute-author: false\n"),
        "{y}"
    );
    assert!(y.contains("attribute-co-authored-by: false"));
    assert!(y.contains("commit-prompt: \""));
    adapters::remove(&a, &f.paths).unwrap();
    round_trip("aider", &f);
}

#[test]
fn not_installed_agents_are_not_detected() {
    let f = fixture();
    let cfg = Config::default();
    let a = adapters::find(&f.env, &cfg, "gemini").unwrap();
    if !Env::on_path("gemini") {
        assert_eq!(adapters::status(&a, &f.paths), Status::NotDetected);
    }
}
