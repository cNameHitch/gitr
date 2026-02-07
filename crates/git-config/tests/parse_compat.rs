//! Parse compatibility tests — verify parsing of real-world config patterns.

use bstr::BStr;
use git_config::{ConfigFile, ConfigKey, ConfigScope};

#[test]
fn parse_typical_git_config() {
    let input = b"\
[core]
\trepositoryformatversion = 0
\tfilemode = true
\tbare = false
\tlogallrefupdates = true
\tignorecase = true
\tprecomposeunicode = true
[remote \"origin\"]
\turl = https://github.com/user/repo.git
\tfetch = +refs/heads/*:refs/remotes/origin/*
[branch \"main\"]
\tremote = origin
\tmerge = refs/heads/main
[user]
\tname = Alice
\temail = alice@example.com
";
    let file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();

    let key = ConfigKey::parse("core.bare").unwrap();
    assert_eq!(file.get(&key), Some(Some(BStr::new("false"))));

    let key = ConfigKey::parse("remote.origin.url").unwrap();
    assert_eq!(
        file.get(&key),
        Some(Some(BStr::new("https://github.com/user/repo.git")))
    );

    let key = ConfigKey::parse("user.name").unwrap();
    assert_eq!(file.get(&key), Some(Some(BStr::new("Alice"))));

    let key = ConfigKey::parse("branch.main.remote").unwrap();
    assert_eq!(file.get(&key), Some(Some(BStr::new("origin"))));
}

#[test]
fn parse_config_with_includes() {
    let input = b"\
[include]
\tpath = extra.config
[includeIf \"gitdir:~/work/\"]
\tpath = work.config
[user]
\tname = Default
";
    let file = ConfigFile::parse(input, None, ConfigScope::Global).unwrap();

    let key = ConfigKey::parse("user.name").unwrap();
    assert_eq!(file.get(&key), Some(Some(BStr::new("Default"))));

    let entries = file.entries();
    assert!(entries.len() >= 3); // include.path, includeIf.*.path, user.name
}

#[test]
fn parse_config_with_url_rewriting() {
    let input = b"\
[url \"git@github.com:\"]
\tinsteadOf = gh:
\tpushInsteadOf = https://github.com/
[url \"git@gitlab.com:\"]
\tinsteadOf = gl:
";
    let file = ConfigFile::parse(input, None, ConfigScope::Global).unwrap();

    let key = ConfigKey::parse("url.git@github.com:.insteadof").unwrap();
    assert_eq!(file.get(&key), Some(Some(BStr::new("gh:"))));

    let key = ConfigKey::parse("url.git@github.com:.pushinsteadof").unwrap();
    assert_eq!(
        file.get(&key),
        Some(Some(BStr::new("https://github.com/")))
    );
}

#[test]
fn parse_config_all_comment_styles() {
    let input = b"\
# Hash comment
; Semicolon comment
[section]
\tkey = value # Inline hash
\tkey2 = value2 ; Inline semicolon
\tkey3 = \"value3 # not a comment\"
\tkey4 = \"value4 ; not a comment\"
";
    let file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();

    let key = ConfigKey::parse("section.key").unwrap();
    assert_eq!(file.get(&key), Some(Some(BStr::new("value"))));

    let key = ConfigKey::parse("section.key2").unwrap();
    assert_eq!(file.get(&key), Some(Some(BStr::new("value2"))));

    let key = ConfigKey::parse("section.key3").unwrap();
    assert_eq!(
        file.get(&key),
        Some(Some(BStr::new("value3 # not a comment")))
    );

    let key = ConfigKey::parse("section.key4").unwrap();
    assert_eq!(
        file.get(&key),
        Some(Some(BStr::new("value4 ; not a comment")))
    );
}

#[test]
fn parse_config_escape_sequences() {
    let input = b"[section]\n\tkey = \"tab\\there\\nnewline\"\n";
    let file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();

    let key = ConfigKey::parse("section.key").unwrap();
    assert_eq!(
        file.get(&key),
        Some(Some(BStr::new("tab\there\nnewline")))
    );
}

#[test]
fn parse_config_line_continuation() {
    let input = b"[section]\n\tkey = hello \\\n\t\tworld\n";
    let file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();

    let key = ConfigKey::parse("section.key").unwrap();
    let value = file.get(&key).unwrap().unwrap();
    let value_str = std::str::from_utf8(value.as_ref()).unwrap();
    assert!(value_str.contains("hello"));
    assert!(value_str.contains("world"));
}

#[test]
fn parse_config_boolean_variants() {
    let input = b"\
[section]
\tbool1
\tbool2 = true
\tbool3 = yes
\tbool4 = on
\tbool5 = 1
\tbool6 = false
\tbool7 = no
\tbool8 = off
\tbool9 = 0
";
    let file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();

    // boolean key with no value
    let key = ConfigKey::parse("section.bool1").unwrap();
    assert_eq!(file.get(&key), Some(None)); // None value = boolean true

    let key = ConfigKey::parse("section.bool2").unwrap();
    assert_eq!(file.get(&key), Some(Some(BStr::new("true"))));
}

#[test]
fn parse_config_multi_valued() {
    let input = b"\
[remote \"origin\"]
\tfetch = +refs/heads/*:refs/remotes/origin/*
\tfetch = +refs/tags/*:refs/tags/*
\tfetch = +refs/notes/*:refs/notes/*
";
    let file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();

    let key = ConfigKey::parse("remote.origin.fetch").unwrap();
    let values = file.get_all(&key);
    assert_eq!(values.len(), 3);
    assert_eq!(
        values[0],
        Some(BStr::new("+refs/heads/*:refs/remotes/origin/*"))
    );
    assert_eq!(values[1], Some(BStr::new("+refs/tags/*:refs/tags/*")));
}

#[test]
fn parse_empty_config() {
    let file = ConfigFile::parse(b"", None, ConfigScope::Local).unwrap();
    assert!(file.entries().is_empty());
}

#[test]
fn parse_config_with_bom() {
    let mut input = Vec::from(b"\xef\xbb\xbf" as &[u8]);
    input.extend_from_slice(b"[user]\n\tname = Alice\n");

    let file = ConfigFile::parse(&input, None, ConfigScope::Local).unwrap();
    let key = ConfigKey::parse("user.name").unwrap();
    assert_eq!(file.get(&key), Some(Some(BStr::new("Alice"))));
}
