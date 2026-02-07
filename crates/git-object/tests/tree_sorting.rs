use std::cmp::Ordering;

use bstr::BString;
use git_hash::ObjectId;
use git_object::{FileMode, Tree, TreeEntry};

fn entry(name: &str, mode: FileMode) -> TreeEntry {
    TreeEntry {
        mode,
        name: BString::from(name),
        oid: ObjectId::NULL_SHA1,
    }
}

fn file(name: &str) -> TreeEntry {
    entry(name, FileMode::Regular)
}

fn dir(name: &str) -> TreeEntry {
    entry(name, FileMode::Tree)
}

fn exe(name: &str) -> TreeEntry {
    entry(name, FileMode::Executable)
}

fn link(name: &str) -> TreeEntry {
    entry(name, FileMode::Symlink)
}

fn gitlink(name: &str) -> TreeEntry {
    entry(name, FileMode::Gitlink)
}

#[test]
fn dir_sorts_as_if_trailing_slash() {
    // "foo" (dir) → "foo/" vs "foo.c" (file) → "foo.c"
    // '/' (0x2F) > '.' (0x2E), so dir sorts AFTER "foo.c"
    assert_eq!(
        TreeEntry::cmp_entries(&dir("foo"), &file("foo.c")),
        Ordering::Greater
    );
}

#[test]
fn dir_sorts_after_hyphenated() {
    // "foo" (dir) → "foo/" vs "foo-bar" → "foo-bar"
    // '/' (0x2F) > '-' (0x2D), so dir sorts AFTER
    assert_eq!(
        TreeEntry::cmp_entries(&dir("foo"), &file("foo-bar")),
        Ordering::Greater
    );
}

#[test]
fn dir_sorts_before_zero() {
    // "foo" (dir) → "foo/" vs "foo0" (file) → "foo0"
    // '/' (0x2F) < '0' (0x30), so dir sorts BEFORE
    assert_eq!(
        TreeEntry::cmp_entries(&dir("foo"), &file("foo0")),
        Ordering::Less
    );
}

#[test]
fn same_name_file_vs_dir() {
    // "abc" (file) vs "abc" (dir)
    // Both exhaust name, then file gets 0x00 and dir gets '/' (0x2F)
    // 0x00 < 0x2F, so file sorts BEFORE dir
    assert_eq!(
        TreeEntry::cmp_entries(&file("abc"), &dir("abc")),
        Ordering::Less
    );
}

#[test]
fn identical_files_are_equal() {
    assert_eq!(
        TreeEntry::cmp_entries(&file("README"), &file("README")),
        Ordering::Equal
    );
}

#[test]
fn identical_dirs_are_equal() {
    assert_eq!(
        TreeEntry::cmp_entries(&dir("src"), &dir("src")),
        Ordering::Equal
    );
}

#[test]
fn alphabetical_files() {
    assert_eq!(
        TreeEntry::cmp_entries(&file("a"), &file("b")),
        Ordering::Less
    );
    assert_eq!(
        TreeEntry::cmp_entries(&file("z"), &file("a")),
        Ordering::Greater
    );
}

#[test]
fn alphabetical_dirs() {
    assert_eq!(
        TreeEntry::cmp_entries(&dir("aaa"), &dir("bbb")),
        Ordering::Less
    );
}

#[test]
fn executable_sorts_same_as_regular() {
    // Both are non-tree entries, so they're compared byte-by-byte without trailing slash
    assert_eq!(
        TreeEntry::cmp_entries(&file("run.sh"), &exe("run.sh")),
        Ordering::Equal
    );
}

#[test]
fn symlink_sorts_same_as_regular() {
    assert_eq!(
        TreeEntry::cmp_entries(&file("link"), &link("link")),
        Ordering::Equal
    );
}

#[test]
fn gitlink_sorts_same_as_regular() {
    // Gitlinks (submodules, mode 160000) are not directories — they don't get trailing '/'
    assert_eq!(
        TreeEntry::cmp_entries(&file("sub"), &gitlink("sub")),
        Ordering::Equal
    );
}

#[test]
fn gitlink_vs_dir_same_name() {
    // Gitlink "foo" → 0x00 at end (not a dir), dir "foo" → '/' at end
    // 0x00 < 0x2F, so gitlink sorts BEFORE dir
    assert_eq!(
        TreeEntry::cmp_entries(&gitlink("foo"), &dir("foo")),
        Ordering::Less
    );
}

#[test]
fn prefix_relationship() {
    // "ab" (file) vs "abc" (file)
    // After common prefix "ab", file "ab" gets 0x00, file "abc" gets 'c'
    // 0x00 < 'c', so shorter sorts first
    assert_eq!(
        TreeEntry::cmp_entries(&file("ab"), &file("abc")),
        Ordering::Less
    );
}

#[test]
fn dir_prefix_of_file() {
    // "ab" (dir) vs "abc" (file)
    // After "ab", dir gets '/' (0x2F), file gets 'c' (0x63)
    // '/' < 'c', so dir sorts BEFORE
    assert_eq!(
        TreeEntry::cmp_entries(&dir("ab"), &file("abc")),
        Ordering::Less
    );
}

#[test]
fn special_chars_in_names() {
    // Names with special characters — byte comparison
    assert_eq!(
        TreeEntry::cmp_entries(&file("a b"), &file("a-b")),
        Ordering::Less // space (0x20) < '-' (0x2D)
    );
}

#[test]
fn tree_serialize_sorts_entries() {
    let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();

    let tree = Tree {
        entries: vec![
            TreeEntry {
                mode: FileMode::Regular,
                name: BString::from("z.txt"),
                oid,
            },
            TreeEntry {
                mode: FileMode::Tree,
                name: BString::from("a-dir"),
                oid,
            },
            TreeEntry {
                mode: FileMode::Executable,
                name: BString::from("m.sh"),
                oid,
            },
        ],
    };

    let serialized = tree.serialize_content();
    let parsed = Tree::parse(&serialized).unwrap();

    // Entries should be in sorted order
    assert_eq!(parsed.entries[0].name, "a-dir");
    assert_eq!(parsed.entries[1].name, "m.sh");
    assert_eq!(parsed.entries[2].name, "z.txt");
}

#[test]
fn mixed_dirs_and_files_complex_sort() {
    // Reproduces a real git scenario with mixed types
    let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();

    let tree = Tree {
        entries: vec![
            TreeEntry {
                mode: FileMode::Regular,
                name: BString::from("foo.c"),
                oid,
            },
            TreeEntry {
                mode: FileMode::Tree,
                name: BString::from("foo"),
                oid,
            },
            TreeEntry {
                mode: FileMode::Regular,
                name: BString::from("foo-bar"),
                oid,
            },
            TreeEntry {
                mode: FileMode::Regular,
                name: BString::from("foo0"),
                oid,
            },
        ],
    };

    let serialized = tree.serialize_content();
    let parsed = Tree::parse(&serialized).unwrap();

    // Expected order:
    // "foo-bar" (file): f o o -  (0x2D at position 3)
    // "foo.c"   (file): f o o .  (0x2E at position 3)
    // "foo"     (dir):  f o o /  (0x2F virtual at position 3)
    // "foo0"    (file): f o o 0  (0x30 at position 3)
    assert_eq!(parsed.entries[0].name, "foo-bar");
    assert_eq!(parsed.entries[1].name, "foo.c");
    assert_eq!(parsed.entries[2].name, "foo");
    assert_eq!(parsed.entries[3].name, "foo0");
}
