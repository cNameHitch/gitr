use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use git_hash::hasher::Hasher;
use git_hash::hex::{hex_decode, hex_to_string};
use git_hash::{HashAlgorithm, ObjectId};

fn hash_throughput(c: &mut Criterion) {
    let data = vec![0xABu8; 1024 * 1024]; // 1 MiB

    let mut group = c.benchmark_group("hash_throughput");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("sha1_1mib", |b| {
        b.iter(|| Hasher::digest(black_box(HashAlgorithm::Sha1), black_box(&data)))
    });

    group.bench_function("sha256_1mib", |b| {
        b.iter(|| Hasher::digest(black_box(HashAlgorithm::Sha256), black_box(&data)))
    });

    group.finish();
}

fn hex_encode_decode(c: &mut Criterion) {
    let bytes = [0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB,
                 0xCD, 0xEF, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
    let hex = hex_to_string(&bytes);

    let mut group = c.benchmark_group("hex");

    group.bench_function("encode_20bytes", |b| {
        b.iter(|| hex_to_string(black_box(&bytes)))
    });

    group.bench_function("decode_40chars", |b| {
        b.iter(|| {
            let mut buf = [0u8; 20];
            hex_decode(black_box(&hex), &mut buf).unwrap();
            buf
        })
    });

    group.finish();
}

fn oid_comparison(c: &mut Criterion) {
    let a = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
    let b = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80700").unwrap();

    let mut group = c.benchmark_group("oid");

    group.bench_function("eq", |b_iter| {
        b_iter.iter(|| black_box(&a) == black_box(&b))
    });

    group.bench_function("cmp", |b_iter| {
        b_iter.iter(|| black_box(&a).cmp(black_box(&b)))
    });

    group.bench_function("hash_object_blob", |b_iter| {
        let data = b"hello world";
        b_iter.iter(|| {
            Hasher::hash_object(
                black_box(HashAlgorithm::Sha1),
                black_box("blob"),
                black_box(data),
            )
        })
    });

    group.finish();
}

criterion_group!(benches, hash_throughput, hex_encode_decode, oid_comparison);
criterion_main!(benches);
