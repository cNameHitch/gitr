//! `gitr commit-graph` — write and verify commit-graph files.

use anyhow::Result;
use clap::{Args, Subcommand};

use git_hash::HashAlgorithm;
use git_object::Object;
use git_revwalk::{CommitGraph, CommitGraphWriter, RevWalk};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct CommitGraphArgs {
    #[command(subcommand)]
    action: CommitGraphAction,
}

#[derive(Subcommand)]
enum CommitGraphAction {
    /// Write a commit-graph file from reachable commits
    Write,
    /// Verify the commit-graph file integrity
    Verify,
}

pub fn run(args: &CommitGraphArgs, cli: &Cli) -> Result<i32> {
    match &args.action {
        CommitGraphAction::Write => run_write(cli),
        CommitGraphAction::Verify => run_verify(cli),
    }
}

fn run_write(cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let objects_dir = repo.odb().objects_dir().to_path_buf();
    let graph_path = objects_dir.join("info").join("commit-graph");

    // Walk all reachable commits.
    let mut walk = RevWalk::new(&repo)?;
    walk.push_all()?;

    let hash_algo = HashAlgorithm::Sha1;
    let mut writer = CommitGraphWriter::new(hash_algo);
    let mut count = 0u32;

    for result in &mut walk {
        let oid = result?;
        // Read full commit to get tree and parents.
        let obj = repo.odb().read(&oid)?;
        if let Some(Object::Commit(commit)) = obj {
            let tree_oid = commit.tree;
            let parents = commit.parents;
            let commit_time = commit.committer.date.timestamp;
            writer.add_commit(oid, tree_oid, parents, commit_time);
            count += 1;
        }
    }

    if count == 0 {
        eprintln!("No commits found.");
        return Ok(0);
    }

    writer.write(&graph_path)?;

    Ok(0)
}

fn run_verify(cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let graph = CommitGraph::open_from_repo(&repo)?;
    graph.verify()?;
    Ok(0)
}
