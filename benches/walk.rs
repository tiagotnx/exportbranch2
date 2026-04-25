use criterion::{criterion_group, criterion_main, Criterion};
use std::fs;
use std::hint::black_box;
use std::path::Path;
use tempfile::TempDir;

fn build_tree(root: &Path, dirs: usize, files_per_dir: usize) {
    const BODY: &[u8] =
        b"static function teste()\r\n   local cTexto := chr(251) + chr(30)\r\nreturn nil\r\n";
    for d in 0..dirs {
        let sub = root.join(format!("dir{d}"));
        fs::create_dir_all(&sub).unwrap();
        for f in 0..files_per_dir {
            fs::write(sub.join(format!("file{f}.prg")), BODY).unwrap();
        }
    }
}

fn run_export(src: &Path, dst: &Path) {
    let args: Vec<String> = vec![
        "exportbranch".into(),
        "-s".into(),
        src.display().to_string(),
        "-d".into(),
        dst.display().to_string(),
    ];
    exportbranch::run(args).unwrap();
}

fn bench_walk_cold(c: &mut Criterion) {
    let mut group = c.benchmark_group("walk_cold");
    group.sample_size(10);
    for &(dirs, per) in &[(10usize, 50usize), (50, 100)] {
        let total = dirs * per;
        group.bench_function(format!("{dirs}x{per}={total}"), |b| {
            b.iter_with_setup(
                || {
                    let src = TempDir::new().unwrap();
                    let dst = TempDir::new().unwrap();
                    build_tree(src.path(), dirs, per);
                    (src, dst)
                },
                |(src, dst)| {
                    run_export(black_box(src.path()), black_box(dst.path()));
                },
            );
        });
    }
    group.finish();
}

fn bench_walk_warm(c: &mut Criterion) {
    // Hot path: every file is already up-to-date, so the walk just stats and
    // skips. This stresses the syscall/HashMap lookup cost (sugestões 1 + 2).
    let mut group = c.benchmark_group("walk_warm");
    group.sample_size(10);
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    build_tree(src.path(), 50, 100);
    run_export(src.path(), dst.path()); // warm up: populate metadata file

    group.bench_function("50x100=5000", |b| {
        b.iter(|| run_export(black_box(src.path()), black_box(dst.path())));
    });
    group.finish();
}

criterion_group!(benches, bench_walk_cold, bench_walk_warm);
criterion_main!(benches);
