//! Round-trip tests: write pack → read back → verify all objects.

use git_hash::{hasher::Hasher, HashAlgorithm, ObjectId};
use git_object::ObjectType;
use git_pack::pack::PackFile;
use git_pack::write::create_pack;

#[test]
fn roundtrip_single_blob() {
    let dir = tempfile::tempdir().unwrap();
    let content = b"roundtrip test blob";

    let (pack_path, _, _) =
        create_pack(dir.path(), "rt1", &[(ObjectType::Blob, content.to_vec())]).unwrap();

    let pack = PackFile::open(&pack_path).unwrap();
    assert_eq!(pack.num_objects(), 1);

    let oid = Hasher::hash_object(HashAlgorithm::Sha1, "blob", content).unwrap();
    let obj = pack.read_object(&oid).unwrap().unwrap();
    assert_eq!(obj.obj_type, ObjectType::Blob);
    assert_eq!(obj.data, content);
}

#[test]
fn roundtrip_multiple_objects() {
    let dir = tempfile::tempdir().unwrap();
    let objects = vec![
        (ObjectType::Blob, b"alpha".to_vec()),
        (ObjectType::Blob, b"beta".to_vec()),
        (ObjectType::Blob, b"gamma".to_vec()),
        (ObjectType::Blob, b"delta".to_vec()),
    ];

    let (pack_path, _, _) = create_pack(dir.path(), "rt2", &objects).unwrap();

    let pack = PackFile::open(&pack_path).unwrap();
    assert_eq!(pack.num_objects(), 4);

    for (obj_type, data) in &objects {
        let oid = Hasher::hash_object(
            HashAlgorithm::Sha1,
            std::str::from_utf8(obj_type.as_bytes()).unwrap(),
            data,
        )
        .unwrap();
        let obj = pack.read_object(&oid).unwrap().unwrap();
        assert_eq!(obj.obj_type, *obj_type);
        assert_eq!(obj.data, *data);
    }
}

#[test]
fn roundtrip_with_delta_objects() {
    let dir = tempfile::tempdir().unwrap();
    let pack_path = dir.path().join("rt3.pack");
    let idx_path = dir.path().join("rt3.idx");

    let base_content = b"This is the base content that will be used for delta compression testing in our roundtrip.";
    let modified_content = b"This is the modified content that will be used for delta compression testing in our roundtrip.";

    let mut writer = git_pack::write::PackWriter::new(&pack_path).unwrap();
    writer.add_object(ObjectType::Blob, base_content).unwrap();

    let base_oid = Hasher::hash_object(HashAlgorithm::Sha1, "blob", base_content).unwrap();
    let modified_oid =
        Hasher::hash_object(HashAlgorithm::Sha1, "blob", modified_content).unwrap();
    let delta = git_pack::delta::compute::compute_delta(base_content, modified_content);
    writer
        .add_delta(base_oid, modified_oid, &delta)
        .unwrap();

    let mut entries: Vec<(ObjectId, u64, u32)> = writer
        .entries()
        .map(|(oid, off, crc)| (*oid, off, crc))
        .collect();
    let (_, checksum) = writer.finish().unwrap();
    git_pack::write::build_pack_index(&idx_path, &mut entries, &checksum).unwrap();

    // Read back
    let pack = PackFile::open(&pack_path).unwrap();
    assert_eq!(pack.num_objects(), 2);

    let obj = pack.read_object(&base_oid).unwrap().unwrap();
    assert_eq!(obj.data, base_content.as_slice());

    let obj = pack.read_object(&modified_oid).unwrap().unwrap();
    assert_eq!(obj.data, modified_content.as_slice());
}

#[test]
fn roundtrip_verify_with_c_git() {
    let dir = tempfile::tempdir().unwrap();
    let objects: Vec<(ObjectType, Vec<u8>)> = (0..10)
        .map(|i| (ObjectType::Blob, format!("object number {i}").into_bytes()))
        .collect();

    let (pack_path, _, _) = create_pack(dir.path(), "rt4", &objects).unwrap();

    let output = std::process::Command::new("git")
        .args(["verify-pack", "-v"])
        .arg(&pack_path)
        .output()
        .expect("failed to run git verify-pack");

    assert!(
        output.status.success(),
        "git verify-pack failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
