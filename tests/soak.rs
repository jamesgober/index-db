//! Soak test: a long, consumer-shaped workload, the way a storage engine would
//! drive a secondary index over its lifetime.
//!
//! A read-heavy mix of point lookups, range scans, inserts, and deletes runs for
//! a couple hundred thousand operations over an evolving working set, with every
//! operation's result checked against a `BTreeMap` reference and the whole map
//! compared in full at regular checkpoints. The operation stream is driven by a
//! fixed PRNG, so the run is deterministic and reproducible — no random seed, no
//! `rand` dependency.

#![allow(clippy::unwrap_used, reason = "test assertions")]

use std::collections::BTreeMap;

use index_db::BPlusTree;

/// A small, fast, deterministic PRNG (SplitMix64). Reproducible across runs and
/// platforms, which keeps the soak a fixed workload rather than a flaky one.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `0..n`.
    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}

#[test]
fn soak_consumer_workload() {
    const KEY_SPACE: u64 = 5_000;
    const OPS: u64 = 200_000;
    const CHECKPOINT: u64 = 20_000;

    let mut tree: BPlusTree<u64, u64> = BPlusTree::new();
    let mut reference: BTreeMap<u64, u64> = BTreeMap::new();
    let mut rng = Rng(0x1234_5678_9ABC_DEF0);
    let mut generation = 0_u64;

    for op in 0..OPS {
        let key = rng.below(KEY_SPACE);
        // Read-heavy mix: ~70% lookups, ~12% range scans, ~10% inserts,
        // ~8% deletes — the shape a query-serving index sees.
        match rng.below(100) {
            0..=69 => {
                assert_eq!(tree.get(&key), reference.get(&key), "get({key}) at op {op}");
            }
            70..=81 => {
                let width = rng.below(64) + 1;
                let hi = (key + width).min(KEY_SPACE);
                let got: Vec<_> = tree.range(key..hi).map(|(&k, &v)| (k, v)).collect();
                let want: Vec<_> = reference.range(key..hi).map(|(&k, &v)| (k, v)).collect();
                assert_eq!(got, want, "range({key}..{hi}) at op {op}");
            }
            82..=91 => {
                generation += 1;
                assert_eq!(
                    tree.insert(key, generation),
                    reference.insert(key, generation),
                    "insert({key}) at op {op}"
                );
            }
            _ => {
                assert_eq!(
                    tree.remove(&key),
                    reference.remove(&key),
                    "remove({key}) at op {op}"
                );
            }
        }

        assert_eq!(tree.len(), reference.len(), "len diverged at op {op}");

        if op % CHECKPOINT == CHECKPOINT - 1 {
            // Full agreement: forward scan, reverse scan, and height sanity.
            let fwd: Vec<_> = tree.iter().map(|(&k, &v)| (k, v)).collect();
            let want: Vec<_> = reference.iter().map(|(&k, &v)| (k, v)).collect();
            assert_eq!(fwd, want, "forward scan diverged at checkpoint {op}");

            let rev: Vec<_> = tree.iter().rev().map(|(&k, &v)| (k, v)).collect();
            let want_rev: Vec<_> = reference.iter().rev().map(|(&k, &v)| (k, v)).collect();
            assert_eq!(rev, want_rev, "reverse scan diverged at checkpoint {op}");
        }
    }

    // Drain the tree completely and confirm it returns to an empty single leaf.
    let remaining: Vec<u64> = tree.iter().map(|(&k, _)| k).collect();
    for k in remaining {
        assert_eq!(tree.remove(&k), reference.remove(&k));
    }
    assert!(tree.is_empty());
    assert!(reference.is_empty());
    assert_eq!(tree.height(), 1);
}
