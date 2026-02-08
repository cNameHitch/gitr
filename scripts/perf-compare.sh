#!/usr/bin/env bash
#
# Performance comparison: gitr (Rust) vs git (C) using hyperfine.
#
# Prerequisites: hyperfine (https://github.com/sharkdp/hyperfine)
#
# Usage:
#   bash scripts/perf-compare.sh            # run all sizes
#   bash scripts/perf-compare.sh small       # run only small
#   bash scripts/perf-compare.sh medium      # run only medium
#   bash scripts/perf-compare.sh large       # run only large

set -euo pipefail

# ──────────────────────────── Prerequisites ────────────────────────────

if ! command -v hyperfine &>/dev/null; then
    echo "Error: hyperfine is not installed."
    echo "Install with: brew install hyperfine  (or cargo install hyperfine)"
    exit 1
fi

if ! command -v git &>/dev/null; then
    echo "Error: git is not installed."
    exit 1
fi

# ──────────────────────────── Build gitr ────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
GITR="$REPO_ROOT/target/release/gitr"

echo "==> Building gitr in release mode..."
cargo build --release --manifest-path="$REPO_ROOT/Cargo.toml" -p git-cli
echo "    Binary: $GITR"
echo ""

# ──────────────────────────── Temp directory cleanup ────────────────────────────

TMPBASE=$(mktemp -d)
trap 'rm -rf "$TMPBASE"' EXIT

# ──────────────────────────── Environment pinning ────────────────────────────

export GIT_AUTHOR_NAME="Bench Author"
export GIT_AUTHOR_EMAIL="bench@example.com"
export GIT_AUTHOR_DATE="1234567890 +0000"
export GIT_COMMITTER_NAME="Bench Committer"
export GIT_COMMITTER_EMAIL="bench@example.com"
export GIT_COMMITTER_DATE="1234567890 +0000"
export TZ=UTC
export LC_ALL=C
export LANG=C
export GIT_CONFIG_NOSYSTEM=1
export GIT_PROTOCOL_FROM_USER=0
export GIT_CONFIG_COUNT=1
export GIT_CONFIG_KEY_0=protocol.file.allow
export GIT_CONFIG_VALUE_0=always

# ──────────────────────────── Repo setup ────────────────────────────

setup_repo() {
    local dir=$1 files=$2 commits=$3 branches=$4
    mkdir -p "$dir"

    git init -b main "$dir" >/dev/null 2>&1
    git -C "$dir" config user.name "Bench Author"
    git -C "$dir" config user.email "bench@example.com"

    local counter=0

    # Create initial files
    if [ "$files" -gt 0 ]; then
        for i in $(seq 0 $((files - 1))); do
            local subdir="dir_$((i % 50))"
            mkdir -p "$dir/$subdir"
            echo "initial content $i" > "$dir/$subdir/file_$i.txt"
        done
        counter=$((counter + 1))
        GIT_AUTHOR_DATE="$((1234567890 + counter)) +0000" \
        GIT_COMMITTER_DATE="$((1234567890 + counter)) +0000" \
            git -C "$dir" add . >/dev/null 2>&1
        GIT_AUTHOR_DATE="$((1234567890 + counter)) +0000" \
        GIT_COMMITTER_DATE="$((1234567890 + counter)) +0000" \
            git -C "$dir" commit -m "initial files" >/dev/null 2>&1
    fi

    # Sequential commits
    local start=1
    [ "$files" -eq 0 ] && start=0
    for i in $(seq $start $((commits - 1))); do
        echo "commit content $i" > "$dir/commit_file_$i.txt"
        counter=$((counter + 1))
        GIT_AUTHOR_DATE="$((1234567890 + counter)) +0000" \
        GIT_COMMITTER_DATE="$((1234567890 + counter)) +0000" \
            git -C "$dir" add "commit_file_$i.txt" >/dev/null 2>&1
        GIT_AUTHOR_DATE="$((1234567890 + counter)) +0000" \
        GIT_COMMITTER_DATE="$((1234567890 + counter)) +0000" \
            git -C "$dir" commit -m "commit $i" >/dev/null 2>&1
    done

    # Create branches
    if [ "$branches" -gt 0 ] && [ "$commits" -gt 0 ]; then
        local interval=$((commits / branches))
        [ "$interval" -lt 1 ] && interval=1
        for b in $(seq 0 $((branches - 1))); do
            local offset=$((b * interval))
            [ "$offset" -ge "$commits" ] && offset=$((commits - 1))
            local rev="HEAD~$((commits - 1 - offset))"
            git -C "$dir" branch "branch-$b" "$rev" 2>/dev/null || true
        done
    fi

    # Create tags
    local tag_interval=10
    [ "$commits" -lt 20 ] && tag_interval=$((commits / 2))
    [ "$tag_interval" -lt 1 ] && tag_interval=1
    local tag_count=0
    for i in $(seq 0 "$tag_interval" $((commits - 1))); do
        local rev="HEAD~$((commits - 1 - i))"
        git -C "$dir" tag "v0.$tag_count" "$rev" 2>/dev/null || true
        tag_count=$((tag_count + 1))
    done
}

dirty_worktree() {
    local dir=$1 count=$2
    for i in $(seq 0 $((count - 1))); do
        local f="$dir/commit_file_$i.txt"
        [ -f "$f" ] && echo "modified content $i" > "$f"
    done
    for i in $(seq 0 4); do
        echo "untracked $i" > "$dir/untracked_$i.txt"
    done
}

# ──────────────────────────── Size definitions ────────────────────────────

declare -A SIZE_FILES=([small]=10 [medium]=1000 [large]=10000)
declare -A SIZE_COMMITS=([small]=10 [medium]=100 [large]=500)
declare -A SIZE_BRANCHES=([small]=2 [medium]=10 [large]=20)

SIZES_TO_RUN=("small" "medium" "large")
if [ $# -gt 0 ]; then
    SIZES_TO_RUN=("$1")
fi

# ──────────────────────────── Build repos ────────────────────────────

declare -A REPO_DIRS
declare -A DIRTY_DIRS

for sz in "${SIZES_TO_RUN[@]}"; do
    echo "==> Setting up $sz repo (${SIZE_FILES[$sz]} files, ${SIZE_COMMITS[$sz]} commits, ${SIZE_BRANCHES[$sz]} branches)..."
    REPO_DIRS[$sz]="$TMPBASE/repo-$sz"
    setup_repo "${REPO_DIRS[$sz]}" "${SIZE_FILES[$sz]}" "${SIZE_COMMITS[$sz]}" "${SIZE_BRANCHES[$sz]}"

    echo "    Setting up $sz dirty repo..."
    DIRTY_DIRS[$sz]="$TMPBASE/dirty-$sz"
    cp -r "${REPO_DIRS[$sz]}" "${DIRTY_DIRS[$sz]}"
    dirty_worktree "${DIRTY_DIRS[$sz]}" 5
    echo ""
done

# ──────────────────────────── Benchmark runner ────────────────────────────

WARMUP=3
RUNS=10

run_bench() {
    local name=$1 dir=$2
    shift 2
    local git_args=("$@")

    echo "--- $name ---"
    hyperfine \
        --warmup "$WARMUP" \
        --min-runs "$RUNS" \
        --export-markdown /dev/null \
        --command-name "git" "git -C '$dir' ${git_args[*]}" \
        --command-name "gitr" "'$GITR' -C '$dir' ${git_args[*]}" \
        2>&1
    echo ""
}

# ──────────────────────────── Run benchmarks ────────────────────────────

echo ""
echo "================================================================"
echo "  Performance Comparison: gitr (Rust) vs git (C)"
echo "================================================================"
echo ""

for sz in "${SIZES_TO_RUN[@]}"; do
    REPO="${REPO_DIRS[$sz]}"
    DIRTY="${DIRTY_DIRS[$sz]}"
    HEAD_OID=$(git -C "$REPO" rev-parse HEAD)

    echo ""
    echo "════════════════════════════════════════════════════════════"
    echo "  Repo size: $sz"
    echo "════════════════════════════════════════════════════════════"
    echo ""

    # Init (uses a fresh temp dir each run)
    echo "--- init ---"
    hyperfine \
        --warmup "$WARMUP" --min-runs "$RUNS" \
        --prepare "rm -rf '$TMPBASE/init-test'" \
        --command-name "git"  "mkdir -p '$TMPBASE/init-test' && git init -b main '$TMPBASE/init-test'" \
        --command-name "gitr" "mkdir -p '$TMPBASE/init-test' && '$GITR' init -b main '$TMPBASE/init-test'" \
        2>&1
    echo ""

    # Object I/O
    run_bench "hash-object/$sz" "$REPO" hash-object commit_file_1.txt
    run_bench "cat-file -p/$sz" "$REPO" cat-file -p "$HEAD_OID"
    run_bench "cat-file -t/$sz" "$REPO" cat-file -t "$HEAD_OID"

    # Index / working tree
    run_bench "status/$sz" "$DIRTY" status
    run_bench "ls-files/$sz" "$REPO" ls-files

    # Commit history
    run_bench "log/$sz" "$REPO" log
    run_bench "log --oneline/$sz" "$REPO" log --oneline
    run_bench "rev-list --count HEAD/$sz" "$REPO" rev-list --count HEAD

    # Diff
    run_bench "diff/$sz" "$DIRTY" diff
    run_bench "diff --cached/$sz" "$DIRTY" diff --cached

    # Refs
    run_bench "branch -a/$sz" "$REPO" branch -a
    run_bench "tag -l/$sz" "$REPO" tag -l
    run_bench "for-each-ref/$sz" "$REPO" for-each-ref
    run_bench "show-ref/$sz" "$REPO" show-ref

    # Inspection
    run_bench "blame/$sz" "$REPO" blame commit_file_1.txt
    run_bench "show HEAD/$sz" "$REPO" show HEAD

    # Rev-parse
    run_bench "rev-parse HEAD/$sz" "$REPO" rev-parse HEAD
    run_bench "rev-parse --git-dir/$sz" "$REPO" rev-parse --git-dir
done

echo ""
echo "================================================================"
echo "  Done. All benchmarks complete."
echo "================================================================"
