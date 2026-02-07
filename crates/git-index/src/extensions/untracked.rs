//! Untracked cache extension (UNTR).
//!
//! Caches which directories have no untracked files, avoiding full directory
//! scans on `git status`. For now we preserve the raw bytes for round-trip
//! compatibility but don't interpret the data.
