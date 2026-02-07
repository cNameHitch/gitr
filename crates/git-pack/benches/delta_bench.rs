use criterion::{criterion_group, criterion_main, Criterion};
use git_pack::delta::{apply::apply_delta, compute::compute_delta};

fn bench_delta_apply(c: &mut Criterion) {
    let source: Vec<u8> = (0..4096).map(|i| (i % 256) as u8).collect();
    let mut target = source.clone();
    target[2048] = 0xFF;
    target[2049] = 0xFE;

    let delta = compute_delta(&source, &target);

    c.bench_function("delta_apply_4k", |b| {
        b.iter(|| {
            apply_delta(&source, &delta).unwrap();
        });
    });
}

fn bench_delta_compute(c: &mut Criterion) {
    let source: Vec<u8> = (0..4096).map(|i| (i % 256) as u8).collect();
    let mut target = source.clone();
    target[2048] = 0xFF;
    target[2049] = 0xFE;

    c.bench_function("delta_compute_4k", |b| {
        b.iter(|| {
            compute_delta(&source, &target);
        });
    });
}

fn bench_delta_apply_large(c: &mut Criterion) {
    let source: Vec<u8> = (0..65536).map(|i| (i % 256) as u8).collect();
    let mut target = source.clone();
    for i in (0..target.len()).step_by(1024) {
        target[i] = 0xFF;
    }

    let delta = compute_delta(&source, &target);

    c.bench_function("delta_apply_64k", |b| {
        b.iter(|| {
            apply_delta(&source, &delta).unwrap();
        });
    });
}

criterion_group!(benches, bench_delta_apply, bench_delta_compute, bench_delta_apply_large);
criterion_main!(benches);
