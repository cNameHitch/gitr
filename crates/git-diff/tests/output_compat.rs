//! Output format compatibility tests.
//!
//! Tests that the various output formats produce correct output.

use bstr::BString;
use git_diff::format;
use git_diff::{
    DiffLine, DiffOptions, DiffOutputFormat, DiffResult, FileDiff, FileStatus, Hunk,
};
use git_hash::ObjectId;
use git_object::FileMode;

/// Create a simple modified FileDiff for testing.
fn sample_modified_diff() -> DiffResult {
    DiffResult {
        files: vec![FileDiff {
            status: FileStatus::Modified,
            old_path: Some(BString::from("hello.txt")),
            new_path: Some(BString::from("hello.txt")),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Regular),
            old_oid: Some(ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap()),
            new_oid: Some(ObjectId::from_hex("ce013625030ba8dba906f756967f9e9ca394464a").unwrap()),
            hunks: vec![Hunk {
                old_start: 1,
                old_count: 3,
                new_start: 1,
                new_count: 3,
                header: None,
                lines: vec![
                    DiffLine::Context(BString::from("line1\n")),
                    DiffLine::Deletion(BString::from("old line\n")),
                    DiffLine::Addition(BString::from("new line\n")),
                    DiffLine::Context(BString::from("line3\n")),
                ],
            }],
            is_binary: false,
            similarity: None,
        }],
    }
}

/// Create a multi-file DiffResult.
fn sample_multi_file_diff() -> DiffResult {
    DiffResult {
        files: vec![
            FileDiff {
                status: FileStatus::Added,
                old_path: None,
                new_path: Some(BString::from("new_file.txt")),
                old_mode: None,
                new_mode: Some(FileMode::Regular),
                old_oid: None,
                new_oid: Some(
                    ObjectId::from_hex("ce013625030ba8dba906f756967f9e9ca394464a").unwrap(),
                ),
                hunks: vec![Hunk {
                    old_start: 0,
                    old_count: 0,
                    new_start: 1,
                    new_count: 2,
                    header: None,
                    lines: vec![
                        DiffLine::Addition(BString::from("first line\n")),
                        DiffLine::Addition(BString::from("second line\n")),
                    ],
                }],
                is_binary: false,
                similarity: None,
            },
            FileDiff {
                status: FileStatus::Deleted,
                old_path: Some(BString::from("removed.txt")),
                new_path: None,
                old_mode: Some(FileMode::Regular),
                new_mode: None,
                old_oid: Some(
                    ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap(),
                ),
                new_oid: None,
                hunks: vec![Hunk {
                    old_start: 1,
                    old_count: 1,
                    new_start: 0,
                    new_count: 0,
                    header: None,
                    lines: vec![DiffLine::Deletion(BString::from("goodbye\n"))],
                }],
                is_binary: false,
                similarity: None,
            },
            FileDiff {
                status: FileStatus::Modified,
                old_path: Some(BString::from("changed.txt")),
                new_path: Some(BString::from("changed.txt")),
                old_mode: Some(FileMode::Regular),
                new_mode: Some(FileMode::Regular),
                old_oid: Some(
                    ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap(),
                ),
                new_oid: Some(
                    ObjectId::from_hex("ce013625030ba8dba906f756967f9e9ca394464a").unwrap(),
                ),
                hunks: vec![Hunk {
                    old_start: 1,
                    old_count: 1,
                    new_start: 1,
                    new_count: 1,
                    header: None,
                    lines: vec![
                        DiffLine::Deletion(BString::from("old\n")),
                        DiffLine::Addition(BString::from("new\n")),
                    ],
                }],
                is_binary: false,
                similarity: None,
            },
        ],
    }
}

// --- Unified format tests ---

#[test]
fn unified_basic_diff() {
    let result = sample_modified_diff();
    let options = DiffOptions::default();
    let output = format::format_diff(&result, &options);

    assert!(output.contains("diff --git a/hello.txt b/hello.txt"));
    assert!(output.contains("--- a/hello.txt"));
    assert!(output.contains("+++ b/hello.txt"));
    assert!(output.contains("@@ -1,3 +1,3 @@"));
    assert!(output.contains(" line1"));
    assert!(output.contains("-old line"));
    assert!(output.contains("+new line"));
    assert!(output.contains(" line3"));
}

#[test]
fn unified_new_file() {
    let result = sample_multi_file_diff();
    let options = DiffOptions::default();
    let output = format::format_diff(&result, &options);

    assert!(output.contains("new file mode 100644"));
    assert!(output.contains("--- /dev/null"));
    assert!(output.contains("+++ b/new_file.txt"));
}

#[test]
fn unified_deleted_file() {
    let result = sample_multi_file_diff();
    let options = DiffOptions::default();
    let output = format::format_diff(&result, &options);

    assert!(output.contains("deleted file mode 100644"));
    assert!(output.contains("--- a/removed.txt"));
    assert!(output.contains("+++ /dev/null"));
}

#[test]
fn unified_binary_file() {
    let result = DiffResult {
        files: vec![FileDiff {
            status: FileStatus::Modified,
            old_path: Some(BString::from("image.png")),
            new_path: Some(BString::from("image.png")),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Regular),
            old_oid: Some(
                ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap(),
            ),
            new_oid: Some(
                ObjectId::from_hex("ce013625030ba8dba906f756967f9e9ca394464a").unwrap(),
            ),
            hunks: vec![],
            is_binary: true,
            similarity: None,
        }],
    };
    let options = DiffOptions::default();
    let output = format::format_diff(&result, &options);
    assert!(output.contains("Binary files"));
}

// --- Stat format tests ---

#[test]
fn stat_format() {
    let result = sample_multi_file_diff();
    let options = DiffOptions {
        output_format: DiffOutputFormat::Stat,
        ..DiffOptions::default()
    };
    let output = format::format_diff(&result, &options);

    assert!(output.contains("new_file.txt"));
    assert!(output.contains("removed.txt"));
    assert!(output.contains("changed.txt"));
    assert!(output.contains("3 files changed"));
}

#[test]
fn shortstat_format() {
    let result = sample_multi_file_diff();
    let options = DiffOptions {
        output_format: DiffOutputFormat::ShortStat,
        ..DiffOptions::default()
    };
    let output = format::format_diff(&result, &options);

    assert!(output.contains("3 files changed"));
}

#[test]
fn numstat_format() {
    let result = sample_multi_file_diff();
    let options = DiffOptions {
        output_format: DiffOutputFormat::NumStat,
        ..DiffOptions::default()
    };
    let output = format::format_diff(&result, &options);

    // Each line: insertions\tdeletions\tpath
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].contains("new_file.txt"));
    assert!(lines[1].contains("removed.txt"));
}

// --- Raw format tests ---

#[test]
fn raw_format() {
    let result = sample_multi_file_diff();
    let options = DiffOptions {
        output_format: DiffOutputFormat::Raw,
        ..DiffOptions::default()
    };
    let output = format::format_diff(&result, &options);

    // Raw format: each line starts with colon and has status before tab
    for line in output.lines() {
        assert!(line.starts_with(':'), "Raw format lines should start with ':'");
    }
    // Status is before the tab, path is after: ":modes oids STATUS\tpath"
    assert!(output.contains("A\t"), "Should contain Added status");
    assert!(output.contains("D\t"), "Should contain Deleted status");
    assert!(output.contains("M\t"), "Should contain Modified status");
}

// --- Name-only and name-status tests ---

#[test]
fn name_only_format() {
    let result = sample_multi_file_diff();
    let options = DiffOptions {
        output_format: DiffOutputFormat::NameOnly,
        ..DiffOptions::default()
    };
    let output = format::format_diff(&result, &options);

    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "new_file.txt");
    assert_eq!(lines[1], "removed.txt");
    assert_eq!(lines[2], "changed.txt");
}

#[test]
fn name_status_format() {
    let result = sample_multi_file_diff();
    let options = DiffOptions {
        output_format: DiffOutputFormat::NameStatus,
        ..DiffOptions::default()
    };
    let output = format::format_diff(&result, &options);

    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with('A'));
    assert!(lines[0].contains("new_file.txt"));
    assert!(lines[1].starts_with('D'));
    assert!(lines[1].contains("removed.txt"));
    assert!(lines[2].starts_with('M'));
    assert!(lines[2].contains("changed.txt"));
}

// --- Summary format tests ---

#[test]
fn summary_format() {
    let result = sample_multi_file_diff();
    let options = DiffOptions {
        output_format: DiffOutputFormat::Summary,
        ..DiffOptions::default()
    };
    let output = format::format_diff(&result, &options);

    assert!(output.contains("create mode 100644 new_file.txt"));
    assert!(output.contains("delete mode 100644 removed.txt"));
}

// --- Rename format tests ---

#[test]
fn rename_in_unified() {
    let result = DiffResult {
        files: vec![FileDiff {
            status: FileStatus::Renamed,
            old_path: Some(BString::from("old_name.txt")),
            new_path: Some(BString::from("new_name.txt")),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Regular),
            old_oid: Some(
                ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap(),
            ),
            new_oid: Some(
                ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap(),
            ),
            hunks: vec![],
            is_binary: false,
            similarity: Some(100),
        }],
    };
    let options = DiffOptions::default();
    let output = format::format_diff(&result, &options);

    assert!(output.contains("similarity index 100%"));
    assert!(output.contains("rename from old_name.txt"));
    assert!(output.contains("rename to new_name.txt"));
}

#[test]
fn rename_in_name_status() {
    let result = DiffResult {
        files: vec![FileDiff {
            status: FileStatus::Renamed,
            old_path: Some(BString::from("old.txt")),
            new_path: Some(BString::from("new.txt")),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Regular),
            old_oid: None,
            new_oid: None,
            hunks: vec![],
            is_binary: false,
            similarity: Some(95),
        }],
    };
    let options = DiffOptions {
        output_format: DiffOutputFormat::NameStatus,
        ..DiffOptions::default()
    };
    let output = format::format_diff(&result, &options);

    assert!(output.starts_with("R095"));
    assert!(output.contains("old.txt"));
    assert!(output.contains("new.txt"));
}

// --- Empty diff tests ---

#[test]
fn empty_diff_all_formats() {
    let result = DiffResult { files: vec![] };

    for fmt in [
        DiffOutputFormat::Unified,
        DiffOutputFormat::Stat,
        DiffOutputFormat::ShortStat,
        DiffOutputFormat::NumStat,
        DiffOutputFormat::Raw,
        DiffOutputFormat::NameOnly,
        DiffOutputFormat::NameStatus,
        DiffOutputFormat::Summary,
    ] {
        let options = DiffOptions {
            output_format: fmt,
            ..DiffOptions::default()
        };
        let output = format::format_diff(&result, &options);
        assert!(output.is_empty(), "Empty diff should produce empty output for {:?}", fmt);
    }
}

// --- Mode change tests ---

#[test]
fn mode_change_in_unified() {
    let result = DiffResult {
        files: vec![FileDiff {
            status: FileStatus::Modified,
            old_path: Some(BString::from("script.sh")),
            new_path: Some(BString::from("script.sh")),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Executable),
            old_oid: Some(
                ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap(),
            ),
            new_oid: Some(
                ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap(),
            ),
            hunks: vec![],
            is_binary: false,
            similarity: None,
        }],
    };
    let options = DiffOptions::default();
    let output = format::format_diff(&result, &options);

    assert!(output.contains("old mode 100644"));
    assert!(output.contains("new mode 100755"));
}
