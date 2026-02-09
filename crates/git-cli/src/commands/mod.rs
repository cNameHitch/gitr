pub mod add;
pub mod am;
pub mod archive;
pub mod bisect;
pub mod blame;
pub mod branch;
pub mod bundle;
pub mod cat_file;
pub mod check_attr;
pub mod check_ignore;
pub mod check_ref_format;
pub mod checkout;
pub mod cherry_pick;
pub mod clean;
pub mod clone;
pub mod commit;
pub mod commit_graph;
pub mod config;
pub mod commit_tree;
pub mod credential;
pub mod daemon;
pub mod describe;
pub mod diff;
pub mod fast_import;
pub mod fetch;
pub mod for_each_ref;
pub mod format_patch;
pub mod fsck;
pub mod gc;
pub mod grep;
pub mod hash_object;
pub mod index_pack;
pub mod init;
pub mod log;
pub mod ls_files;
pub mod ls_tree;
pub mod merge;
pub mod mktag;
pub mod mktree;
pub mod mv;
pub mod notes;
pub mod pack_objects;
pub mod prune;
pub mod pull;
pub mod push;
pub mod rebase;
pub mod reflog;
pub mod remote;
pub mod repack;
pub mod replace;
pub mod reset;
pub mod restore;
pub mod rev_list;
pub mod rev_parse;
pub mod revert;
pub mod rm;
pub mod shortlog;
pub mod show;
pub mod show_ref;
pub mod stash;
pub mod status;
pub mod submodule;
pub mod switch;
pub mod symbolic_ref;
pub mod tag;
pub mod update_index;
pub mod update_ref;
pub mod var;
pub mod verify_commit;
pub mod verify_pack;
pub mod verify_tag;
pub mod worktree;
pub mod write_tree;

use anyhow::Result;
use clap::Subcommand;

use crate::Cli;

#[derive(Subcommand)]
pub enum Commands {
    /// Provide content or type and size information for repository objects
    CatFile(cat_file::CatFileArgs),
    /// Compute object ID and optionally create a blob from a file
    HashObject(hash_object::HashObjectArgs),
    /// Pick out and massage parameters
    RevParse(rev_parse::RevParseArgs),
    /// Update the object name stored in a ref safely
    UpdateRef(update_ref::UpdateRefArgs),
    /// Output information on each ref
    ForEachRef(for_each_ref::ForEachRefArgs),
    /// List references in a local repository
    ShowRef(show_ref::ShowRefArgs),
    /// Read, modify and delete symbolic refs
    SymbolicRef(symbolic_ref::SymbolicRefArgs),
    /// Show information about files in the index and the working tree
    LsFiles(ls_files::LsFilesArgs),
    /// List the contents of a tree object
    LsTree(ls_tree::LsTreeArgs),
    /// Register file contents in the working tree to the index
    UpdateIndex(update_index::UpdateIndexArgs),
    /// Debug gitignore / exclude files
    CheckIgnore(check_ignore::CheckIgnoreArgs),
    /// Display gitattributes information
    CheckAttr(check_attr::CheckAttrArgs),
    /// Build a tree-object from ls-tree formatted text
    Mktree(mktree::MktreeArgs),
    /// Creates a tag object with extra validation
    Mktag(mktag::MktagArgs),
    /// Record changes to the repository
    Commit(commit::CommitArgs),
    /// Create a new commit object
    CommitTree(commit_tree::CommitTreeArgs),
    /// Validate packed archive files
    VerifyPack(verify_pack::VerifyPackArgs),
    /// Ensure that a reference name is well formed
    CheckRefFormat(check_ref_format::CheckRefFormatArgs),
    /// Show a Git logical variable
    Var(var::VarArgs),
    /// Create a tree object from the current index
    WriteTree(write_tree::WriteTreeArgs),
    /// Create an empty Git repository or reinitialize an existing one
    Init(init::InitArgs),
    /// Clone a repository into a new directory
    Clone(clone::CloneArgs),
    /// Get and set repository or global options
    Config(config::ConfigArgs),
    /// Add file contents to the index
    Add(add::AddArgs),
    /// Remove files from the working tree and from the index
    Rm(rm::RmArgs),
    /// Move or rename a file, a directory, or a symlink
    Mv(mv::MvArgs),
    /// Show the working tree status
    Status(status::StatusArgs),
    /// Restore working tree files
    Restore(restore::RestoreArgs),
    /// List, create, or delete branches
    Branch(branch::BranchArgs),
    /// Switch branches
    Switch(switch::SwitchArgs),
    /// Switch branches or restore working tree files
    Checkout(checkout::CheckoutArgs),
    /// Join two or more development histories together
    Merge(merge::MergeArgs),
    /// Manage set of tracked repositories
    Remote(remote::RemoteArgs),
    /// Download objects and refs from another repository
    Fetch(fetch::FetchArgs),
    /// Fetch from and integrate with another repository or a local branch
    Pull(pull::PullArgs),
    /// Update remote refs along with associated objects
    Push(push::PushArgs),
    /// Reset current HEAD to the specified state
    Reset(reset::ResetArgs),
    /// Create, list, delete or verify a tag object
    Tag(tag::TagArgs),
    /// Stash the changes in a dirty working directory
    Stash(stash::StashArgs),
    /// Remove untracked files from the working tree
    Clean(clean::CleanArgs),
    /// Reapply commits on top of another base tip
    Rebase(rebase::RebaseArgs),
    /// Show commit logs
    Log(log::LogArgs),
    /// Lists commit objects in reverse chronological order
    RevList(rev_list::RevListArgs),
    /// Show various types of objects
    Show(show::ShowArgs),
    /// Show changes between commits, commit and working tree, etc
    Diff(diff::DiffArgs),
    /// Show what revision and author last modified each line of a file
    Blame(blame::BlameArgs),
    /// Use binary search to find the commit that introduced a bug
    Bisect(bisect::BisectArgs),
    /// Summarize git log output
    Shortlog(shortlog::ShortlogArgs),
    /// Give an object a human readable name based on an available ref
    Describe(describe::DescribeArgs),
    /// Print lines matching a pattern
    Grep(grep::GrepArgs),
    /// Apply the changes introduced by some existing commits
    CherryPick(cherry_pick::CherryPickArgs),
    /// Revert some existing commits
    Revert(revert::RevertArgs),
    /// Manage reflog information
    Reflog(reflog::ReflogArgs),
    /// Prepare patches for e-mail submission
    FormatPatch(format_patch::FormatPatchArgs),
    /// Apply a series of patches from a mailbox
    Am(am::AmArgs),

    // === Spec 018: Advanced features ===
    /// Cleanup unnecessary files and optimize the local repository
    Gc(gc::GcArgs),
    /// Pack unpacked objects in a repository
    Repack(repack::RepackArgs),
    /// Prune all unreachable objects from the object database
    Prune(prune::PruneArgs),
    /// Verifies the connectivity and validity of the objects in the database
    Fsck(fsck::FsckArgs),
    /// Create a packed archive of objects
    PackObjects(pack_objects::PackObjectsArgs),
    /// Build pack index file for an existing packed archive
    IndexPack(index_pack::IndexPackArgs),
    /// Initialize, update or inspect submodules
    Submodule(submodule::SubmoduleArgs),
    /// Manage multiple working trees
    Worktree(worktree::WorktreeArgs),
    /// Add or inspect object notes
    Notes(notes::NotesArgs),
    /// Create, list, delete refs to replace objects
    Replace(replace::ReplaceArgs),
    /// Create an archive of files from a named tree
    Archive(archive::ArchiveArgs),
    /// Verify GPG signature of commits
    VerifyCommit(verify_commit::VerifyCommitArgs),
    /// Verify GPG signature of tags
    VerifyTag(verify_tag::VerifyTagArgs),
    /// Retrieve and store user credentials
    Credential(credential::CredentialArgs),
    /// Backend for fast Git data importers
    FastImport(fast_import::FastImportArgs),
    /// Create, unpack, and manipulate bundle files
    Bundle(bundle::BundleArgs),
    /// A really simple server for Git repositories
    Daemon(daemon::DaemonArgs),

    // === Spec 022: Performance optimization ===
    /// Write and verify commit-graph files
    CommitGraph(commit_graph::CommitGraphArgs),
}

/// Open a repository, respecting --git-dir override.
pub fn open_repo(cli: &Cli) -> Result<git_repository::Repository> {
    let repo = if let Some(ref git_dir) = cli.git_dir {
        git_repository::Repository::open(git_dir)?
    } else {
        git_repository::Repository::discover(".")?
    };
    Ok(repo)
}

pub fn run(cli: Cli) -> Result<i32> {
    match &cli.command {
        Commands::CatFile(args) => cat_file::run(args, &cli),
        Commands::HashObject(args) => hash_object::run(args, &cli),
        Commands::RevParse(args) => rev_parse::run(args, &cli),
        Commands::UpdateRef(args) => update_ref::run(args, &cli),
        Commands::ForEachRef(args) => for_each_ref::run(args, &cli),
        Commands::ShowRef(args) => show_ref::run(args, &cli),
        Commands::SymbolicRef(args) => symbolic_ref::run(args, &cli),
        Commands::LsFiles(args) => ls_files::run(args, &cli),
        Commands::LsTree(args) => ls_tree::run(args, &cli),
        Commands::UpdateIndex(args) => update_index::run(args, &cli),
        Commands::CheckIgnore(args) => check_ignore::run(args, &cli),
        Commands::CheckAttr(args) => check_attr::run(args, &cli),
        Commands::Mktree(args) => mktree::run(args, &cli),
        Commands::Mktag(args) => mktag::run(args, &cli),
        Commands::Commit(args) => commit::run(args, &cli),
        Commands::CommitTree(args) => commit_tree::run(args, &cli),
        Commands::VerifyPack(args) => verify_pack::run(args, &cli),
        Commands::CheckRefFormat(args) => check_ref_format::run(args),
        Commands::Var(args) => var::run(args, &cli),
        Commands::WriteTree(args) => write_tree::run(args, &cli),
        Commands::Init(args) => init::run(args, &cli),
        Commands::Clone(args) => clone::run(args, &cli),
        Commands::Config(args) => config::run(args, &cli),
        Commands::Add(args) => add::run(args, &cli),
        Commands::Rm(args) => rm::run(args, &cli),
        Commands::Mv(args) => mv::run(args, &cli),
        Commands::Status(args) => status::run(args, &cli),
        Commands::Restore(args) => restore::run(args, &cli),
        Commands::Branch(args) => branch::run(args, &cli),
        Commands::Switch(args) => switch::run(args, &cli),
        Commands::Checkout(args) => checkout::run(args, &cli),
        Commands::Merge(args) => merge::run(args, &cli),
        Commands::Remote(args) => remote::run(args, &cli),
        Commands::Fetch(args) => fetch::run(args, &cli),
        Commands::Pull(args) => pull::run(args, &cli),
        Commands::Push(args) => push::run(args, &cli),
        Commands::Reset(args) => reset::run(args, &cli),
        Commands::Tag(args) => tag::run(args, &cli),
        Commands::Stash(args) => stash::run(args, &cli),
        Commands::Clean(args) => clean::run(args, &cli),
        Commands::Rebase(args) => rebase::run(args, &cli),
        Commands::Log(args) => log::run(args, &cli),
        Commands::RevList(args) => rev_list::run(args, &cli),
        Commands::Show(args) => show::run(args, &cli),
        Commands::Diff(args) => diff::run(args, &cli),
        Commands::Blame(args) => blame::run(args, &cli),
        Commands::Bisect(args) => bisect::run(args, &cli),
        Commands::Shortlog(args) => shortlog::run(args, &cli),
        Commands::Describe(args) => describe::run(args, &cli),
        Commands::Grep(args) => grep::run(args, &cli),
        Commands::CherryPick(args) => cherry_pick::run(args, &cli),
        Commands::Revert(args) => revert::run(args, &cli),
        Commands::Reflog(args) => reflog::run(args, &cli),
        Commands::FormatPatch(args) => format_patch::run(args, &cli),
        Commands::Am(args) => am::run(args, &cli),
        // Spec 018: Advanced features
        Commands::Gc(args) => gc::run(args, &cli),
        Commands::Repack(args) => repack::run(args, &cli),
        Commands::Prune(args) => prune::run(args, &cli),
        Commands::Fsck(args) => fsck::run(args, &cli),
        Commands::PackObjects(args) => pack_objects::run(args, &cli),
        Commands::IndexPack(args) => index_pack::run(args, &cli),
        Commands::Submodule(args) => submodule::run(args, &cli),
        Commands::Worktree(args) => worktree::run(args, &cli),
        Commands::Notes(args) => notes::run(args, &cli),
        Commands::Replace(args) => replace::run(args, &cli),
        Commands::Archive(args) => archive::run(args, &cli),
        Commands::VerifyCommit(args) => verify_commit::run(args, &cli),
        Commands::VerifyTag(args) => verify_tag::run(args, &cli),
        Commands::Credential(args) => credential::run(args, &cli),
        Commands::FastImport(args) => fast_import::run(args, &cli),
        Commands::Bundle(args) => bundle::run(args, &cli),
        Commands::Daemon(args) => daemon::run(args, &cli),
        // Spec 022: Performance optimization
        Commands::CommitGraph(args) => commit_graph::run(args, &cli),
    }
}
