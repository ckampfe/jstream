use std::io::Read;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use jstream::path_value_writer::json_pointer::Options as JSONPointerWriterOptions;
use jstream::path_value_writer::json_pointer::Writer as JSONPointerWriter;
use jstream::stream;

fn json_pointer_benchmark(c: &mut Criterion) {
    let mut larger_inputs_group = c.benchmark_group("larger inputs");

    larger_inputs_group.measurement_time(std::time::Duration::from_secs(20));

    let mut f = std::fs::File::open("fixtures/big.json").unwrap();
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).unwrap();

    larger_inputs_group.bench_function("jindex jsonpointer big.json", |b| {
        b.iter(|| {
            let mut writer = vec![];
            let options = JSONPointerWriterOptions::default();
            let mut sink = JSONPointerWriter::new(&mut writer, options);
            // jindex(&mut sink, black_box(&json)).unwrap()
            stream(black_box(&buf), &mut sink).unwrap();
        })
    });

    larger_inputs_group.finish();

    /////////////////////////////////////////////////

    let mut smaller_inputs_group = c.benchmark_group("smaller inputs");

    let mut f = std::fs::File::open("fixtures/github.json").unwrap();
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).unwrap();

    smaller_inputs_group.bench_function("jindex jsonpointer github.json", |b| {
        b.iter(|| {
            let mut writer = vec![];
            let options = JSONPointerWriterOptions::default();
            let mut sink = JSONPointerWriter::new(&mut writer, options);
            stream(black_box(&buf), &mut sink).unwrap()
        })
    });

    let mut f = std::fs::File::open("fixtures/three.json").unwrap();
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).unwrap();

    smaller_inputs_group.bench_function("jindex jsonpointer three.json", |b| {
        b.iter(|| {
            let mut writer = vec![];
            let options = JSONPointerWriterOptions::default();
            let mut sink = JSONPointerWriter::new(&mut writer, options);
            stream(black_box(&buf), &mut sink).unwrap()
        })
    });

    smaller_inputs_group.finish();
}

criterion_group!(benches, json_pointer_benchmark,);
criterion_main!(benches);
