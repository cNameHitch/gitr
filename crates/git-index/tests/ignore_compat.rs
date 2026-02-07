//! Gitignore compatibility tests.

use std::path::Path;

use bstr::BStr;
use git_index::ignore::IgnoreStack;

fn stack_from(content: &[u8]) -> IgnoreStack {
    let mut stack = IgnoreStack::new();
    stack.add_patterns(content, Path::new(".gitignore"), Path::new("."));
    stack
}

#[test]
fn simple_wildcard() {
    let stack = stack_from(b"*.o\n");
    assert!(stack.is_ignored(BStr::new(b"test.o"), false));
    assert!(stack.is_ignored(BStr::new(b"dir/test.o"), false));
    assert!(!stack.is_ignored(BStr::new(b"test.c"), false));
}

#[test]
fn negation() {
    let stack = stack_from(b"*.o\n!important.o\n");
    assert!(stack.is_ignored(BStr::new(b"test.o"), false));
    assert!(!stack.is_ignored(BStr::new(b"important.o"), false));
}

#[test]
fn directory_only() {
    let stack = stack_from(b"build/\n");
    assert!(stack.is_ignored(BStr::new(b"build"), true));
    assert!(!stack.is_ignored(BStr::new(b"build"), false));
}

#[test]
fn anchored_pattern() {
    let stack = stack_from(b"/TODO\n");
    assert!(stack.is_ignored(BStr::new(b"TODO"), false));
    // Anchored patterns should match from base dir only
    // When used with basename matching, "TODO" in a subdirectory
    // would need more context (base_dir) to properly test
}

#[test]
fn double_star() {
    let stack = stack_from(b"**/foo\n");
    assert!(stack.is_ignored(BStr::new(b"foo"), false));
    assert!(stack.is_ignored(BStr::new(b"dir/foo"), false));
    assert!(stack.is_ignored(BStr::new(b"dir/sub/foo"), false));
}

#[test]
fn double_star_slash() {
    let stack = stack_from(b"**/foo/bar\n");
    assert!(stack.is_ignored(BStr::new(b"foo/bar"), false));
    assert!(stack.is_ignored(BStr::new(b"dir/foo/bar"), false));
}

#[test]
fn trailing_double_star() {
    let stack = stack_from(b"abc/**\n");
    assert!(stack.is_ignored(BStr::new(b"abc/x"), false));
    assert!(stack.is_ignored(BStr::new(b"abc/x/y"), false));
    assert!(!stack.is_ignored(BStr::new(b"other/x"), false));
}

#[test]
fn comment_and_empty_lines() {
    let stack = stack_from(b"# This is a comment\n\n*.o\n");
    assert_eq!(stack.len(), 1);
    assert!(stack.is_ignored(BStr::new(b"test.o"), false));
}

#[test]
fn escaped_hash() {
    let stack = stack_from(b"\\#important\n");
    assert!(stack.is_ignored(BStr::new(b"#important"), false));
}

#[test]
fn character_class() {
    let stack = stack_from(b"*.[oa]\n");
    assert!(stack.is_ignored(BStr::new(b"test.o"), false));
    assert!(stack.is_ignored(BStr::new(b"test.a"), false));
    assert!(!stack.is_ignored(BStr::new(b"test.c"), false));
}

#[test]
fn multiple_patterns() {
    let stack = stack_from(b"*.o\n*.a\n*.so\n!libkeep.so\n");
    assert!(stack.is_ignored(BStr::new(b"test.o"), false));
    assert!(stack.is_ignored(BStr::new(b"test.a"), false));
    assert!(stack.is_ignored(BStr::new(b"test.so"), false));
    assert!(!stack.is_ignored(BStr::new(b"libkeep.so"), false));
    assert!(!stack.is_ignored(BStr::new(b"test.c"), false));
}

#[test]
fn path_with_slash() {
    let stack = stack_from(b"doc/frotz/\n");
    assert!(stack.is_ignored(BStr::new(b"doc/frotz"), true));
    assert!(!stack.is_ignored(BStr::new(b"doc/frotz"), false));
}

#[test]
fn not_ignored_by_default() {
    let stack = stack_from(b"*.o\n");
    assert!(!stack.is_ignored(BStr::new(b"Makefile"), false));
    assert!(!stack.is_ignored(BStr::new(b"src/main.rs"), false));
}
