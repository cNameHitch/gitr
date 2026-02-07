# Feature Specification: Merge Engine

**Feature Branch**: `012-merge-engine`
**Created**: 2026-02-05
**Status**: Draft
**Depends On**: 001-foundation-utilities, 003-object-model, 006-object-database, 007-index-staging-area, 010-repository-and-setup, 011-diff-engine

## User Scenarios & Testing

### User Story 1 - Three-Way File Merge (Priority: P1)

As a gitr library, I need to merge two versions of a file using their common ancestor so that `git merge` can combine changes from different branches.

**Why this priority**: Content-level merge is the core of the merge engine.

**Independent Test**: Create base, ours, theirs versions with non-overlapping changes, merge, verify clean result.

**Acceptance Scenarios**:

1. **Given** base, ours, theirs with non-overlapping changes, **When** merged, **Then** a clean merge combining both changes is produced.
2. **Given** overlapping changes in the same region, **When** merged, **Then** a conflict is reported with conflict markers.
3. **Given** one side modified and the other unchanged, **When** merged, **Then** the modified side wins cleanly.
4. **Given** both sides made identical changes, **When** merged, **Then** the change is accepted without conflict.

---

### User Story 2 - Tree-Level Merge (ORT) (Priority: P1)

As a gitr library, I need to merge two tree hierarchies using the ORT (Ostensibly Recursive's Twin) strategy so that full directory merges work.

**Why this priority**: The ORT strategy is git's primary merge algorithm, handling renames, directory conflicts, and nested merges.

**Independent Test**: Create two branches with various changes (add, delete, rename, modify), merge, verify result matches C git.

**Acceptance Scenarios**:

1. **Given** two branches where different files are modified, **When** merged, **Then** both changes appear in the result.
2. **Given** a file renamed on one side and modified on the other, **When** merged, **Then** the rename is followed and content is merged.
3. **Given** a file deleted on one side and modified on the other, **Then** a conflict is reported (modify/delete).
4. **Given** both sides add a file with the same name but different content, **Then** an add/add conflict is reported.

---

### User Story 3 - Merge Conflict Handling (Priority: P1)

As a git user, I need merge conflicts to be recorded in the index and working tree so that I can resolve them manually.

**Why this priority**: Conflict resolution is a critical part of the merge workflow.

**Independent Test**: Create a conflict, verify conflict markers in working tree and stages 1/2/3 in index.

**Acceptance Scenarios**:

1. **Given** a content conflict, **When** the merge completes with conflicts, **Then** the working tree file contains standard conflict markers (`<<<<<<<`, `=======`, `>>>>>>>`).
2. **Given** conflicts, **When** the index is updated, **Then** stages 1 (base), 2 (ours), 3 (theirs) are set for conflicted files.
3. **Given** a resolved conflict, **When** `git add` is run, **Then** the conflict entries are replaced with a stage-0 entry.

---

### User Story 4 - Merge Strategies (Priority: P2)

As a git user, I need different merge strategies (recursive, ort, ours, subtree) to handle different merge scenarios.

**Why this priority**: Different situations call for different strategies.

**Acceptance Scenarios**:

1. **Given** `--strategy=ours`, **When** merging, **Then** the result is always our tree, ignoring theirs.
2. **Given** `--strategy=ort` (default), **When** merging, **Then** the ORT algorithm is used with rename detection.
3. **Given** strategy options like `--strategy-option=theirs`, **When** content conflicts, **Then** their side is preferred.

---

### User Story 5 - Cherry-Pick and Revert (Priority: P2)

As a gitr library, I need to apply individual commits (cherry-pick) and reverse-apply commits (revert) using the merge machinery.

**Why this priority**: Cherry-pick and revert are common operations that reuse the merge engine.

**Acceptance Scenarios**:

1. **Given** a commit to cherry-pick, **When** applied, **Then** the commit's changes are merged onto the current branch using the commit's parent as the merge base.
2. **Given** a commit to revert, **When** applied, **Then** the inverse of the commit's changes are applied.
3. **Given** a cherry-pick with conflicts, **When** they occur, **Then** they are recorded identically to merge conflicts.

---

### User Story 6 - Sequencer (Priority: P3)

As a gitr library, I need a sequencer for multi-commit operations (rebase, cherry-pick sequence) with interruption and continuation.

**Why this priority**: Interactive rebase and multi-pick use the sequencer.

**Acceptance Scenarios**:

1. **Given** a sequence of commits to cherry-pick, **When** one conflicts, **Then** the operation pauses and can be continued.
2. **Given** a paused operation, **When** `--continue` is used, **Then** the remaining commits are applied.
3. **Given** a paused operation, **When** `--abort` is used, **Then** the original state is restored.

### Edge Cases

- Merge with empty tree (should succeed)
- Criss-cross merge (multiple merge bases)
- Rename/rename conflict (both sides rename the same file differently)
- Directory/file conflict (one side adds a file where the other adds a directory)
- Recursive merge (merge bases that are themselves merges)
- Binary file conflicts
- Merge with submodule changes
- Three-way merge where base doesn't exist (treat as empty)

## Requirements

### Functional Requirements

- **FR-001**: System MUST implement three-way content merge with conflict detection
- **FR-002**: System MUST implement the ORT merge strategy (tree-level merge with rename following)
- **FR-003**: System MUST produce standard conflict markers in working tree files
- **FR-004**: System MUST update the index with conflict stages (1=base, 2=ours, 3=theirs)
- **FR-005**: System MUST detect and handle rename/rename, modify/delete, add/add, and directory/file conflicts
- **FR-006**: System MUST support merge strategies: ort (default), recursive, ours, subtree
- **FR-007**: System MUST support strategy options: ours, theirs, patience, histogram, rename-threshold
- **FR-008**: System MUST implement cherry-pick (apply commit as merge against its parent)
- **FR-009**: System MUST implement revert (apply inverse of commit)
- **FR-010**: System MUST implement the sequencer for multi-commit operations with pause/continue/abort
- **FR-011**: System MUST handle criss-cross merges (virtual merge base computation)
- **FR-012**: System MUST support `git apply` for applying patch files

### Key Entities

- **MergeResult**: Outcome of a merge operation (clean or conflicts list)
- **ContentMerge**: Three-way content merge result
- **ConflictEntry**: A file with merge conflicts
- **MergeStrategy**: Pluggable merge strategy
- **Sequencer**: State machine for multi-commit operations

## Success Criteria

### Measurable Outcomes

- **SC-001**: Three-way merge produces identical results to C git for a corpus of 200+ merge scenarios
- **SC-002**: Conflict markers match C git's format exactly
- **SC-003**: Rename-following merge correctly handles all rename scenarios
- **SC-004**: Cherry-pick and revert produce identical results to C git
- **SC-005**: Sequencer correctly handles interruption and continuation
