//! Integration tests for the public [`BPlusTree`] surface.
//!
//! These exercise the crate the way a consumer would — only public API — and
//! cross-check behaviour against the standard library's `BTreeMap`, which is a
//! known-correct ordered map.

#![allow(clippy::unwrap_used, reason = "test assertions")]

use std::collections::BTreeMap;

use index_db::BPlusTree;
use proptest::prelude::*;

#[test]
fn empty_tree_has_no_entries() {
    let tree: BPlusTree<u64, u64> = BPlusTree::new();
    assert!(tree.is_empty());
    assert_eq!(tree.len(), 0);
    assert_eq!(tree.get(&0), None);
    assert!(!tree.contains_key(&0));
    assert_eq!(tree.height(), 1);
}

#[test]
fn default_matches_new() {
    let tree: BPlusTree<u64, u64> = BPlusTree::default();
    assert!(tree.is_empty());
}

#[test]
fn insert_returns_previous_value_on_overwrite() {
    let mut tree = BPlusTree::new();
    assert_eq!(tree.insert("alpha", 1), None);
    assert_eq!(tree.insert("alpha", 2), Some(1));
    assert_eq!(tree.get(&"alpha"), Some(&2));
    assert_eq!(tree.len(), 1);
}

#[test]
fn lookups_find_every_inserted_key() {
    let mut tree = BPlusTree::new();
    for k in 0..10_000_u32 {
        assert_eq!(tree.insert(k, k.wrapping_mul(3)), None);
    }
    assert_eq!(tree.len(), 10_000);
    for k in 0..10_000_u32 {
        assert_eq!(tree.get(&k), Some(&k.wrapping_mul(3)));
    }
    assert_eq!(tree.get(&10_000), None);
    // A million entries would still be only a handful of levels; ten thousand
    // is enough to prove the tree grew past a single leaf.
    assert!(tree.height() >= 2);
}

#[test]
fn clear_empties_the_tree() {
    let mut tree = BPlusTree::new();
    for k in 0..1_000_u32 {
        let _previous = tree.insert(k, k);
    }
    tree.clear();
    assert!(tree.is_empty());
    assert_eq!(tree.get(&0), None);
    assert_eq!(tree.height(), 1);
    // The tree is usable again after clearing.
    assert_eq!(tree.insert(5, 5), None);
    assert_eq!(tree.get(&5), Some(&5));
}

#[test]
fn string_keys_order_correctly() {
    let mut tree = BPlusTree::new();
    let words = ["pear", "apple", "fig", "cherry", "date", "banana"];
    for (i, w) in words.iter().enumerate() {
        assert_eq!(tree.insert(w.to_string(), i), None);
    }
    assert_eq!(tree.get(&"cherry".to_string()), Some(&3));
    assert_eq!(tree.get(&"kiwi".to_string()), None);
    assert_eq!(tree.len(), words.len());
}

#[test]
fn remove_returns_value_and_shrinks() {
    let mut tree = BPlusTree::new();
    for k in 0..1_000_u32 {
        let _previous = tree.insert(k, k * 2);
    }
    for k in 0..1_000_u32 {
        assert_eq!(tree.remove(&k), Some(k * 2));
    }
    assert!(tree.is_empty());
    assert_eq!(tree.remove(&0), None);
    assert_eq!(tree.height(), 1);
}

#[test]
fn iterate_in_order() {
    let mut tree = BPlusTree::new();
    for k in [40_u32, 10, 30, 20, 50] {
        let _previous = tree.insert(k, k);
    }
    let keys: Vec<_> = tree.iter().map(|(&k, _)| k).collect();
    assert_eq!(keys, vec![10, 20, 30, 40, 50]);

    // `&tree` works as an IntoIterator.
    let via_into: Vec<_> = (&tree).into_iter().map(|(&k, _)| k).collect();
    assert_eq!(via_into, keys);

    let reversed: Vec<_> = tree.iter().rev().map(|(&k, _)| k).collect();
    assert_eq!(reversed, vec![50, 40, 30, 20, 10]);
}

#[test]
fn range_scan_matches_expected() {
    let mut tree = BPlusTree::new();
    for k in 0..100_u32 {
        let _previous = tree.insert(k, k);
    }
    let window: Vec<_> = tree.range(20..30).map(|(&k, _)| k).collect();
    assert_eq!(window, (20..30).collect::<Vec<_>>());

    let inclusive: Vec<_> = tree.range(95..=99).map(|(&k, _)| k).collect();
    assert_eq!(inclusive, vec![95, 96, 97, 98, 99]);

    let empty: Vec<_> = tree.range(200..300).map(|(&k, _)| k).collect();
    assert!(empty.is_empty());
}

proptest! {
    /// Against an arbitrary mix of inserts, the tree answers every lookup
    /// exactly as `BTreeMap` does, and reports the same length.
    #[test]
    fn behaves_like_btreemap(entries in prop::collection::vec((0_u64..500, any::<u64>()), 0..600)) {
        let mut tree = BPlusTree::new();
        let mut reference = BTreeMap::new();

        for (k, v) in entries {
            prop_assert_eq!(tree.insert(k, v), reference.insert(k, v));
        }

        prop_assert_eq!(tree.len(), reference.len());
        for k in 0_u64..500 {
            prop_assert_eq!(tree.get(&k), reference.get(&k));
            prop_assert_eq!(tree.contains_key(&k), reference.contains_key(&k));
        }
    }

    /// A full insert / remove / scan workload tracks `BTreeMap` end to end.
    #[test]
    fn full_workload_matches_btreemap(
        inserts in prop::collection::vec(0_u64..300, 0..400),
        removes in prop::collection::vec(0_u64..300, 0..200),
    ) {
        let mut tree = BPlusTree::new();
        let mut reference = BTreeMap::new();

        for k in inserts {
            prop_assert_eq!(tree.insert(k, k), reference.insert(k, k));
        }
        for k in removes {
            prop_assert_eq!(tree.remove(&k), reference.remove(&k));
        }

        prop_assert_eq!(tree.len(), reference.len());
        let tree_entries: Vec<_> = tree.iter().map(|(&k, &v)| (k, v)).collect();
        let ref_entries: Vec<_> = reference.iter().map(|(&k, &v)| (k, v)).collect();
        prop_assert_eq!(tree_entries, ref_entries);
    }
}
