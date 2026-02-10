# Ignored Parity Tests

This document catalogs all E2E parity tests currently marked `#[ignore]` across the test suite. Each test compares gitr output against C git for identical inputs. Tests are ignored because gitr does not yet match git's behavior for that specific case.

**To activate a test**: fix the underlying parity gap in gitr, then remove the `#[ignore]` annotation. The test will start running in CI automatically.

**To run ignored tests locally**: `cargo test --test <file> -- --ignored`

---

## Summary

| Category | Count | Files |
|----------|------:|-------|
| Rebase | 10 | `parity_flags_rebase_pick_revert.rs` |
| Cherry-pick | 7 | `parity_flags_rebase_pick_revert.rs` |
| Branch flags | 7 | `parity_flags_commit_branch_merge_tag.rs` |
| Tag flags | 7 | `parity_flags_commit_branch_merge_tag.rs` |
| Revert | 6 | `parity_flags_rebase_pick_revert.rs` |
| Checkout/switch flags | 6 | `parity_flags_checkout_status_add.rs` |
| Diff plumbing output format | 11 | `parity_flags_plumbing_deep.rs` |
| Error path exit codes | 20 | `parity_error_tests.rs` |
| Stderr message text | 11 | `parity_stderr_tests.rs` |
| Log/show display | 9 | `parity_flags_log_diff_show.rs` |
| Edge cases | 9 | `parity_edge_cases_tests.rs` |
| Plumbing & misc flags | 10 | `parity_flags_plumbing_deep.rs`, `parity_flags_porcelain_deep.rs` |
| Diff display flags | 7 | `parity_flags_tier1_gaps.rs`, `parity_flags_log_diff_show.rs` |
| Commit flags | 6 | `parity_flags_tier1_gaps.rs`, `parity_flags_commit_branch_merge_tag.rs` |
| Mv/rm dry-run | 4 | `parity_flags_checkout_status_add.rs` |
| Whatchanged deprecation | 4 | `e2e_missing_commands_tests.rs`, `parity_flags_porcelain_deep.rs` |
| Blame output format | 4 | `parity_flags_porcelain_deep.rs` |
| Format-patch output | 4 | `parity_flags_porcelain_deep.rs` |
| Add flags | 4 | `parity_flags_checkout_status_add.rs` |
| Clone/fetch/pull | 3 | `parity_flags_rebase_pick_revert.rs` |
| Merge flags | 4 | `parity_flags_tier1_gaps.rs`, `parity_flags_commit_branch_merge_tag.rs` |
| Status flags | 2 | `parity_flags_checkout_status_add.rs` |
| Push (hanging) | 2 | `parity_flags_rebase_pick_revert.rs` |
| Clean flags | 1 | `parity_flags_checkout_status_add.rs` |
| **Total** | **148** | |

---

## 1. Rebase (10 tests)

gitr's rebase does not produce matching results for most operations.

| Test | Command | Gap |
|------|---------|-----|
| `test_rebase_basic` | `rebase main` | Basic rebase not matching git |
| `test_rebase_onto` | `rebase --onto main HEAD~1` | Selective commit replay with `--onto` not matching |
| `test_rebase_continue_after_conflict` | `rebase --continue` | Continue after conflict resolution not matching |
| `test_rebase_stat` | `rebase --stat main` | Diffstat output during rebase not matching |
| `test_rebase_quiet` | `rebase -q main` | Quiet mode (short flag) not matching |
| `test_rebase_quiet_long_flag` | `rebase --quiet main` | Quiet mode (long flag) not matching |
| `test_rebase_no_stat` | `rebase --no-stat main` | Suppressed diffstat not matching |
| `test_rebase_signoff` | `rebase --signoff main` | Signed-off-by trailer not matching |
| `test_rebase_keep_empty` | `rebase --keep-empty main` | Preserving empty commits not matching |
| `test_rebase_verifies_worktree` | `rebase main` | Post-rebase worktree state not matching |

**File**: `parity_flags_rebase_pick_revert.rs`

---

## 2. Cherry-pick (7 tests)

| Test | Command | Gap |
|------|---------|-----|
| `test_cherry_pick_basic` | `cherry-pick <oid>` | Basic single-commit cherry-pick not matching |
| `test_cherry_pick_no_commit` | `cherry-pick -n <oid>` | No-commit mode (short flag) not matching |
| `test_cherry_pick_no_commit_flag_long` | `cherry-pick --no-commit <oid>` | No-commit mode (long flag) not matching |
| `test_cherry_pick_x_flag` | `cherry-pick -x <oid>` | "(cherry picked from commit ...)" line not appended |
| `test_cherry_pick_signoff` | `cherry-pick --signoff <oid>` | Signed-off-by trailer not appended |
| `test_cherry_pick_range` | `cherry-pick feature~2..feature` | Commit range cherry-pick not matching |
| `test_cherry_pick_multiple_commits` | `cherry-pick <oid1> <oid2>` | Multiple individual commits not matching |

**File**: `parity_flags_rebase_pick_revert.rs`

---

## 3. Revert (6 tests)

| Test | Command | Gap |
|------|---------|-----|
| `test_revert_basic` | `revert --no-edit HEAD` | Basic revert not matching |
| `test_revert_no_commit` | `revert -n HEAD` | No-commit mode (short flag) not matching |
| `test_revert_no_commit_long_flag` | `revert --no-commit HEAD` | No-commit mode (long flag) not matching |
| `test_revert_signoff` | `revert --signoff --no-edit HEAD` | Signed-off-by trailer not appended |
| `test_revert_range` | `revert --no-edit HEAD~2..HEAD` | Commit range revert not matching |
| `test_revert_merge_m1` | `revert -m 1 --no-edit HEAD` | Merge commit revert with parent selection not matching |

**File**: `parity_flags_rebase_pick_revert.rs`

---

## 4. Branch Flags (7 tests)

| Test | Command | Gap |
|------|---------|-----|
| `test_branch_force_move` | `branch -f movable HEAD~1` | Force-move branch to different commit not supported |
| `test_branch_remotes` | `branch -r` | Remote-tracking branch listing not matching |
| `test_branch_merged` | `branch --merged` | Listing merged branches not matching |
| `test_branch_no_merged` | `branch --no-merged` | Listing unmerged branches not matching |
| `test_branch_format` | `branch --format=%(refname:short)` | Custom format string not supported |
| `test_branch_list_pattern` | `branch --list feat*` | Glob pattern filtering not supported |
| `test_branch_delete_unmerged_fails` | `branch -d feature` (unmerged) | Exit code for deleting unmerged branch differs |
| `test_branch_copy` | `branch -c feature copy` | Branch copy (`-c`) not implemented |

**Files**: `parity_flags_commit_branch_merge_tag.rs`, `parity_flags_tier1_gaps.rs`

---

## 5. Tag Flags (7 tests)

| Test | Command | Gap |
|------|---------|-----|
| `test_tag_annotated` | `tag -a v2.0 -m "version 2.0"` | Annotated tag creation not matching |
| `test_tag_contains` | `tag --contains HEAD~1` | Listing tags containing a commit not supported |
| `test_tag_points_at` | `tag --points-at HEAD` | Listing tags pointing at a commit not supported |
| `test_tag_points_at_older_commit` | `tag --points-at HEAD~2` | Points-at for older commits not supported |
| `test_tag_list_pattern` | `tag -l "v*"` | Glob pattern filtering not supported |
| `test_tag_pattern_no_match` | `tag -l "nonexistent*"` | Empty pattern match output not matching |
| `test_tag_no_merged` | `tag --no-merged main` | Listing unmerged tags not supported |

**File**: `parity_flags_commit_branch_merge_tag.rs`

---

## 6. Checkout/Switch Flags (6 tests)

| Test | Command | Gap |
|------|---------|-----|
| `checkout_detach_head` | `checkout --detach HEAD~1` | Detached HEAD mode not matching |
| `checkout_orphan` | `checkout --orphan new-root` | Orphan branch creation not matching |
| `checkout_ours_during_conflict` | `checkout --ours conflict.txt` | Conflict resolution with `--ours` not supported |
| `checkout_theirs_during_conflict` | `checkout --theirs conflict.txt` | Conflict resolution with `--theirs` not supported |
| `switch_create_branch_c` | `switch -c new-branch` | Create-and-switch with `-c` not matching |
| `switch_force_create_big_c` | `switch -C existing` | Force-recreate with `-C` not matching |
| `test_switch_create_force` | `switch -C feature` | Force-create via switch not matching |
| `test_switch_orphan` | `switch --orphan branch` | Orphan branch creation not matching |

**Files**: `parity_flags_checkout_status_add.rs`, `parity_flags_tier1_gaps.rs`

---

## 7. Diff Plumbing Output Format (11 tests)

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

## 8. Error Path Exit Codes (20 tests)

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

## 9. Stderr Message Text (11 tests)

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

## 10. Log/Show Display (9 tests)

Log and show formatting options that do not match git's output.

| Test | Command | Gap |
|------|---------|-----|
| `log_graph` | `log --graph --oneline` | ASCII graph rendering not matching |
| `log_abbrev_commit` | `log --abbrev-commit` | Abbreviated commit hash format not matching |
| `log_pretty_format_short_hash_subject` | `log --pretty=format:%h %s` | Short hash `%h` in format not matching |
| `log_decorate` | `log --decorate --oneline` | Ref name decoration not matching |
| `log_source` | `log --source --all --oneline` | Source ref annotation not supported |
| `log_follow` | `log --follow -- renamed.txt` | Rename tracking across history not supported |
| `log_find_renames` | `log -M --name-status --oneline` | Rename detection in log not matching |
| `log_diff_filter_added` | `log --diff-filter=A --name-only` | Diff-filter for added files not supported |
| `show_decorate` | `show --decorate -s` | Ref name decoration in show not matching |
| `test_log_walk_reflogs` | `log -g --oneline` | Reflog walk (`-g`) not implemented |
| `test_log_walk_reflogs_long` | `log --walk-reflogs` | Reflog walk (long form) not implemented |
| `test_log_simplify_by_decoration` | `log --simplify-by-decoration` | Decoration-based simplification not implemented |

**Files**: `parity_flags_log_diff_show.rs`, `parity_flags_tier1_gaps.rs`

---

## 11. Edge Cases (9 tests)

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

## 12. Plumbing & Miscellaneous Flags (10 tests)

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

## 13. Diff Display Flags (7 tests)

Advanced diff display modes and flags that are not yet implemented or not matching.

| Test | Command | Gap |
|------|---------|-----|
| `test_diff_find_renames_bare` | `diff -M HEAD~1 HEAD` | Rename detection (`-M`) not implemented |
| `test_diff_find_copies` | `diff -C HEAD~1 HEAD` | Copy detection (`-C`) not implemented |
| `test_diff_word_diff` | `diff --word-diff` | Word-level diff mode not implemented |
| `test_diff_color_words` | `diff --color-words` | Word-level colorized diff not implemented |
| `diff_raw` | `diff --raw` | Raw diff output format not supported |
| `diff_full_index` | `diff --full-index` | Full blob OID in diff index line not supported |
| `diff_reverse` | `diff -R` | Reverse diff (swap a/b sides) not supported |

**Files**: `parity_flags_tier1_gaps.rs`, `parity_flags_log_diff_show.rs`

---

## 14. Commit Flags (6 tests)

| Test | Command | Gap |
|------|---------|-----|
| `test_commit_dry_run` | `commit --dry-run` | Dry-run simulation not implemented |
| `test_commit_date_override` | `commit --date=<date>` | Custom author date override not implemented |
| `test_commit_allow_empty_message` | `commit --allow-empty-message -m ""` | Empty message flag not working |
| `test_commit_signoff` | `commit -s -m "msg"` | Signed-off-by trailer not appended |

**Files**: `parity_flags_tier1_gaps.rs`, `parity_flags_commit_branch_merge_tag.rs`

---

## 15. Mv/Rm Dry-Run (4 tests)

| Test | Command | Gap |
|------|---------|-----|
| `mv_dry_run` | `mv -n file.txt new.txt` | Dry-run output not matching |
| `mv_dry_run_long` | `mv --dry-run file.txt new.txt` | Dry-run output not matching |
| `rm_dry_run` | `rm -n file.txt` | Dry-run output not matching |
| `rm_dry_run_long` | `rm --dry-run file.txt` | Dry-run output not matching |

**File**: `parity_flags_checkout_status_add.rs`

---

## 16. Whatchanged Deprecation (4 tests)

git >= 2.47 deprecated `whatchanged` and now requires `--i-still-use-this` flag to run. These tests cannot pass on newer git versions.

| Test | Command | Gap |
|------|---------|-----|
| `test_whatchanged_basic` | `whatchanged` | Deprecated in git >= 2.47 |
| `test_whatchanged_max_count` | `whatchanged -n 2` | Deprecated in git >= 2.47 |
| `test_whatchanged_name_only` | `whatchanged --name-only` | Deprecated in git >= 2.47 |
| `test_whatchanged_name_status` | `whatchanged --name-status` | Deprecated in git >= 2.47 |
| `test_whatchanged_first_parent` | `whatchanged --first-parent` | First-parent raw diff output differs |

**Files**: `e2e_missing_commands_tests.rs`, `parity_flags_porcelain_deep.rs`

---

## 17. Blame Output Format (4 tests)

gitr's blame output formatting differs from git for several display flags.

| Test | Command | Gap |
|------|---------|-----|
| `test_blame_porcelain` | `blame --porcelain` | Machine-readable porcelain format differs |
| `test_blame_show_number` | `blame -n` | Original line number display differs |
| `test_blame_show_name` | `blame -f` | Filename display differs |
| `test_blame_suppress_author` | `blame -s` | Suppressed author format differs |

**File**: `parity_flags_porcelain_deep.rs`

---

## 18. Format-Patch Output (4 tests)

gitr's format-patch does not produce identical mbox-formatted patch output.

| Test | Command | Gap |
|------|---------|-----|
| `test_format_patch_stdout` | `format-patch --stdout -1` | Patch headers/diff format differs |
| `test_format_patch_numbered` | `format-patch --stdout -n -2` | `[PATCH n/m]` subject line differs |
| `test_format_patch_subject_prefix` | `format-patch --subject-prefix="RFC PATCH"` | Custom prefix in subject differs |
| `test_format_patch_signoff` | `format-patch -s` | Signed-off-by trailer handling differs |

**File**: `parity_flags_porcelain_deep.rs`

---

## 19. Add Flags (4 tests)

| Test | Command | Gap |
|------|---------|-----|
| `add_dry_run` | `add -n file.txt` | Dry-run output not matching |
| `add_dry_run_long` | `add --dry-run file.txt` | Dry-run output not matching |
| `add_verbose` | `add -v file.txt` | Verbose output not matching |
| `add_verbose_long` | `add --verbose file.txt` | Verbose output not matching |

**File**: `parity_flags_checkout_status_add.rs`

---

## 20. Clone/Fetch/Pull (3 tests)

| Test | Command | Gap |
|------|---------|-----|
| `test_clone_depth_1` | `clone --depth 1` | Shallow clone not supported |
| `test_fetch_new_branch` | `fetch --all` (new branch) | New remote branches not matching show-ref |
| `test_pull_rebase` | `pull --rebase` | Pull with rebase not matching |

**File**: `parity_flags_rebase_pick_revert.rs`

---

## 21. Merge Flags (4 tests)

| Test | Command | Gap |
|------|---------|-----|
| `test_merge_continue` | `merge --continue` | Merge continue after conflict not implemented |
| `test_cherry_pick_edit` | `cherry-pick --no-edit` | `--no-edit` behavior differs |
| `test_merge_three_way_clean` | `merge branch -m "3-way"` | Three-way merge commit not matching |

**Files**: `parity_flags_tier1_gaps.rs`, `parity_flags_commit_branch_merge_tag.rs`

---

## 22. Status Flags (2 tests)

| Test | Command | Gap |
|------|---------|-----|
| `status_porcelain_v2` | `status --porcelain=v2` | Porcelain v2 format not implemented |
| `status_nul_terminated` | `status -z` | NUL-terminated output not matching |

**File**: `parity_flags_checkout_status_add.rs`

---

## 23. Push — Hanging (2 tests)

These tests hang indefinitely because gitr's push command blocks on a subprocess in certain scenarios.

| Test | Command | Gap |
|------|---------|-----|
| `test_push_nothing_to_push` | `push` (no new commits) | gitr push blocks on subprocess |
| `test_push_tags` | `push --tags` | gitr push blocks on subprocess |

**File**: `parity_flags_rebase_pick_revert.rs`

**Fix priority**: High. Hanging tests block CI if not ignored.

---

## 24. Clean Flags (1 test)

| Test | Command | Gap |
|------|---------|-----|
| `clean_only_ignored` | `clean -fX` | Remove only ignored files (uppercase `-X`) not matching |

**File**: `parity_flags_checkout_status_add.rs`

---

## Working on These Gaps

When fixing a parity gap:

1. Run the ignored test to see the current failure: `cargo test --test <file> <test_name> -- --ignored`
2. Fix the implementation in the relevant crate
3. Verify the test passes: `cargo test --test <file> <test_name> -- --ignored`
4. Remove the `#[ignore]` annotation
5. Run the full test suite: `cargo test --workspace`

The coverage matrix script can track progress: `./scripts/ci/parity-coverage.sh --no-color`
