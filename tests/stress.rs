//! Stress tests: large-scale and adversarial workloads cross-checked against
//! `BTreeMap`, plus a concurrent-reader test.
//!
//! These exercise only the public API. They run at sizes large enough to drive
//! many splits, merges, and a tall tree, but small enough to stay quick.

#![allow(clippy::unwrap_used, reason = "test assertions")]

use std::collections::BTreeMap;
use std::thread;

use index_db::BPlusTree;

/// A cheap deterministic permutation of `0..n`, so inserts and deletes arrive in
/// a scattered order without a random-number dependency.
fn scattered(n: u64) -> Vec<u64> {
    let mut out: Vec<u64> = (0..n).collect();
    // Fisher-Yates with a fixed LCG, so the order is fixed but not sorted.
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    for i in (1..out.len()).rev() {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let j = (state >> 33) as usize % (i + 1);
        out.swap(i, j);
    }
    out
}

#[test]
fn large_scattered_insert_then_delete() {
    const N: u64 = 50_000;
    let mut tree = BPlusTree::new();
    let mut reference = BTreeMap::new();

    for &k in &scattered(N) {
        let _ = tree.insert(k, k.wrapping_mul(3));
        let _ = reference.insert(k, k.wrapping_mul(3));
    }
    assert_eq!(tree.len(), reference.len());
    assert!(tree.height() >= 3);

    // Every key reads back, and a full scan matches the reference in order.
    for k in 0..N {
        assert_eq!(tree.get(&k), Some(&k.wrapping_mul(3)));
    }
    let scan: Vec<_> = tree.iter().map(|(&k, &v)| (k, v)).collect();
    let want: Vec<_> = reference.iter().map(|(&k, &v)| (k, v)).collect();
    assert_eq!(scan, want);

    // Delete half (scattered), confirm the survivors, then delete the rest.
    let order = scattered(N);
    for &k in order.iter().take((N / 2) as usize) {
        assert_eq!(tree.remove(&k), Some(k.wrapping_mul(3)));
        let _ = reference.remove(&k);
    }
    assert_eq!(tree.len(), reference.len());
    let scan: Vec<_> = tree.iter().map(|(&k, _)| k).collect();
    let want: Vec<_> = reference.keys().copied().collect();
    assert_eq!(scan, want);

    for &k in order.iter().skip((N / 2) as usize) {
        let _ = tree.remove(&k);
    }
    assert!(tree.is_empty());
    assert_eq!(tree.height(), 1);
}

#[test]
fn sustained_insert_delete_churn() {
    // Hold a bounded key space and churn it: alternating waves of scattered
    // inserts and deletes, with the whole map verified against the reference
    // after each wave.
    const SPACE: u64 = 4_000;
    let mut tree = BPlusTree::new();
    let mut reference = BTreeMap::new();
    let order = scattered(SPACE);

    for wave in 0..12_u64 {
        // Insert a rotating slice of the space.
        let start = (wave * 503) % SPACE;
        for i in 0..1_500 {
            let k = order[((start + i) % SPACE) as usize];
            let _ = tree.insert(k, wave);
            let _ = reference.insert(k, wave);
        }
        // Delete a different rotating slice.
        let dstart = (wave * 911) % SPACE;
        for i in 0..900 {
            let k = order[((dstart + i) % SPACE) as usize];
            assert_eq!(tree.remove(&k), reference.remove(&k));
        }
        assert_eq!(tree.len(), reference.len());
        let scan: Vec<_> = tree.iter().map(|(&k, &v)| (k, v)).collect();
        let want: Vec<_> = reference.iter().map(|(&k, &v)| (k, v)).collect();
        assert_eq!(scan, want, "diverged after wave {wave}");
    }
}

#[test]
fn bulk_load_then_heavy_delete() {
    const N: u64 = 30_000;
    let mut tree = BPlusTree::from_sorted((0..N).map(|k| (k, k)));
    let mut reference: BTreeMap<u64, u64> = (0..N).map(|k| (k, k)).collect();
    assert_eq!(tree.len(), reference.len());

    // Delete every third key (scattered across the tree), then verify.
    for &k in scattered(N).iter().filter(|&&k| k % 3 == 0) {
        assert_eq!(tree.remove(&k), reference.remove(&k));
    }
    assert_eq!(tree.len(), reference.len());
    for k in 0..N {
        assert_eq!(tree.get(&k), reference.get(&k));
    }
}

#[test]
fn adversarial_string_keys() {
    // Keys sharing long common prefixes and varying in length, a pattern that
    // stresses comparison-heavy nodes.
    let mut tree = BPlusTree::new();
    let mut reference = BTreeMap::new();
    for i in 0..3_000_u32 {
        let key = format!("prefix/{:08}/{}", i % 200, "x".repeat((i % 17) as usize));
        let _ = tree.insert(key.clone(), i);
        let _ = reference.insert(key, i);
    }
    assert_eq!(tree.len(), reference.len());
    let scan: Vec<_> = tree.iter().map(|(k, &v)| (k.clone(), v)).collect();
    let want: Vec<_> = reference.iter().map(|(k, &v)| (k.clone(), v)).collect();
    assert_eq!(scan, want);
}

#[test]
fn concurrent_readers_traverse_a_shared_tree() {
    // The in-memory tree is `Sync`, so many threads can read it at once without
    // any locking: `get`, `range`, and `iter` all take `&self`. This test would
    // not compile if `BPlusTree<u64, u64>` were not `Sync`.
    const N: u64 = 20_000;
    let tree = BPlusTree::from_sorted((0..N).map(|k| (k, k.wrapping_mul(7))));
    let full_sum: u128 = (0..N).map(|k| k.wrapping_mul(7) as u128).sum();

    thread::scope(|scope| {
        for t in 0..8_u64 {
            let tree = &tree;
            let _ = scope.spawn(move || {
                // Point lookups across the whole key space.
                for k in 0..N {
                    assert_eq!(tree.get(&k), Some(&k.wrapping_mul(7)));
                }
                // A full scan sums to the same total from every thread.
                let sum: u128 = tree.iter().map(|(_, &v)| v as u128).sum();
                assert_eq!(sum, full_sum, "thread {t} scan diverged");
                // A bounded range, forward and reverse, agree on their contents.
                let lo = (t * 1_000) % N;
                let hi = (lo + 2_500).min(N);
                let fwd: Vec<_> = tree.range(lo..hi).map(|(&k, _)| k).collect();
                let mut rev: Vec<_> = tree.range(lo..hi).rev().map(|(&k, _)| k).collect();
                rev.reverse();
                assert_eq!(fwd, rev);
            });
        }
    });
}
