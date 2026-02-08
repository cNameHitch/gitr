//! Performance comparison benchmarks: gitr (Rust) vs git (C).
//!
//! Uses Criterion for statistical analysis. Each benchmark group spawns
//! both `git` and `gitr` as subprocesses against pre-built test repos
//! at three sizes (small, medium, large).
//!
//! Run with: `cargo bench -p git-cli --bench perf_compare`
//! HTML reports are generated in `target/criterion/`.

mod perf_helpers;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use perf_helpers::{
    dirty_worktree, git_stdout, run_git, run_gitr, setup_repo, stage_changes, RepoSize,
};
use std::sync::OnceLock;
use tempfile::TempDir;

// ──────────────────────────── Repo Cache ────────────────────────────

/// A cached test repo that persists for the lifetime of the process.
struct CachedRepo {
    _dir: TempDir,
    path: std::path::PathBuf,
}

impl CachedRepo {
    fn new(size: RepoSize) -> Self {
        let dir = TempDir::new().expect("failed to create temp dir");
        let path = dir.path().to_path_buf();
        setup_repo(&path, size);
        CachedRepo { _dir: dir, path }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

// One repo per size, built once per process.
static SMALL_REPO: OnceLock<CachedRepo> = OnceLock::new();
static MEDIUM_REPO: OnceLock<CachedRepo> = OnceLock::new();
static LARGE_REPO: OnceLock<CachedRepo> = OnceLock::new();

fn get_repo(size: RepoSize) -> &'static CachedRepo {
    match size {
        RepoSize::Small => SMALL_REPO.get_or_init(|| CachedRepo::new(RepoSize::Small)),
        RepoSize::Medium => MEDIUM_REPO.get_or_init(|| CachedRepo::new(RepoSize::Medium)),
        RepoSize::Large => LARGE_REPO.get_or_init(|| CachedRepo::new(RepoSize::Large)),
    }
}

/// Repos with dirty worktrees (separate so we don't pollute the clean repos).
static SMALL_DIRTY: OnceLock<CachedRepo> = OnceLock::new();
static MEDIUM_DIRTY: OnceLock<CachedRepo> = OnceLock::new();
static LARGE_DIRTY: OnceLock<CachedRepo> = OnceLock::new();

fn get_dirty_repo(size: RepoSize) -> &'static CachedRepo {
    match size {
        RepoSize::Small => SMALL_DIRTY.get_or_init(|| {
            let r = CachedRepo::new(RepoSize::Small);
            dirty_worktree(r.path(), 3);
            r
        }),
        RepoSize::Medium => MEDIUM_DIRTY.get_or_init(|| {
            let r = CachedRepo::new(RepoSize::Medium);
            dirty_worktree(r.path(), 20);
            r
        }),
        RepoSize::Large => LARGE_DIRTY.get_or_init(|| {
            let r = CachedRepo::new(RepoSize::Large);
            dirty_worktree(r.path(), 50);
            r
        }),
    }
}

/// Repos with staged changes for `diff --cached`.
static SMALL_STAGED: OnceLock<CachedRepo> = OnceLock::new();
static MEDIUM_STAGED: OnceLock<CachedRepo> = OnceLock::new();
static LARGE_STAGED: OnceLock<CachedRepo> = OnceLock::new();

fn get_staged_repo(size: RepoSize) -> &'static CachedRepo {
    match size {
        RepoSize::Small => SMALL_STAGED.get_or_init(|| {
            let r = CachedRepo::new(RepoSize::Small);
            stage_changes(r.path(), 3);
            r
        }),
        RepoSize::Medium => MEDIUM_STAGED.get_or_init(|| {
            let r = CachedRepo::new(RepoSize::Medium);
            stage_changes(r.path(), 20);
            r
        }),
        RepoSize::Large => LARGE_STAGED.get_or_init(|| {
            let r = CachedRepo::new(RepoSize::Large);
            stage_changes(r.path(), 50);
            r
        }),
    }
}

const ALL_SIZES: [RepoSize; 3] = [RepoSize::Small, RepoSize::Medium, RepoSize::Large];

// ──────────────────────────── Benchmark: init ────────────────────────────

fn bench_init(c: &mut Criterion) {
    let mut group = c.benchmark_group("init");
    // init always runs against a fresh empty dir — no size variants needed,
    // but we measure it once per tool for consistency.
    group.bench_function("git", |b| {
        b.iter(|| {
            let dir = TempDir::new().unwrap();
            run_git(dir.path(), &["init", "-b", "main"]);
        })
    });
    group.bench_function("gitr", |b| {
        b.iter(|| {
            let dir = TempDir::new().unwrap();
            run_gitr(dir.path(), &["init", "-b", "main"]);
        })
    });
    group.finish();
}

// ──────────────────────────── Benchmark: hash-object ────────────────────────────

fn bench_hash_object(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash-object");
    for size in ALL_SIZES {
        let repo = get_repo(size);
        let file = "commit_file_1.txt";
        group.bench_with_input(BenchmarkId::new("git", size.label()), &(), |b, _| {
            b.iter(|| run_git(repo.path(), &["hash-object", file]))
        });
        group.bench_with_input(BenchmarkId::new("gitr", size.label()), &(), |b, _| {
            b.iter(|| run_gitr(repo.path(), &["hash-object", file]))
        });
    }
    group.finish();
}

// ──────────────────────────── Benchmark: cat-file ────────────────────────────

fn bench_cat_file(c: &mut Criterion) {
    let mut group = c.benchmark_group("cat-file");
    for size in ALL_SIZES {
        let repo = get_repo(size);
        // Resolve HEAD to an OID for cat-file
        let head_oid = git_stdout(repo.path(), &["rev-parse", "HEAD"]);

        group.bench_with_input(
            BenchmarkId::new("git/-p", size.label()),
            &(),
            |b, _| b.iter(|| run_git(repo.path(), &["cat-file", "-p", &head_oid])),
        );
        group.bench_with_input(
            BenchmarkId::new("gitr/-p", size.label()),
            &(),
            |b, _| b.iter(|| run_gitr(repo.path(), &["cat-file", "-p", &head_oid])),
        );
        group.bench_with_input(
            BenchmarkId::new("git/-t", size.label()),
            &(),
            |b, _| b.iter(|| run_git(repo.path(), &["cat-file", "-t", &head_oid])),
        );
        group.bench_with_input(
            BenchmarkId::new("gitr/-t", size.label()),
            &(),
            |b, _| b.iter(|| run_gitr(repo.path(), &["cat-file", "-t", &head_oid])),
        );
    }
    group.finish();
}

// ──────────────────────────── Benchmark: status ────────────────────────────

fn bench_status(c: &mut Criterion) {
    let mut group = c.benchmark_group("status");
    for size in ALL_SIZES {
        let repo = get_dirty_repo(size);
        group.bench_with_input(BenchmarkId::new("git", size.label()), &(), |b, _| {
            b.iter(|| run_git(repo.path(), &["status"]))
        });
        group.bench_with_input(BenchmarkId::new("gitr", size.label()), &(), |b, _| {
            b.iter(|| run_gitr(repo.path(), &["status"]))
        });
    }
    group.finish();
}

// ──────────────────────────── Benchmark: ls-files ────────────────────────────

fn bench_ls_files(c: &mut Criterion) {
    let mut group = c.benchmark_group("ls-files");
    for size in ALL_SIZES {
        let repo = get_repo(size);
        group.bench_with_input(BenchmarkId::new("git", size.label()), &(), |b, _| {
            b.iter(|| run_git(repo.path(), &["ls-files"]))
        });
        group.bench_with_input(BenchmarkId::new("gitr", size.label()), &(), |b, _| {
            b.iter(|| run_gitr(repo.path(), &["ls-files"]))
        });
    }
    group.finish();
}

// ──────────────────────────── Benchmark: log ────────────────────────────

fn bench_log(c: &mut Criterion) {
    let mut group = c.benchmark_group("log");
    for size in ALL_SIZES {
        let repo = get_repo(size);
        group.bench_with_input(BenchmarkId::new("git", size.label()), &(), |b, _| {
            b.iter(|| run_git(repo.path(), &["log"]))
        });
        group.bench_with_input(BenchmarkId::new("gitr", size.label()), &(), |b, _| {
            b.iter(|| run_gitr(repo.path(), &["log"]))
        });
    }
    group.finish();
}

fn bench_log_oneline(c: &mut Criterion) {
    let mut group = c.benchmark_group("log_oneline");
    for size in ALL_SIZES {
        let repo = get_repo(size);
        group.bench_with_input(BenchmarkId::new("git", size.label()), &(), |b, _| {
            b.iter(|| run_git(repo.path(), &["log", "--oneline"]))
        });
        group.bench_with_input(BenchmarkId::new("gitr", size.label()), &(), |b, _| {
            b.iter(|| run_gitr(repo.path(), &["log", "--oneline"]))
        });
    }
    group.finish();
}

// ──────────────────────────── Benchmark: rev-list ────────────────────────────

fn bench_rev_list(c: &mut Criterion) {
    let mut group = c.benchmark_group("rev-list");
    for size in ALL_SIZES {
        let repo = get_repo(size);
        group.bench_with_input(BenchmarkId::new("git", size.label()), &(), |b, _| {
            b.iter(|| run_git(repo.path(), &["rev-list", "--count", "HEAD"]))
        });
        group.bench_with_input(BenchmarkId::new("gitr", size.label()), &(), |b, _| {
            b.iter(|| run_gitr(repo.path(), &["rev-list", "--count", "HEAD"]))
        });
    }
    group.finish();
}

// ──────────────────────────── Benchmark: diff (working tree) ────────────────────────────

fn bench_diff(c: &mut Criterion) {
    let mut group = c.benchmark_group("diff");
    for size in ALL_SIZES {
        let repo = get_dirty_repo(size);
        group.bench_with_input(BenchmarkId::new("git", size.label()), &(), |b, _| {
            b.iter(|| run_git(repo.path(), &["diff"]))
        });
        group.bench_with_input(BenchmarkId::new("gitr", size.label()), &(), |b, _| {
            b.iter(|| run_gitr(repo.path(), &["diff"]))
        });
    }
    group.finish();
}

// ──────────────────────────── Benchmark: diff --cached ────────────────────────────

fn bench_diff_cached(c: &mut Criterion) {
    let mut group = c.benchmark_group("diff_cached");
    for size in ALL_SIZES {
        let repo = get_staged_repo(size);
        group.bench_with_input(BenchmarkId::new("git", size.label()), &(), |b, _| {
            b.iter(|| run_git(repo.path(), &["diff", "--cached"]))
        });
        group.bench_with_input(BenchmarkId::new("gitr", size.label()), &(), |b, _| {
            b.iter(|| run_gitr(repo.path(), &["diff", "--cached"]))
        });
    }
    group.finish();
}

// ──────────────────────────── Benchmark: branch -a ────────────────────────────

fn bench_branch(c: &mut Criterion) {
    let mut group = c.benchmark_group("branch");
    for size in ALL_SIZES {
        let repo = get_repo(size);
        group.bench_with_input(BenchmarkId::new("git", size.label()), &(), |b, _| {
            b.iter(|| run_git(repo.path(), &["branch", "-a"]))
        });
        group.bench_with_input(BenchmarkId::new("gitr", size.label()), &(), |b, _| {
            b.iter(|| run_gitr(repo.path(), &["branch", "-a"]))
        });
    }
    group.finish();
}

// ──────────────────────────── Benchmark: tag -l ────────────────────────────

fn bench_tag(c: &mut Criterion) {
    let mut group = c.benchmark_group("tag");
    for size in ALL_SIZES {
        let repo = get_repo(size);
        group.bench_with_input(BenchmarkId::new("git", size.label()), &(), |b, _| {
            b.iter(|| run_git(repo.path(), &["tag", "-l"]))
        });
        group.bench_with_input(BenchmarkId::new("gitr", size.label()), &(), |b, _| {
            b.iter(|| run_gitr(repo.path(), &["tag", "-l"]))
        });
    }
    group.finish();
}

// ──────────────────────────── Benchmark: for-each-ref ────────────────────────────

fn bench_for_each_ref(c: &mut Criterion) {
    let mut group = c.benchmark_group("for-each-ref");
    for size in ALL_SIZES {
        let repo = get_repo(size);
        group.bench_with_input(BenchmarkId::new("git", size.label()), &(), |b, _| {
            b.iter(|| run_git(repo.path(), &["for-each-ref"]))
        });
        group.bench_with_input(BenchmarkId::new("gitr", size.label()), &(), |b, _| {
            b.iter(|| run_gitr(repo.path(), &["for-each-ref"]))
        });
    }
    group.finish();
}

// ──────────────────────────── Benchmark: show-ref ────────────────────────────

fn bench_show_ref(c: &mut Criterion) {
    let mut group = c.benchmark_group("show-ref");
    for size in ALL_SIZES {
        let repo = get_repo(size);
        group.bench_with_input(BenchmarkId::new("git", size.label()), &(), |b, _| {
            b.iter(|| run_git(repo.path(), &["show-ref"]))
        });
        group.bench_with_input(BenchmarkId::new("gitr", size.label()), &(), |b, _| {
            b.iter(|| run_gitr(repo.path(), &["show-ref"]))
        });
    }
    group.finish();
}

// ──────────────────────────── Benchmark: blame ────────────────────────────

fn bench_blame(c: &mut Criterion) {
    let mut group = c.benchmark_group("blame");
    for size in ALL_SIZES {
        let repo = get_repo(size);
        let file = "commit_file_1.txt";
        group.bench_with_input(BenchmarkId::new("git", size.label()), &(), |b, _| {
            b.iter(|| run_git(repo.path(), &["blame", file]))
        });
        group.bench_with_input(BenchmarkId::new("gitr", size.label()), &(), |b, _| {
            b.iter(|| run_gitr(repo.path(), &["blame", file]))
        });
    }
    group.finish();
}

// ──────────────────────────── Benchmark: show HEAD ────────────────────────────

fn bench_show(c: &mut Criterion) {
    let mut group = c.benchmark_group("show");
    for size in ALL_SIZES {
        let repo = get_repo(size);
        group.bench_with_input(BenchmarkId::new("git", size.label()), &(), |b, _| {
            b.iter(|| run_git(repo.path(), &["show", "HEAD"]))
        });
        group.bench_with_input(BenchmarkId::new("gitr", size.label()), &(), |b, _| {
            b.iter(|| run_gitr(repo.path(), &["show", "HEAD"]))
        });
    }
    group.finish();
}

// ──────────────────────────── Benchmark: rev-parse ────────────────────────────

fn bench_rev_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("rev-parse");
    for size in ALL_SIZES {
        let repo = get_repo(size);
        group.bench_with_input(
            BenchmarkId::new("git/HEAD", size.label()),
            &(),
            |b, _| b.iter(|| run_git(repo.path(), &["rev-parse", "HEAD"])),
        );
        group.bench_with_input(
            BenchmarkId::new("gitr/HEAD", size.label()),
            &(),
            |b, _| b.iter(|| run_gitr(repo.path(), &["rev-parse", "HEAD"])),
        );
        group.bench_with_input(
            BenchmarkId::new("git/--git-dir", size.label()),
            &(),
            |b, _| b.iter(|| run_git(repo.path(), &["rev-parse", "--git-dir"])),
        );
        group.bench_with_input(
            BenchmarkId::new("gitr/--git-dir", size.label()),
            &(),
            |b, _| b.iter(|| run_gitr(repo.path(), &["rev-parse", "--git-dir"])),
        );
    }
    group.finish();
}

// ──────────────────────────── Benchmark: add ────────────────────────────

fn bench_add(c: &mut Criterion) {
    let mut group = c.benchmark_group("add");
    // `git add .` is destructive (changes index), so each iteration needs a fresh dirty state.
    // We only benchmark this at small size to keep iteration time reasonable.
    let size = RepoSize::Small;
    group.bench_function("git", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                setup_repo(dir.path(), size);
                dirty_worktree(dir.path(), 3);
                dir
            },
            |dir| run_git(dir.path(), &["add", "."]),
        )
    });
    group.bench_function("gitr", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                setup_repo(dir.path(), size);
                dirty_worktree(dir.path(), 3);
                dir
            },
            |dir| run_gitr(dir.path(), &["add", "."]),
        )
    });
    group.finish();
}

// ──────────────────────────── Benchmark: commit ────────────────────────────

fn bench_commit(c: &mut Criterion) {
    let mut group = c.benchmark_group("commit");
    // `git commit` is destructive, so each iteration needs fresh state.
    let size = RepoSize::Small;
    group.bench_function("git", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                setup_repo(dir.path(), size);
                dirty_worktree(dir.path(), 3);
                run_git(dir.path(), &["add", "."]);
                dir
            },
            |dir| run_git(dir.path(), &["commit", "-m", "bench commit"]),
        )
    });
    group.bench_function("gitr", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                setup_repo(dir.path(), size);
                dirty_worktree(dir.path(), 3);
                run_gitr(dir.path(), &["add", "."]);
                dir
            },
            |dir| run_gitr(dir.path(), &["commit", "-m", "bench commit"]),
        )
    });
    group.finish();
}

// ──────────────────────────── Group Registration ────────────────────────────

criterion_group!(
    benches,
    bench_init,
    bench_hash_object,
    bench_cat_file,
    bench_status,
    bench_ls_files,
    bench_log,
    bench_log_oneline,
    bench_rev_list,
    bench_diff,
    bench_diff_cached,
    bench_branch,
    bench_tag,
    bench_for_each_ref,
    bench_show_ref,
    bench_blame,
    bench_show,
    bench_rev_parse,
    bench_add,
    bench_commit,
);

criterion_main!(benches);
