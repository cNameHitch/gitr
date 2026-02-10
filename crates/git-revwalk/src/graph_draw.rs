//! ASCII graph drawing for `git log --graph`.
//!
//! Draws commit history as ASCII art alongside commit messages,
//! tracking active branch columns.

use git_hash::ObjectId;

/// Draws ASCII graph lines for commit history.
pub struct GraphDrawer {
    /// Active columns: each contains the OID of the commit being tracked.
    columns: Vec<Option<ObjectId>>,
}

impl GraphDrawer {
    /// Create a new graph drawer.
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
        }
    }

    /// Draw graph lines for a commit.
    ///
    /// Returns the graph prefix lines:
    /// - First line: the commit marker line (with `*` for the commit)
    /// - Additional lines: merge edges if needed
    pub fn draw_commit(&mut self, oid: &ObjectId, parents: &[ObjectId]) -> Vec<String> {
        let mut lines = Vec::new();

        // Find or allocate a column for this commit.
        let col = self.find_column(oid);

        // Build the commit line.
        let mut commit_line = String::new();
        for (i, slot) in self.columns.iter().enumerate() {
            if i == col {
                commit_line.push('*');
            } else if slot.is_some() {
                commit_line.push('|');
            } else {
                commit_line.push(' ');
            }
            if i + 1 < self.columns.len() {
                commit_line.push(' ');
            }
        }
        lines.push(commit_line);

        // Handle parents.
        match parents.len() {
            0 => {
                // Root commit: close this column.
                if col < self.columns.len() {
                    self.columns[col] = None;
                }
                self.compact_columns();
            }
            1 => {
                // Single parent: this column continues with the parent.
                if col < self.columns.len() {
                    self.columns[col] = Some(parents[0]);
                }

                // Only draw continuation line if there are multiple active columns
                // (on linear history, no extra | lines between commits)
                let active_count = self.columns.iter().filter(|s| s.is_some()).count();
                if active_count > 1 {
                    let mut cont_line = String::new();
                    for (i, slot) in self.columns.iter().enumerate() {
                        if slot.is_some() {
                            cont_line.push('|');
                        } else {
                            cont_line.push(' ');
                        }
                        if i + 1 < self.columns.len() {
                            cont_line.push(' ');
                        }
                    }
                    lines.push(cont_line);
                }
            }
            _ => {
                // Merge commit: first parent stays in this column,
                // other parents get new or existing columns.
                if col < self.columns.len() {
                    self.columns[col] = Some(parents[0]);
                }

                // Draw merge edges.
                let mut merge_line = String::new();
                let mut edge_targets = Vec::new();

                for (_pi, parent) in parents.iter().enumerate().skip(1) {
                    // Find or allocate a column for this parent.
                    let pcol = self.find_or_create_column(parent);
                    edge_targets.push((col, pcol));
                }

                // Build the merge line showing connections.
                for (i, slot) in self.columns.iter().enumerate() {
                    let is_edge_target = edge_targets.iter().any(|(_, t)| *t == i);
                    let is_edge_source = i == col;

                    if is_edge_source || is_edge_target || slot.is_some() {
                        merge_line.push('|');
                    } else {
                        // Check if an edge crosses this column.
                        let crossed = edge_targets.iter().any(|(s, t)| {
                            let lo = (*s).min(*t);
                            let hi = (*s).max(*t);
                            i > lo && i < hi
                        });
                        if crossed {
                            merge_line.push('-');
                        } else {
                            merge_line.push(' ');
                        }
                    }
                    if i + 1 < self.columns.len() {
                        let crossed = edge_targets.iter().any(|(s, t)| {
                            let lo = (*s).min(*t);
                            let hi = (*s).max(*t);
                            i >= lo && i < hi
                        });
                        if crossed {
                            merge_line.push('-');
                        } else {
                            merge_line.push(' ');
                        }
                    }
                }
                if !merge_line.trim().is_empty() {
                    lines.push(merge_line);
                }

                // Draw continuation line.
                let mut cont_line = String::new();
                for (i, slot) in self.columns.iter().enumerate() {
                    if slot.is_some() {
                        cont_line.push('|');
                    } else {
                        cont_line.push(' ');
                    }
                    if i + 1 < self.columns.len() {
                        cont_line.push(' ');
                    }
                }
                lines.push(cont_line);
            }
        }

        lines
    }

    /// Find the column index for a commit OID, or allocate a new one.
    fn find_column(&mut self, oid: &ObjectId) -> usize {
        // Look for an existing column tracking this OID.
        for (i, slot) in self.columns.iter().enumerate() {
            if slot.as_ref() == Some(oid) {
                return i;
            }
        }

        // Look for an empty slot.
        for (i, slot) in self.columns.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(*oid);
                return i;
            }
        }

        // Create a new column.
        self.columns.push(Some(*oid));
        self.columns.len() - 1
    }

    /// Find or create a column for a parent OID.
    fn find_or_create_column(&mut self, oid: &ObjectId) -> usize {
        // Look for an existing column tracking this OID.
        for (i, slot) in self.columns.iter().enumerate() {
            if slot.as_ref() == Some(oid) {
                return i;
            }
        }

        // Look for an empty slot.
        for (i, slot) in self.columns.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(*oid);
                return i;
            }
        }

        // Create a new column.
        self.columns.push(Some(*oid));
        self.columns.len() - 1
    }

    /// Remove trailing empty columns.
    fn compact_columns(&mut self) {
        while self.columns.last() == Some(&None) {
            self.columns.pop();
        }
    }
}

impl Default for GraphDrawer {
    fn default() -> Self {
        Self::new()
    }
}
