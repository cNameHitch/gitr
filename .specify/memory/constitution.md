# Gitr Constitution

## Core Principles

### I. Safety-First

Leverage Rust's type system, ownership model, and lifetime system to eliminate entire classes of bugs present in the C codebase. All public APIs must return `Result<T, E>` for fallible operations — no panics in library code. Use of `unsafe` requires documented justification in a comment block explaining why safe alternatives are insufficient, what invariants must be upheld, and a link to the relevant spec section. All byte buffers must have bounds checking. All file handles must be managed through RAII. Integer overflow must use checked or saturating arithmetic where correctness matters.

### II. C-Compatibility

The Rust implementation must produce byte-identical output to C git for all supported operations. All on-disk formats (loose objects, packfiles, index, config, refs) must be read and written in exact compatibility with C git. The goal is a drop-in replacement: existing repositories, scripts, and tooling must work unchanged. The git test suite (t/ directory) is the ultimate compatibility oracle. Where C git behavior is undocumented or ambiguous, match the behavior of the latest stable C git release.

### III. Modular Crates

The project is organized as a Cargo workspace of small, focused crates. Each subsystem (hashing, objects, storage, refs, config, etc.) lives in its own crate with its own unit tests. The CLI binary is a thin layer that wires crates together — no business logic in the CLI layer. Crate boundaries follow the dependency graph: a crate may only depend on crates with a lower spec number. Circular dependencies are forbidden. Each crate must compile and pass tests independently.

### IV. Trait-Based Abstraction

Pluggable backends are expressed as traits. Object storage, reference storage, transport, and hash algorithm are all trait-based to allow alternative implementations (e.g., reftable vs files backend, HTTP vs SSH transport). Public APIs use trait objects or generics — no concrete types where polymorphism is needed. Traits must be object-safe where dynamic dispatch is required. Default implementations are provided where a sensible default exists.

### V. Test-Driven

Every spec must define acceptance tests before implementation begins. Unit tests cover individual functions and types. Integration tests verify byte-compatibility against C git by running both implementations on identical inputs and comparing outputs. Property-based tests (using `proptest` or `quickcheck`) validate data structure invariants. The CI pipeline must run `cargo test --workspace` and the relevant subset of git's t/ test suite. Code coverage must not decrease with new changes.

## Technical Standards

### Error Handling
- Use `thiserror` for defining error types in library crates
- Use `anyhow` only in the CLI binary, never in library crates
- Error types must be specific and actionable — no generic "something went wrong" errors
- Error chains must preserve context: wrap lower-level errors with higher-level meaning

### Performance
- Memory-mapped I/O for packfile access (`memmap2`)
- Lazy loading: parse objects on demand, not eagerly
- Zero-copy parsing where format allows (packfile deltas, tree entries)
- Parallel operations where safe (object hashing, pack indexing)
- Benchmark critical paths with `criterion` and track regressions

### Byte String Handling
- Use `bstr` crate for all git paths and user-facing strings
- Git paths are byte sequences, not guaranteed UTF-8
- Conversion to `str`/`String` only at display boundaries
- File system operations use `OsStr`/`OsString` where the platform requires

### Serialization
- All on-disk formats use manual serialization — no serde for git formats
- Serialization must be byte-identical to C git output
- Format versions must be explicitly handled (index v2/v3/v4, pack v2/v3)
- Unknown extensions and future format fields must be preserved on round-trip

## Governance

- This constitution supersedes all other development practices for gitr
- Amendments require documentation of the change, rationale, and migration plan
- All code reviews must verify compliance with these principles
- Spec violations must be justified in the relevant plan.md Complexity Tracking table

**Version**: 1.0.0 | **Ratified**: 2026-02-05 | **Last Amended**: 2026-02-05
