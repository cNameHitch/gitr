use std::io::{self, Write};

use anyhow::Result;
use bstr::ByteSlice;
use clap::Args;
use git_hash::ObjectId;
use git_object::Object;
use git_revwalk::RevWalk;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct RangeDiffArgs {
    /// Disable color output
    #[arg(long)]
    no_color: bool,

    /// Percentage threshold for commit matching (default 60)
    #[arg(long = "creation-factor", default_value = "60")]
    creation_factor: u32,

    /// Range arguments: either range1..range2 or base rev1 rev2
    args: Vec<String>,
}

/// A commit entry with its metadata for range comparison.
struct RangeCommit {
    oid: ObjectId,
    subject: String,
    patch_id: String,
}

pub fn run(args: &RangeDiffArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Parse range arguments into two commit ranges
    let (range1, range2) = parse_ranges(&args.args)?;

    // Collect commits in each range
    let commits1 = collect_range_commits(&repo, &range1.0, &range1.1)?;
    let commits2 = collect_range_commits(&repo, &range2.0, &range2.1)?;

    // Match commits between ranges by patch similarity
    let pairs = match_commits(&commits1, &commits2);

    // Output the range diff
    for pair in &pairs {
        match pair {
            MatchedPair::Equal(idx1, idx2) => {
                let c1 = &commits1[*idx1];
                let c2 = &commits2[*idx2];
                writeln!(
                    out,
                    "{}:  {} = {}:  {} {}",
                    idx1 + 1,
                    abbrev_oid(&c1.oid),
                    idx2 + 1,
                    abbrev_oid(&c2.oid),
                    c1.subject
                )?;
            }
            MatchedPair::Modified(idx1, idx2) => {
                let c1 = &commits1[*idx1];
                let c2 = &commits2[*idx2];
                writeln!(
                    out,
                    "{}:  {} ! {}:  {} {}",
                    idx1 + 1,
                    abbrev_oid(&c1.oid),
                    idx2 + 1,
                    abbrev_oid(&c2.oid),
                    c2.subject
                )?;
            }
            MatchedPair::Deleted(idx1) => {
                let c1 = &commits1[*idx1];
                writeln!(
                    out,
                    "{}:  {} < -:  ------- {}",
                    idx1 + 1,
                    abbrev_oid(&c1.oid),
                    c1.subject
                )?;
            }
            MatchedPair::Added(idx2) => {
                let c2 = &commits2[*idx2];
                writeln!(
                    out,
                    "-:  ------- > {}:  {} {}",
                    idx2 + 1,
                    abbrev_oid(&c2.oid),
                    c2.subject
                )?;
            }
        }
    }

    Ok(0)
}

/// Abbreviate an OID to 7 hex characters.
fn abbrev_oid(oid: &ObjectId) -> String {
    let hex = oid.to_hex();
    hex[..7.min(hex.len())].to_string()
}

/// Parse range arguments into two (base, tip) pairs.
///
/// Supports three forms:
///   1. `A..B  C..D`  - two explicit ranges
///   2. `A...B`       - symmetric difference (base is merge-base)
///   3. `base rev1 rev2` - base, old tip, new tip
fn parse_ranges(args: &[String]) -> Result<((String, String), (String, String))> {
    if args.len() == 1 {
        // A...B form (symmetric)
        let arg = &args[0];
        if let Some(pos) = arg.find("...") {
            let a = &arg[..pos];
            let b = &arg[pos + 3..];
            if a.is_empty() || b.is_empty() {
                anyhow::bail!("invalid symmetric range: {}", arg);
            }
            // For symmetric range, both ranges share a common base (merge-base)
            // range1 = merge-base..A, range2 = merge-base..B
            // We'll use a placeholder and let the caller handle merge-base resolution
            return Ok((
                (b.to_string(), a.to_string()),
                (a.to_string(), b.to_string()),
            ));
        }
        anyhow::bail!(
            "range-diff requires two ranges: either 'A..B C..D', 'A...B', or 'base rev1 rev2'"
        );
    }

    if args.len() == 2 {
        // Two explicit A..B ranges
        let range1 = parse_dotdot_range(&args[0])?;
        let range2 = parse_dotdot_range(&args[1])?;
        return Ok((range1, range2));
    }

    if args.len() == 3 {
        // base rev1 rev2
        let base = &args[0];
        let rev1 = &args[1];
        let rev2 = &args[2];
        return Ok((
            (base.clone(), rev1.clone()),
            (base.clone(), rev2.clone()),
        ));
    }

    anyhow::bail!(
        "range-diff requires two ranges: either 'A..B C..D', 'A...B', or 'base rev1 rev2'"
    );
}

/// Parse an `A..B` range string into (A, B).
fn parse_dotdot_range(s: &str) -> Result<(String, String)> {
    if let Some(pos) = s.find("..") {
        // Make sure it's not "..." (symmetric)
        if s[pos..].starts_with("...") {
            anyhow::bail!("expected A..B range, got symmetric range: {}", s);
        }
        let a = &s[..pos];
        let b = &s[pos + 2..];
        if a.is_empty() || b.is_empty() {
            anyhow::bail!("invalid range: {}", s);
        }
        Ok((a.to_string(), b.to_string()))
    } else {
        anyhow::bail!("expected A..B range, got: {}", s);
    }
}

/// Collect commits in a range (base..tip) with their metadata.
fn collect_range_commits(
    repo: &git_repository::Repository,
    base: &str,
    tip: &str,
) -> Result<Vec<RangeCommit>> {
    let base_oid = git_revwalk::resolve_revision(repo, base)?;
    let tip_oid = git_revwalk::resolve_revision(repo, tip)?;

    let mut walker = RevWalk::new(repo)?;
    walker.push(tip_oid)?;
    walker.hide(base_oid)?;

    let mut commits = Vec::new();
    for oid_result in walker {
        let oid = oid_result?;
        let obj = repo
            .odb()
            .read(&oid)?
            .ok_or_else(|| anyhow::anyhow!("object not found: {}", oid))?;

        if let Object::Commit(commit) = obj {
            let subject = commit.summary().to_str_lossy().to_string();
            let patch_id = compute_simple_patch_id(&commit, &oid);
            commits.push(RangeCommit {
                oid,
                subject,
                patch_id,
            });
        }
    }

    // Reverse so commits are in chronological order (oldest first)
    commits.reverse();
    Ok(commits)
}

/// Compute a simplified patch ID for matching commits across ranges.
/// Uses the commit message and tree OID as a fingerprint.
fn compute_simple_patch_id(commit: &git_object::Commit, _oid: &ObjectId) -> String {
    let mut hasher = git_hash::hasher::Hasher::new(git_hash::HashAlgorithm::Sha1);
    hasher.update(&commit.message);
    hasher.update(commit.tree.as_bytes());
    match hasher.finalize() {
        Ok(oid) => oid.to_hex(),
        Err(_) => String::new(),
    }
}

/// Result of matching commits between two ranges.
enum MatchedPair {
    /// Commits match exactly (same patch ID)
    Equal(usize, usize),
    /// Commits match by subject but patches differ
    Modified(usize, usize),
    /// Commit only in range1 (deleted)
    Deleted(usize),
    /// Commit only in range2 (added)
    Added(usize),
}

/// Match commits between two ranges.
///
/// Uses a greedy approach: first match by patch ID (exact), then by subject
/// (modified), then mark remaining as deleted/added.
fn match_commits(range1: &[RangeCommit], range2: &[RangeCommit]) -> Vec<MatchedPair> {
    let mut matched1 = vec![false; range1.len()];
    let mut matched2 = vec![false; range2.len()];
    let mut pairs = Vec::new();

    // Pass 1: exact patch ID match
    for (i, c1) in range1.iter().enumerate() {
        for (j, c2) in range2.iter().enumerate() {
            if !matched2[j] && c1.patch_id == c2.patch_id {
                pairs.push(MatchedPair::Equal(i, j));
                matched1[i] = true;
                matched2[j] = true;
                break;
            }
        }
    }

    // Pass 2: subject match (modified)
    for (i, c1) in range1.iter().enumerate() {
        if matched1[i] {
            continue;
        }
        for (j, c2) in range2.iter().enumerate() {
            if !matched2[j] && c1.subject == c2.subject {
                pairs.push(MatchedPair::Modified(i, j));
                matched1[i] = true;
                matched2[j] = true;
                break;
            }
        }
    }

    // Pass 3: unmatched in range1 -> deleted
    for (i, _) in range1.iter().enumerate() {
        if !matched1[i] {
            pairs.push(MatchedPair::Deleted(i));
        }
    }

    // Pass 4: unmatched in range2 -> added
    for (j, _) in range2.iter().enumerate() {
        if !matched2[j] {
            pairs.push(MatchedPair::Added(j));
        }
    }

    // Sort by position to maintain order
    pairs.sort_by_key(|p| match p {
        MatchedPair::Equal(i, j) | MatchedPair::Modified(i, j) => (*i, *j),
        MatchedPair::Deleted(i) => (*i, usize::MAX),
        MatchedPair::Added(j) => (usize::MAX, *j),
    });

    pairs
}
