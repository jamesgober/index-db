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
}
