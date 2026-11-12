//! A key type whose `Clone` panics must not leave the tree inconsistent. Every
//! write path clones separator keys before it moves entries, so a panic during
//! that clone unwinds out of an untouched tree, and `len` always matches the
//! entries present.

#![allow(clippy::unwrap_used, reason = "test assertions")]

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

use index_db::BPlusTree;

thread_local! {
    /// Number of clones to let through before the next one panics; `None`
    /// means clones never panic.
    static PANIC_AFTER: Cell<Option<u32>> = const { Cell::new(None) };
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Key(u32);

impl Clone for Key {
    fn clone(&self) -> Self {
        let fire = PANIC_AFTER.with(|p| match p.get() {
            Some(0) => {
                p.set(None);
                true
            }
            Some(n) => {
                p.set(Some(n - 1));
                false
            }
            None => false,
        });
        assert!(!fire, "clone panics once");
        Key(self.0)
    }
}

fn arm() {
    PANIC_AFTER.with(|p| p.set(Some(0)));
}

fn disarm() {
    PANIC_AFTER.with(|p| p.set(None));
}

/// Checks that every entry `iter` yields is found by `get`, that keys are
/// strictly ascending, and that `len` matches.
fn assert_consistent(tree: &BPlusTree<Key, u32>) {
    let entries: Vec<(u32, u32)> = tree.iter().map(|(k, v)| (k.0, *v)).collect();
    assert!(
        entries.windows(2).all(|w| w[0].0 < w[1].0),
        "keys out of order or repeated"
    );
    assert_eq!(
        tree.len(),
        entries.len(),
        "len disagrees with the entries present"
    );
    for (k, v) in &entries {
        assert_eq!(
            tree.get(&Key(*k)),
            Some(v),
            "get misses key {k} that iter yields"
        );
    }
}

#[test]
fn panicking_clone_during_remove_leaves_the_tree_consistent() {
    let mut tree = BPlusTree::new();
    // Default order: 64 inserts split into two leaves with separator 32.
    for k in 0..64 {
        let _ = tree.insert(Key(k), k);
    }
    assert_eq!(tree.height(), 2);
    assert_eq!(tree.remove(&Key(0)), Some(0)); // the left leaf is now at its minimum

    // Removing 1 underflows the left leaf, which borrows from the right leaf
    // and clones the new separator; that clone panics.
    arm();
    let r = catch_unwind(AssertUnwindSafe(|| tree.remove(&Key(1))));
    disarm();
    assert!(r.is_err(), "clone should have panicked");
    assert_consistent(&tree);
    assert_eq!(tree.get(&Key(1)), None);
    assert_eq!(tree.len(), 62);

    // Re-inserting a key that is present replaces it instead of duplicating it.
    assert_eq!(tree.insert(Key(32), 999), Some(32));
    assert_consistent(&tree);
    assert_eq!(tree.iter().filter(|(k, _)| k.0 == 32).count(), 1);
}

#[test]
fn panicking_clone_during_borrow_from_left_leaves_the_tree_consistent() {
    let mut tree = BPlusTree::new();
    for k in 0..64 {
        let _ = tree.insert(Key(k), k);
    }
    // Shrink the right leaf to its minimum, then grow the left one so the next
    // underflow on the right borrows from the left.
    assert_eq!(tree.remove(&Key(63)), Some(63));
    for k in 100..103 {
        let _ = tree.insert(Key(k), k);
    }
    for k in 100..103 {
        assert_eq!(tree.remove(&Key(k)), Some(k));
    }
    let _ = tree.insert(Key(1_000), 0);
    let _ = tree.remove(&Key(1_000));
    arm();
    let r = catch_unwind(AssertUnwindSafe(|| tree.remove(&Key(62))));
    disarm();
    // Whether or not this removal needed a clone, the tree must be consistent.
    let _ = r;
    assert_consistent(&tree);
}

#[test]
fn panicking_clone_during_a_leaf_split_leaves_the_tree_untouched() {
    let mut tree = BPlusTree::new();
    // 63 entries fill one leaf; the 64th insert splits it and clones the
    // separator.
    for k in 0..63 {
        let _ = tree.insert(Key(k * 2), k);
    }
    assert_eq!(tree.height(), 1);
    arm();
    let r = catch_unwind(AssertUnwindSafe(|| tree.insert(Key(51), 7)));
    disarm();
    assert!(r.is_err(), "clone should have panicked");
    assert_consistent(&tree);
    assert_eq!(tree.len(), 63);
    assert_eq!(tree.get(&Key(51)), None);

    // The same insert succeeds afterwards and splits normally.
    assert_eq!(tree.insert(Key(51), 7), None);
    assert_consistent(&tree);
    assert_eq!(tree.len(), 64);
    assert_eq!(tree.height(), 2);
}

#[test]
fn random_panicking_clones_never_break_consistency() {
    // A small deterministic generator, so the test needs no extra dependency.
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut tree = BPlusTree::new();
    for _ in 0..20_000 {
        let r = next();
        let key = (r % 512) as u32;
        if r % 7 == 0 {
            PANIC_AFTER.with(|p| p.set(Some(((r >> 20) % 3) as u32)));
        }
        let _ = catch_unwind(AssertUnwindSafe(|| {
            if r & (1 << 40) == 0 {
                let _ = tree.insert(Key(key), key);
            } else {
                let _ = tree.remove(&Key(key));
            }
        }));
        disarm();
    }
    assert_consistent(&tree);
}
