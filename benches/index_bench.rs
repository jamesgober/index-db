//! Criterion benchmarks for the B+tree hot paths: point lookup and insert.
//!
//! Run with `cargo bench`. Lookups are measured against a pre-built tree so the
//! cost reflects pure traversal; inserts are measured by building a tree from a
//! fixed key sequence so the split path is exercised.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use index_db::BPlusTree;

/// Build a tree holding `0..n` mapped to themselves.
fn build(n: u64) -> BPlusTree<u64, u64> {
    let mut tree = BPlusTree::new();
    for k in 0..n {
        let _previous = tree.insert(k, k);
    }
    tree
}

/// Point lookups on a pre-built tree, across a range of tree sizes.
fn bench_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("get");
    for &n in &[1_000_u64, 100_000, 1_000_000] {
        let tree = build(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            let mut k = 0_u64;
            b.iter(|| {
                // Sweep keys so the access pattern is not a single hot path.
                k = (k + 2_654_435_761) % n;
                black_box(tree.get(black_box(&k)))
            });
        });
    }
    group.finish();
}

/// Building a tree from scratch, which drives the insert-and-split path.
fn bench_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_build");
    for &n in &[1_000_u64, 100_000] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| black_box(build(black_box(n))));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_get, bench_insert);
criterion_main!(benches);
