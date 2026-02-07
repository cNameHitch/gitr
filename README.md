<div align="center">

# gitr

**A complete reimplementation of Git in Rust.**

Byte-compatible drop-in replacement for C Git, built with Rust's safety guarantees.

[![Tests](https://github.com/cNameHitch/gitr/actions/workflows/test.yml/badge.svg)](https://github.com/cNameHitch/gitr/actions/workflows/test.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

</div>

---

## Overview

Gitr is a ground-up Git implementation written in Rust across 16 modular library crates and a CLI binary. It targets **byte-identical output** with C Git — existing repositories, scripts, hooks, and tooling work unchanged.

### Why gitr?

- **Safety-first** — Rust's type system, ownership model, and `Result<T, E>` everywhere. No panics in library code.
- **Byte-compatible** — Output matches C Git byte-for-byte. Verified by 1,200+ interop tests that compare gitr vs `git` directly.
- **Modular** — 16 focused crates with clear boundaries and no circular dependencies.
- **Fast** — Memory-mapped packfile I/O, zero-copy parsing, lazy object loading, and parallel operations via `rayon`.

## Quick Start

### Prerequisites

- [Rust 1.75+](https://rustup.rs/) (install via `rustup`)
- C Git (for interop tests only)

### Build

```bash
git clone https://github.com/cNameHitch/gitr.git
cd gitr
cargo build --release
```

The binary is written to `target/release/gitr`.

### Install

```bash
cargo install --path crates/git-cli
```

### Usage

Gitr mirrors the Git CLI interface:

```bash
# Initialize a repository
gitr init my-project
cd my-project

# Stage and commit
gitr add .
gitr commit -m "Initial commit"

# Branching
gitr branch feature
gitr switch feature

# View history
gitr log --oneline --graph

# Diff and merge
gitr diff HEAD~1
gitr merge main

# Remote operations
gitr remote add origin git@github.com:user/repo.git
gitr fetch origin
gitr push origin main
```

## Supported Commands

Gitr implements **74+ Git commands** across four categories:

<details>
<summary><strong>Plumbing</strong> — low-level object and ref operations</summary>

`cat-file` `hash-object` `rev-parse` `update-ref` `for-each-ref` `show-ref` `symbolic-ref` `ls-files` `ls-tree` `update-index` `check-ignore` `check-attr` `mktree` `mktag` `commit-tree` `write-tree` `verify-pack` `pack-objects` `index-pack` `read-tree`

</details>

<details>
<summary><strong>Porcelain</strong> — everyday user-facing commands</summary>

`init` `clone` `config` `add` `rm` `mv` `status` `restore` `clean` `checkout` `switch` `branch` `merge` `rebase` `cherry-pick` `revert` `commit` `reset` `remote` `fetch` `push` `pull` `tag`

</details>

<details>
<summary><strong>History & Inspection</strong></summary>

`log` `show` `diff` `blame` `bisect` `shortlog` `describe` `grep` `rev-list` `reflog`

</details>

<details>
<summary><strong>Advanced</strong></summary>

`stash` `worktree` `submodule` `bundle` `archive` `notes` `gc` `repack` `prune` `fsck` `fast-import` `format-patch` `am` `verify-commit` `verify-tag` `credential` `daemon`

</details>

## Architecture

Gitr is organized as a Cargo workspace of focused, layered crates:

| Crate | Description |
|-------|-------------|
| **git-utils** | Foundation: byte strings, paths, date parsing, lock files, subprocesses |
| **git-hash** | Hash algorithms (SHA-1, SHA-256) and object identity |
| **git-object** | Object model: blob, tree, commit, tag parsing and serialization |
| **git-loose** | Loose object storage with zlib compression |
| **git-pack** | Packfile v2: reading, delta resolution, bitmaps, memory-mapped I/O |
| **git-odb** | Unified object database over loose + packed backends |
| **git-index** | Index / staging area with extensions |
| **git-ref** | References: branches, tags, symbolic refs, reflogs, concurrent access |
| **git-config** | Multi-scope INI-like configuration (system, global, local, worktree) |
| **git-repository** | Repository discovery, initialization, environment handling |
| **git-diff** | Diff engine: Myers, histogram, and patience algorithms |
| **git-merge** | Merge engine: ORT strategy with full conflict handling |
| **git-revwalk** | Revision walking, commit graph traversal, merge-base computation |
| **git-transport** | Transport layer: local `file://`, HTTP(S), SSH, `git://` |
| **git-protocol** | Wire protocol: pkt-line framing, protocol v1/v2, fetch/push negotiation |
| **git-cli** | CLI binary (`gitr`): all plumbing, porcelain, history, and advanced commands |

### Dependency Graph

```
git-utils ──┬──▶ git-hash ──▶ git-object ──┬──▶ git-loose ──┐
            │                               │                │
            │                               └──▶ git-pack ───┤
            │                                                │
            │                                       git-odb ◀┘
            │                                          │
            ├──────────────▶ git-config ─┐             │
            │                            ├──▶ git-repository
            ├──────────────▶ git-ref ────┘             │
            │                              ┌───────────┘
            │                         git-index
            │                              │
            │       git-diff ◀─────────────┤
            │       git-merge ◀────────────┤
            │       git-revwalk ◀──────────┤
            │       git-transport ◀────────┤
            │       git-protocol ◀─────────┤
            │                              │
            └──────▶ git-cli ◀─────────────┘
```

## Testing

```bash
# Run the full test suite
cargo test --workspace

# Run tests for a specific crate
cargo test -p git-diff

# Run with output visible
cargo test --workspace -- --nocapture
```

The test suite includes:

- **Unit tests** — per-crate correctness checks
- **Integration tests** — cross-crate workflows
- **E2E interop tests** — run the same operation in gitr and C Git, compare output byte-for-byte
- **Property-based tests** — randomized input via `proptest`
- **Benchmarks** — performance tracking via `criterion`

### Linting

```bash
cargo clippy --workspace -- -D warnings
```

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Ensure all tests pass (`cargo test --workspace`)
4. Ensure no clippy warnings (`cargo clippy --workspace -- -D warnings`)
5. Format your code (`cargo fmt --all`)
6. Open a pull request against `main`

### Design Principles

- All public APIs return `Result<T, E>` — no panics in library code
- Byte-identical output to C Git is the primary compatibility target
- Each crate has a single responsibility with no circular dependencies
- Trait-based abstractions enable pluggable backends (storage, transport, hashing)
- Manual serialization for Git formats — no `serde` for on-disk Git structures

## License

This project is licensed under the [MIT License](LICENSE).
