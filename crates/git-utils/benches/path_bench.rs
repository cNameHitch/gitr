use bstr::BStr;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use git_utils::path::GitPath;
use git_utils::wildmatch::{wildmatch, WildmatchFlags};

fn bench_path_normalize(c: &mut Criterion) {
    c.bench_function("path_normalize_simple", |b| {
        b.iter(|| {
            let p = GitPath::new(black_box("src/utils/../path.rs"));
            black_box(p.normalize())
        })
    });

    c.bench_function("path_normalize_deep", |b| {
        b.iter(|| {
            let p = GitPath::new(black_box("a/b/c/d/../../e/f/../g/h/../../i"));
            black_box(p.normalize())
        })
    });
}

fn bench_path_join(c: &mut Criterion) {
    c.bench_function("path_join", |b| {
        b.iter(|| {
            let p = GitPath::new(black_box("src/utils"));
            black_box(p.join(black_box("path.rs")))
        })
    });
}

fn bench_path_relative_to(c: &mut Criterion) {
    c.bench_function("path_relative_to", |b| {
        let prefix = GitPath::new("/home/user/project");
        b.iter(|| {
            let p = GitPath::new(black_box("/home/user/project/src/main.rs"));
            black_box(p.relative_to(&prefix))
        })
    });
}

fn bench_wildmatch(c: &mut Criterion) {
    c.bench_function("wildmatch_simple_star", |b| {
        let pattern = BStr::new(b"*.rs");
        let text = BStr::new(b"src/main.rs");
        b.iter(|| wildmatch(black_box(pattern), black_box(text), WildmatchFlags::empty()))
    });

    c.bench_function("wildmatch_doublestar", |b| {
        let pattern = BStr::new(b"**/test_*.rs");
        let text = BStr::new(b"src/tests/unit/test_parser.rs");
        b.iter(|| wildmatch(black_box(pattern), black_box(text), WildmatchFlags::PATHNAME))
    });

    c.bench_function("wildmatch_character_class", |b| {
        let pattern = BStr::new(b"[a-z]*.[ch]");
        let text = BStr::new(b"wildmatch.c");
        b.iter(|| wildmatch(black_box(pattern), black_box(text), WildmatchFlags::empty()))
    });
}

criterion_group!(
    benches,
    bench_path_normalize,
    bench_path_join,
    bench_path_relative_to,
    bench_wildmatch
);
criterion_main!(benches);
