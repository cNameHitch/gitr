# Feature Specification: Revision Walking

**Feature Branch**: `013-revision-walking`
**Created**: 2026-02-05
**Status**: Draft
**Depends On**: 001-foundation-utilities, 003-object-model, 006-object-database, 008-reference-system, 010-repository-and-setup

## User Scenarios & Testing

### User Story 1 - Commit Traversal (Priority: P1)

As a gitr library consumer, I need to walk commit history in various orders so that `git log`, `git rev-list`, and reachability analysis work.

**Why this priority**: History traversal is fundamental to log, merge-base, fetch negotiation, and gc.

**Independent Test**: Walk history of a known repository and verify commit order matches C git's `git rev-list`.

**Acceptance Scenarios**:

1. **Given** a starting commit, **When** walking with chronological order, **Then** commits are returned from newest to oldest by commit date.
2. **Given** a starting commit, **When** walking with topological order, **Then** parents always appear after children.
3. **Given** `--author-date-order`, **When** walking, **Then** commits are sorted by author date (not committer date).
4. **Given** `--ancestry-path`, **When** walking from A to B, **Then** only commits on the path between A and B are included.
5. **Given** `--first-parent`, **When** walking, **Then** only the first parent of merge commits is followed.

---

### User Story 2 - Revision Ranges (Priority: P1)

As a git user, I need revision range syntax (A..B, A...B, ^A B) so that I can specify subsets of history.

**Why this priority**: Range syntax is used by log, diff, format-patch, and many other commands.

**Independent Test**: Specify various ranges and verify the resulting commit list matches C git.

**Acceptance Scenarios**:

1. **Given** `A..B`, **When** evaluated, **Then** commits reachable from B but not A are returned.
2. **Given** `A...B`, **When** evaluated, **Then** commits reachable from either but not both (symmetric difference) are returned.
3. **Given** `^A B C`, **When** evaluated, **Then** commits reachable from B or C but not A are returned.
4. **Given** `--all`, **When** walking, **Then** all refs are used as starting points.

---

### User Story 3 - Commit-Graph Acceleration (Priority: P2)

As a gitr library, I need to use the commit-graph file for fast commit access without parsing pack objects.

**Why this priority**: Commit-graph dramatically speeds up log and merge-base in large repositories.

**Independent Test**: Walk history with and without commit-graph, verify same results but faster with graph.

**Acceptance Scenarios**:

1. **Given** a repository with a commit-graph file, **When** walking history, **Then** the commit-graph is used for fast parent/date access.
2. **Given** a commit-graph, **When** computing merge base, **Then** the generation number is used to prune unreachable commits.
3. **Given** no commit-graph, **When** walking, **Then** falls back to reading commits from ODB.

---

### User Story 4 - Merge Base Computation (Priority: P1)

As a gitr library, I need to find the merge base of two or more commits for merge, rebase, and diff operations.

**Why this priority**: Merge base is needed by merge, rebase, diff A...B, and many other operations.

**Independent Test**: Compute merge base for known history graphs and verify it matches C git.

**Acceptance Scenarios**:

1. **Given** two commits with a single common ancestor, **When** merge-base is computed, **Then** the LCA (lowest common ancestor) is returned.
2. **Given** a criss-cross merge history, **When** merge-base is computed, **Then** all best merge bases are returned.
3. **Given** two commits with no common ancestor, **When** merge-base is computed, **Then** no base is returned.

---

### User Story 5 - Pretty-Print Commits (Priority: P2)

As a git user, I need formatted commit output so that `git log --format` works with all format specifiers.

**Why this priority**: Log formatting is a key user-facing feature.

**Acceptance Scenarios**:

1. **Given** `--oneline` format, **When** printing, **Then** short OID + first line of message is shown.
2. **Given** `--format='%H %an %s'`, **When** printing, **Then** full OID, author name, and subject are shown.
3. **Given** `--graph`, **When** printing, **Then** ASCII art graph lines are drawn alongside commit messages.

---

### User Story 6 - Object Listing and Filtering (Priority: P2)

As a gitr library, I need to list all objects reachable from commits with optional filters for fetch/clone operations.

**Why this priority**: Object enumeration is needed for pack generation, fetch, and gc.

**Acceptance Scenarios**:

1. **Given** a set of commits, **When** listing reachable objects, **Then** all blobs, trees, and commits are included.
2. **Given** `--filter=blob:limit=1m`, **When** listing, **Then** blobs over 1MB are excluded (partial clone).
3. **Given** `--objects`, **When** walking, **Then** tree and blob OIDs are emitted alongside commits.

### Edge Cases

- Walk starting from a tag (peel to commit)
- Walk with no commits (empty repo)
- Extremely deep history (>100K commits) — stack overflow risk
- Grafts and replace objects altering history
- Shallow clone with missing parents
- Commit-graph with generation number overflow
- Multiple roots (independent histories merged)

## Requirements

### Functional Requirements

- **FR-001**: System MUST traverse commit history following parent links
- **FR-002**: System MUST support sorting: chronological, topological, author-date, reverse
- **FR-003**: System MUST support revision ranges: A..B, A...B, ^A B, --all, --branches, --tags
- **FR-004**: System MUST compute merge base(s) of two or more commits
- **FR-005**: System MUST support --first-parent to follow only first parents
- **FR-006**: System MUST support --ancestry-path filtering
- **FR-007**: System MUST use commit-graph file for accelerated access when available
- **FR-008**: System MUST support commit-graph generation numbers for efficient pruning
- **FR-009**: System MUST support pretty-printing with all format specifiers (%H, %h, %an, %ae, %s, %b, etc.)
- **FR-010**: System MUST support --graph for ASCII art history visualization
- **FR-011**: System MUST list reachable objects (commits, trees, blobs) for pack generation
- **FR-012**: System MUST support object filters for partial clone (blob:limit, blob:none, tree:depth)
- **FR-013**: System MUST handle shallow clones (commits with missing parents)

### Key Entities

- **RevWalk**: Iterator over commits in specified order
- **RevisionRange**: Parsed revision range specification
- **CommitGraph**: Parsed commit-graph file for fast access
- **MergeBase**: Result of merge base computation

## Success Criteria

### Measurable Outcomes

- **SC-001**: Commit walk order matches C git's `git rev-list` for all sort options
- **SC-002**: Merge base computation matches C git's `git merge-base` for all test cases
- **SC-003**: Commit-graph accelerated walks are 10x+ faster than ODB-only walks on large repos
- **SC-004**: Pretty-print format matches C git's `git log --format` output exactly
- **SC-005**: Object listing matches C git's `git rev-list --objects` exactly
