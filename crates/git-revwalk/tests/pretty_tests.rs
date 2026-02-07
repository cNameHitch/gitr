//! Pretty-print formatting tests.

use bstr::BString;
use git_hash::ObjectId;
use git_object::Commit;
use git_utils::date::{GitDate, Signature};

use git_revwalk::{format_commit, format_builtin, BuiltinFormat, FormatOptions};

fn make_commit() -> (Commit, ObjectId) {
    let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
    let tree = ObjectId::from_hex("4b825dc642cb6eb9a060e54bf899d15363da7566").unwrap();
    let parent = ObjectId::from_hex("0000000000000000000000000000000000000001").unwrap();

    let commit = Commit {
        tree,
        parents: vec![parent],
        author: Signature {
            name: BString::from("John Doe"),
            email: BString::from("john@example.com"),
            date: GitDate::new(1700000000, 0),
        },
        committer: Signature {
            name: BString::from("Jane Doe"),
            email: BString::from("jane@example.com"),
            date: GitDate::new(1700001000, -300),
        },
        encoding: None,
        gpgsig: None,
        extra_headers: vec![],
        message: BString::from("Initial commit\n\nThis is the body.\n"),
    };

    (commit, oid)
}

#[test]
fn format_full_hash() {
    let (commit, oid) = make_commit();
    let opts = FormatOptions::default();
    let result = format_commit(&commit, &oid, "%H", &opts);
    assert_eq!(result, "da39a3ee5e6b4b0d3255bfef95601890afd80709");
}

#[test]
fn format_short_hash() {
    let (commit, oid) = make_commit();
    let opts = FormatOptions::default();
    let result = format_commit(&commit, &oid, "%h", &opts);
    assert_eq!(result, "da39a3e");
}

#[test]
fn format_tree_hash() {
    let (commit, oid) = make_commit();
    let opts = FormatOptions::default();
    let result = format_commit(&commit, &oid, "%T", &opts);
    assert_eq!(result, "4b825dc642cb6eb9a060e54bf899d15363da7566");
}

#[test]
fn format_parent_hashes() {
    let (commit, oid) = make_commit();
    let opts = FormatOptions::default();
    let result = format_commit(&commit, &oid, "%P", &opts);
    assert_eq!(result, "0000000000000000000000000000000000000001");
}

#[test]
fn format_author_name() {
    let (commit, oid) = make_commit();
    let opts = FormatOptions::default();
    let result = format_commit(&commit, &oid, "%an", &opts);
    assert_eq!(result, "John Doe");
}

#[test]
fn format_author_email() {
    let (commit, oid) = make_commit();
    let opts = FormatOptions::default();
    let result = format_commit(&commit, &oid, "%ae", &opts);
    assert_eq!(result, "john@example.com");
}

#[test]
fn format_committer_name() {
    let (commit, oid) = make_commit();
    let opts = FormatOptions::default();
    let result = format_commit(&commit, &oid, "%cn", &opts);
    assert_eq!(result, "Jane Doe");
}

#[test]
fn format_subject() {
    let (commit, oid) = make_commit();
    let opts = FormatOptions::default();
    let result = format_commit(&commit, &oid, "%s", &opts);
    assert_eq!(result, "Initial commit");
}

#[test]
fn format_body() {
    let (commit, oid) = make_commit();
    let opts = FormatOptions::default();
    let result = format_commit(&commit, &oid, "%b", &opts);
    assert_eq!(result, "This is the body.\n");
}

#[test]
fn format_full_message() {
    let (commit, oid) = make_commit();
    let opts = FormatOptions::default();
    let result = format_commit(&commit, &oid, "%B", &opts);
    assert_eq!(result, "Initial commit\n\nThis is the body.\n");
}

#[test]
fn format_newline() {
    let (commit, oid) = make_commit();
    let opts = FormatOptions::default();
    let result = format_commit(&commit, &oid, "%H%n%s", &opts);
    assert_eq!(
        result,
        "da39a3ee5e6b4b0d3255bfef95601890afd80709\nInitial commit"
    );
}

#[test]
fn format_literal_percent() {
    let (commit, oid) = make_commit();
    let opts = FormatOptions::default();
    let result = format_commit(&commit, &oid, "100%%", &opts);
    assert_eq!(result, "100%");
}

#[test]
fn format_author_unix_timestamp() {
    let (commit, oid) = make_commit();
    let opts = FormatOptions::default();
    let result = format_commit(&commit, &oid, "%at", &opts);
    assert_eq!(result, "1700000000");
}

#[test]
fn format_committer_unix_timestamp() {
    let (commit, oid) = make_commit();
    let opts = FormatOptions::default();
    let result = format_commit(&commit, &oid, "%ct", &opts);
    assert_eq!(result, "1700001000");
}

#[test]
fn format_combined() {
    let (commit, oid) = make_commit();
    let opts = FormatOptions::default();
    let result = format_commit(&commit, &oid, "%h %an %s", &opts);
    assert_eq!(result, "da39a3e John Doe Initial commit");
}

#[test]
fn builtin_oneline() {
    let (commit, oid) = make_commit();
    let opts = FormatOptions::default();
    let result = format_builtin(&commit, &oid, BuiltinFormat::Oneline, &opts);
    assert_eq!(result, "da39a3e Initial commit");
}

#[test]
fn builtin_short() {
    let (commit, oid) = make_commit();
    let opts = FormatOptions::default();
    let result = format_builtin(&commit, &oid, BuiltinFormat::Short, &opts);
    assert!(result.contains("commit da39a3ee5e6b4b0d3255bfef95601890afd80709"));
    assert!(result.contains("Author: John Doe <john@example.com>"));
    assert!(result.contains("    Initial commit"));
}

#[test]
fn builtin_raw() {
    let (commit, oid) = make_commit();
    let opts = FormatOptions::default();
    let result = format_builtin(&commit, &oid, BuiltinFormat::Raw, &opts);
    assert!(result.contains("commit da39a3ee5e6b4b0d3255bfef95601890afd80709"));
    assert!(result.contains("tree 4b825dc642cb6eb9a060e54bf899d15363da7566"));
    assert!(result.contains("parent 0000000000000000000000000000000000000001"));
    assert!(result.contains("author John Doe"));
    assert!(result.contains("committer Jane Doe"));
}
