use git_hash::{HashAlgorithm, ObjectId};
use git_hash::hasher::Hasher;

// ── SHA-1 raw digest test vectors ───────────────────────────────────

#[test]
fn sha1_empty_string() {
    let oid = Hasher::digest(HashAlgorithm::Sha1, b"").unwrap();
    assert_eq!(
        oid.to_hex(),
        "da39a3ee5e6b4b0d3255bfef95601890afd80709"
    );
}

#[test]
fn sha1_hello_world() {
    let oid = Hasher::digest(HashAlgorithm::Sha1, b"hello world").unwrap();
    assert_eq!(
        oid.to_hex(),
        "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed"
    );
}

// ── SHA-256 raw digest test vectors ─────────────────────────────────

#[test]
fn sha256_empty_string() {
    let oid = Hasher::digest(HashAlgorithm::Sha256, b"").unwrap();
    assert_eq!(
        oid.to_hex(),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn sha256_hello_world() {
    let oid = Hasher::digest(HashAlgorithm::Sha256, b"hello world").unwrap();
    assert_eq!(
        oid.to_hex(),
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
}

// ── git hash-object compatible test vectors ──────────────────────────
// These match `git hash-object --stdin` output (SHA-1 mode).
// git prepends "blob <len>\0" to the content before hashing.

#[test]
fn git_hash_object_empty_blob() {
    let oid = Hasher::hash_object(HashAlgorithm::Sha1, "blob", b"").unwrap();
    assert_eq!(
        oid.to_hex(),
        "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"
    );
}

#[test]
fn git_hash_object_hello_world() {
    let oid = Hasher::hash_object(HashAlgorithm::Sha1, "blob", b"hello world").unwrap();
    assert_eq!(
        oid.to_hex(),
        "95d09f2b10159347eece71399a7e2e907ea3df4f"
    );
}

#[test]
fn git_hash_object_hello_world_newline() {
    // "Hello, World!\n" as a blob — 14 bytes content
    let oid =
        Hasher::hash_object(HashAlgorithm::Sha1, "blob", b"Hello, World!\n").unwrap();
    assert_eq!(
        oid.to_hex(),
        "8ab686eafeb1f44702738c8b0f24f2567c36da6d"
    );
}

// ── Streaming hash (multi-chunk) ────────────────────────────────────

#[test]
fn streaming_matches_oneshot() {
    let data = b"the quick brown fox jumps over the lazy dog";
    let oneshot = Hasher::digest(HashAlgorithm::Sha1, data).unwrap();

    let mut hasher = Hasher::new(HashAlgorithm::Sha1);
    hasher.update(&data[..10]);
    hasher.update(&data[10..20]);
    hasher.update(&data[20..]);
    let streamed = hasher.finalize().unwrap();

    assert_eq!(oneshot, streamed);
}

#[test]
fn streaming_matches_oneshot_sha256() {
    let data = b"the quick brown fox jumps over the lazy dog";
    let oneshot = Hasher::digest(HashAlgorithm::Sha256, data).unwrap();

    let mut hasher = Hasher::new(HashAlgorithm::Sha256);
    for chunk in data.chunks(7) {
        hasher.update(chunk);
    }
    let streamed = hasher.finalize().unwrap();

    assert_eq!(oneshot, streamed);
}

// ── Write trait usage ───────────────────────────────────────────────

#[test]
fn write_trait() {
    use std::io::Write;

    let data = b"hello world";
    let expected = Hasher::digest(HashAlgorithm::Sha1, data).unwrap();

    let mut hasher = Hasher::new(HashAlgorithm::Sha1);
    hasher.write_all(data).unwrap();
    let result = hasher.finalize().unwrap();

    assert_eq!(expected, result);
}

// ── git object types ────────────────────────────────────────────────

#[test]
fn hash_object_tree_type() {
    // Verify that different object types produce different hashes for the same content.
    let data = b"some content";
    let blob = Hasher::hash_object(HashAlgorithm::Sha1, "blob", data).unwrap();
    let tree = Hasher::hash_object(HashAlgorithm::Sha1, "tree", data).unwrap();
    let commit = Hasher::hash_object(HashAlgorithm::Sha1, "commit", data).unwrap();

    assert_ne!(blob, tree);
    assert_ne!(blob, commit);
    assert_ne!(tree, commit);
}

// ── Algorithm selection ─────────────────────────────────────────────

#[test]
fn sha1_and_sha256_differ() {
    let data = b"same input";
    let sha1 = Hasher::digest(HashAlgorithm::Sha1, data).unwrap();
    let sha256 = Hasher::digest(HashAlgorithm::Sha256, data).unwrap();

    assert_eq!(sha1.algorithm(), HashAlgorithm::Sha1);
    assert_eq!(sha256.algorithm(), HashAlgorithm::Sha256);
    assert_ne!(sha1.as_bytes().len(), sha256.as_bytes().len());
}

// ── ObjectId from hash result ───────────────────────────────────────

#[test]
fn hash_result_display_parse_roundtrip() {
    let oid = Hasher::hash_object(HashAlgorithm::Sha1, "blob", b"test content").unwrap();
    let hex = oid.to_string();
    let parsed: ObjectId = hex.parse().unwrap();
    assert_eq!(oid, parsed);
}
