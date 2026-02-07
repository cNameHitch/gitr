//! Pathspec matching compatibility tests.

use bstr::BStr;
use git_index::pathspec::Pathspec;

#[test]
fn simple_glob() {
    let ps = Pathspec::parse(&["*.rs"]).unwrap();
    assert!(ps.matches(BStr::new(b"main.rs"), false));
    assert!(!ps.matches(BStr::new(b"main.txt"), false));
}

#[test]
fn directory_glob() {
    let ps = Pathspec::parse(&["src/*.rs"]).unwrap();
    assert!(ps.matches(BStr::new(b"src/main.rs"), false));
    assert!(!ps.matches(BStr::new(b"src/sub/main.rs"), false));
    assert!(!ps.matches(BStr::new(b"lib/main.rs"), false));
}

#[test]
fn exclude_magic() {
    let ps = Pathspec::parse(&["*.rs", ":(exclude)*.test.rs"]).unwrap();
    assert!(ps.matches(BStr::new(b"main.rs"), false));
    assert!(!ps.matches(BStr::new(b"main.test.rs"), false));
}

#[test]
fn short_exclude() {
    let ps = Pathspec::parse(&["*.rs", ":!*.test.rs"]).unwrap();
    assert!(ps.matches(BStr::new(b"main.rs"), false));
    assert!(!ps.matches(BStr::new(b"main.test.rs"), false));
}

#[test]
fn top_magic() {
    let ps = Pathspec::parse(&[":(top)README"]).unwrap();
    assert!(ps.patterns[0].magic.top);
}

#[test]
fn icase_magic() {
    let ps = Pathspec::parse(&[":(icase)readme.md"]).unwrap();
    assert!(ps.matches(BStr::new(b"README.MD"), false));
    assert!(ps.matches(BStr::new(b"readme.md"), false));
    assert!(ps.matches(BStr::new(b"Readme.Md"), false));
}

#[test]
fn literal_magic() {
    let ps = Pathspec::parse(&[":(literal)src/*.rs"]).unwrap();
    // Should match literally, not as glob
    assert!(ps.matches(BStr::new(b"src/*.rs"), false));
    assert!(!ps.matches(BStr::new(b"src/main.rs"), false));
}

#[test]
fn prefix_match() {
    let ps = Pathspec::parse(&["src"]).unwrap();
    assert!(ps.matches(BStr::new(b"src/main.rs"), false));
    assert!(ps.matches(BStr::new(b"src/sub/file.rs"), false));
    assert!(!ps.matches(BStr::new(b"lib/main.rs"), false));
}

#[test]
fn multiple_includes() {
    let ps = Pathspec::parse(&["src", "lib"]).unwrap();
    assert!(ps.matches(BStr::new(b"src/main.rs"), false));
    assert!(ps.matches(BStr::new(b"lib/main.rs"), false));
    assert!(!ps.matches(BStr::new(b"bin/main.rs"), false));
}

#[test]
fn empty_pathspec_matches_all() {
    let ps = Pathspec::parse(&[]).unwrap();
    assert!(ps.matches(BStr::new(b"anything"), false));
    assert!(ps.matches(BStr::new(b"src/deep/path.rs"), false));
}

#[test]
fn combined_magic() {
    let ps = Pathspec::parse(&[":(icase,exclude)*.TMP"]).unwrap();
    assert!(ps.patterns[0].magic.icase);
    assert!(ps.patterns[0].magic.exclude);
}

#[test]
fn invalid_magic() {
    let result = Pathspec::parse(&[":(unknown)pattern"]);
    assert!(result.is_err());
}
