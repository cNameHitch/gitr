//! Write round-trip tests — write, re-parse, verify values and formatting.

use bstr::BStr;
use git_config::{ConfigFile, ConfigKey, ConfigScope};


#[test]
fn roundtrip_preserves_formatting() {
    let input = b"# Main config\n[user]\n\tname = Alice\n\temail = alice@example.com\n\n# Core settings\n[core]\n\tbare = false\n";
    let file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();
    let output = file.to_bytes();
    assert_eq!(output, input);
}

#[test]
fn roundtrip_after_set() {
    let input = b"[user]\n\tname = Alice\n\temail = alice@example.com\n";
    let mut file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();

    let key = ConfigKey::parse("user.name").unwrap();
    file.set(&key, BStr::new("Bob"));

    // Re-parse the output
    let output = file.to_bytes();
    let file2 = ConfigFile::parse(&output, None, ConfigScope::Local).unwrap();

    assert_eq!(file2.get(&key), Some(Some(BStr::new("Bob"))));

    // Email should be preserved
    let email_key = ConfigKey::parse("user.email").unwrap();
    assert_eq!(
        file2.get(&email_key),
        Some(Some(BStr::new("alice@example.com")))
    );
}

#[test]
fn roundtrip_after_add_new_key() {
    let input = b"[user]\n\tname = Alice\n";
    let mut file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();

    let key = ConfigKey::parse("user.email").unwrap();
    file.set(&key, BStr::new("alice@example.com"));

    let output = file.to_bytes();
    let file2 = ConfigFile::parse(&output, None, ConfigScope::Local).unwrap();

    assert_eq!(
        file2.get(&key),
        Some(Some(BStr::new("alice@example.com")))
    );

    // Original key should still be there
    let name_key = ConfigKey::parse("user.name").unwrap();
    assert_eq!(file2.get(&name_key), Some(Some(BStr::new("Alice"))));
}

#[test]
fn roundtrip_after_add_new_section() {
    let input = b"[user]\n\tname = Alice\n";
    let mut file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();

    let key = ConfigKey::parse("core.bare").unwrap();
    file.set(&key, BStr::new("false"));

    let output = file.to_bytes();
    let file2 = ConfigFile::parse(&output, None, ConfigScope::Local).unwrap();

    assert_eq!(file2.get(&key), Some(Some(BStr::new("false"))));

    let name_key = ConfigKey::parse("user.name").unwrap();
    assert_eq!(file2.get(&name_key), Some(Some(BStr::new("Alice"))));
}

#[test]
fn roundtrip_after_remove() {
    let input = b"[user]\n\tname = Alice\n\temail = alice@example.com\n";
    let mut file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();

    let key = ConfigKey::parse("user.name").unwrap();
    assert!(file.remove(&key));

    let output = file.to_bytes();
    let file2 = ConfigFile::parse(&output, None, ConfigScope::Local).unwrap();

    assert_eq!(file2.get(&key), None);

    let email_key = ConfigKey::parse("user.email").unwrap();
    assert_eq!(
        file2.get(&email_key),
        Some(Some(BStr::new("alice@example.com")))
    );
}

#[test]
fn roundtrip_after_remove_section() {
    let input = b"[user]\n\tname = Alice\n[core]\n\tbare = false\n";
    let mut file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();

    assert!(file.remove_section(BStr::new("user"), None));

    let output = file.to_bytes();
    let file2 = ConfigFile::parse(&output, None, ConfigScope::Local).unwrap();

    let name_key = ConfigKey::parse("user.name").unwrap();
    assert_eq!(file2.get(&name_key), None);

    let bare_key = ConfigKey::parse("core.bare").unwrap();
    assert_eq!(file2.get(&bare_key), Some(Some(BStr::new("false"))));
}

#[test]
fn write_to_file_and_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config");

    let input = b"[user]\n\tname = Alice\n\temail = alice@example.com\n";
    let file = ConfigFile::parse(input, Some(&config_path), ConfigScope::Local).unwrap();
    file.write_to(&config_path).unwrap();

    // Read back
    let file2 = ConfigFile::load(&config_path, ConfigScope::Local).unwrap();
    let key = ConfigKey::parse("user.name").unwrap();
    assert_eq!(file2.get(&key), Some(Some(BStr::new("Alice"))));
}

#[test]
fn write_special_characters() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config");

    let mut file = ConfigFile::parse(b"", None, ConfigScope::Local).unwrap();
    let key = ConfigKey::parse("section.key").unwrap();
    file.set(&key, BStr::new("value with spaces"));
    file.write_to(&config_path).unwrap();

    let file2 = ConfigFile::load(&config_path, ConfigScope::Local).unwrap();
    assert_eq!(
        file2.get(&key),
        Some(Some(BStr::new("value with spaces")))
    );
}

#[test]
fn write_value_requiring_quotes() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config");

    let mut file = ConfigFile::parse(b"", None, ConfigScope::Local).unwrap();

    // Value with # should be quoted
    let key = ConfigKey::parse("section.key").unwrap();
    file.set(&key, BStr::new("value # with hash"));
    file.write_to(&config_path).unwrap();

    let file2 = ConfigFile::load(&config_path, ConfigScope::Local).unwrap();
    assert_eq!(
        file2.get(&key),
        Some(Some(BStr::new("value # with hash")))
    );
}
