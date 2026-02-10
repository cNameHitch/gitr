//! Multi-commit operation sequencer.
//!
//! Manages cherry-pick sequences, reverts, and rebases with support for
//! interruption (on conflict) and continuation/abort/skip.
//!
//! State is persisted to `.git/sequencer/` for compatibility with C git.

use std::fs;
use std::path::PathBuf;

use git_hash::ObjectId;
use git_object::Object;
use git_ref::RefStore;
use git_repository::Repository;

use crate::cherry_pick;
use crate::revert;
use crate::{MergeError, MergeOptions, MergeResult};

/// Type of multi-commit operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequencerOperation {
    CherryPick,
    Revert,
    Rebase,
}

/// Action to perform on a single commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SequencerAction {
    /// Apply the commit (cherry-pick).
    Pick,
    /// Revert the commit.
    Revert,
    /// Apply then reword the commit message.
    Reword,
    /// Apply then stop for editing.
    Edit,
    /// Squash into previous commit.
    Squash,
    /// Fixup (squash without changing message).
    Fixup,
    /// Drop the commit entirely.
    Drop,
    /// Execute a shell command.
    Exec(String),
    /// Stop for manual intervention.
    Break,
    /// Set a label for later reference.
    Label(String),
    /// Reset HEAD to a label.
    Reset(String),
    /// Merge a branch.
    Merge,
}

/// A single entry in the sequencer todo list.
#[derive(Debug, Clone)]
pub struct SequencerEntry {
    /// The commit to operate on.
    pub commit: ObjectId,
    /// The action to perform.
    pub action: SequencerAction,
}

/// A single entry in an interactive rebase todo list.
#[derive(Debug, Clone)]
pub struct RebaseTodoEntry {
    /// The action to perform.
    pub action: SequencerAction,
    /// The commit ID (None for exec, break, label, reset).
    pub commit_id: Option<ObjectId>,
    /// Short hash for display (7 chars).
    pub short_hash: String,
    /// Commit subject line.
    pub subject: String,
    /// The original line from the todo file (for comments/empty lines).
    pub original_line: Option<String>,
}

/// State for an interactive rebase in progress.
#[derive(Debug, Clone)]
pub struct RebaseState {
    /// Remaining todo entries.
    pub todo_entries: Vec<RebaseTodoEntry>,
    /// Number of completed entries.
    pub done_count: usize,
    /// Original HEAD before rebase started.
    pub original_head: ObjectId,
    /// The commit to rebase onto.
    pub onto: ObjectId,
    /// Branch name being rebased (e.g., "refs/heads/main").
    pub head_name: String,
    /// Accumulated fixup/squash message.
    pub current_fixup_message: Option<String>,
    /// Whether autosquash was enabled.
    pub autosquash: bool,
}

/// Result of a sequencer step.
#[derive(Debug)]
pub enum SequencerResult {
    /// All operations completed successfully.
    Complete,
    /// Operation paused due to conflict or edit request.
    Paused {
        /// The current entry that caused the pause.
        current_index: usize,
        /// The merge result (may contain conflicts).
        result: MergeResult,
    },
}

/// The multi-commit sequencer.
#[derive(Debug)]
pub struct Sequencer {
    /// Path to the repository's .git directory.
    git_dir: PathBuf,
    /// Original HEAD when the operation started.
    original_head: ObjectId,
    /// List of operations to perform.
    todo: Vec<SequencerEntry>,
    /// Index of the next operation to perform.
    current: usize,
    /// Type of operation.
    operation: SequencerOperation,
    /// Merge options.
    options: MergeOptions,
}

impl Sequencer {
    /// Create a new sequencer for the given operation.
    pub fn new(
        repo: &Repository,
        operation: SequencerOperation,
        options: MergeOptions,
    ) -> Result<Self, MergeError> {
        let head = repo
            .head_oid()?
            .ok_or(MergeError::NoMergeBase)?;

        // Check if a sequencer is already in progress.
        let seq_dir = repo.git_dir().join("sequencer");
        if seq_dir.exists() {
            return Err(MergeError::SequencerInProgress);
        }

        Ok(Self {
            git_dir: repo.git_dir().to_path_buf(),
            original_head: head,
            todo: Vec::new(),
            current: 0,
            operation,
            options,
        })
    }

    /// Add an entry to the todo list.
    pub fn add(&mut self, commit: ObjectId, action: SequencerAction) {
        self.todo.push(SequencerEntry { commit, action });
    }

    /// Execute the sequencer, processing entries until completion or conflict.
    pub fn execute(
        &mut self,
        repo: &mut Repository,
    ) -> Result<SequencerResult, MergeError> {
        self.save()?;

        while self.current < self.todo.len() {
            let entry = &self.todo[self.current];

            let result = match &entry.action {
                SequencerAction::Pick => {
                    cherry_pick::cherry_pick(repo, &entry.commit, &self.options)?
                }
                SequencerAction::Revert => {
                    revert::revert(repo, &entry.commit, &self.options)?
                }
                SequencerAction::Edit => {
                    let result = cherry_pick::cherry_pick(repo, &entry.commit, &self.options)?;
                    // Always pause for editing, even if clean.
                    self.save()?;
                    return Ok(SequencerResult::Paused {
                        current_index: self.current,
                        result,
                    });
                }
                SequencerAction::Break => {
                    self.save()?;
                    return Ok(SequencerResult::Paused {
                        current_index: self.current,
                        result: MergeResult::clean(
                            repo.head_oid()?.ok_or(MergeError::NoMergeBase)?,
                        ),
                    });
                }
                SequencerAction::Exec(cmd) => {
                    // Execute shell command (in a real implementation).
                    let _ = cmd;
                    self.current += 1;
                    continue;
                }
                SequencerAction::Reword => {
                    let result = cherry_pick::cherry_pick(repo, &entry.commit, &self.options)?;
                    // Pause for message editing (handled by the caller).
                    self.save()?;
                    return Ok(SequencerResult::Paused {
                        current_index: self.current,
                        result,
                    });
                }
                SequencerAction::Squash | SequencerAction::Fixup => {
                    // For squash/fixup, apply the commit but mark for
                    // message combination (handled by the caller).
                    cherry_pick::cherry_pick(repo, &entry.commit, &self.options)?
                }
                SequencerAction::Drop => {
                    // Skip this commit entirely.
                    self.current += 1;
                    continue;
                }
                SequencerAction::Label(_) | SequencerAction::Reset(_) | SequencerAction::Merge => {
                    // Handled by interactive rebase caller.
                    self.current += 1;
                    continue;
                }
            };

            if !result.is_clean {
                // Conflict — pause.
                self.save()?;
                return Ok(SequencerResult::Paused {
                    current_index: self.current,
                    result,
                });
            }

            self.current += 1;
        }

        // All done — clean up sequencer state.
        self.cleanup()?;
        Ok(SequencerResult::Complete)
    }

    /// Continue the operation after conflict resolution.
    pub fn continue_operation(
        &mut self,
        repo: &mut Repository,
    ) -> Result<SequencerResult, MergeError> {
        // Skip the entry that caused the conflict (user has resolved it).
        self.current += 1;
        self.execute(repo)
    }

    /// Abort the operation, restoring HEAD and index to the original state.
    pub fn abort(&self, repo: &mut Repository) -> Result<(), MergeError> {
        // Check if sequencer state actually exists
        let seq_dir = self.git_dir.join("sequencer");
        if !seq_dir.exists() {
            return Err(MergeError::InvalidPatch(
                "no cherry-pick or revert in progress".into(),
            ));
        }

        // 1. Read the original_head commit to get its tree
        let obj = repo
            .odb()
            .read(&self.original_head)?
            .ok_or(MergeError::ObjectNotFound(self.original_head))?;
        let tree_oid = match obj {
            Object::Commit(c) => c.tree,
            _ => {
                return Err(MergeError::UnexpectedObjectType {
                    oid: self.original_head,
                    expected: "commit",
                    actual: obj.object_type().to_string(),
                })
            }
        };

        // 2. Update HEAD ref to original_head
        let head_ref =
            git_ref::RefName::new(bstr::BString::from("HEAD")).map_err(|e| {
                MergeError::InvalidPatch(format!("invalid ref name: {}", e))
            })?;
        let resolved = repo.refs().resolve(&head_ref).map_err(|e| {
            MergeError::InvalidPatch(format!("failed to resolve HEAD: {}", e))
        })?;
        match resolved {
            Some(git_ref::Reference::Symbolic { target, .. }) => {
                repo.refs().write_ref(&target, &self.original_head).map_err(|e| {
                    MergeError::InvalidPatch(format!("failed to update ref: {}", e))
                })?;
            }
            _ => {
                repo.refs().write_ref(&head_ref, &self.original_head).map_err(|e| {
                    MergeError::InvalidPatch(format!("failed to update HEAD: {}", e))
                })?;
            }
        }

        // 3. Restore the index from original_head's tree (read-tree)
        let mut new_index = git_index::Index::new();
        Self::build_index_from_tree(repo.odb(), &tree_oid, &bstr::BString::from(""), &mut new_index)?;
        repo.set_index(new_index);
        repo.write_index().map_err(|e| {
            MergeError::InvalidPatch(format!("failed to write index: {}", e))
        })?;

        // 4. Cleanup sequencer state
        self.cleanup()?;

        Ok(())
    }

    /// Recursively build an index from a tree object.
    fn build_index_from_tree(
        odb: &git_odb::ObjectDatabase,
        tree_oid: &ObjectId,
        prefix: &bstr::BString,
        index: &mut git_index::Index,
    ) -> Result<(), MergeError> {
        let obj = odb
            .read(tree_oid)?
            .ok_or(MergeError::ObjectNotFound(*tree_oid))?;
        let tree = match obj {
            Object::Tree(t) => t,
            _ => {
                return Err(MergeError::UnexpectedObjectType {
                    oid: *tree_oid,
                    expected: "tree",
                    actual: obj.object_type().to_string(),
                })
            }
        };

        for entry in &tree.entries {
            let path = if prefix.is_empty() {
                entry.name.clone()
            } else {
                let mut p = prefix.clone();
                p.push(b'/');
                p.extend_from_slice(&entry.name);
                p
            };

            if entry.mode.is_tree() {
                Self::build_index_from_tree(odb, &entry.oid, &path, index)?;
            } else {
                index.add(git_index::IndexEntry {
                    path,
                    oid: entry.oid,
                    mode: entry.mode,
                    stage: git_index::Stage::Normal,
                    stat: git_index::StatData::default(),
                    flags: git_index::EntryFlags::default(),
                });
            }
        }
        Ok(())
    }

    /// Skip the current entry and continue.
    pub fn skip(
        &mut self,
        repo: &mut Repository,
    ) -> Result<SequencerResult, MergeError> {
        self.current += 1;
        self.execute(repo)
    }

    /// Save the sequencer state to `.git/sequencer/`.
    pub fn save(&self) -> Result<(), MergeError> {
        let seq_dir = self.git_dir.join("sequencer");
        fs::create_dir_all(&seq_dir)?;

        // Write head file.
        fs::write(
            seq_dir.join("head"),
            self.original_head.to_hex(),
        )?;

        // Write todo file.
        let mut todo_content = String::new();
        for (i, entry) in self.todo.iter().enumerate() {
            let prefix = if i < self.current { "done" } else { "todo" };
            let action_str = match &entry.action {
                SequencerAction::Pick => "pick",
                SequencerAction::Revert => "revert",
                SequencerAction::Reword => "reword",
                SequencerAction::Edit => "edit",
                SequencerAction::Squash => "squash",
                SequencerAction::Fixup => "fixup",
                SequencerAction::Drop => "drop",
                SequencerAction::Exec(_) => "exec",
                SequencerAction::Break => "break",
                SequencerAction::Label(_) => "label",
                SequencerAction::Reset(_) => "reset",
                SequencerAction::Merge => "merge",
            };
            todo_content.push_str(&format!(
                "{} {} {}\n",
                prefix,
                action_str,
                entry.commit.to_hex()
            ));
        }
        fs::write(seq_dir.join("todo"), &todo_content)?;

        // Write opts file.
        let operation_str = match self.operation {
            SequencerOperation::CherryPick => "cherry-pick",
            SequencerOperation::Revert => "revert",
            SequencerOperation::Rebase => "rebase",
        };
        fs::write(seq_dir.join("opts"), operation_str)?;

        Ok(())
    }

    /// Load sequencer state from `.git/sequencer/`.
    pub fn load(repo: &Repository) -> Result<Option<Self>, MergeError> {
        let seq_dir = repo.git_dir().join("sequencer");
        if !seq_dir.exists() {
            return Ok(None);
        }

        let head_hex = fs::read_to_string(seq_dir.join("head"))?;
        let original_head = ObjectId::from_hex(head_hex.trim())
            .map_err(|_| MergeError::InvalidPatch("invalid head in sequencer state".into()))?;

        let operation_str = fs::read_to_string(seq_dir.join("opts"))?;
        let operation = match operation_str.trim() {
            "cherry-pick" => SequencerOperation::CherryPick,
            "revert" => SequencerOperation::Revert,
            "rebase" => SequencerOperation::Rebase,
            other => {
                return Err(MergeError::InvalidPatch(format!(
                    "unknown sequencer operation: {}",
                    other
                )))
            }
        };

        let todo_content = fs::read_to_string(seq_dir.join("todo"))?;
        let mut todo = Vec::new();
        let mut done_count = 0;

        for line in todo_content.lines() {
            let parts: Vec<&str> = line.splitn(3, ' ').collect();
            if parts.len() < 3 {
                continue;
            }

            let is_done = parts[0] == "done";
            let action = match parts[1] {
                "pick" | "p" => SequencerAction::Pick,
                "revert" => SequencerAction::Revert,
                "reword" | "r" => SequencerAction::Reword,
                "edit" | "e" => SequencerAction::Edit,
                "squash" | "s" => SequencerAction::Squash,
                "fixup" | "f" => SequencerAction::Fixup,
                "drop" | "d" => SequencerAction::Drop,
                "exec" | "x" => SequencerAction::Exec(parts[2].to_string()),
                "break" | "b" => SequencerAction::Break,
                "label" | "l" => SequencerAction::Label(parts[2].to_string()),
                "reset" | "t" => SequencerAction::Reset(parts[2].to_string()),
                "merge" | "m" => SequencerAction::Merge,
                _ => continue,
            };

            let commit = ObjectId::from_hex(parts[2].trim()).map_err(|_| {
                MergeError::InvalidPatch(format!("invalid commit OID in todo: {}", parts[2]))
            })?;

            todo.push(SequencerEntry { commit, action });

            if is_done {
                done_count += 1;
            }
        }

        Ok(Some(Self {
            git_dir: repo.git_dir().to_path_buf(),
            original_head,
            todo,
            current: done_count,
            operation,
            options: MergeOptions::default(),
        }))
    }

    /// Remove sequencer state files.
    fn cleanup(&self) -> Result<(), MergeError> {
        let seq_dir = self.git_dir.join("sequencer");
        if seq_dir.exists() {
            fs::remove_dir_all(&seq_dir)?;
        }
        Ok(())
    }

    /// Get the operation type.
    pub fn operation(&self) -> SequencerOperation {
        self.operation
    }

    /// Get the current index.
    pub fn current(&self) -> usize {
        self.current
    }

    /// Get the total number of entries.
    pub fn total(&self) -> usize {
        self.todo.len()
    }

    /// Get the original HEAD.
    pub fn original_head(&self) -> &ObjectId {
        &self.original_head
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequencer_action_equality() {
        assert_eq!(SequencerAction::Pick, SequencerAction::Pick);
        assert_ne!(SequencerAction::Pick, SequencerAction::Revert);
    }

    #[test]
    fn sequencer_operation_equality() {
        assert_eq!(SequencerOperation::CherryPick, SequencerOperation::CherryPick);
        assert_ne!(SequencerOperation::CherryPick, SequencerOperation::Revert);
    }
}
