//! Include directive tests.

use git_config::{ConfigFile, ConfigKey, ConfigScope, ConfigSet};

use std::fs;

#[test]
fn include_path_loads_file() {
    let dir = tempfile::tempdir().unwrap();

    // Create the included file
    let extra_path = dir.path().join("extra.config");
    fs::write(
        &extra_path,
        b"[extra]\n\tkey = from-include\n",
    )
    .unwrap();

    // Create the main config that includes it
    let main_content = format!(
        "[include]\n\tpath = {}\n[user]\n\tname = Main\n",
        extra_path.display()
    );
    let main_path = dir.path().join("config");
    fs::write(&main_path, main_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    // Process includes
    git_config::include::process_includes(set.files_mut(), None, None).unwrap();

    // The included file's values should be accessible
    let key = ConfigKey::parse("extra.key").unwrap();
    let found = set.files().iter().any(|f| f.get(&key).is_some());
    assert!(found, "included file's key should be accessible");
}

#[test]
fn include_relative_path() {
    let dir = tempfile::tempdir().unwrap();

    // Create the included file
    let extra_path = dir.path().join("extra.config");
    fs::write(
        &extra_path,
        b"[extra]\n\tkey = relative-include\n",
    )
    .unwrap();

    // Create the main config with a relative include
    let main_path = dir.path().join("config");
    fs::write(
        &main_path,
        b"[include]\n\tpath = extra.config\n[user]\n\tname = Main\n",
    )
    .unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    git_config::include::process_includes(set.files_mut(), None, None).unwrap();

    let key = ConfigKey::parse("extra.key").unwrap();
    let found = set.files().iter().any(|f| f.get(&key).is_some());
    assert!(found, "relative include should work");
}

#[test]
fn include_if_onbranch_matches() {
    let dir = tempfile::tempdir().unwrap();

    let extra_path = dir.path().join("branch.config");
    fs::write(&extra_path, b"[branch-config]\n\tactive = true\n").unwrap();

    let main_content = format!(
        "[includeIf \"onbranch:main\"]\n\tpath = {}\n",
        extra_path.display()
    );
    let main_path = dir.path().join("config");
    fs::write(&main_path, main_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    // Process with branch = main
    git_config::include::process_includes(
        set.files_mut(),
        None,
        Some("refs/heads/main"),
    )
    .unwrap();

    let key = ConfigKey::parse("branch-config.active").unwrap();
    let found = set.files().iter().any(|f| f.get(&key).is_some());
    assert!(found, "onbranch:main should match when on main");
}

#[test]
fn include_if_onbranch_no_match() {
    let dir = tempfile::tempdir().unwrap();

    let extra_path = dir.path().join("branch.config");
    fs::write(&extra_path, b"[branch-config]\n\tactive = true\n").unwrap();

    let main_content = format!(
        "[includeIf \"onbranch:main\"]\n\tpath = {}\n",
        extra_path.display()
    );
    let main_path = dir.path().join("config");
    fs::write(&main_path, main_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    // Process with a different branch
    git_config::include::process_includes(
        set.files_mut(),
        None,
        Some("refs/heads/develop"),
    )
    .unwrap();

    let key = ConfigKey::parse("branch-config.active").unwrap();
    let found = set.files().iter().any(|f| f.get(&key).is_some());
    assert!(!found, "onbranch:main should NOT match when on develop");
}

#[test]
fn include_missing_file_is_silent() {
    let dir = tempfile::tempdir().unwrap();

    let main_path = dir.path().join("config");
    fs::write(
        &main_path,
        b"[include]\n\tpath = /nonexistent/file.config\n[user]\n\tname = Main\n",
    )
    .unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    // Should not error on missing include file
    git_config::include::process_includes(set.files_mut(), None, None).unwrap();

    let key = ConfigKey::parse("user.name").unwrap();
    assert!(set.files().iter().any(|f| f.get(&key).is_some()));
}
