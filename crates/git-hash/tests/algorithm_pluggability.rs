use git_hash::collections::{OidArray, OidMap, OidSet};
use git_hash::fanout::FanoutTable;
use git_hash::hasher::Hasher;
use git_hash::{HashAlgorithm, ObjectId};

/// Run the full test suite for a given hash algorithm.
fn full_workflow(algo: HashAlgorithm) {
    // 1. Hash computation
    let oid = Hasher::digest(algo, b"test content").unwrap();
    assert_eq!(oid.algorithm(), algo);
    assert_eq!(oid.as_bytes().len(), algo.digest_len());
    assert_eq!(oid.to_hex().len(), algo.hex_len());

    // 2. Streaming hash matches one-shot
    let mut hasher = Hasher::new(algo);
    hasher.update(b"test ");
    hasher.update(b"content");
    assert_eq!(hasher.finalize().unwrap(), oid);

    // 3. git object hashing
    let blob_oid = Hasher::hash_object(algo, "blob", b"test").unwrap();
    assert_eq!(blob_oid.algorithm(), algo);

    // 4. Hex round-trip
    let hex = oid.to_hex();
    let parsed: ObjectId = hex.parse().unwrap();
    assert_eq!(parsed, oid);

    // 5. Null OID
    let null = algo.null_oid();
    assert!(null.is_null());
    assert_eq!(null.algorithm(), algo);

    // 6. From bytes round-trip
    let reconstructed = ObjectId::from_bytes(oid.as_bytes(), algo).unwrap();
    assert_eq!(reconstructed, oid);

    // 7. Collections with this algorithm
    let oids: Vec<ObjectId> = (0..100u32)
        .map(|n| Hasher::digest(algo, &n.to_be_bytes()).unwrap())
        .collect();

    // OidArray
    let mut arr = OidArray::new();
    for &oid in &oids {
        arr.push(oid);
    }
    assert!(arr.contains(&oids[50]));
    assert!(!arr.contains(&null));

    // OidSet
    let mut set = OidSet::new();
    for &oid in &oids {
        set.insert(oid);
    }
    assert!(set.contains(&oids[50]));
    assert_eq!(set.len(), 100);

    // OidMap
    let mut map = OidMap::new();
    for (i, &oid) in oids.iter().enumerate() {
        map.insert(oid, i);
    }
    assert_eq!(map.get(&oids[50]), Some(&50));

    // 8. FanoutTable
    let mut sorted_oids = oids.clone();
    sorted_oids.sort();
    let ft = FanoutTable::build(&sorted_oids);
    assert_eq!(ft.total() as usize, sorted_oids.len());

    // Verify fan-out byte-roundtrip
    let bytes = ft.to_bytes();
    let ft2 = FanoutTable::from_bytes(&bytes).unwrap();
    for b in 0..=255u8 {
        assert_eq!(ft.range(b), ft2.range(b));
    }
}

#[test]
fn full_workflow_sha1() {
    full_workflow(HashAlgorithm::Sha1);
}

#[test]
fn full_workflow_sha256() {
    full_workflow(HashAlgorithm::Sha256);
}

#[test]
fn cross_algorithm_oids_are_distinct() {
    let sha1 = Hasher::digest(HashAlgorithm::Sha1, b"same").unwrap();
    let sha256 = Hasher::digest(HashAlgorithm::Sha256, b"same").unwrap();

    // Different algorithms → different OID types → not equal
    assert_ne!(sha1, sha256);

    // But each is consistent with itself
    assert_eq!(sha1, Hasher::digest(HashAlgorithm::Sha1, b"same").unwrap());
    assert_eq!(
        sha256,
        Hasher::digest(HashAlgorithm::Sha256, b"same").unwrap()
    );
}

#[test]
fn hasher_accepts_algorithm_parameter() {
    for algo in [HashAlgorithm::Sha1, HashAlgorithm::Sha256] {
        let h = Hasher::new(algo);
        let oid = h.finalize().unwrap();
        assert_eq!(oid.algorithm(), algo);
    }
}
