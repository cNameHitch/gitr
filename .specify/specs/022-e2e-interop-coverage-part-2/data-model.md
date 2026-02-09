# Data Model: 021-e2e-interop-coverage

**Date**: 2026-02-07

## Overview

This feature is test-only — no persistent data entities are introduced. The "data" in this context is the test infrastructure extensions and test case catalog.

## Test Harness Extensions

### New Helper Functions (common/mod.rs)

| Function | Inputs | Output | Purpose |
|----------|--------|--------|---------|
| `git_stdin(dir, args, stdin)` | `&Path, &[&str], &[u8]` | `CommandResult` | Run C git with piped stdin |
| `gitr_stdin(dir, args, stdin)` | `&Path, &[&str], &[u8]` | `CommandResult` | Run gitr with piped stdin |
| `git_stdin_with_date(dir, args, stdin, epoch)` | `&Path, &[&str], &[u8], &str` | `CommandResult` | Run C git with stdin + date override |
| `gitr_stdin_with_date(dir, args, stdin, epoch)` | `&Path, &[&str], &[u8], &str` | `CommandResult` | Run gitr with stdin + date override |
| `setup_submodule_repo(dir, sub_dir)` | `&Path, &Path` | `()` | Create repo with submodule via file:// |
| `setup_untracked_files(dir)` | `&Path` | `()` | Create repo with untracked + ignored files |
| `setup_large_repo(dir, commits, branches, files)` | `&Path, usize, usize, usize` | `()` | Parameterized large repo |

### Assertion Helpers (existing, reused)

| Function | Purpose |
|----------|---------|
| `assert_output_eq` | Compare stdout + exit code |
| `assert_stdout_eq` | Compare stdout only |
| `assert_exit_code_eq` | Compare exit code only |
| `assert_fsck_clean` | Verify repo integrity |
| `assert_repo_state_eq` | Compare HEAD, refs, loose objects |

## Test Case Catalog

### e2e_porcelain_coverage_tests.rs (~22 tests)

| Test | Command | Validates |
|------|---------|-----------|
| `test_clean_dry_run` | `clean -n` | Lists files without removing |
| `test_clean_force` | `clean -f` | Removes untracked files |
| `test_clean_force_dirs` | `clean -fd` | Removes untracked files and dirs |
| `test_clean_ignored` | `clean -fx` | Removes ignored files too |
| `test_clean_no_untracked` | `clean -f` | No-op when nothing to clean |
| `test_submodule_add_init_update` | `submodule add/init/update` | Full submodule setup workflow |
| `test_submodule_status` | `submodule status` | Shows submodule commit state |
| `test_submodule_sync` | `submodule sync` | Updates remote URLs |
| `test_submodule_deinit` | `submodule deinit` | Unregisters submodule |
| `test_submodule_foreach` | `submodule foreach` | Runs command in each submodule |
| `test_submodule_cross_tool` | submodule ops | gitr submodule readable by C git |
| `test_submodule_nested` | `submodule update --recursive` | Recursive submodule init |
| `test_worktree_add_list` | `worktree add/list` | Create and list worktrees |
| `test_worktree_remove` | `worktree remove` | Clean removal of worktree |
| `test_worktree_prune` | `worktree prune` | Prune stale worktrees |
| `test_worktree_detach` | `worktree add --detach` | Detached HEAD worktree |
| `test_worktree_cross_tool` | worktree ops | gitr worktree visible to C git |
| `test_format_patch_single` | `format-patch` | Generate patch for single commit |
| `test_format_patch_range` | `format-patch` | Generate patches for commit range |
| `test_am_apply_patch` | `am` | Apply patch and verify commit |
| `test_format_patch_am_roundtrip` | both | Create with gitr, apply with git (and vice versa) |
| `test_am_three_way` | `am --three-way` | Three-way merge application |

### e2e_plumbing_coverage_tests.rs (~20 tests)

| Test | Command | Validates |
|------|---------|-----------|
| `test_mktag_from_stdin` | `mktag` | Create tag object from stdin |
| `test_mktag_invalid_target` | `mktag` | Error on nonexistent target |
| `test_mktree_from_stdin` | `mktree` | Build tree from ls-tree format |
| `test_mktree_missing_flag` | `mktree --missing` | Allow missing objects |
| `test_commit_tree_basic` | `commit-tree` | Create commit from tree OID |
| `test_commit_tree_with_parents` | `commit-tree -p` | Multi-parent commit |
| `test_pack_objects_stdout` | `pack-objects --stdout` | Pack objects to stdout |
| `test_pack_objects_revs` | `pack-objects --revs` | Pack from revision input |
| `test_pack_objects_roundtrip` | `pack-objects + index-pack` | Create pack, build index, verify |
| `test_index_pack_verify` | `index-pack` | Build index for existing pack |
| `test_verify_pack_valid` | `verify-pack -v` | Verify pack integrity |
| `test_verify_pack_stats` | `verify-pack -s` | Stats-only output |
| `test_update_index_add` | `update-index --add` | Stage file via plumbing |
| `test_update_index_cacheinfo` | `update-index --cacheinfo` | Stage by OID |
| `test_update_index_remove` | `update-index --remove` | Unstage file |
| `test_update_ref_create` | `update-ref` | Create new ref |
| `test_update_ref_delete` | `update-ref -d` | Delete ref |
| `test_update_ref_stdin_transaction` | `update-ref --stdin` | Batch ref updates |
| `test_check_attr_output` | `check-attr` | Query gitattributes |
| `test_check_ignore_output` | `check-ignore` | Query gitignore patterns |

### e2e_bundle_archive_notes_tests.rs (~16 tests)

| Test | Command | Validates |
|------|---------|-----------|
| `test_bundle_create_verify` | `bundle create/verify` | Create and verify bundle |
| `test_bundle_gitr_create_git_unbundle` | `bundle` | Cross-tool: gitr creates, git reads |
| `test_bundle_git_create_gitr_unbundle` | `bundle` | Cross-tool: git creates, gitr reads |
| `test_bundle_list_heads` | `bundle list-heads` | List bundle refs |
| `test_archive_tar` | `archive --format=tar` | Create tar, verify contents |
| `test_archive_zip` | `archive --format=zip` | Create zip, verify contents |
| `test_archive_prefix` | `archive --prefix` | Prefix paths in archive |
| `test_archive_cross_tool` | `archive` | gitr archive readable by standard tools |
| `test_notes_add_show` | `notes add/show` | Add and retrieve note |
| `test_notes_list` | `notes list` | List all notes |
| `test_notes_remove` | `notes remove` | Delete a note |
| `test_notes_append` | `notes append` | Append to existing note |
| `test_notes_cross_tool` | `notes` | gitr notes visible to C git |
| `test_replace_object` | `replace` | Create replacement ref |
| `test_replace_delete` | `replace -d` | Remove replacement |
| `test_replace_cross_tool` | `replace` | gitr replacements honored by C git |

### e2e_maintenance_hooks_scale_tests.rs (~14 tests)

| Test | Command | Validates |
|------|---------|-----------|
| `test_prune_unreachable` | `prune` | Removes unreachable objects |
| `test_prune_dry_run` | `prune -n` | Lists without removing |
| `test_prune_preserves_reachable` | `prune` | Reachable objects untouched |
| `test_fast_import_basic` | `fast-import` | Import commits from stream |
| `test_fast_import_cross_tool` | `fast-import` | gitr import readable by C git |
| `test_fast_import_marks` | `fast-import --export-marks` | Mark export file matches |
| `test_hook_pre_commit_fires` | commit + hook | Hook script executes |
| `test_hook_pre_commit_blocks` | commit + hook | Non-zero hook blocks commit |
| `test_hook_post_commit_fires` | commit + hook | Post-commit hook runs |
| `test_hook_commit_msg` | commit + hook | commit-msg hook receives file arg |
| `test_large_repo_log` | log | 100+ commits, output matches |
| `test_large_repo_many_branches` | branch, for-each-ref | 50+ branches, output matches |
| `test_large_repo_many_files` | ls-files, ls-tree, status | 500+ files, output matches |
| `test_config_local_overrides_global` | config | Local scope takes precedence |
