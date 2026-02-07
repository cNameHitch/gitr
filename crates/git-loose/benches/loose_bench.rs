use criterion::{criterion_group, criterion_main, Criterion};
use git_hash::HashAlgorithm;
use git_loose::LooseObjectStore;
use git_object::ObjectType;
use std::process::Command;

fn setup_bench_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    Command::new("git")
        .args(["init", "--bare"])
        .current_dir(dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    let objects_dir = dir.path().join("objects");
    (dir, objects_dir)
}

fn bench_write(c: &mut Criterion) {
    let (_dir, objects_dir) = setup_bench_repo();
    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);

    let mut i = 0u64;
    c.bench_function("write_blob_1kb", |b| {
        b.iter(|| {
            i += 1;
            let content = format!("benchmark content {}", i);
            store
                .write_raw(ObjectType::Blob, content.as_bytes())
                .unwrap();
        })
    });
}

fn bench_read(c: &mut Criterion) {
    let (_dir, objects_dir) = setup_bench_repo();
    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);

    let content = b"benchmark read content for testing performance\n";
    let oid = store.write_raw(ObjectType::Blob, content).unwrap();

    c.bench_function("read_blob", |b| {
        b.iter(|| {
            store.read(&oid).unwrap().unwrap();
        })
    });
}

fn bench_read_header(c: &mut Criterion) {
    let (_dir, objects_dir) = setup_bench_repo();
    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);

    let content = b"header-only read benchmark content\n";
    let oid = store.write_raw(ObjectType::Blob, content).unwrap();

    c.bench_function("read_header", |b| {
        b.iter(|| {
            store.read_header(&oid).unwrap().unwrap();
        })
    });
}

fn bench_contains(c: &mut Criterion) {
    let (_dir, objects_dir) = setup_bench_repo();
    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);

    let oid = store
        .write_raw(ObjectType::Blob, b"exists check benchmark")
        .unwrap();

    c.bench_function("contains", |b| {
        b.iter(|| {
            store.contains(&oid);
        })
    });
}

criterion_group!(loose, bench_write, bench_read, bench_read_header, bench_contains);
criterion_main!(loose);
