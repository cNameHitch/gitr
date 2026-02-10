use std::collections::HashMap;
use std::io::{self, Write};

use anyhow::Result;
use clap::Args;
use git_hash::ObjectId;
use git_object::Object;
use git_ref::RefStore;
use git_revwalk::RevWalk;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct NameRevArgs {
    /// Only use tags to name the commits
    #[arg(long)]
    tags: bool,

    /// Only use refs matching this pattern
    #[arg(long = "refs", value_name = "pattern")]
    refs_pattern: Option<String>,

    /// Die with error code if a name cannot be found
    #[arg(long)]
    no_undefined: bool,

    /// Show abbreviated OID as fallback when no name is found
    #[arg(long)]
    always: bool,

    /// Print only the name (without the OID)
    #[arg(long)]
    name_only: bool,

    /// Commits to name
    commits: Vec<String>,
}

/// A ref candidate for naming a commit.
struct RefCandidate {
    name: String,
    oid: ObjectId,
}

pub fn run(args: &NameRevArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Resolve each input commit
    let mut targets = Vec::new();
    for rev in &args.commits {
        let oid = git_revwalk::resolve_revision(&repo, rev)?;
        targets.push(oid);
    }

    if targets.is_empty() {
        // Default to HEAD
        if let Some(oid) = repo.head_oid()? {
            targets.push(oid);
        } else {
            anyhow::bail!("HEAD does not point to a valid object");
        }
    }

    // Collect all relevant refs
    let candidates = collect_ref_candidates(&repo, args)?;

    // Build a map from OID -> (ref name, distance) for each target
    for target in &targets {
        let name = find_name_for_commit(&repo, target, &candidates)?;

        match name {
            Some(ref desc) => {
                if args.name_only {
                    writeln!(out, "{}", desc)?;
                } else {
                    writeln!(out, "{} {}", target.to_hex(), desc)?;
                }
            }
            None => {
                if args.no_undefined {
                    anyhow::bail!("cannot describe '{}'", target.to_hex());
                } else if args.always {
                    let hex = target.to_hex();
                    let abbrev = &hex[..7.min(hex.len())];
                    if args.name_only {
                        writeln!(out, "{}", abbrev)?;
                    } else {
                        writeln!(out, "{} {}", target.to_hex(), abbrev)?;
                    }
                } else if args.name_only {
                    writeln!(out, "undefined")?;
                } else {
                    writeln!(out, "{} undefined", target.to_hex())?;
                }
            }
        }
    }

    Ok(0)
}

/// Collect ref candidates based on the --tags and --refs flags.
fn collect_ref_candidates(
    repo: &git_repository::Repository,
    args: &NameRevArgs,
) -> Result<Vec<RefCandidate>> {
    let mut candidates = Vec::new();

    let prefixes: Vec<&str> = if args.tags {
        vec!["refs/tags/"]
    } else if let Some(ref pattern) = args.refs_pattern {
        // Use the pattern as a prefix filter
        vec![pattern.as_str()]
    } else {
        vec!["refs/tags/", "refs/heads/", "refs/remotes/"]
    };

    for prefix in prefixes {
        if let Ok(refs) = repo.refs().iter(Some(prefix)) {
            for r in refs {
                let r = r?;
                let full_name = r.name().as_str().to_string();
                let short_name = shorten_ref_name(&full_name);

                // Peel tags to their target commit
                let oid = if let Ok(peeled) = r.peel_to_oid(repo.refs()) {
                    peeled
                } else if let Some(oid) = r.target_oid() {
                    // Try to peel annotated tags
                    peel_to_commit(repo, &oid).unwrap_or(oid)
                } else {
                    continue;
                };

                candidates.push(RefCandidate {
                    name: short_name,
                    oid,
                });
            }
        }
    }

    Ok(candidates)
}

/// Peel an OID to a commit (follow annotated tags).
fn peel_to_commit(
    repo: &git_repository::Repository,
    oid: &ObjectId,
) -> Result<ObjectId> {
    let obj = repo
        .odb()
        .read(oid)?
        .ok_or_else(|| anyhow::anyhow!("object not found: {}", oid))?;
    match obj {
        Object::Tag(tag) => peel_to_commit(repo, &tag.target),
        Object::Commit(_) => Ok(*oid),
        _ => anyhow::bail!("object {} is not a commit or tag", oid),
    }
}

/// Shorten a full ref name to its display form.
fn shorten_ref_name(full: &str) -> String {
    if let Some(s) = full.strip_prefix("refs/tags/") {
        format!("tags/{}", s)
    } else if let Some(s) = full.strip_prefix("refs/heads/") {
        s.to_string()
    } else if let Some(s) = full.strip_prefix("refs/remotes/") {
        format!("remotes/{}", s)
    } else {
        full.to_string()
    }
}

/// Find the best name for a commit by walking backwards from each ref
/// and checking distance.
fn find_name_for_commit(
    repo: &git_repository::Repository,
    target: &ObjectId,
    candidates: &[RefCandidate],
) -> Result<Option<String>> {
    // First check for exact matches
    for cand in candidates {
        if cand.oid == *target {
            return Ok(Some(cand.name.clone()));
        }
    }

    // Walk from each candidate ref tip and find the one closest to the target
    let mut best: Option<(String, u32)> = None;

    for cand in candidates {
        if let Some(distance) = walk_distance(repo, &cand.oid, target)? {
            match best {
                Some((_, best_dist)) if distance < best_dist => {
                    best = Some((cand.name.clone(), distance));
                }
                None => {
                    best = Some((cand.name.clone(), distance));
                }
                _ => {}
            }
        }
    }

    Ok(best.map(|(name, distance)| {
        if distance == 0 {
            name
        } else {
            format!("{}~{}", name, distance)
        }
    }))
}

/// Walk from `from` towards `target`, returning the distance if `target`
/// is an ancestor of `from`.
fn walk_distance(
    repo: &git_repository::Repository,
    from: &ObjectId,
    target: &ObjectId,
) -> Result<Option<u32>> {
    if from == target {
        return Ok(Some(0));
    }

    // Collect ancestors of `from` via first-parent walk, recording distances
    let mut walker = RevWalk::new(repo)?;
    walker.push(*from)?;

    let mut distances: HashMap<ObjectId, u32> = HashMap::new();
    distances.insert(*from, 0);

    // Limit the walk to avoid excessive traversal
    let max_walk = 10_000u32;
    let mut count = 0u32;

    for oid_result in walker {
        let oid = oid_result?;
        let dist = distances.get(&oid).copied().unwrap_or(0);

        if oid == *target {
            return Ok(Some(dist));
        }

        count += 1;
        if count > max_walk {
            break;
        }

        // Read commit parents to propagate distances
        if let Some(Object::Commit(commit)) = repo.odb().read(&oid)? {
            for parent in &commit.parents {
                distances.entry(*parent).or_insert(dist + 1);
            }
        }
    }

    Ok(None)
}
