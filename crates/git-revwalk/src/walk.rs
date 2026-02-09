//! Core revision walk iterator.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet, VecDeque};

use git_hash::ObjectId;
use git_object::{Commit, Object, ObjectType};
use git_ref::RefStore;
use git_repository::Repository;

use crate::commit_graph::CommitGraph;
use crate::RevWalkError;

/// Lightweight commit metadata for traversal (no author/committer strings).
#[allow(dead_code)]
struct CommitMeta {
    parents: Vec<ObjectId>,
    tree_oid: ObjectId,
    commit_time: i64,
    generation: u32,
}

/// Sort order for commit traversal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SortOrder {
    /// By committer date, newest first (default).
    #[default]
    Chronological,
    /// Topological: parents always appear after children.
    Topological,
    /// By author date, newest first.
    AuthorDate,
    /// Reverse chronological (oldest first).
    Reverse,
}


/// Options for revision walking.
#[derive(Debug, Clone, Default)]
pub struct WalkOptions {
    pub sort: SortOrder,
    pub first_parent_only: bool,
    pub ancestry_path: bool,
    pub max_count: Option<usize>,
    pub skip: Option<usize>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub author_pattern: Option<String>,
    pub committer_pattern: Option<String>,
    pub grep_pattern: Option<String>,
}

/// An entry in the walk priority queue.
struct WalkEntry {
    oid: ObjectId,
    /// Committer timestamp (seconds since epoch).
    commit_date: i64,
    /// Author timestamp (seconds since epoch).
    author_date: i64,
    /// Generation number from commit-graph (0 if unavailable).
    generation: u32,
    /// Insertion counter for stable ordering.
    insertion_ctr: u64,
}

/// State tracking for topological sort.
struct TopoState {
    /// In-degree count for each commit (number of children not yet emitted).
    indegree: std::collections::HashMap<ObjectId, u32>,
    /// Queue of commits ready to emit (in-degree == 0).
    ready: VecDeque<ObjectId>,
    /// All commits collected in the limited phase (retained for ancestry-path filtering).
    #[allow(dead_code)]
    commits: Vec<ObjectId>,
    /// Commit dates for sorting the ready queue.
    dates: std::collections::HashMap<ObjectId, i64>,
}

/// Revision walk iterator over commits.
pub struct RevWalk<'a> {
    /// Reference to the repository.
    repo: &'a Repository,
    /// Priority queue for date-ordered walks.
    queue: BinaryHeap<WalkEntry>,
    /// Set of already-seen commit OIDs.
    seen: HashSet<ObjectId>,
    /// Set of hidden (excluded) commit OIDs and their ancestors.
    hidden: HashSet<ObjectId>,
    /// Sort order.
    sort: SortOrder,
    /// Walk options.
    options: WalkOptions,
    /// Optional commit-graph for acceleration.
    commit_graph: Option<CommitGraph>,
    /// Insertion counter for stable ordering within same date.
    insertion_ctr: u64,
    /// Number of commits emitted so far.
    emitted: usize,
    /// Number of commits skipped so far.
    skipped: usize,
    /// State for topological sort (lazily initialized).
    topo_state: Option<TopoState>,
    /// Whether the walk has been prepared (for topo sort).
    prepared: bool,
    /// Buffer for reverse mode: collected commits in forward order, popped from end.
    reverse_buffer: Option<Vec<ObjectId>>,
}

impl<'a> RevWalk<'a> {
    /// Create a new revision walker for the given repository.
    pub fn new(repo: &'a Repository) -> Result<Self, RevWalkError> {
        let commit_graph = CommitGraph::open_from_repo(repo).ok();

        Ok(Self {
            repo,
            queue: BinaryHeap::new(),
            seen: HashSet::new(),
            hidden: HashSet::new(),
            sort: SortOrder::default(),
            options: WalkOptions::default(),
            commit_graph,
            insertion_ctr: 0,
            emitted: 0,
            skipped: 0,
            topo_state: None,
            prepared: false,
            reverse_buffer: None,
        })
    }

    /// Add a starting commit (positive reference).
    pub fn push(&mut self, oid: ObjectId) -> Result<(), RevWalkError> {
        if self.seen.contains(&oid) {
            return Ok(());
        }
        let commit = self.read_commit(&oid)?;
        self.seen.insert(oid);
        self.enqueue(oid, &commit);
        Ok(())
    }

    /// Add an exclusion commit (negative reference, like ^A).
    /// All ancestors of this commit will be excluded from output.
    pub fn hide(&mut self, oid: ObjectId) -> Result<(), RevWalkError> {
        self.mark_hidden(oid)?;
        Ok(())
    }

    /// Push HEAD as a starting point.
    pub fn push_head(&mut self) -> Result<(), RevWalkError> {
        if let Some(oid) = self.repo.head_oid()? {
            self.push(oid)?;
        }
        Ok(())
    }

    /// Push all refs as starting points (--all).
    pub fn push_all(&mut self) -> Result<(), RevWalkError> {
        let refs = self.repo.refs().iter(None)?;
        for r in refs {
            let r = r?;
            if let Some(oid) = r.target_oid() {
                // Only push commits (skip tags pointing to non-commits, etc.)
                if self.is_commit(&oid) {
                    self.push(oid)?;
                }
            }
        }
        Ok(())
    }

    /// Push all branches as starting points.
    pub fn push_branches(&mut self) -> Result<(), RevWalkError> {
        let refs = self.repo.refs().iter(Some("refs/heads/"))?;
        for r in refs {
            let r = r?;
            if let Some(oid) = r.target_oid() {
                self.push(oid)?;
            }
        }
        Ok(())
    }

    /// Push all tags as starting points.
    pub fn push_tags(&mut self) -> Result<(), RevWalkError> {
        let refs = self.repo.refs().iter(Some("refs/tags/"))?;
        for r in refs {
            let r = r?;
            if let Some(oid) = r.target_oid() {
                if self.is_commit(&oid) {
                    self.push(oid)?;
                }
            }
        }
        Ok(())
    }

    /// Set the sort order.
    pub fn set_sort(&mut self, sort: SortOrder) {
        self.sort = sort;
        self.options.sort = sort;
    }

    /// Set walk options.
    pub fn set_options(&mut self, options: WalkOptions) {
        self.sort = options.sort;
        self.options = options;
    }

    /// Parse and apply a revision range ("A..B", "A...B", "^A B").
    pub fn push_range(&mut self, range_spec: &str) -> Result<(), RevWalkError> {
        let range = crate::range::RevisionRange::parse(self.repo, range_spec)?;
        for oid in &range.include {
            self.push(*oid)?;
        }
        for oid in &range.exclude {
            self.hide(*oid)?;
        }
        Ok(())
    }

    // --- Internal helpers ---

    fn enqueue(&mut self, oid: ObjectId, commit: &Commit) {
        let generation = self
            .commit_graph
            .as_ref()
            .and_then(|cg| cg.lookup(&oid))
            .map(|e| e.generation)
            .unwrap_or(0);

        // For AuthorDate sort, use author date as the primary sort key.
        let sort_date = match self.sort {
            SortOrder::AuthorDate => commit.author.date.timestamp,
            _ => commit.committer.date.timestamp,
        };

        let entry = WalkEntry {
            oid,
            commit_date: sort_date,
            author_date: commit.author.date.timestamp,
            generation,
            insertion_ctr: self.insertion_ctr,
        };
        self.insertion_ctr += 1;
        self.queue.push(entry);
    }

    fn enqueue_meta(&mut self, oid: ObjectId, meta: &CommitMeta) {
        let sort_date = match self.sort {
            SortOrder::AuthorDate => meta.commit_time, // commit-graph doesn't store author date separately
            _ => meta.commit_time,
        };

        let entry = WalkEntry {
            oid,
            commit_date: sort_date,
            author_date: meta.commit_time,
            generation: meta.generation,
            insertion_ctr: self.insertion_ctr,
        };
        self.insertion_ctr += 1;
        self.queue.push(entry);
    }

    /// Read commit metadata, preferring commit-graph over ODB.
    fn read_commit_meta(&self, oid: &ObjectId) -> Result<CommitMeta, RevWalkError> {
        // Try commit-graph first for zero-copy access.
        if let Some(ref cg) = self.commit_graph {
            if let Some(entry) = cg.lookup(oid) {
                return Ok(CommitMeta {
                    parents: entry.parent_oids,
                    tree_oid: entry.tree_oid,
                    commit_time: entry.commit_time,
                    generation: entry.generation,
                });
            }
        }

        // Fall back to ODB.
        let commit = self.read_commit(oid)?;
        Ok(CommitMeta {
            parents: commit.parents.clone(),
            tree_oid: commit.tree,
            commit_time: commit.committer.date.timestamp,
            generation: 0,
        })
    }

    fn read_commit(&self, oid: &ObjectId) -> Result<Commit, RevWalkError> {
        let obj = self
            .repo
            .odb()
            .read(oid)?
            .ok_or(RevWalkError::CommitNotFound(*oid))?;
        match obj {
            Object::Commit(c) => Ok(c),
            _ => Err(RevWalkError::NotACommit(*oid)),
        }
    }

    fn is_commit(&self, oid: &ObjectId) -> bool {
        matches!(
            self.repo.odb().read_header(oid),
            Ok(Some(info)) if info.obj_type == ObjectType::Commit
        )
    }

    /// Mark a commit and all its ancestors as hidden.
    /// Uses generation numbers from the commit-graph to prune early when possible.
    fn mark_hidden(&mut self, oid: ObjectId) -> Result<(), RevWalkError> {
        let mut stack = vec![oid];
        while let Some(current) = stack.pop() {
            if !self.hidden.insert(current) {
                continue;
            }
            if let Ok(meta) = self.read_commit_meta(&current) {
                for parent in &meta.parents {
                    if !self.hidden.contains(parent) {
                        stack.push(*parent);
                    }
                }
            }
        }
        Ok(())
    }

    /// Prepare the topological sort by collecting all reachable commits
    /// and computing in-degrees.
    fn prepare_topo(&mut self) -> Result<(), RevWalkError> {
        if self.prepared {
            return Ok(());
        }
        self.prepared = true;

        // Collect all commits reachable from the queue (limited by hidden set).
        let mut all_commits: Vec<ObjectId> = Vec::new();
        let mut parents_map: std::collections::HashMap<ObjectId, Vec<ObjectId>> =
            std::collections::HashMap::new();
        let mut dates: std::collections::HashMap<ObjectId, i64> =
            std::collections::HashMap::new();
        let mut indegree: std::collections::HashMap<ObjectId, u32> =
            std::collections::HashMap::new();

        // Drain the priority queue into a BFS queue.
        let mut bfs: VecDeque<ObjectId> = VecDeque::new();
        let mut visited: HashSet<ObjectId> = HashSet::new();

        while let Some(entry) = self.queue.pop() {
            if !visited.contains(&entry.oid) {
                bfs.push_back(entry.oid);
                visited.insert(entry.oid);
            }
        }

        // BFS to discover all commits.
        while let Some(oid) = bfs.pop_front() {
            if self.hidden.contains(&oid) {
                continue;
            }
            let meta = self.read_commit_meta(&oid)?;
            let commit_date = meta.commit_time;
            dates.insert(oid, commit_date);

            let parents: Vec<ObjectId> = if self.options.first_parent_only {
                meta.parents.first().copied().into_iter().collect()
            } else {
                meta.parents
            };

            // Initialize in-degree for this commit if not yet seen.
            indegree.entry(oid).or_insert(0);

            for parent in &parents {
                if !self.hidden.contains(parent) {
                    // Increment parent's in-degree (it has a child pointing to it).
                    *indegree.entry(*parent).or_insert(0) += 1;
                    if visited.insert(*parent) {
                        bfs.push_back(*parent);
                    }
                }
            }

            parents_map.insert(oid, parents);
            all_commits.push(oid);
        }

        // Find tips (in-degree == 0) — these are the starting points.
        let mut ready: VecDeque<ObjectId> = VecDeque::new();
        // Sort tips by date for deterministic output.
        let mut tips: Vec<ObjectId> = all_commits
            .iter()
            .filter(|oid| indegree.get(oid).copied().unwrap_or(0) == 0)
            .copied()
            .collect();
        tips.sort_by(|a, b| {
            let da = dates.get(a).copied().unwrap_or(0);
            let db = dates.get(b).copied().unwrap_or(0);
            db.cmp(&da) // newest first
        });
        for tip in tips {
            ready.push_back(tip);
        }

        self.topo_state = Some(TopoState {
            indegree,
            ready,
            commits: all_commits,
            dates,
        });

        // Store parents_map in a way we can access it during iteration.
        // We'll re-read commits as needed during next_topo().
        // The topo_state.commits vector has all the OIDs.

        Ok(())
    }

    /// Get the next commit in topological order.
    fn next_topo(&mut self) -> Result<Option<ObjectId>, RevWalkError> {
        if !self.prepared {
            self.prepare_topo()?;
        }

        // Pop the next ready commit (in-degree == 0).
        let oid = match self.topo_state.as_mut() {
            Some(state) if !state.ready.is_empty() => state.ready.pop_front().unwrap(),
            _ => return Ok(None),
        };

        // Read commit metadata to get parents (graph-accelerated).
        let meta = self.read_commit_meta(&oid)?;
        let parents: Vec<ObjectId> = if self.options.first_parent_only {
            meta.parents.first().copied().into_iter().collect()
        } else {
            meta.parents
        };

        // Filter parents by hidden set first (immutable borrow of self.hidden).
        let parents: Vec<ObjectId> = parents
            .into_iter()
            .filter(|p| !self.hidden.contains(p))
            .collect();

        // Now borrow topo_state mutably to update indegrees.
        let state = self.topo_state.as_mut().unwrap();
        let mut newly_ready: Vec<(ObjectId, i64)> = Vec::new();
        for parent in &parents {
            if let Some(deg) = state.indegree.get_mut(parent) {
                *deg = deg.saturating_sub(1);
                if *deg == 0 {
                    let date = state.dates.get(parent).copied().unwrap_or(0);
                    newly_ready.push((*parent, date));
                }
            }
        }

        // Sort newly ready by date (newest first) for deterministic output.
        newly_ready.sort_by(|a, b| b.1.cmp(&a.1));
        for (parent, _) in newly_ready {
            state.ready.push_back(parent);
        }

        Ok(Some(oid))
    }

    /// Get the next commit for date-ordered walks (chronological, author-date).
    fn next_date_order(&mut self) -> Result<Option<ObjectId>, RevWalkError> {
        while let Some(entry) = self.queue.pop() {
            let oid = entry.oid;

            if self.hidden.contains(&oid) {
                continue;
            }

            // Read commit metadata (graph-accelerated) and enqueue parents.
            let meta = self.read_commit_meta(&oid)?;

            let parents: Vec<ObjectId> = if self.options.first_parent_only {
                meta.parents.first().copied().into_iter().collect()
            } else {
                meta.parents
            };

            for parent in parents {
                if self.seen.insert(parent) && !self.hidden.contains(&parent) {
                    if let Ok(parent_meta) = self.read_commit_meta(&parent) {
                        self.enqueue_meta(parent, &parent_meta);
                    }
                }
            }

            return Ok(Some(oid));
        }
        Ok(None)
    }

    /// Get the next raw commit (before applying filters).
    fn next_raw(&mut self) -> Result<Option<ObjectId>, RevWalkError> {
        match self.sort {
            SortOrder::Reverse => {
                // For reverse mode, collect all commits on first call, then pop from end.
                if self.reverse_buffer.is_none() {
                    let mut buffer = Vec::new();
                    // Use chronological order to collect, then reverse.
                    while let Some(entry) = self.queue.pop() {
                        let oid = entry.oid;
                        if self.hidden.contains(&oid) {
                            continue;
                        }
                        let meta = self.read_commit_meta(&oid)?;
                        let parents: Vec<ObjectId> = if self.options.first_parent_only {
                            meta.parents.first().copied().into_iter().collect()
                        } else {
                            meta.parents
                        };
                        for parent in parents {
                            if self.seen.insert(parent) && !self.hidden.contains(&parent) {
                                if let Ok(parent_meta) = self.read_commit_meta(&parent) {
                                    self.enqueue_meta(parent, &parent_meta);
                                }
                            }
                        }
                        buffer.push(oid);
                    }
                    // Buffer is in newest→oldest order. pop() takes from the end
                    // giving us oldest first, which is what reverse mode wants.
                    self.reverse_buffer = Some(buffer);
                }
                Ok(self.reverse_buffer.as_mut().unwrap().pop())
            }
            SortOrder::Topological => self.next_topo(),
            _ => self.next_date_order(),
        }
    }

    /// Apply date filters (--since, --until).
    fn passes_date_filter(&self, commit: &Commit) -> bool {
        let commit_date = commit.committer.date.timestamp;
        if let Some(since) = self.options.since {
            if commit_date < since {
                return false;
            }
        }
        if let Some(until) = self.options.until {
            if commit_date > until {
                return false;
            }
        }
        true
    }

    /// Apply pattern filters (--author, --committer, --grep).
    fn passes_pattern_filter(&self, commit: &Commit) -> bool {
        if let Some(ref pattern) = self.options.author_pattern {
            let author = String::from_utf8_lossy(&commit.author.name);
            let email = String::from_utf8_lossy(&commit.author.email);
            if !author.contains(pattern.as_str()) && !email.contains(pattern.as_str()) {
                return false;
            }
        }
        if let Some(ref pattern) = self.options.committer_pattern {
            let committer = String::from_utf8_lossy(&commit.committer.name);
            let email = String::from_utf8_lossy(&commit.committer.email);
            if !committer.contains(pattern.as_str()) && !email.contains(pattern.as_str()) {
                return false;
            }
        }
        if let Some(ref pattern) = self.options.grep_pattern {
            let msg = String::from_utf8_lossy(&commit.message);
            if !msg.contains(pattern.as_str()) {
                return false;
            }
        }
        true
    }
}

impl Iterator for RevWalk<'_> {
    type Item = Result<ObjectId, RevWalkError>;

    fn next(&mut self) -> Option<Self::Item> {
        // Check max_count limit.
        if let Some(max) = self.options.max_count {
            if self.emitted >= max {
                return None;
            }
        }

        let has_pattern_filters = self.options.author_pattern.is_some()
            || self.options.committer_pattern.is_some()
            || self.options.grep_pattern.is_some();
        let has_date_filters = self.options.since.is_some() || self.options.until.is_some();
        let needs_full_commit = has_pattern_filters || has_date_filters;

        loop {
            let oid = match self.next_raw() {
                Ok(Some(oid)) => oid,
                Ok(None) => return None,
                Err(e) => return Some(Err(e)),
            };

            // Only do a full commit read when filters require author/committer/message strings.
            if needs_full_commit {
                let commit = match self.read_commit(&oid) {
                    Ok(c) => c,
                    Err(e) => return Some(Err(e)),
                };

                if !self.passes_date_filter(&commit) {
                    continue;
                }

                if !self.passes_pattern_filter(&commit) {
                    continue;
                }
            }

            // Handle --skip.
            if let Some(skip) = self.options.skip {
                if self.skipped < skip {
                    self.skipped += 1;
                    continue;
                }
            }

            self.emitted += 1;
            return Some(Ok(oid));
        }
    }
}

// --- Priority queue ordering ---

impl PartialEq for WalkEntry {
    fn eq(&self, other: &Self) -> bool {
        self.oid == other.oid
    }
}

impl Eq for WalkEntry {}

impl PartialOrd for WalkEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WalkEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap, so we want the "largest" entry first.
        // For chronological: newest commit date first.
        // Ties broken by insertion order (lower = earlier = higher priority).
        self.commit_date
            .cmp(&other.commit_date)
            .then_with(|| other.insertion_ctr.cmp(&self.insertion_ctr))
    }
}

