# Ignored Parity Tests

This document catalogs the 85 E2E parity tests currently marked `#[ignore]` across the test suite. Each test compares gitr output against C git for identical inputs. Tests are ignored because gitr does not yet match git's behavior for that specific case.

**To activate a test**: fix the underlying parity gap in gitr, then remove the `#[ignore]` annotation. The test will start running in CI automatically.

**To run ignored tests locally**: `cargo test --test <file> -- --ignored`

---

## Summary

| Category | Count | Files |
|----------|------:|-------|
| Diff plumbing output format | 11 | `parity_flags_plumbing_deep.rs` |
| Error path exit codes | 20 | `parity_error_tests.rs` |
| Stderr message text | 11 | `parity_stderr_tests.rs` |
| Edge cases | 9 | `parity_edge_cases_tests.rs` |
| Plumbing & misc flags | 10 | `parity_flags_plumbing_deep.rs`, `parity_flags_porcelain_deep.rs` |
| Blame output format | 4 | `parity_flags_porcelain_deep.rs` |
| Format-patch output | 4 | `parity_flags_porcelain_deep.rs` |
| Diff display flags | 4 | `parity_flags_tier1_gaps.rs` |
| Log/revwalk flags | 4 | `parity_flags_tier1_gaps.rs`, `parity_flags_porcelain_deep.rs` |
| Branch/switch flags | 3 | `parity_flags_tier1_gaps.rs` |
| Merge/rebase flags | 3 | `parity_flags_tier1_gaps.rs` |
| Commit flags | 2 | `parity_flags_tier1_gaps.rs` |
| **Total** | **85** | |

---

## 1. Diff Plumbing Output Format (11 tests)

The diff plumbing commands (`diff-files`, `diff-index`, `diff-tree`) produce output that does not match git's raw diff format. These commands are used by scripts and tools that parse git's low-level diff output.

| Test | Command | Gap |
|------|---------|-----|
| `test_diff_files_patch` | `diff-files -p` | Unified diff output format mismatch |
| `test_diff_files_quiet` | `diff-files -q` | Exit code mismatch (should be 1 when files differ) |
| `test_diff_files_name_only` | `diff-files --name-only` | Name-only listing does not match |
| `test_diff_files_name_status` | `diff-files --name-status` | Name+status listing (e.g. `M file.txt`) does not match |
| `test_diff_index_name_only` | `diff-index --name-only HEAD~1` | Name-only listing against tree-ish does not match |
| `test_diff_index_name_status` | `diff-index --name-status HEAD~1` | Name+status listing does not match |
| `test_diff_index_patch` | `diff-index -p HEAD~1` | Unified diff against tree-ish does not match |
| `test_diff_index_cached_name_only` | `diff-index --cached --name-only HEAD` | Cached mode name-only listing does not match |
| `test_diff_tree_recursive` | `diff-tree -r HEAD~1 HEAD` | Recursive raw diff output does not match |
| `test_diff_tree_patch` | `diff-tree -p HEAD~1 HEAD` | Unified diff between two trees does not match |
| `test_diff_tree_root` | `diff-tree --root -r <commit>` | Root commit diff against empty tree does not match |

**Fix priority**: High. Many git tools and scripts depend on exact plumbing output.

---

## 2. Error Path Exit Codes (20 tests)

gitr returns different exit codes than git for various error conditions. Some commands succeed when they should fail, and vice versa.

| Test | Command | Gap |
|------|---------|-----|
| `test_error_commit_no_message` | `commit` (no `-m`) | Different error behavior in non-interactive mode |
| `test_error_branch_delete_current` | `branch -d main` | Different exit code when deleting current branch |
| `test_error_tag_delete_nonexistent` | `tag -d nonexistent` | Different exit code for missing tag |
| `test_error_reset_invalid_ref` | `reset nonexistent-ref` | Different exit code for invalid ref |
| `test_error_rebase_no_upstream` | `rebase` (no args) | Different error when no upstream configured |
| `test_error_diff_nonexistent_path` | `diff -- nonexistent.txt` | Different exit code for missing path |
| `test_error_stash_empty_repo` | `stash` (empty repo) | Different exit code on unborn HEAD |
| `test_error_cherry_pick_abort_no_cp` | `cherry-pick --abort` | Different error when no cherry-pick in progress |
| `test_error_revert_abort_no_revert` | `revert --abort` | Different error when no revert in progress |
| `test_error_stash_pop_no_stash` | `stash pop` | Different exit code for empty stash |
| `test_error_stash_drop_no_stash` | `stash drop` | Different exit code for empty stash |
| `test_error_cat_file_no_args` | `cat-file` (no args) | Different exit code for missing arguments |
| `test_error_diff_tree_no_args` | `diff-tree` (no args) | Different exit code for missing arguments |
| `test_error_update_ref_no_args` | `update-ref` (no args) | Different exit code for missing arguments |
| `test_error_config_unset_nonexistent` | `config --unset nonexistent.key` | Different exit code for missing config key |
| `test_error_diff_not_a_repo` | `diff` (outside repo) | Different exit code outside a repository |
| `test_error_commit_nothing_staged` | `commit -m "nothing"` | Different error when nothing is staged |
| `test_error_commit_empty_not_allowed` | `commit -m "empty"` | Different error without `--allow-empty` |
| `test_error_apply_bad_patch` | `apply bad.patch` | Different exit code for malformed patch |
| `test_error_restore_nonexistent` | `restore nonexistent.txt` | Different exit code for missing file |

**Fix priority**: Medium. Scripts that check `$?` will behave differently.

---

## 3. Stderr Message Text (11 tests)

gitr produces different error/warning message text than git on stderr. The exit code may or may not also differ.

| Test | Command | Gap |
|------|---------|-----|
| `test_stderr_switch_nonexistent_branch` | `switch nonexistent` | Error message wording differs |
| `test_stderr_branch_delete_current` | `branch -d main` | Error message wording differs |
| `test_stderr_tag_delete_nonexistent` | `tag -d nonexistent` | Error message wording differs |
| `test_stderr_reset_invalid_ref` | `reset nonexistent` | Error message wording differs |
| `test_stderr_cherry_pick_invalid` | `cherry-pick nonexistent` | Error message wording differs |
| `test_stderr_revert_invalid` | `revert nonexistent` | Error message wording differs |
| `test_stderr_commit_nothing` | `commit -m "nothing"` | "nothing to commit" message differs |
| `test_stderr_clean_no_force` | `clean` (no `-f`) | "requires -f" message differs |
| `test_stderr_stash_pop_empty` | `stash pop` | Empty stash error message differs |
| `test_stderr_merge_abort_no_merge` | `merge --abort` | "no merge in progress" message differs |
| `test_stderr_rebase_abort_no_rebase` | `rebase --abort` | "no rebase in progress" message differs |

**Fix priority**: Low-medium. Affects user experience and script parsing of stderr.

---

## 4. Edge Cases (9 tests)

Various edge cases where gitr's behavior diverges from git.

| Test | Command | Gap |
|------|---------|-----|
| `test_edge_show_binary` | `show --stat` (binary files) | Binary file summary line format differs |
| `test_edge_diff_unicode` | `diff` (unicode filenames) | Unicode filename quoting/encoding differs |
| `test_edge_log_unicode` | `log --name-only` (unicode) | Unicode filename quoting in log differs |
| `test_edge_diff_empty_file` | `diff` (empty file gains content) | Diff format for empty-to-content differs |
| `test_edge_permission_diff` | `diff` (permission change) | `old mode`/`new mode` output differs |
| `test_edge_merge_conflict_status` | `status --porcelain` (conflict) | Conflict markers (`UU`) in status differ |
| `test_edge_merge_conflict_diff` | `diff` (during conflict) | Combined diff during conflict differs |
| `test_edge_cherry_pick_conflict_status` | `status --porcelain` (cp conflict) | Conflict markers in cherry-pick status differ |
| `test_state_after_reset_hard` | `reset --hard HEAD~1` | Index/worktree/HEAD state after reset differs |

**Fix priority**: Medium. These affect real-world workflows with non-ASCII files, binary assets, and conflict resolution.

---

## 5. Plumbing & Miscellaneous Flags (10 tests)

Various plumbing commands and flags that don't produce matching output.

| Test | Command | Gap |
|------|---------|-----|
| `test_ls_files_cached` | `ls-files -c` | Cached file listing does not match |
| `test_ls_files_others` | `ls-files -o --exclude-standard` | Untracked file listing does not match |
| `test_cat_file_blob` | `cat-file -p HEAD:file.txt` | Blob resolution via `tree:path` spec fails |
| `test_for_each_ref_contains` | `for-each-ref --contains=HEAD~1` | `--contains` filter not implemented |
| `test_describe_contains` | `describe --contains HEAD~1` | `--contains` mode not implemented |
| `test_fsck_full` | `fsck --full` | Full connectivity check behaves differently |
| `test_var_git_default_branch` | `var GIT_DEFAULT_BRANCH` | Returns different default branch name |
| `test_fmt_merge_msg_message` | `fmt-merge-msg -m "msg"` | Custom merge message formatting differs |
| `test_notes_list` | `notes list` | Note object listing format differs |
| `test_fetch_depth` | `fetch --depth=1` | Shallow fetch not implemented |

---

## 6. Blame Output Format (4 tests)

gitr's blame output formatting differs from git for several display flags.

| Test | Command | Gap |
|------|---------|-----|
| `test_blame_porcelain` | `blame --porcelain` | Machine-readable porcelain format differs |
| `test_blame_show_number` | `blame -n` | Original line number display differs |
| `test_blame_show_name` | `blame -f` | Filename display differs |
| `test_blame_suppress_author` | `blame -s` | Suppressed author format differs |

---

## 7. Format-Patch Output (4 tests)

gitr's format-patch does not produce identical mbox-formatted patch output.

| Test | Command | Gap |
|------|---------|-----|
| `test_format_patch_stdout` | `format-patch --stdout -1` | Patch headers/diff format differs |
| `test_format_patch_numbered` | `format-patch --stdout -n -2` | `[PATCH n/m]` subject line differs |
| `test_format_patch_subject_prefix` | `format-patch --subject-prefix="RFC PATCH"` | Custom prefix in subject differs |
| `test_format_patch_signoff` | `format-patch -s` | Signed-off-by trailer handling differs |

---

## 8. Diff Display Flags (4 tests)

Advanced diff display modes that are not yet implemented.

| Test | Command | Gap |
|------|---------|-----|
| `test_diff_find_renames_bare` | `diff -M HEAD~1 HEAD` | Rename detection (`-M`) not implemented |
| `test_diff_find_copies` | `diff -C HEAD~1 HEAD` | Copy detection (`-C`) not implemented |
| `test_diff_word_diff` | `diff --word-diff` | Word-level diff mode not implemented |
| `test_diff_color_words` | `diff --color-words` | Word-level colorized diff not implemented |

---

## 9. Log/Revwalk Flags (4 tests)

Log flags that walk non-standard commit graphs.

| Test | Command | Gap |
|------|---------|-----|
| `test_log_walk_reflogs` | `log -g --oneline` | Reflog walk (`-g`) not implemented |
| `test_log_walk_reflogs_long` | `log --walk-reflogs` | Reflog walk (long form) not implemented |
| `test_log_simplify_by_decoration` | `log --simplify-by-decoration` | Decoration-based simplification not implemented |
| `test_whatchanged_first_parent` | `whatchanged --first-parent` | First-parent raw diff output differs |

---

## 10. Branch/Switch Flags (3 tests)

| Test | Command | Gap |
|------|---------|-----|
| `test_branch_copy` | `branch -c feature copy` | Branch copy (`-c`) not implemented |
| `test_switch_create_force` | `switch -C feature` | Force-create (`-C`) not implemented |
| `test_switch_orphan` | `switch --orphan branch` | Orphan branch creation not implemented |

---

## 11. Merge/Rebase Flags (3 tests)

| Test | Command | Gap |
|------|---------|-----|
| `test_merge_continue` | `merge --continue` | Merge continue after conflict resolution not implemented |
| `test_rebase_keep_empty` | `rebase --keep-empty` | Preserving empty commits during rebase not implemented |
| `test_cherry_pick_edit` | `cherry-pick --no-edit` | `--no-edit` behavior/exit code differs |

---

## 12. Commit Flags (2 tests)

| Test | Command | Gap |
|------|---------|-----|
| `test_commit_dry_run` | `commit --dry-run` | Dry-run simulation not implemented |
| `test_commit_date_override` | `commit --date=<date>` | Custom author date override not implemented |

---

## Working on These Gaps

When fixing a parity gap:

1. Run the ignored test to see the current failure: `cargo test --test <file> <test_name> -- --ignored`
2. Fix the implementation in the relevant crate
3. Verify the test passes: `cargo test --test <file> <test_name> -- --ignored`
4. Remove the `#[ignore]` annotation
5. Run the full test suite: `cargo test --workspace`

The coverage matrix script can track progress: `./scripts/ci/parity-coverage.sh --no-color`
