//! Micro-benchmarks of the shared codecs (T01.15, STRATEGIE_DE_TESTS.md
//! section 9.3). Run nightly by bench/micro.sh; `cargo bench -- --test` runs
//! each once in CI so they keep compiling and working.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use ndaal_common::base32::{decode_crockford, encode_crockford};
use ndaal_common::bytes::{get_varint, put_varint};

/// Values per iteration: enough to cover every varint length.
const VALUES: usize = 1024;

/// Fixed values spread over all varint lengths (1 to 10 bytes), from a
/// deterministic sequence: the same input every night.
fn values() -> Vec<u64> {
    (0..VALUES as u64)
        .map(|i| i.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (i % 64))
        .collect()
}

fn varint(c: &mut Criterion) {
    let values = values();
    let mut encoded = Vec::new();
    for v in &values {
        put_varint(&mut encoded, *v);
    }
    c.bench_function("varint/encode", |b| {
        let mut out = Vec::with_capacity(encoded.len());
        b.iter(|| {
            out.clear();
            for v in &values {
                put_varint(&mut out, black_box(*v));
            }
            black_box(&out);
        });
    });
    c.bench_function("varint/decode", |b| {
        b.iter(|| {
            let mut input = black_box(encoded.as_slice());
            let mut sum = 0u64;
            while !input.is_empty() {
                sum = sum.wrapping_add(get_varint(&mut input).expect("encoded above"));
            }
            sum
        });
    });
}

fn crockford(c: &mut Criterion) {
    let ids: Vec<String> = values()
        .iter()
        .map(|v| {
            let mut bytes = [0u8; 16];
            bytes[..8].copy_from_slice(&v.to_be_bytes());
            bytes[8..].copy_from_slice(&v.rotate_left(17).to_be_bytes());
            encode_crockford(&bytes)
        })
        .collect();
    c.bench_function("crockford/decode", |b| {
        b.iter(|| {
            for id in &ids {
                black_box(decode_crockford(black_box(id)).expect("encoded above"));
            }
        });
    });
}

criterion_group!(benches, varint, crockford);
criterion_main!(benches);
