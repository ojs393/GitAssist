use gitassist_setup::envpath::augmented_path;

#[test]
fn keeps_current_path_first_adds_known_dirs_and_dedups() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join(".local/bin")).unwrap();
    let joined = augmented_path(home.path(), false);
    let dirs: Vec<_> = std::env::split_paths(&joined).collect();
    let current: Vec<_> =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
    if let Some(first) = current.first() {
        assert_eq!(
            dirs.first(),
            Some(first),
            "existing PATH order is preserved"
        );
    }
    assert!(
        dirs.contains(&home.path().join(".local/bin")),
        "user bin dirs are added when they exist"
    );
    assert!(
        !dirs.contains(&home.path().join(".cargo/bin")),
        "missing dirs are not added"
    );
    let mut sorted = dirs.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), dirs.len(), "no duplicates");
}
