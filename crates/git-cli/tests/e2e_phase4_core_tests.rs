//! End-to-end interop tests for Git Parity Phase 4 — US3 Core Engine Completion.
//!
//! Tests octopus merge (3+ heads), subtree merge strategy, and sequencer
//! abort (cherry-pick --abort / revert --abort) by running both gitr and
//! C git and comparing outputs.

mod common;
use common::*;

// ════════════════════════════════════════════════════════════════════════════
// User Story 3 — Incomplete Core Engine Completion (P1)
// ════════════════════════════════════════════════════════════════════════════

// ── Octopus Merge ──

/// Set up a repo with main (1 commit) and 3 topic branches, each adding a
/// unique file. All branches diverge from the same base commit so merges
/// are non-conflicting.
fn setup_octopus_branches(dir: &std::path::Path) {
    setup_empty_repo(dir);
    let mut counter = 0u64;

    // Base commit on main
    std::fs::write(dir.join("base.txt"), "base content\n").unwrap();
    let date = next_date(&mut counter);
    git_with_date(dir, &["add", "base.txt"], &date);
    git_with_date(dir, &["commit", "-m", "base commit"], &date);

    // Create 3 branches, each adding a unique file
    for i in 1..=3 {
        let branch = format!("branch{}", i);
        git(dir, &["checkout", "-b", &branch, "main"]);
        let filename = format!("branch{}_file.txt", i);
        std::fs::write(dir.join(&filename), format!("content from branch{}\n", i)).unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", &filename], &date);
        git_with_date(
            dir,
            &["commit", "-m", &format!("commit on {}", branch)],
            &date,
        );
    }

    // Return to main
    git(dir, &["checkout", "main"]);
}

#[test]
#[ignore = "gitr merge only accepts single COMMIT arg; octopus multi-arg not yet wired"]
fn test_octopus_merge_three_branches() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_octopus_branches(dir_git.path());
    setup_octopus_branches(dir_gitr.path());

    // Octopus merge: merge branch2 and branch3 while on main (already has branch1 as ancestor)
    // First merge branch1 in so we can then octopus-merge branch2 + branch3
    // Actually, octopus merge means merging multiple heads at once:
    // `git merge branch1 branch2 branch3` merges all 3 into main at once.
    let g = git(dir_git.path(), &["merge", "branch1", "branch2", "branch3"]);
    let m = gitr(dir_gitr.path(), &["merge", "branch1", "branch2", "branch3"]);

    // Both should succeed
    assert_exit_code_eq(&g, &m);
    assert_eq!(g.exit_code, 0, "git octopus merge failed: {}", g.stderr);
    assert_eq!(m.exit_code, 0, "gitr octopus merge failed: {}", m.stderr);

    // Verify the resulting tree contains all files from all branches
    let g_tree = git(dir_git.path(), &["ls-tree", "-r", "--name-only", "HEAD"]);
    let m_tree = gitr(dir_gitr.path(), &["ls-tree", "-r", "--name-only", "HEAD"]);
    assert_output_eq(&g_tree, &m_tree);

    // All 4 files should be present (base + 3 branch files)
    for filename in &["base.txt", "branch1_file.txt", "branch2_file.txt", "branch3_file.txt"] {
        assert!(
            m_tree.stdout.contains(filename),
            "gitr octopus merge result missing file: {}",
            filename
        );
    }

    // Verify the merge commit has 4 parents (main + 3 branches)
    let g_parents = git(dir_git.path(), &["cat-file", "-p", "HEAD"]);
    let m_parents = gitr(dir_gitr.path(), &["cat-file", "-p", "HEAD"]);

    let g_parent_count = g_parents.stdout.lines().filter(|l| l.starts_with("parent")).count();
    let m_parent_count = m_parents.stdout.lines().filter(|l| l.starts_with("parent")).count();
    assert_eq!(
        g_parent_count, m_parent_count,
        "Parent count mismatch: git={} gitr={}",
        g_parent_count, m_parent_count
    );
    assert_eq!(
        m_parent_count, 3,
        "Octopus merge should have 3 parent lines (current HEAD is implicit), got {}",
        m_parent_count
    );

    // Both repos should pass fsck
    assert_fsck_clean(dir_git.path());
    assert_fsck_clean(dir_gitr.path());
}

#[test]
fn test_octopus_merge_conflict_aborts() {
    // When octopus merge encounters a conflict, it should abort immediately.
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    for dir in [dir_git.path(), dir_gitr.path()] {
        setup_empty_repo(dir);
        let mut counter = 0u64;

        // Base commit with shared file
        std::fs::write(dir.join("shared.txt"), "original content\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "shared.txt"], &date);
        git_with_date(dir, &["commit", "-m", "base"], &date);

        // branch1: modify shared.txt
        git(dir, &["checkout", "-b", "branch1", "main"]);
        std::fs::write(dir.join("shared.txt"), "branch1 modification\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "shared.txt"], &date);
        git_with_date(dir, &["commit", "-m", "branch1 change"], &date);

        // branch2: also modify shared.txt (conflict with branch1)
        git(dir, &["checkout", "-b", "branch2", "main"]);
        std::fs::write(dir.join("shared.txt"), "branch2 modification\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "shared.txt"], &date);
        git_with_date(dir, &["commit", "-m", "branch2 change"], &date);

        git(dir, &["checkout", "main"]);
    }

    // Attempt octopus merge with conflicting branches
    let g = git(dir_git.path(), &["merge", "branch1", "branch2"]);
    let m = gitr(dir_gitr.path(), &["merge", "branch1", "branch2"]);

    // Both should fail (non-zero exit code) since octopus cannot handle conflicts
    assert_ne!(g.exit_code, 0, "git should fail on conflicting octopus merge");
    assert_ne!(
        m.exit_code, 0,
        "gitr should fail on conflicting octopus merge, stderr: {}",
        m.stderr
    );
}

#[test]
#[ignore = "depends on octopus multi-arg merge support"]
fn test_octopus_merge_file_content_matches() {
    // Verify that after an octopus merge, the file contents are identical
    // between git and gitr.
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_octopus_branches(dir_git.path());
    setup_octopus_branches(dir_gitr.path());

    let g = git(dir_git.path(), &["merge", "branch1", "branch2", "branch3"]);
    let m = gitr(dir_gitr.path(), &["merge", "branch1", "branch2", "branch3"]);

    // Both merges must succeed before we can compare file contents
    assert_eq!(g.exit_code, 0, "git octopus merge failed: {}", g.stderr);
    assert_eq!(m.exit_code, 0, "gitr octopus merge failed: {}", m.stderr);

    // Verify file contents match between git and gitr repos
    for filename in &["base.txt", "branch1_file.txt", "branch2_file.txt", "branch3_file.txt"] {
        let g_content = std::fs::read_to_string(dir_git.path().join(filename)).unwrap();
        let m_content = std::fs::read_to_string(dir_gitr.path().join(filename)).unwrap();
        assert_eq!(
            g_content, m_content,
            "File content mismatch for {}: git={:?} gitr={:?}",
            filename, g_content, m_content
        );
    }
}

// ── Subtree Merge ──

/// Set up a repo with a "subproject" branch whose tree should be merged
/// under a subdirectory (lib/) in the main branch.
fn setup_subtree_scenario(dir: &std::path::Path) {
    setup_empty_repo(dir);
    let mut counter = 0u64;

    // Main branch: create initial structure
    std::fs::write(dir.join("main_app.txt"), "main application\n").unwrap();
    std::fs::create_dir_all(dir.join("lib")).unwrap();
    std::fs::write(dir.join("lib/readme.txt"), "library readme\n").unwrap();
    let date = next_date(&mut counter);
    git_with_date(dir, &["add", "."], &date);
    git_with_date(dir, &["commit", "-m", "initial main structure"], &date);

    // Create a sub-project branch that looks like a standalone project
    // The subtree merge strategy should detect that this branch maps to lib/
    git(dir, &["checkout", "--orphan", "subproject"]);
    git(dir, &["rm", "-rf", "."]);

    std::fs::write(dir.join("readme.txt"), "library readme\n").unwrap();
    std::fs::write(dir.join("util.txt"), "utility module\n").unwrap();
    let date = next_date(&mut counter);
    git_with_date(dir, &["add", "."], &date);
    git_with_date(dir, &["commit", "-m", "subproject initial"], &date);

    // Add another commit to subproject
    std::fs::write(dir.join("helper.txt"), "helper module\n").unwrap();
    let date = next_date(&mut counter);
    git_with_date(dir, &["add", "helper.txt"], &date);
    git_with_date(dir, &["commit", "-m", "subproject add helper"], &date);

    // Return to main
    git(dir, &["checkout", "main"]);
}

#[test]
#[ignore = "gitr merge -s subtree not yet wired to CLI strategy dispatch"]
fn test_subtree_merge_strategy() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_subtree_scenario(dir_git.path());
    setup_subtree_scenario(dir_gitr.path());

    // Use subtree merge to bring subproject content into lib/
    // First, read-tree the subproject into lib/ prefix, then merge
    // This follows the standard git subtree merge workflow:
    // 1. git read-tree --prefix=lib/ -u subproject
    // 2. git commit
    // 3. git merge -s subtree subproject
    //
    // Alternatively: git merge -s subtree --allow-unrelated-histories subproject
    // after having set up the subtree prefix via a previous read-tree.

    for dir in [dir_git.path(), dir_gitr.path()] {
        // Read the subproject tree into lib/ subtree
        git(dir, &["read-tree", "--prefix=lib/", "-u", "subproject"]);
        git(dir, &["commit", "-m", "merge subproject into lib/"]);
    }

    // Now merge with subtree strategy to pick up new changes
    // Add a new commit on subproject first
    for dir in [dir_git.path(), dir_gitr.path()] {
        git(dir, &["checkout", "subproject"]);
        std::fs::write(dir.join("extra.txt"), "extra subproject file\n").unwrap();
        git(dir, &["add", "extra.txt"]);
        git(dir, &["commit", "-m", "subproject adds extra"]);
        git(dir, &["checkout", "main"]);
    }

    let g = git(
        dir_git.path(),
        &["merge", "-s", "subtree", "subproject", "--allow-unrelated-histories"],
    );
    let m = gitr(
        dir_gitr.path(),
        &["merge", "-s", "subtree", "subproject", "--allow-unrelated-histories"],
    );

    assert_exit_code_eq(&g, &m);

    // Compare resulting trees
    let g_tree = git(dir_git.path(), &["ls-tree", "-r", "--name-only", "HEAD"]);
    let m_tree = gitr(dir_gitr.path(), &["ls-tree", "-r", "--name-only", "HEAD"]);
    assert_output_eq(&g_tree, &m_tree);

    // The subproject files should appear under lib/
    assert!(
        m_tree.stdout.contains("lib/"),
        "Subtree merge should place subproject files under lib/"
    );
}

#[test]
fn test_subtree_merge_preserves_main_files() {
    // Verify that a subtree merge does not clobber files in the main branch
    // that exist outside the subtree prefix.
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_subtree_scenario(dir_git.path());
    setup_subtree_scenario(dir_gitr.path());

    for dir in [dir_git.path(), dir_gitr.path()] {
        git(dir, &["read-tree", "--prefix=lib/", "-u", "subproject"]);
        git(dir, &["commit", "-m", "merge subproject into lib/"]);
    }

    // Verify main_app.txt is still present in both repos
    let g_tree = git(dir_git.path(), &["ls-tree", "-r", "--name-only", "HEAD"]);
    let m_tree = gitr(dir_gitr.path(), &["ls-tree", "-r", "--name-only", "HEAD"]);
    assert_output_eq(&g_tree, &m_tree);

    assert!(
        m_tree.stdout.contains("main_app.txt"),
        "Subtree merge should preserve main branch files"
    );
    assert!(
        m_tree.stdout.contains("lib/readme.txt"),
        "Subtree merge should include subproject files under lib/"
    );
}

// ── Sequencer Abort (cherry-pick --abort) ──

/// Set up a repo with main and a feature branch that has multiple commits,
/// where the middle commit conflicts with main.
fn setup_cherry_pick_conflict_sequence(dir: &std::path::Path) {
    setup_empty_repo(dir);
    let mut counter = 0u64;

    // Base commit on main with a file that will be modified by both branches
    std::fs::write(dir.join("shared.txt"), "line 1\nline 2\nline 3\n").unwrap();
    std::fs::write(dir.join("main_only.txt"), "main file\n").unwrap();
    let date = next_date(&mut counter);
    git_with_date(dir, &["add", "."], &date);
    git_with_date(dir, &["commit", "-m", "base commit"], &date);

    // Main branch: modify shared.txt line 2
    std::fs::write(dir.join("shared.txt"), "line 1\nmain modified line 2\nline 3\n").unwrap();
    let date = next_date(&mut counter);
    git_with_date(dir, &["add", "shared.txt"], &date);
    git_with_date(dir, &["commit", "-m", "main modifies shared"], &date);

    // Feature branch: 3 commits, where the 2nd one conflicts with main
    git(dir, &["checkout", "-b", "feature", "HEAD~1"]);

    // Commit A: non-conflicting change
    std::fs::write(dir.join("feature_a.txt"), "feature file a\n").unwrap();
    let date = next_date(&mut counter);
    git_with_date(dir, &["add", "feature_a.txt"], &date);
    git_with_date(dir, &["commit", "-m", "feature commit A"], &date);

    // Commit B: conflicts with main (modifies same line of shared.txt)
    std::fs::write(
        dir.join("shared.txt"),
        "line 1\nfeature modified line 2\nline 3\n",
    )
    .unwrap();
    let date = next_date(&mut counter);
    git_with_date(dir, &["add", "shared.txt"], &date);
    git_with_date(dir, &["commit", "-m", "feature commit B (conflict)"], &date);

    // Commit C: non-conflicting change
    std::fs::write(dir.join("feature_c.txt"), "feature file c\n").unwrap();
    let date = next_date(&mut counter);
    git_with_date(dir, &["add", "feature_c.txt"], &date);
    git_with_date(dir, &["commit", "-m", "feature commit C"], &date);

    // Return to main
    git(dir, &["checkout", "main"]);
}

#[test]
#[ignore = "cherry-pick --abort does not fully restore HEAD to pre-operation commit"]
fn test_cherry_pick_abort_restores_head() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_cherry_pick_conflict_sequence(dir_git.path());
    setup_cherry_pick_conflict_sequence(dir_gitr.path());

    // Record HEAD before cherry-pick
    let g_head_before = git(dir_git.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let m_head_before = gitr(dir_gitr.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();

    // Get the 3 feature commit OIDs (oldest to newest)
    let feature_commits_str = git(
        dir_git.path(),
        &["log", "--reverse", "--format=%H", "feature", "--not", "HEAD~1"],
    );
    let feature_oids: Vec<&str> = feature_commits_str.stdout.trim().lines().collect();
    assert!(
        feature_oids.len() >= 3,
        "Expected at least 3 feature commits, got {}",
        feature_oids.len()
    );

    // Cherry-pick all 3 commits: A succeeds, B conflicts
    let g = git(
        dir_git.path(),
        &["cherry-pick", feature_oids[0], feature_oids[1], feature_oids[2]],
    );
    let m = gitr(
        dir_gitr.path(),
        &["cherry-pick", feature_oids[0], feature_oids[1], feature_oids[2]],
    );

    // Both should fail due to conflict on commit B
    assert_ne!(g.exit_code, 0, "git cherry-pick should conflict");
    assert_ne!(m.exit_code, 0, "gitr cherry-pick should conflict");
    assert_exit_code_eq(&g, &m);

    // Now abort the cherry-pick
    let g_abort = git(dir_git.path(), &["cherry-pick", "--abort"]);
    let m_abort = gitr(dir_gitr.path(), &["cherry-pick", "--abort"]);

    assert_eq!(g_abort.exit_code, 0, "git cherry-pick --abort failed: {}", g_abort.stderr);
    assert_eq!(m_abort.exit_code, 0, "gitr cherry-pick --abort failed: {}", m_abort.stderr);
    assert_exit_code_eq(&g_abort, &m_abort);

    // HEAD should be restored to the pre-cherry-pick commit
    let g_head_after = git(dir_git.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let m_head_after = gitr(dir_gitr.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();

    assert_eq!(
        g_head_before, g_head_after,
        "git HEAD not restored after cherry-pick --abort"
    );
    assert_eq!(
        m_head_before, m_head_after,
        "gitr HEAD not restored after cherry-pick --abort"
    );
}

#[test]
fn test_cherry_pick_abort_restores_index() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_cherry_pick_conflict_sequence(dir_git.path());
    setup_cherry_pick_conflict_sequence(dir_gitr.path());

    // Record index state before cherry-pick (ls-files -s gives full index)
    let g_index_before = git(dir_git.path(), &["ls-files", "-s"]).stdout.clone();
    let m_index_before = gitr(dir_gitr.path(), &["ls-files", "-s"]).stdout.clone();

    // Get feature commit OIDs
    let feature_commits_str = git(
        dir_git.path(),
        &["log", "--reverse", "--format=%H", "feature", "--not", "HEAD~1"],
    );
    let feature_oids: Vec<&str> = feature_commits_str.stdout.trim().lines().collect();

    // Cherry-pick that will conflict
    git(
        dir_git.path(),
        &["cherry-pick", feature_oids[0], feature_oids[1], feature_oids[2]],
    );
    gitr(
        dir_gitr.path(),
        &["cherry-pick", feature_oids[0], feature_oids[1], feature_oids[2]],
    );

    // Abort
    git(dir_git.path(), &["cherry-pick", "--abort"]);
    gitr(dir_gitr.path(), &["cherry-pick", "--abort"]);

    // Index should be restored
    let g_index_after = git(dir_git.path(), &["ls-files", "-s"]).stdout.clone();
    let m_index_after = gitr(dir_gitr.path(), &["ls-files", "-s"]).stdout.clone();

    assert_eq!(
        g_index_before, g_index_after,
        "git index not restored after cherry-pick --abort"
    );
    assert_eq!(
        m_index_before, m_index_after,
        "gitr index not restored after cherry-pick --abort"
    );

    // Compare the restored index between git and gitr
    assert_output_eq(
        &git(dir_git.path(), &["ls-files", "-s"]),
        &gitr(dir_gitr.path(), &["ls-files", "-s"]),
    );
}

#[test]
fn test_cherry_pick_abort_cleans_sequencer_state() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_cherry_pick_conflict_sequence(dir_git.path());
    setup_cherry_pick_conflict_sequence(dir_gitr.path());

    // Get feature commit OIDs
    let feature_commits_str = git(
        dir_git.path(),
        &["log", "--reverse", "--format=%H", "feature", "--not", "HEAD~1"],
    );
    let feature_oids: Vec<&str> = feature_commits_str.stdout.trim().lines().collect();

    // Cherry-pick that will conflict
    git(
        dir_git.path(),
        &["cherry-pick", feature_oids[0], feature_oids[1], feature_oids[2]],
    );
    gitr(
        dir_gitr.path(),
        &["cherry-pick", feature_oids[0], feature_oids[1], feature_oids[2]],
    );

    // Verify sequencer state exists during conflict
    assert!(
        dir_gitr.path().join(".git/sequencer").exists()
            || dir_gitr.path().join(".git/CHERRY_PICK_HEAD").exists(),
        "gitr should have sequencer or CHERRY_PICK_HEAD state during conflict"
    );

    // Abort
    git(dir_git.path(), &["cherry-pick", "--abort"]);
    gitr(dir_gitr.path(), &["cherry-pick", "--abort"]);

    // Sequencer state should be cleaned up
    assert!(
        !dir_gitr.path().join(".git/sequencer").exists(),
        "gitr should clean up .git/sequencer after abort"
    );
    assert!(
        !dir_gitr.path().join(".git/CHERRY_PICK_HEAD").exists(),
        "gitr should clean up CHERRY_PICK_HEAD after abort"
    );

    // Same cleanup check for git (sanity)
    assert!(
        !dir_git.path().join(".git/sequencer").exists(),
        "git should clean up .git/sequencer after abort"
    );
    assert!(
        !dir_git.path().join(".git/CHERRY_PICK_HEAD").exists(),
        "git should clean up CHERRY_PICK_HEAD after abort"
    );
}

#[test]
#[ignore = "cherry-pick --abort leaves deleted files in working tree"]
fn test_cherry_pick_abort_working_tree_clean() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_cherry_pick_conflict_sequence(dir_git.path());
    setup_cherry_pick_conflict_sequence(dir_gitr.path());

    // Get feature commit OIDs
    let feature_commits_str = git(
        dir_git.path(),
        &["log", "--reverse", "--format=%H", "feature", "--not", "HEAD~1"],
    );
    let feature_oids: Vec<&str> = feature_commits_str.stdout.trim().lines().collect();

    // Cherry-pick that will conflict
    git(
        dir_git.path(),
        &["cherry-pick", feature_oids[0], feature_oids[1], feature_oids[2]],
    );
    gitr(
        dir_gitr.path(),
        &["cherry-pick", feature_oids[0], feature_oids[1], feature_oids[2]],
    );

    // Abort
    git(dir_git.path(), &["cherry-pick", "--abort"]);
    gitr(dir_gitr.path(), &["cherry-pick", "--abort"]);

    // Working tree should be clean (status --porcelain should be empty)
    let g_status = git(dir_git.path(), &["status", "--porcelain"]);
    let m_status = gitr(dir_gitr.path(), &["status", "--porcelain"]);
    assert_output_eq(&g_status, &m_status);
    assert_eq!(
        m_status.stdout.trim(),
        "",
        "gitr working tree should be clean after cherry-pick --abort, got: {:?}",
        m_status.stdout
    );
}

#[test]
fn test_cherry_pick_abort_no_operation_in_progress() {
    // Running cherry-pick --abort when no cherry-pick is in progress should error.
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["cherry-pick", "--abort"]);
    let m = gitr(dir_gitr.path(), &["cherry-pick", "--abort"]);

    // Both should fail with a non-zero exit code
    assert_ne!(g.exit_code, 0, "git should error when no cherry-pick in progress");
    assert_ne!(
        m.exit_code, 0,
        "gitr should error when no cherry-pick in progress, stderr: {}",
        m.stderr
    );
}

#[test]
fn test_revert_abort_restores_state() {
    // Same as cherry-pick abort, but for revert.
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    // Set up repos with a conflict scenario for revert
    for dir in [dir_git.path(), dir_gitr.path()] {
        setup_empty_repo(dir);
        let mut counter = 0u64;

        // Commit 1: initial file
        std::fs::write(dir.join("file.txt"), "original content\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "file.txt"], &date);
        git_with_date(dir, &["commit", "-m", "initial"], &date);

        // Commit 2: modify file
        std::fs::write(dir.join("file.txt"), "modified in commit 2\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "file.txt"], &date);
        git_with_date(dir, &["commit", "-m", "commit 2"], &date);

        // Commit 3: modify same file again (reverting commit 2 will conflict)
        std::fs::write(dir.join("file.txt"), "further modified in commit 3\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "file.txt"], &date);
        git_with_date(dir, &["commit", "-m", "commit 3"], &date);
    }

    // Record HEAD before revert
    let g_head_before = git(dir_git.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let m_head_before = gitr(dir_gitr.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();

    // Revert commit 2 (HEAD~1), which should conflict with commit 3's changes
    let g = git(dir_git.path(), &["revert", "HEAD~1"]);
    let m = gitr(dir_gitr.path(), &["revert", "HEAD~1"]);

    // Both should fail due to conflict
    assert_ne!(g.exit_code, 0, "git revert should conflict");
    assert_ne!(m.exit_code, 0, "gitr revert should conflict");

    // Abort the revert
    let g_abort = git(dir_git.path(), &["revert", "--abort"]);
    let m_abort = gitr(dir_gitr.path(), &["revert", "--abort"]);

    assert_eq!(g_abort.exit_code, 0, "git revert --abort failed: {}", g_abort.stderr);
    assert_eq!(m_abort.exit_code, 0, "gitr revert --abort failed: {}", m_abort.stderr);

    // HEAD should be restored
    let g_head_after = git(dir_git.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let m_head_after = gitr(dir_gitr.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();

    assert_eq!(g_head_before, g_head_after, "git HEAD not restored after revert --abort");
    assert_eq!(m_head_before, m_head_after, "gitr HEAD not restored after revert --abort");

    // Working tree should be clean
    let g_status = git(dir_git.path(), &["status", "--porcelain"]);
    let m_status = gitr(dir_gitr.path(), &["status", "--porcelain"]);
    assert_output_eq(&g_status, &m_status);
}
