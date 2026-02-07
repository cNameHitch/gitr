use criterion::{criterion_group, criterion_main, Criterion};
use git_hash::ObjectId;
use git_pack::pack::PackFile;

fn fixture_pack() -> PackFile {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let pack_path = format!("{manifest_dir}/tests/fixtures/test.pack");
    PackFile::open(&pack_path).expect("failed to open fixture pack")
}

fn bench_index_lookup(c: &mut Criterion) {
    let pack = fixture_pack();
    let oid = ObjectId::from_hex("8ab686eafeb1f44702738c8b0f24f2567c36da6d").unwrap();

    c.bench_function("index_lookup", |b| {
        b.iter(|| {
            pack.index().lookup(&oid);
        });
    });
}

fn bench_read_blob(c: &mut Criterion) {
    let pack = fixture_pack();
    let oid = ObjectId::from_hex("8ab686eafeb1f44702738c8b0f24f2567c36da6d").unwrap();

    c.bench_function("read_blob", |b| {
        b.iter(|| {
            pack.read_object(&oid).unwrap();
        });
    });
}

fn bench_read_delta_object(c: &mut Criterion) {
    let pack = fixture_pack();
    let oid = ObjectId::from_hex("98330ec338a352a9c88af3f844f26e9c1f0b1ce0").unwrap();

    c.bench_function("read_delta_object", |b| {
        b.iter(|| {
            pack.read_object(&oid).unwrap();
        });
    });
}

fn bench_read_all_objects(c: &mut Criterion) {
    let pack = fixture_pack();

    c.bench_function("read_all_9_objects", |b| {
        b.iter(|| {
            for result in pack.iter() {
                result.unwrap();
            }
        });
    });
}

fn bench_verify_checksum(c: &mut Criterion) {
    let pack = fixture_pack();

    c.bench_function("verify_checksum", |b| {
        b.iter(|| {
            pack.verify_checksum().unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_index_lookup,
    bench_read_blob,
    bench_read_delta_object,
    bench_read_all_objects,
    bench_verify_checksum,
);
criterion_main!(benches);
