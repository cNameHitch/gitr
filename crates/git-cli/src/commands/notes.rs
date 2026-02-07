use std::io::{self, Write};

use anyhow::{bail, Result};
use bstr::BString;
use clap::{Args, Subcommand};
use git_hash::ObjectId;
use git_object::{Object, ObjectType, Tree, TreeEntry};
use git_ref::{RefName, RefStore};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct NotesArgs {
    /// Use notes ref <ref> (default: refs/notes/commits)
    #[arg(long = "ref")]
    notes_ref: Option<String>,

    #[command(subcommand)]
    command: Option<NotesSubcommand>,
}

#[derive(Subcommand)]
pub enum NotesSubcommand {
    /// List notes
    List {
        /// Object to show notes for
        object: Option<String>,
    },

    /// Add a note to an object
    Add {
        /// Note message
        #[arg(short, long)]
        message: Option<String>,

        /// Read message from file
        #[arg(short = 'F', long)]
        file: Option<String>,

        /// Force overwrite existing note
        #[arg(short, long)]
        force: bool,

        /// Allow an empty note
        #[arg(long)]
        allow_empty: bool,

        /// Object to annotate (defaults to HEAD)
        object: Option<String>,
    },

    /// Show the note for an object
    Show {
        /// Object to show note for (defaults to HEAD)
        object: Option<String>,
    },

    /// Remove the note for an object
    Remove {
        /// Objects to remove notes from
        objects: Vec<String>,
    },

    /// Copy a note from one object to another
    Copy {
        /// Force overwrite
        #[arg(short, long)]
        force: bool,

        /// Source object
        from: String,

        /// Destination object
        to: String,
    },

    /// Append to a note
    Append {
        /// Note message
        #[arg(short, long)]
        message: Option<String>,

        /// Allow an empty note
        #[arg(long)]
        allow_empty: bool,

        /// Object to annotate (defaults to HEAD)
        object: Option<String>,
    },

    /// Remove notes for non-existing/unreachable objects
    Prune {
        /// Dry run
        #[arg(short = 'n', long)]
        dry_run: bool,

        /// Verbose
        #[arg(short, long)]
        verbose: bool,
    },

    /// Print the notes ref
    GetRef,
}

pub fn run(args: &NotesArgs, cli: &Cli) -> Result<i32> {
    let notes_ref_name = args
        .notes_ref
        .as_deref()
        .unwrap_or("refs/notes/commits");

    match &args.command {
        None | Some(NotesSubcommand::List { .. }) => {
            let object = match &args.command {
                Some(NotesSubcommand::List { object }) => object.as_deref(),
                _ => None,
            };
            notes_list(cli, notes_ref_name, object)
        }
        Some(NotesSubcommand::Add {
            message,
            file,
            force,
            allow_empty,
            object,
        }) => notes_add(
            cli,
            notes_ref_name,
            message.as_deref(),
            file.as_deref(),
            *force,
            *allow_empty,
            object.as_deref(),
        ),
        Some(NotesSubcommand::Show { object }) => {
            notes_show(cli, notes_ref_name, object.as_deref())
        }
        Some(NotesSubcommand::Remove { objects }) => {
            notes_remove(cli, notes_ref_name, objects)
        }
        Some(NotesSubcommand::Copy { force, from, to }) => {
            notes_copy(cli, notes_ref_name, *force, from, to)
        }
        Some(NotesSubcommand::Append {
            message,
            allow_empty,
            object,
        }) => notes_append(
            cli,
            notes_ref_name,
            message.as_deref(),
            *allow_empty,
            object.as_deref(),
        ),
        Some(NotesSubcommand::Prune { dry_run, verbose }) => {
            notes_prune(cli, notes_ref_name, *dry_run, *verbose)
        }
        Some(NotesSubcommand::GetRef) => {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            writeln!(out, "{}", notes_ref_name)?;
            Ok(0)
        }
    }
}

/// Get the notes tree OID from the notes ref.
fn get_notes_tree(
    repo: &git_repository::Repository,
    notes_ref_name: &str,
) -> Result<Option<ObjectId>> {
    let refname = RefName::new(BString::from(notes_ref_name))?;
    let oid = match repo.refs().resolve_to_oid(&refname)? {
        Some(oid) => oid,
        None => return Ok(None),
    };

    // The notes ref points to a commit; the tree of that commit is the notes tree
    let obj = repo
        .odb()
        .read(&oid)?
        .ok_or_else(|| anyhow::anyhow!("notes commit not found"))?;

    match obj {
        Object::Commit(commit) => Ok(Some(commit.tree)),
        _ => bail!("notes ref does not point to a commit"),
    }
}

/// Look up a note for the given object OID in the notes tree.
fn find_note(
    repo: &git_repository::Repository,
    notes_tree_oid: &ObjectId,
    target_oid: &ObjectId,
) -> Result<Option<ObjectId>> {
    let tree_obj = repo
        .odb()
        .read(notes_tree_oid)?
        .ok_or_else(|| anyhow::anyhow!("notes tree not found"))?;

    let tree = match tree_obj {
        Object::Tree(t) => t,
        _ => bail!("expected tree object"),
    };

    // Notes are stored with the target OID as the entry name
    let target_hex = target_oid.to_hex();

    // First try direct match (flat notes)
    for entry in &tree.entries {
        let name = String::from_utf8_lossy(entry.name.as_ref());
        if name == target_hex {
            return Ok(Some(entry.oid));
        }
    }

    // Try fanout (first 2 chars as directory)
    let fanout = &target_hex[..2];
    let rest = &target_hex[2..];
    for entry in &tree.entries {
        let name = String::from_utf8_lossy(entry.name.as_ref());
        if name == fanout {
            // This is a subtree, look inside it
            if let Some(Object::Tree(subtree)) = repo.odb().read(&entry.oid)? {
                for sub_entry in &subtree.entries {
                    let sub_name = String::from_utf8_lossy(sub_entry.name.as_ref());
                    if sub_name == rest {
                        return Ok(Some(sub_entry.oid));
                    }
                }
            }
        }
    }

    Ok(None)
}

fn notes_list(cli: &Cli, notes_ref_name: &str, object: Option<&str>) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let notes_tree_oid = match get_notes_tree(&repo, notes_ref_name)? {
        Some(oid) => oid,
        None => return Ok(0),
    };

    if let Some(spec) = object {
        // Show note for specific object
        let target_oid = git_revwalk::resolve_revision(&repo, spec)?;
        if let Some(note_oid) = find_note(&repo, &notes_tree_oid, &target_oid)? {
            writeln!(out, "{} {}", note_oid.to_hex(), target_oid.to_hex())?;
        }
    } else {
        // List all notes
        let tree_obj = repo.odb().read(&notes_tree_oid)?
            .ok_or_else(|| anyhow::anyhow!("notes tree not found"))?;

        if let Object::Tree(tree) = tree_obj {
            for entry in &tree.entries {
                let name = String::from_utf8_lossy(entry.name.as_ref());
                writeln!(out, "{} {}", entry.oid.to_hex(), name)?;
            }
        }
    }

    Ok(0)
}

fn notes_add(
    cli: &Cli,
    notes_ref_name: &str,
    message: Option<&str>,
    file: Option<&str>,
    force: bool,
    allow_empty: bool,
    object: Option<&str>,
) -> Result<i32> {
    let repo = open_repo(cli)?;

    let target_oid = if let Some(spec) = object {
        git_revwalk::resolve_revision(&repo, spec)?
    } else {
        repo.head_oid()?
            .ok_or_else(|| anyhow::anyhow!("HEAD is not valid"))?
    };

    // Get note content
    let content = if let Some(msg) = message {
        msg.to_string()
    } else if let Some(f) = file {
        std::fs::read_to_string(f)?
    } else {
        bail!("no note message given, use -m or -F");
    };

    if content.is_empty() && !allow_empty {
        bail!("Refusing to add empty note. Use --allow-empty to override.");
    }

    // Check if note already exists
    if let Some(notes_tree_oid) = get_notes_tree(&repo, notes_ref_name)? {
        if find_note(&repo, &notes_tree_oid, &target_oid)?.is_some() && !force {
            bail!(
                "Cannot add notes. Found existing notes for object {}. Use -f to overwrite.",
                target_oid.to_hex()
            );
        }
    }

    // Write the note content as a blob
    let note_oid = repo.odb().write_raw(ObjectType::Blob, content.as_bytes())?;

    // Update the notes tree
    update_note(&repo, notes_ref_name, &target_oid, Some(note_oid))?;

    Ok(0)
}

fn notes_show(cli: &Cli, notes_ref_name: &str, object: Option<&str>) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let target_oid = if let Some(spec) = object {
        git_revwalk::resolve_revision(&repo, spec)?
    } else {
        repo.head_oid()?
            .ok_or_else(|| anyhow::anyhow!("HEAD is not valid"))?
    };

    let notes_tree_oid = get_notes_tree(&repo, notes_ref_name)?
        .ok_or_else(|| anyhow::anyhow!("no note found for object {}", target_oid.to_hex()))?;

    let note_oid = find_note(&repo, &notes_tree_oid, &target_oid)?
        .ok_or_else(|| anyhow::anyhow!("no note found for object {}", target_oid.to_hex()))?;

    let note_obj = repo
        .odb()
        .read(&note_oid)?
        .ok_or_else(|| anyhow::anyhow!("note blob not found"))?;

    if let Object::Blob(blob) = note_obj {
        out.write_all(&blob.data)?;
        if !blob.data.ends_with(b"\n") {
            writeln!(out)?;
        }
    }

    Ok(0)
}

fn notes_remove(cli: &Cli, notes_ref_name: &str, objects: &[String]) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    for spec in objects {
        let target_oid = git_revwalk::resolve_revision(&repo, spec)?;
        update_note(&repo, notes_ref_name, &target_oid, None)?;
        writeln!(
            err,
            "Removing note for object {}",
            target_oid.to_hex()
        )?;
    }

    Ok(0)
}

fn notes_copy(
    cli: &Cli,
    notes_ref_name: &str,
    force: bool,
    from: &str,
    to: &str,
) -> Result<i32> {
    let repo = open_repo(cli)?;

    let from_oid = git_revwalk::resolve_revision(&repo, from)?;
    let to_oid = git_revwalk::resolve_revision(&repo, to)?;

    let notes_tree_oid = get_notes_tree(&repo, notes_ref_name)?
        .ok_or_else(|| anyhow::anyhow!("no notes found"))?;

    let note_oid = find_note(&repo, &notes_tree_oid, &from_oid)?
        .ok_or_else(|| anyhow::anyhow!("no note found for object {}", from_oid.to_hex()))?;

    // Check if target already has a note
    if find_note(&repo, &notes_tree_oid, &to_oid)?.is_some() && !force {
        bail!("Cannot copy notes. Found existing notes for object {}.", to_oid.to_hex());
    }

    update_note(&repo, notes_ref_name, &to_oid, Some(note_oid))?;

    Ok(0)
}

fn notes_append(
    cli: &Cli,
    notes_ref_name: &str,
    message: Option<&str>,
    allow_empty: bool,
    object: Option<&str>,
) -> Result<i32> {
    let repo = open_repo(cli)?;

    let target_oid = if let Some(spec) = object {
        git_revwalk::resolve_revision(&repo, spec)?
    } else {
        repo.head_oid()?
            .ok_or_else(|| anyhow::anyhow!("HEAD is not valid"))?
    };

    let append_text = message.unwrap_or("");
    if append_text.is_empty() && !allow_empty {
        bail!("Refusing to add empty note. Use --allow-empty to override.");
    }

    // Get existing note content
    let mut content = String::new();
    if let Some(notes_tree_oid) = get_notes_tree(&repo, notes_ref_name)? {
        if let Some(note_oid) = find_note(&repo, &notes_tree_oid, &target_oid)? {
            if let Some(Object::Blob(blob)) = repo.odb().read(&note_oid)? {
                content = String::from_utf8_lossy(&blob.data).to_string();
                if !content.ends_with('\n') {
                    content.push('\n');
                }
            }
        }
    }

    if !content.is_empty() {
        content.push('\n'); // blank line separator between old and new content
    }
    content.push_str(append_text);
    if !content.ends_with('\n') {
        content.push('\n');
    }

    let note_oid = repo.odb().write_raw(ObjectType::Blob, content.as_bytes())?;
    update_note(&repo, notes_ref_name, &target_oid, Some(note_oid))?;

    Ok(0)
}

fn notes_prune(cli: &Cli, notes_ref_name: &str, dry_run: bool, verbose: bool) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    let notes_tree_oid = match get_notes_tree(&repo, notes_ref_name)? {
        Some(oid) => oid,
        None => return Ok(0),
    };

    let tree_obj = repo.odb().read(&notes_tree_oid)?
        .ok_or_else(|| anyhow::anyhow!("notes tree not found"))?;

    if let Object::Tree(tree) = tree_obj {
        for entry in &tree.entries {
            let name = String::from_utf8_lossy(entry.name.as_ref());
            if let Ok(target_oid) = ObjectId::from_hex(&name) {
                if !repo.odb().contains(&target_oid) {
                    if verbose || dry_run {
                        writeln!(
                            err,
                            "{}note for {} ({})",
                            if dry_run { "Would remove " } else { "Removing " },
                            name,
                            entry.oid.to_hex()
                        )?;
                    }
                    if !dry_run {
                        update_note(&repo, notes_ref_name, &target_oid, None)?;
                    }
                }
            }
        }
    }

    Ok(0)
}

/// Update (add/remove) a note in the notes tree and update the notes ref.
fn update_note(
    repo: &git_repository::Repository,
    notes_ref_name: &str,
    target_oid: &ObjectId,
    note_oid: Option<ObjectId>,
) -> Result<()> {
    use git_object::FileMode;

    let target_hex = target_oid.to_hex();

    // Get existing notes tree entries
    let mut entries: Vec<TreeEntry> = Vec::new();
    if let Some(notes_tree_oid) = get_notes_tree(repo, notes_ref_name)? {
        if let Some(Object::Tree(tree)) = repo.odb().read(&notes_tree_oid)? {
            entries = tree.entries;
        }
    }

    // Remove existing entry for this target
    entries.retain(|e| {
        let name = String::from_utf8_lossy(e.name.as_ref());
        name != target_hex
    });

    // Add new entry if note_oid is Some
    if let Some(oid) = note_oid {
        entries.push(TreeEntry {
            mode: FileMode::Regular,
            name: BString::from(target_hex),
            oid,
        });
    }

    // Sort entries by name
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    // Write new tree
    let new_tree = Tree { entries };
    let tree_oid = repo.odb().write(&Object::Tree(new_tree))?;

    // Create a notes commit
    let parent = {
        let refname = RefName::new(BString::from(notes_ref_name))?;
        repo.refs().resolve_to_oid(&refname)?
    };

    let author = super::tag::build_signature(repo)?;
    let commit = git_object::Commit {
        tree: tree_oid,
        parents: parent.into_iter().collect(),
        author: author.clone(),
        committer: author,
        encoding: None,
        gpgsig: None,
        extra_headers: Vec::new(),
        message: BString::from("Notes added by 'git notes'\n"),
    };

    let commit_oid = repo.odb().write(&Object::Commit(commit))?;

    // Update the notes ref
    let refname = RefName::new(BString::from(notes_ref_name))?;
    repo.refs().write_ref(&refname, &commit_oid)?;

    Ok(())
}
