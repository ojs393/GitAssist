use gitassist_core::config::{Config, Mode, RuleSetting};

#[test]
fn default_round_trips() {
    let cfg = Config::default();
    let s = cfg.to_toml_string().unwrap();
    assert_eq!(Config::from_toml_str(&s).unwrap(), cfg);
}

#[test]
fn missing_version_migrates_to_current() {
    let cfg = Config::from_toml_str("enabled = false\n").unwrap();
    assert_eq!(cfg.version, 1);
    assert!(!cfg.enabled);
}

#[test]
fn newer_version_is_rejected() {
    assert!(Config::from_toml_str("version = 99\n").is_err());
}

#[test]
fn invalid_toml_is_error() {
    assert!(Config::from_toml_str("version = = 1").is_err());
}

#[test]
fn load_missing_file_gives_defaults() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        Config::load(&dir.path().join("nope.toml")).unwrap(),
        Config::default()
    );
}

#[test]
fn save_preserves_unknown_keys_but_not_removed_overrides() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(
        &path,
        "version = 1\nfuture_flag = true\n[global.style]\nfuture_style = \"x\"\n[tabs.commit]\nsubject_max = { value = 50, mode = \"block\" }\n",
    )
    .unwrap();
    let mut cfg = Config::load(&path).unwrap();
    assert_eq!(
        cfg.tabs.commit.subject_max,
        Some(RuleSetting::new(50, Mode::Block))
    );
    cfg.tabs.commit.subject_max = None;
    cfg.save_atomic(&path).unwrap();

    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(raw.contains("future_flag = true"), "{raw}");
    assert!(raw.contains("future_style"), "{raw}");
    assert!(!raw.contains("subject_max"), "{raw}");
    assert_eq!(Config::load(&path).unwrap().tabs.commit.subject_max, None);
}
