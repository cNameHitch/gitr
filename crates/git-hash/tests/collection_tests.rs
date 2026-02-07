use git_hash::collections::{OidArray, OidMap, OidSet};
use git_hash::hasher::Hasher;
use git_hash::{HashAlgorithm, ObjectId};

fn make_oid(n: u32) -> ObjectId {
    Hasher::digest(HashAlgorithm::Sha1, &n.to_be_bytes()).unwrap()
}

// ── OidArray tests ──────────────────────────────────────────────────

#[test]
fn oid_array_push_and_lookup() {
    let mut arr = OidArray::new();
    let oid = make_oid(42);
    arr.push(oid);
    assert!(arr.contains(&oid));
    assert_eq!(arr.lookup(&oid), Some(0));
}

#[test]
fn oid_array_sorted_lookup() {
    let mut arr = OidArray::new();
    for i in 0..100 {
        arr.push(make_oid(i));
    }
    // Should find all inserted OIDs.
    for i in 0..100 {
        assert!(arr.contains(&make_oid(i)));
    }
    // Should not find others.
    assert!(!arr.contains(&make_oid(999)));
}

#[test]
fn oid_array_for_each_unique() {
    let mut arr = OidArray::new();
    let oid = make_oid(1);
    arr.push(oid);
    arr.push(oid); // duplicate
    arr.push(oid); // duplicate
    arr.push(make_oid(2));

    let mut unique = Vec::new();
    arr.for_each_unique(|o| {
        unique.push(*o);
        Ok(())
    })
    .unwrap();
    assert_eq!(unique.len(), 2);
}

#[test]
fn oid_array_10k_oids() {
    let mut arr = OidArray::new();
    for i in 0..10_000u32 {
        arr.push(make_oid(i));
    }
    assert_eq!(arr.len(), 10_000);

    // Binary search should find all.
    for i in (0..10_000u32).step_by(100) {
        assert!(arr.contains(&make_oid(i)));
    }
}

#[test]
fn oid_array_find_by_prefix() {
    let mut arr = OidArray::new();
    for i in 0..100 {
        arr.push(make_oid(i));
    }
    let oid = make_oid(42);
    let prefix = &oid.to_hex()[..8];
    let matches = arr.find_by_prefix(prefix);
    assert!(matches.contains(&oid));
}

#[test]
fn oid_array_from_iterator() {
    let oids: Vec<ObjectId> = (0..10).map(make_oid).collect();
    let arr: OidArray = oids.into_iter().collect();
    assert_eq!(arr.len(), 10);
}

// ── OidMap tests ────────────────────────────────────────────────────

#[test]
fn oid_map_insert_get() {
    let mut map = OidMap::new();
    let oid = make_oid(1);
    map.insert(oid, "hello");
    assert_eq!(map.get(&oid), Some(&"hello"));
}

#[test]
fn oid_map_replace() {
    let mut map = OidMap::new();
    let oid = make_oid(1);
    assert!(map.insert(oid, "first").is_none());
    assert_eq!(map.insert(oid, "second"), Some("first"));
    assert_eq!(map.get(&oid), Some(&"second"));
}

#[test]
fn oid_map_remove() {
    let mut map = OidMap::new();
    let oid = make_oid(1);
    map.insert(oid, 42);
    assert_eq!(map.remove(&oid), Some(42));
    assert!(!map.contains_key(&oid));
}

#[test]
fn oid_map_10k_entries() {
    let mut map = OidMap::new();
    for i in 0..10_000u32 {
        map.insert(make_oid(i), i);
    }
    assert_eq!(map.len(), 10_000);
    for i in (0..10_000u32).step_by(100) {
        assert_eq!(map.get(&make_oid(i)), Some(&i));
    }
}

// ── OidSet tests ────────────────────────────────────────────────────

#[test]
fn oid_set_insert_contains() {
    let mut set = OidSet::new();
    let oid = make_oid(1);
    assert!(set.insert(oid)); // new
    assert!(!set.insert(oid)); // duplicate
    assert!(set.contains(&oid));
}

#[test]
fn oid_set_remove() {
    let mut set = OidSet::new();
    let oid = make_oid(1);
    set.insert(oid);
    assert!(set.remove(&oid));
    assert!(!set.contains(&oid));
}

#[test]
fn oid_set_10k_membership() {
    let mut set = OidSet::new();
    for i in 0..10_000u32 {
        set.insert(make_oid(i));
    }
    assert_eq!(set.len(), 10_000);
    for i in (0..10_000u32).step_by(100) {
        assert!(set.contains(&make_oid(i)));
    }
    assert!(!set.contains(&make_oid(99_999)));
}

// ── FanoutTable tests ───────────────────────────────────────────────

#[test]
fn fanout_with_real_oids() {
    use git_hash::fanout::FanoutTable;

    let mut oids: Vec<ObjectId> = (0..1_000u32).map(make_oid).collect();
    oids.sort();

    let ft = FanoutTable::build(&oids);
    assert_eq!(ft.total(), 1_000);

    // Verify that range lookups are consistent.
    let mut total_in_ranges = 0;
    for b in 0..=255u8 {
        let range = ft.range(b);
        total_in_ranges += range.len();
        for idx in range.clone() {
            assert_eq!(oids[idx].first_byte(), b);
        }
    }
    assert_eq!(total_in_ranges, 1_000);
}
