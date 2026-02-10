use anyhow::Result;
use clap::Args;

use crate::Cli;
use super::{fetch, merge, rebase};

#[derive(Args)]
pub struct PullArgs {
    /// Rebase instead of merge
    #[arg(long)]
    rebase: bool,

    /// Don't rebase
    #[arg(long)]
    no_rebase: bool,

    /// Only allow fast-forward
    #[arg(long)]
    ff_only: bool,

    /// Be quiet
    #[arg(short, long)]
    quiet: bool,

    /// Be verbose
    #[arg(short, long)]
    verbose: bool,

    /// Show diffstat after merge
    #[arg(long)]
    stat: bool,

    /// Don't show diffstat
    #[arg(long)]
    no_stat: bool,

    /// Append one-line log messages to merge commit
    #[arg(long)]
    log: bool,

    /// Don't append one-line log messages
    #[arg(long)]
    no_log: bool,

    /// Squash the merge into a single set of changes
    #[arg(long)]
    squash: bool,

    /// Perform the merge and commit the result
    #[arg(long)]
    commit: bool,

    /// Perform the merge but don't commit
    #[arg(long)]
    no_commit: bool,

    /// Open editor for merge commit message
    #[arg(short, long)]
    edit: bool,

    /// Allow fast-forward merges
    #[arg(long)]
    ff: bool,

    /// Don't allow fast-forward merges
    #[arg(long)]
    no_ff: bool,

    /// Merge strategy to use
    #[arg(long)]
    strategy: Option<String>,

    /// Pass option to the merge strategy
    #[arg(short = 'X', long = "strategy-option")]
    strategy_option: Vec<String>,

    /// Fetch all remotes
    #[arg(long)]
    all: bool,

    /// Limit fetching to specified depth
    #[arg(long)]
    depth: Option<u32>,

    /// Fetch all tags
    #[arg(long)]
    tags: bool,

    /// Prune remote-tracking refs that no longer exist
    #[arg(short, long)]
    prune: bool,

    /// Automatically stash/unstash before and after
    #[arg(long)]
    autostash: bool,

    /// Remote to pull from
    remote: Option<String>,

    /// Branch to pull
    branch: Option<String>,
}

pub fn run(args: &PullArgs, cli: &Cli) -> Result<i32> {
    // Step 1: Fetch
    let remote = args.remote.as_deref().unwrap_or("origin");
    let fetch_args = fetch::FetchArgs {
        all: args.all,
        prune: args.prune,
        depth: args.depth,
        tags: args.tags,
        quiet: args.quiet,
        verbose: args.verbose,
        force: false,
        dry_run: false,
        jobs: None,
        shallow_since: None,
        shallow_exclude: None,
        unshallow: false,
        deepen: None,
        recurse_submodules: false,
        set_upstream: false,
        remote: Some(remote.to_string()),
        refspec: if let Some(ref branch) = args.branch {
            vec![format!("refs/heads/{}:refs/remotes/{}/{}", branch, remote, branch)]
        } else {
            vec![]
        },
    };

    let fetch_result = fetch::run(&fetch_args, cli)?;
    if fetch_result != 0 {
        return Ok(fetch_result);
    }

    // Step 2: Determine upstream branch to merge
    let repo = super::open_repo(cli)?;
    let current = repo.current_branch()?
        .ok_or_else(|| anyhow::anyhow!("You are not currently on a branch."))?;
    drop(repo);

    let merge_branch = if let Some(ref branch) = args.branch {
        format!("{}/{}", remote, branch)
    } else {
        format!("{}/{}", remote, current)
    };

    // Check pull.rebase config as default
    let use_rebase = if args.no_rebase {
        false
    } else if args.rebase {
        true
    } else {
        // Check pull.rebase config
        let repo = super::open_repo(cli)?;
        let pull_rebase = repo.config().get_bool("pull.rebase").unwrap_or(Some(false));
        drop(repo);
        pull_rebase.unwrap_or(false)
    };

    // Step 3: Merge (or rebase)
    if use_rebase {
        let rebase_args = rebase::RebaseArgs {
            onto: None,
            abort: false,
            r#continue: false,
            skip: false,
            interactive: false,
            quiet: args.quiet,
            verbose: args.verbose,
            signoff: false,
            force_rebase: false,
            autosquash: false,
            no_autosquash: false,
            autostash: args.autostash,
            no_autostash: false,
            update_refs: false,
            exec: None,
            root: false,
            strategy: args.strategy.clone(),
            strategy_option: args.strategy_option.clone(),
            upstream: Some(merge_branch),
        };

        return rebase::run(&rebase_args, cli);
    }

    let merge_args = merge::MergeArgs {
        no_ff: args.no_ff,
        ff_only: args.ff_only,
        squash: args.squash,
        abort: false,
        cont: false,
        no_commit: args.no_commit,
        no_edit: false,
        message: None,
        strategy: args.strategy.clone(),
        strategy_option: args.strategy_option.clone(),
        verbose: args.verbose,
        quiet: args.quiet,
        stat: args.stat,
        no_stat: args.no_stat,
        edit: args.edit,
        allow_unrelated_histories: false,
        signoff: false,
        verify: false,
        no_verify: false,
        commit: vec![merge_branch],
    };

    merge::run(&merge_args, cli)
}
