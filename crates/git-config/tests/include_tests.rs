//! Include directive tests.

use bstr::ByteSlice;
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

// ==========================================
// T090: Comprehensive include/includeIf tests
// ==========================================

#[test]
fn circular_include_detected() {
    let dir = tempfile::tempdir().unwrap();

    // Create file A that includes file B
    let a_path = dir.path().join("a.config");
    let b_path = dir.path().join("b.config");

    let a_content = format!(
        "[include]\n\tpath = {}\n[section-a]\n\tkey = from-a\n",
        b_path.display()
    );
    let b_content = format!(
        "[include]\n\tpath = {}\n[section-b]\n\tkey = from-b\n",
        a_path.display()
    );

    fs::write(&a_path, a_content.as_bytes()).unwrap();
    fs::write(&b_path, b_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&a_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    let result = git_config::include::process_includes(set.files_mut(), None, None);
    assert!(result.is_err(), "circular include should produce an error");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("circular") || err_msg.contains("depth"),
        "error should mention circular include or depth: {}",
        err_msg
    );
}

#[test]
fn missing_relative_include_is_silent() {
    let dir = tempfile::tempdir().unwrap();

    // Main config includes a relative path that doesn't exist
    let main_path = dir.path().join("config");
    fs::write(
        &main_path,
        b"[include]\n\tpath = nonexistent-relative.config\n[user]\n\tname = Still Works\n",
    )
    .unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    // Should succeed — missing files are silently ignored
    git_config::include::process_includes(set.files_mut(), None, None).unwrap();

    let key = ConfigKey::parse("user.name").unwrap();
    let found = set.files().iter().any(|f| {
        f.get(&key)
            .and_then(|v| v.map(|val| val.to_str_lossy().to_string()))
            == Some("Still Works".to_string())
    });
    assert!(found, "config values should still be accessible when include is missing");
}

#[test]
fn include_relative_path_subdirectory() {
    let dir = tempfile::tempdir().unwrap();

    // Create a subdirectory with a config file
    let sub_dir = dir.path().join("subdir");
    fs::create_dir(&sub_dir).unwrap();
    let extra_path = sub_dir.join("extra.config");
    fs::write(&extra_path, b"[sub]\n\tkey = from-subdir\n").unwrap();

    // Main config includes with relative path including subdirectory
    let main_path = dir.path().join("config");
    fs::write(
        &main_path,
        b"[include]\n\tpath = subdir/extra.config\n",
    )
    .unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    git_config::include::process_includes(set.files_mut(), None, None).unwrap();

    let key = ConfigKey::parse("sub.key").unwrap();
    let found = set.files().iter().any(|f| f.get(&key).is_some());
    assert!(found, "relative include with subdirectory should work");
}

#[test]
fn include_if_gitdir_matches() {
    let dir = tempfile::tempdir().unwrap();
    let git_dir = dir.path().join("work/.git");
    fs::create_dir_all(&git_dir).unwrap();

    let extra_path = dir.path().join("work-config");
    fs::write(&extra_path, b"[work]\n\tenabled = true\n").unwrap();

    // Pattern matches the git_dir path
    let pattern = format!("{}/", dir.path().join("work/.git").display());
    let main_content = format!(
        "[includeIf \"gitdir:{}\"]\n\tpath = {}\n",
        pattern,
        extra_path.display()
    );
    let main_path = dir.path().join("config");
    fs::write(&main_path, main_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    git_config::include::process_includes(
        set.files_mut(),
        Some(&git_dir),
        None,
    )
    .unwrap();

    let key = ConfigKey::parse("work.enabled").unwrap();
    let found = set.files().iter().any(|f| f.get(&key).is_some());
    assert!(found, "gitdir: condition should match when git_dir matches pattern");
}

#[test]
fn include_if_gitdir_no_match() {
    let dir = tempfile::tempdir().unwrap();
    let git_dir = dir.path().join("personal/.git");
    fs::create_dir_all(&git_dir).unwrap();

    let extra_path = dir.path().join("work-config");
    fs::write(&extra_path, b"[work]\n\tenabled = true\n").unwrap();

    // Pattern does NOT match the actual git_dir
    let main_content = format!(
        "[includeIf \"gitdir:/some/other/path/\"]\n\tpath = {}\n",
        extra_path.display()
    );
    let main_path = dir.path().join("config");
    fs::write(&main_path, main_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    git_config::include::process_includes(
        set.files_mut(),
        Some(&git_dir),
        None,
    )
    .unwrap();

    let key = ConfigKey::parse("work.enabled").unwrap();
    let found = set.files().iter().any(|f| f.get(&key).is_some());
    assert!(!found, "gitdir: condition should NOT match when git_dir differs");
}

#[test]
fn include_if_gitdir_case_insensitive() {
    let dir = tempfile::tempdir().unwrap();
    let git_dir = dir.path().join("Work/.git");
    fs::create_dir_all(&git_dir).unwrap();

    let extra_path = dir.path().join("ci-config");
    fs::write(&extra_path, b"[ci]\n\tenabled = true\n").unwrap();

    // Use gitdir/i: for case-insensitive matching with lowercase pattern
    let pattern = format!("{}/", dir.path().join("work/.git").display());
    let main_content = format!(
        "[includeIf \"gitdir/i:{}\"]\n\tpath = {}\n",
        pattern,
        extra_path.display()
    );
    let main_path = dir.path().join("config");
    fs::write(&main_path, main_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    git_config::include::process_includes(
        set.files_mut(),
        Some(&git_dir),
        None,
    )
    .unwrap();

    let key = ConfigKey::parse("ci.enabled").unwrap();
    let found = set.files().iter().any(|f| f.get(&key).is_some());
    assert!(
        found,
        "gitdir/i: condition should match case-insensitively"
    );
}

#[test]
fn include_if_onbranch_wildcard_deep() {
    let dir = tempfile::tempdir().unwrap();

    let extra_path = dir.path().join("feature.config");
    fs::write(&extra_path, b"[feature]\n\tactive = true\n").unwrap();

    let main_content = format!(
        "[includeIf \"onbranch:feature/**\"]\n\tpath = {}\n",
        extra_path.display()
    );
    let main_path = dir.path().join("config");
    fs::write(&main_path, main_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    // Should match nested feature branch
    git_config::include::process_includes(
        set.files_mut(),
        None,
        Some("refs/heads/feature/my-feature/sub"),
    )
    .unwrap();

    let key = ConfigKey::parse("feature.active").unwrap();
    let found = set.files().iter().any(|f| f.get(&key).is_some());
    assert!(found, "onbranch:feature/** should match deep nested branches");
}

#[test]
fn include_if_onbranch_no_branch_returns_false() {
    let dir = tempfile::tempdir().unwrap();

    let extra_path = dir.path().join("branch.config");
    fs::write(&extra_path, b"[branch-cfg]\n\tactive = true\n").unwrap();

    let main_content = format!(
        "[includeIf \"onbranch:main\"]\n\tpath = {}\n",
        extra_path.display()
    );
    let main_path = dir.path().join("config");
    fs::write(&main_path, main_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    // Process with no branch (detached HEAD)
    git_config::include::process_includes(set.files_mut(), None, None).unwrap();

    let key = ConfigKey::parse("branch-cfg.active").unwrap();
    let found = set.files().iter().any(|f| f.get(&key).is_some());
    assert!(!found, "onbranch: should not match when no branch is set (detached HEAD)");
}

#[test]
fn include_if_hasconfig_remote_url_matches() {
    let dir = tempfile::tempdir().unwrap();

    let extra_path = dir.path().join("github.config");
    fs::write(&extra_path, b"[github]\n\tuser = myuser\n").unwrap();

    // Main config has a remote url and a conditional include that matches it
    let main_content = format!(
        "[remote \"origin\"]\n\turl = https://github.com/user/repo.git\n\
         [includeIf \"hasconfig:remote.*.url:https://github.com/**\"]\n\tpath = {}\n",
        extra_path.display()
    );
    let main_path = dir.path().join("config");
    fs::write(&main_path, main_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    git_config::include::process_includes(set.files_mut(), None, None).unwrap();

    let key = ConfigKey::parse("github.user").unwrap();
    let found = set.files().iter().any(|f| f.get(&key).is_some());
    assert!(found, "hasconfig:remote.*.url should match when a remote URL matches the pattern");
}

#[test]
fn include_if_hasconfig_remote_url_no_match() {
    let dir = tempfile::tempdir().unwrap();

    let extra_path = dir.path().join("github.config");
    fs::write(&extra_path, b"[github]\n\tuser = myuser\n").unwrap();

    // Main config has a remote url to gitlab, condition checks for github
    let main_content = format!(
        "[remote \"origin\"]\n\turl = https://gitlab.com/user/repo.git\n\
         [includeIf \"hasconfig:remote.*.url:https://github.com/**\"]\n\tpath = {}\n",
        extra_path.display()
    );
    let main_path = dir.path().join("config");
    fs::write(&main_path, main_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    git_config::include::process_includes(set.files_mut(), None, None).unwrap();

    let key = ConfigKey::parse("github.user").unwrap();
    let found = set.files().iter().any(|f| f.get(&key).is_some());
    assert!(!found, "hasconfig:remote.*.url should NOT match when no remote URL matches");
}

#[test]
fn include_multiple_directives_in_one_file() {
    let dir = tempfile::tempdir().unwrap();

    let a_path = dir.path().join("a.config");
    let b_path = dir.path().join("b.config");
    fs::write(&a_path, b"[from-a]\n\tkey = value-a\n").unwrap();
    fs::write(&b_path, b"[from-b]\n\tkey = value-b\n").unwrap();

    let main_content = format!(
        "[include]\n\tpath = {}\n[include]\n\tpath = {}\n[user]\n\tname = Main\n",
        a_path.display(),
        b_path.display()
    );
    let main_path = dir.path().join("config");
    fs::write(&main_path, main_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    git_config::include::process_includes(set.files_mut(), None, None).unwrap();

    let key_a = ConfigKey::parse("from-a.key").unwrap();
    let key_b = ConfigKey::parse("from-b.key").unwrap();
    let found_a = set.files().iter().any(|f| f.get(&key_a).is_some());
    let found_b = set.files().iter().any(|f| f.get(&key_b).is_some());
    assert!(found_a, "first include should be loaded");
    assert!(found_b, "second include should be loaded");
}

#[test]
fn include_overrides_earlier_values() {
    let dir = tempfile::tempdir().unwrap();

    // Included file overrides a value set before the include directive
    let extra_path = dir.path().join("override.config");
    fs::write(&extra_path, b"[user]\n\tname = Overridden\n").unwrap();

    let main_content = format!(
        "[user]\n\tname = Original\n[include]\n\tpath = {}\n",
        extra_path.display()
    );
    let main_path = dir.path().join("config");
    fs::write(&main_path, main_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    git_config::include::process_includes(set.files_mut(), None, None).unwrap();

    // The included file's value should be present (it comes after the original)
    let key = ConfigKey::parse("user.name").unwrap();
    let mut found_overridden = false;
    // Check in file order — the included file should appear after the main file
    for file in set.files() {
        if let Some(Some(val)) = file.get(&key) {
            if val.to_str_lossy() == "Overridden" {
                found_overridden = true;
            }
        }
    }
    assert!(found_overridden, "included file's value should be accessible");
}

#[test]
fn include_if_unknown_condition_ignored() {
    let dir = tempfile::tempdir().unwrap();

    let extra_path = dir.path().join("unknown.config");
    fs::write(&extra_path, b"[unknown]\n\tkey = value\n").unwrap();

    // Use an unknown condition type
    let main_content = format!(
        "[includeIf \"unknownCondition:foo\"]\n\tpath = {}\n[user]\n\tname = Main\n",
        extra_path.display()
    );
    let main_path = dir.path().join("config");
    fs::write(&main_path, main_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    // Should succeed — unknown conditions are simply not included
    git_config::include::process_includes(set.files_mut(), None, None).unwrap();

    let key = ConfigKey::parse("unknown.key").unwrap();
    let found = set.files().iter().any(|f| f.get(&key).is_some());
    assert!(!found, "unknown condition should be treated as false");

    // Main file values should still be present
    let user_key = ConfigKey::parse("user.name").unwrap();
    let found_user = set.files().iter().any(|f| f.get(&user_key).is_some());
    assert!(found_user, "main config values should still be accessible");
}

#[test]
fn include_chained_includes() {
    let dir = tempfile::tempdir().unwrap();

    // Chain: main -> a -> b
    let b_path = dir.path().join("b.config");
    fs::write(&b_path, b"[from-b]\n\tkey = deep-value\n").unwrap();

    let a_content = format!(
        "[include]\n\tpath = {}\n[from-a]\n\tkey = a-value\n",
        b_path.display()
    );
    let a_path = dir.path().join("a.config");
    fs::write(&a_path, a_content.as_bytes()).unwrap();

    let main_content = format!(
        "[include]\n\tpath = {}\n[main]\n\tkey = main-value\n",
        a_path.display()
    );
    let main_path = dir.path().join("config");
    fs::write(&main_path, main_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    git_config::include::process_includes(set.files_mut(), None, None).unwrap();

    let key_main = ConfigKey::parse("main.key").unwrap();
    let key_a = ConfigKey::parse("from-a.key").unwrap();
    let key_b = ConfigKey::parse("from-b.key").unwrap();

    let found_main = set.files().iter().any(|f| f.get(&key_main).is_some());
    let found_a = set.files().iter().any(|f| f.get(&key_a).is_some());
    let found_b = set.files().iter().any(|f| f.get(&key_b).is_some());

    assert!(found_main, "main config values should be present");
    assert!(found_a, "first-level include values should be present");
    assert!(found_b, "second-level (chained) include values should be present");
}

#[test]
fn include_if_gitdir_no_git_dir_returns_false() {
    let dir = tempfile::tempdir().unwrap();

    let extra_path = dir.path().join("gitdir.config");
    fs::write(&extra_path, b"[gitdir-cfg]\n\tactive = true\n").unwrap();

    let main_content = format!(
        "[includeIf \"gitdir:/some/path/\"]\n\tpath = {}\n",
        extra_path.display()
    );
    let main_path = dir.path().join("config");
    fs::write(&main_path, main_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    // Process with no git_dir (None)
    git_config::include::process_includes(set.files_mut(), None, None).unwrap();

    let key = ConfigKey::parse("gitdir-cfg.active").unwrap();
    let found = set.files().iter().any(|f| f.get(&key).is_some());
    assert!(!found, "gitdir: should not match when no git_dir is provided");
}

#[test]
fn include_preserves_scope_from_parent() {
    let dir = tempfile::tempdir().unwrap();

    let extra_path = dir.path().join("extra.config");
    fs::write(&extra_path, b"[extra]\n\tkey = from-include\n").unwrap();

    let main_content = format!(
        "[include]\n\tpath = {}\n",
        extra_path.display()
    );
    let main_path = dir.path().join("config");
    fs::write(&main_path, main_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Global).unwrap();
    set.add_file(file);

    git_config::include::process_includes(set.files_mut(), None, None).unwrap();

    // The included file should inherit the scope from the parent
    let mut found_scope = None;
    for file in set.files() {
        let key = ConfigKey::parse("extra.key").unwrap();
        if file.get(&key).is_some() {
            found_scope = Some(file.scope());
        }
    }
    assert_eq!(
        found_scope,
        Some(ConfigScope::Global),
        "included file should inherit scope from parent config file"
    );
}

#[test]
fn include_absolute_path_works() {
    let dir = tempfile::tempdir().unwrap();

    let extra_path = dir.path().join("absolute.config");
    fs::write(&extra_path, b"[absolute]\n\tkey = yes\n").unwrap();

    let main_content = format!(
        "[include]\n\tpath = {}\n",
        extra_path.display()
    );
    let main_path = dir.path().join("config");
    fs::write(&main_path, main_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    git_config::include::process_includes(set.files_mut(), None, None).unwrap();

    let key = ConfigKey::parse("absolute.key").unwrap();
    let found = set.files().iter().any(|f| f.get(&key).is_some());
    assert!(found, "absolute path include should work");
}

#[test]
fn include_if_gitdir_with_glob_pattern() {
    let dir = tempfile::tempdir().unwrap();
    let git_dir = dir.path().join("projects/myrepo/.git");
    fs::create_dir_all(&git_dir).unwrap();

    let extra_path = dir.path().join("projects.config");
    fs::write(&extra_path, b"[projects]\n\tactive = true\n").unwrap();

    // Use glob pattern matching any repo under projects/
    let pattern = format!("{}/*/", dir.path().join("projects").display());
    let main_content = format!(
        "[includeIf \"gitdir:{}\"]\n\tpath = {}\n",
        pattern,
        extra_path.display()
    );
    let main_path = dir.path().join("config");
    fs::write(&main_path, main_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    git_config::include::process_includes(
        set.files_mut(),
        Some(&git_dir),
        None,
    )
    .unwrap();

    let key = ConfigKey::parse("projects.active").unwrap();
    let found = set.files().iter().any(|f| f.get(&key).is_some());
    assert!(found, "gitdir: with glob pattern should match");
}

#[test]
fn include_empty_file_is_ok() {
    let dir = tempfile::tempdir().unwrap();

    let extra_path = dir.path().join("empty.config");
    fs::write(&extra_path, b"").unwrap();

    let main_content = format!(
        "[include]\n\tpath = {}\n[user]\n\tname = Main\n",
        extra_path.display()
    );
    let main_path = dir.path().join("config");
    fs::write(&main_path, main_content.as_bytes()).unwrap();

    let mut set = ConfigSet::new();
    let file = ConfigFile::load(&main_path, ConfigScope::Local).unwrap();
    set.add_file(file);

    git_config::include::process_includes(set.files_mut(), None, None).unwrap();

    let key = ConfigKey::parse("user.name").unwrap();
    let found = set.files().iter().any(|f| f.get(&key).is_some());
    assert!(found, "including an empty file should not cause issues");
}
