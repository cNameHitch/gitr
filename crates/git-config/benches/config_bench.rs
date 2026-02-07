use bstr::BStr;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use git_config::{ConfigFile, ConfigKey, ConfigScope, ConfigSet};

const TYPICAL_CONFIG: &[u8] = b"\
[core]
\trepositoryformatversion = 0
\tfilemode = true
\tbare = false
\tlogallrefupdates = true
\tignorecase = true
\tprecomposeunicode = true
[remote \"origin\"]
\turl = https://github.com/user/repo.git
\tfetch = +refs/heads/*:refs/remotes/origin/*
[branch \"main\"]
\tremote = origin
\tmerge = refs/heads/main
[user]
\tname = Alice
\temail = alice@example.com
[push]
\tdefault = simple
\tfollowTags = true
[color]
\tui = auto
[diff]
\talgorithm = histogram
";

fn bench_parse(c: &mut Criterion) {
    c.bench_function("parse_typical_config", |b| {
        b.iter(|| {
            let file =
                ConfigFile::parse(black_box(TYPICAL_CONFIG), None, ConfigScope::Local).unwrap();
            black_box(file);
        });
    });
}

fn bench_lookup(c: &mut Criterion) {
    let file = ConfigFile::parse(TYPICAL_CONFIG, None, ConfigScope::Local).unwrap();
    let key = ConfigKey::parse("user.name").unwrap();

    c.bench_function("lookup_key", |b| {
        b.iter(|| {
            let val = file.get(black_box(&key));
            black_box(val);
        });
    });
}

fn bench_typed_bool(c: &mut Criterion) {
    c.bench_function("parse_bool", |b| {
        b.iter(|| {
            let val = git_config::parse_bool(Some(black_box(BStr::new("true"))));
            black_box(val);
        });
    });
}

fn bench_typed_int(c: &mut Criterion) {
    c.bench_function("parse_int_with_suffix", |b| {
        b.iter(|| {
            let val = git_config::parse_int(black_box(BStr::new("512m")));
            black_box(val);
        });
    });
}

fn bench_config_set_lookup(c: &mut Criterion) {
    let mut set = ConfigSet::new();
    set.add_file(
        ConfigFile::parse(
            b"[user]\n\tname = Global\n",
            None,
            ConfigScope::Global,
        )
        .unwrap(),
    );
    set.add_file(
        ConfigFile::parse(TYPICAL_CONFIG, None, ConfigScope::Local).unwrap(),
    );

    c.bench_function("config_set_get_string", |b| {
        b.iter(|| {
            let val = set.get_string(black_box("user.name"));
            black_box(val);
        });
    });
}

criterion_group!(
    benches,
    bench_parse,
    bench_lookup,
    bench_typed_bool,
    bench_typed_int,
    bench_config_set_lookup
);
criterion_main!(benches);
