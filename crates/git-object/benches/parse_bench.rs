use bstr::BString;
use criterion::{criterion_group, criterion_main, Criterion};
use git_hash::ObjectId;
use git_object::{Blob, Commit, FileMode, Object, ObjectType, Tag, Tree, TreeEntry};
use git_utils::date::{GitDate, Signature};

fn make_signature(name: &str, email: &str, ts: i64) -> Signature {
    Signature {
        name: BString::from(name),
        email: BString::from(email),
        date: GitDate {
            timestamp: ts,
            tz_offset: 0,
        },
    }
}

fn sample_commit_bytes() -> Vec<u8> {
    let commit = Commit {
        tree: ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap(),
        parents: vec![
            ObjectId::from_hex("0000000000000000000000000000000000000001").unwrap(),
        ],
        author: make_signature("Alice Author", "alice@example.com", 1700000000),
        committer: make_signature("Bob Committer", "bob@example.com", 1700000100),
        encoding: None,
        gpgsig: None,
        extra_headers: vec![],
        message: BString::from("Implement feature X\n\nThis commit adds the feature X with full test coverage.\n"),
    };
    commit.serialize_content()
}

fn sample_tree_bytes() -> Vec<u8> {
    let oid1 = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
    let oid2 = ObjectId::from_hex("0000000000000000000000000000000000000001").unwrap();

    let tree = Tree {
        entries: vec![
            TreeEntry {
                mode: FileMode::Regular,
                name: BString::from("Cargo.toml"),
                oid: oid1,
            },
            TreeEntry {
                mode: FileMode::Regular,
                name: BString::from("README.md"),
                oid: oid2,
            },
            TreeEntry {
                mode: FileMode::Executable,
                name: BString::from("build.sh"),
                oid: oid1,
            },
            TreeEntry {
                mode: FileMode::Tree,
                name: BString::from("src"),
                oid: oid2,
            },
            TreeEntry {
                mode: FileMode::Tree,
                name: BString::from("tests"),
                oid: oid1,
            },
        ],
    };
    tree.serialize_content()
}

fn sample_tag_bytes() -> Vec<u8> {
    let tag = Tag {
        target: ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap(),
        target_type: ObjectType::Commit,
        tag_name: BString::from("v1.0.0"),
        tagger: Some(make_signature("Release Bot", "release@example.com", 1700000000)),
        message: BString::from("Release version 1.0.0\n"),
        gpgsig: None,
    };
    tag.serialize_content()
}

fn parse_benchmarks(c: &mut Criterion) {
    let commit_data = sample_commit_bytes();
    let tree_data = sample_tree_bytes();
    let tag_data = sample_tag_bytes();
    let blob_data = b"Hello, world! This is some sample blob content.\n".to_vec();

    c.bench_function("parse_commit", |b| {
        b.iter(|| Commit::parse(&commit_data).unwrap());
    });

    c.bench_function("parse_tree_5_entries", |b| {
        b.iter(|| Tree::parse(&tree_data).unwrap());
    });

    c.bench_function("parse_tag", |b| {
        b.iter(|| Tag::parse(&tag_data).unwrap());
    });

    c.bench_function("parse_blob", |b| {
        b.iter(|| Blob::parse(&blob_data));
    });

    c.bench_function("serialize_commit", |b| {
        let commit = Commit::parse(&commit_data).unwrap();
        b.iter(|| commit.serialize_content());
    });

    c.bench_function("serialize_tree_5_entries", |b| {
        let tree = Tree::parse(&tree_data).unwrap();
        b.iter(|| tree.serialize_content());
    });

    c.bench_function("serialize_tag", |b| {
        let tag = Tag::parse(&tag_data).unwrap();
        b.iter(|| tag.serialize_content());
    });

    c.bench_function("roundtrip_commit", |b| {
        let obj = Object::Commit(Commit::parse(&commit_data).unwrap());
        b.iter(|| {
            let bytes = obj.serialize();
            Object::parse(&bytes).unwrap()
        });
    });

    c.bench_function("compute_oid_blob_48b", |b| {
        let obj = Object::Blob(Blob::new(blob_data.clone()));
        b.iter(|| obj.compute_oid(git_hash::HashAlgorithm::Sha1).unwrap());
    });
}

criterion_group!(benches, parse_benchmarks);
criterion_main!(benches);
