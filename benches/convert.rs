use criterion::{criterion_group, criterion_main, Criterion};
use exportbranch::convert_file::{convert_buffer, convert_stream};
use std::hint::black_box;

fn make_fixture(target_size: usize) -> Vec<u8> {
    // Mix CP850 bytes that hit the lookup table, multibyte patterns that
    // exercise the dispatch, plain ASCII, and CRLFs to keep the workload
    // representative of real Harbour source.
    const SEED: &[u8] =
        b"static function teste()\r\n   local cTexto := chr(251) + chr(30) + 'O\xa0l\xa0'\r\n   ";
    let mut buf = Vec::with_capacity(target_size + SEED.len());
    while buf.len() < target_size {
        buf.extend_from_slice(SEED);
    }
    buf.truncate(target_size);
    buf
}

fn bench_convert_buffer(c: &mut Criterion) {
    let mut group = c.benchmark_group("convert_buffer");
    for &size in &[4 * 1024, 64 * 1024, 1024 * 1024] {
        let input = make_fixture(size);
        group.throughput(criterion::Throughput::Bytes(input.len() as u64));
        group.bench_function(format!("{size}B"), |b| {
            b.iter(|| convert_buffer(black_box(&input)));
        });
    }
    group.finish();
}

fn bench_convert_stream(c: &mut Criterion) {
    let input = make_fixture(1024 * 1024);
    let mut group = c.benchmark_group("convert_stream");
    group.throughput(criterion::Throughput::Bytes(input.len() as u64));
    for &chunk in &[8 * 1024, 64 * 1024, 256 * 1024] {
        group.bench_function(format!("chunk={chunk}"), |b| {
            b.iter(|| {
                let mut reader = std::io::Cursor::new(black_box(&input));
                let mut writer: Vec<u8> = Vec::with_capacity(input.len());
                convert_stream(&mut reader, &mut writer, chunk).unwrap();
                writer
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_convert_buffer, bench_convert_stream);
criterion_main!(benches);
