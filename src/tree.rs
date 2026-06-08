//! The [`BPlusTree`] public type: an ordered map laid out as a B+tree.

use alloc::vec::Vec;
use core::ops::RangeBounds;

use crate::iter::Iter;
use crate::node::{Insert, Internal, Node};

/// Smallest fan-out a node may have. With fewer than three children a split
/// cannot leave both halves non-empty, so the tree could not stay balanced.
const MIN_ORDER: usize = 3;

/// Default fan-out: up to 64 children per node, so up to 63 keys. A binary
/// search over a node is then at most six comparisons, and a tree of a million
/// keys stands four levels tall.
const DEFAULT_ORDER: usize = 64;

/// An ordered map backed by a B+tree.
///
/// Keys are kept in sorted order across a tree of fixed-fan-out nodes. Point
/// operations — [`get`](BPlusTree::get), [`insert`](BPlusTree::insert),
/// [`contains_key`](BPlusTree::contains_key) — run in time logarithmic in the
/// number of entries: each level is one binary search over a node, and the
/// height grows with the logarithm of the entry count.
///
/// The structure is the same one storage engines use for an on-disk index, laid
/// out so each node maps onto a fixed-size page. This release keeps the tree in
/// memory; the node layout is the durable one a pager will later persist.
///
/// `K` must be [`Ord`]; [`insert`](BPlusTree::insert) additionally needs
/// [`Clone`], because splitting a leaf copies a separator key up into the parent.
///
/// # Examples
///
/// ```
/// use index_db::BPlusTree;
///
/// let mut index = BPlusTree::new();
/// index.insert(3_u32, "three");
/// index.insert(1, "one");
/// index.insert(2, "two");
///
/// assert_eq!(index.get(&2), Some(&"two"));
/// assert_eq!(index.len(), 3);
/// ```
pub struct BPlusTree<K, V> {
    /// Root of the tree. A fresh tree's root is an empty leaf.
    root: Node<K, V>,
    /// Maximum fan-out: the most children an internal node may hold, and one
    /// more than the most keys any node may hold.
    order: usize,
    /// Number of entries in the tree.
    len: usize,
}

impl<K, V> BPlusTree<K, V> {
    /// Create an empty tree with the default node fan-out.
    ///
    /// # Examples
    ///
    /// ```
    /// use index_db::BPlusTree;
    ///
    /// let index: BPlusTree<u32, &str> = BPlusTree::new();
    /// assert!(index.is_empty());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::with_order(DEFAULT_ORDER)
    }

    /// Create an empty tree with an explicit node fan-out, clamped up to the
    /// minimum a balanced tree requires. Used by the test suite to force splits
    /// at small key counts; the public surface fixes the fan-out via [`new`].
    ///
    /// [`new`]: BPlusTree::new
    #[must_use]
    pub(crate) fn with_order(order: usize) -> Self {
        BPlusTree {
            root: Node::empty_leaf(),
            order: if order < MIN_ORDER { MIN_ORDER } else { order },
            len: 0,
        }
    }

    /// The number of entries in the tree.
    ///
    /// # Examples
    ///
    /// ```
    /// use index_db::BPlusTree;
    ///
    /// let mut index = BPlusTree::new();
    /// index.insert("k", 1);
    /// assert_eq!(index.len(), 1);
    /// ```
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the tree holds no entries.
    ///
    /// # Examples
    ///
    /// ```
    /// use index_db::BPlusTree;
    ///
    /// let mut index = BPlusTree::new();
    /// assert!(index.is_empty());
    /// index.insert("k", 1);
    /// assert!(!index.is_empty());
    /// ```
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The height of the tree in levels: a tree whose root is a leaf has height
    /// one, and every level of internal nodes above the leaves adds one more.
    ///
    /// Because the tree is balanced, this is the number of nodes touched on any
    /// root-to-leaf path, and so the cost of a point lookup in node visits.
    ///
    /// # Examples
    ///
    /// ```
    /// use index_db::BPlusTree;
    ///
    /// let mut index = BPlusTree::new();
    /// assert_eq!(index.height(), 1); // just the root leaf
    /// for k in 0..1_000_u32 {
    ///     index.insert(k, k);
    /// }
    /// assert!(index.height() >= 2); // splits have grown the tree taller
    /// ```
    #[must_use]
    pub fn height(&self) -> usize {
        let mut height = 1;
        let mut node = &self.root;
        while let Node::Internal(internal) = node {
            height += 1;
            node = &internal.children[0];
        }
        height
    }

    /// Remove every entry, returning the tree to its empty state.
    ///
    /// # Examples
    ///
    /// ```
    /// use index_db::BPlusTree;
    ///
    /// let mut index = BPlusTree::new();
    /// index.insert(1_u32, "a");
    /// index.clear();
    /// assert!(index.is_empty());
    /// assert_eq!(index.get(&1), None);
    /// ```
    pub fn clear(&mut self) {
        self.root = Node::empty_leaf();
        self.len = 0;
    }

    /// An iterator over every entry, in ascending key order.
    ///
    /// The iterator is double-ended: call [`rev`](Iterator::rev) for descending
    /// order, or drive it from both ends.
    ///
    /// # Examples
    ///
    /// ```
    /// use index_db::BPlusTree;
    ///
    /// let mut index = BPlusTree::new();
    /// index.insert(2_u32, "b");
    /// index.insert(1, "a");
    /// index.insert(3, "c");
    ///
    /// let collected: Vec<_> = index.iter().map(|(&k, &v)| (k, v)).collect();
    /// assert_eq!(collected, vec![(1, "a"), (2, "b"), (3, "c")]);
    ///
    /// // Reverse with `.rev()`.
    /// let keys: Vec<_> = index.iter().rev().map(|(&k, _)| k).collect();
    /// assert_eq!(keys, vec![3, 2, 1]);
    /// ```
    #[must_use]
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter::full(&self.root)
    }
}

impl<K: Ord, V> BPlusTree<K, V> {
    /// Look up the value stored under `key`, or `None` if the key is absent.
    ///
    /// # Examples
    ///
    /// ```
    /// use index_db::BPlusTree;
    ///
    /// let mut index = BPlusTree::new();
    /// index.insert(10_u32, "ten");
    /// assert_eq!(index.get(&10), Some(&"ten"));
    /// assert_eq!(index.get(&11), None);
    /// ```
    #[must_use]
    #[inline]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.root.get(key)
    }

    /// Whether the tree holds an entry for `key`.
    ///
    /// # Examples
    ///
    /// ```
    /// use index_db::BPlusTree;
    ///
    /// let mut index = BPlusTree::new();
    /// index.insert(10_u32, "ten");
    /// assert!(index.contains_key(&10));
    /// assert!(!index.contains_key(&11));
    /// ```
    #[must_use]
    #[inline]
    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    /// An iterator over the entries whose keys fall in `range`, in ascending key
    /// order.
    ///
    /// `range` is any standard range expression — `a..b`, `a..=b`, `..b`, `a..`,
    /// or `..` — interpreted over the key order. Like [`iter`](Self::iter) the
    /// result is double-ended, so a range can be walked forward or in reverse
    /// with [`rev`](Iterator::rev).
    ///
    /// # Examples
    ///
    /// ```
    /// use index_db::BPlusTree;
    ///
    /// let mut index = BPlusTree::new();
    /// for k in 0..10_u32 {
    ///     index.insert(k, k);
    /// }
    ///
    /// // Half-open range [3, 7).
    /// let keys: Vec<_> = index.range(3..7).map(|(&k, _)| k).collect();
    /// assert_eq!(keys, vec![3, 4, 5, 6]);
    ///
    /// // Inclusive range, walked in reverse.
    /// let rev: Vec<_> = index.range(2..=4).rev().map(|(&k, _)| k).collect();
    /// assert_eq!(rev, vec![4, 3, 2]);
    ///
    /// // Open-ended range.
    /// let tail: Vec<_> = index.range(8..).map(|(&k, _)| k).collect();
    /// assert_eq!(tail, vec![8, 9]);
    /// ```
    #[must_use]
    pub fn range<R: RangeBounds<K>>(&self, range: R) -> Iter<'_, K, V> {
        Iter::range(&self.root, range.start_bound(), range.end_bound())
    }
}

impl<'a, K, V> IntoIterator for &'a BPlusTree<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<K: Ord + Clone, V> BPlusTree<K, V> {
    /// Insert `key` with `value`. If the key was already present its previous
    /// value is replaced and returned; otherwise the entry is added and `None`
    /// is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use index_db::BPlusTree;
    ///
    /// let mut index = BPlusTree::new();
    /// assert_eq!(index.insert(1_u32, "a"), None);    // new key
    /// assert_eq!(index.insert(1, "b"), Some("a"));   // replaced
    /// assert_eq!(index.get(&1), Some(&"b"));
    /// ```
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.root.insert(key, value, self.order) {
            Insert::Replaced(old) => Some(old),
            Insert::Inserted => {
                self.len = self.len.saturating_add(1);
                None
            }
            Insert::Split { sep, right } => {
                self.grow_root(sep, right);
                self.len = self.len.saturating_add(1);
                None
            }
        }
    }

    /// Remove `key`, returning its value if it was present, or `None` if the
    /// tree held no such key.
    ///
    /// Removing keeps the tree balanced: an under-full node borrows an entry
    /// from a sibling or merges with one, and when the root is left with a single
    /// child the tree collapses a level. Every leaf stays at the same depth.
    ///
    /// # Examples
    ///
    /// ```
    /// use index_db::BPlusTree;
    ///
    /// let mut index = BPlusTree::new();
    /// index.insert(1_u32, "a");
    /// index.insert(2, "b");
    ///
    /// assert_eq!(index.remove(&1), Some("a")); // returns the removed value
    /// assert_eq!(index.remove(&1), None);       // already gone
    /// assert_eq!(index.get(&1), None);
    /// assert_eq!(index.len(), 1);
    /// ```
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let removed = self.root.remove(key, self.min_keys());
        if removed.is_some() {
            self.len -= 1;
            self.shrink_root();
        }
        removed
    }

    /// The minimum number of keys a non-root node must hold: a node is at least
    /// half full, so two under-full siblings always fit in one node on merge.
    #[inline]
    fn min_keys(&self) -> usize {
        self.order.div_ceil(2) - 1
    }

    /// Collapse the root when it is an internal node left with a single child:
    /// that child becomes the new root, lowering the tree by one level. This is
    /// the only operation that decreases the tree's height.
    fn shrink_root(&mut self) {
        let only_child = match &mut self.root {
            Node::Internal(internal) if internal.children.len() == 1 => internal.children.pop(),
            _ => None,
        };
        if let Some(child) = only_child {
            self.root = child;
        }
    }

    /// Replace the root with a new internal node over the old root and the right
    /// half promoted by a split. This is the only operation that increases the
    /// tree's height, and it keeps every leaf at the same depth.
    fn grow_root(&mut self, sep: K, right: Node<K, V>) {
        let old_root = core::mem::replace(&mut self.root, Node::empty_leaf());
        let mut keys = Vec::with_capacity(self.order);
        keys.push(sep);
        let mut children = Vec::with_capacity(self.order + 1);
        children.push(old_root);
        children.push(right);
        self.root = Node::Internal(Internal { keys, children });
    }
}

impl<K, V> Default for BPlusTree<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, reason = "test assertions")]
mod tests {
    use alloc::vec::Vec;

    use proptest::prelude::*;

    use super::*;

    /// Recursively verify the structural invariants of the subtree rooted at
    /// `node`, returning its `(min_key, max_key, height)` for the parent to
    /// check separators against. Panics with a description on the first
    /// violation so a failing property shrinks to a readable case.
    fn check<K: Ord + Clone + core::fmt::Debug, V>(
        node: &Node<K, V>,
        order: usize,
        min_keys: usize,
        is_root: bool,
    ) -> (K, K, usize) {
        match node {
            Node::Leaf(leaf) => {
                assert!(!leaf.keys.is_empty(), "non-root leaf is empty");
                assert!(
                    leaf.keys.len() < order,
                    "leaf over capacity: {} >= {order}",
                    leaf.keys.len()
                );
                assert!(
                    is_root || leaf.keys.len() >= min_keys,
                    "non-root leaf under capacity: {} < {min_keys}",
                    leaf.keys.len()
                );
                assert_eq!(
                    leaf.keys.len(),
                    leaf.vals.len(),
                    "keys/vals length mismatch"
                );
                for w in leaf.keys.windows(2) {
                    assert!(w[0] < w[1], "leaf keys not strictly ascending");
                }
                (
                    leaf.keys[0].clone(),
                    leaf.keys[leaf.keys.len() - 1].clone(),
                    1,
                )
            }
            Node::Internal(internal) => {
                assert!(!internal.keys.is_empty(), "internal node has no separators");
                assert!(
                    internal.keys.len() < order,
                    "internal node over capacity: {} >= {order}",
                    internal.keys.len()
                );
                assert!(
                    is_root || internal.keys.len() >= min_keys,
                    "non-root internal node under capacity: {} < {min_keys}",
                    internal.keys.len()
                );
                assert_eq!(
                    internal.children.len(),
                    internal.keys.len() + 1,
                    "child count must be separator count + 1"
                );
                for w in internal.keys.windows(2) {
                    assert!(w[0] < w[1], "separators not strictly ascending");
                }

                let mut child_height = None;
                let mut subtree_min = None;
                let mut last_max: Option<K> = None;
                for (i, child) in internal.children.iter().enumerate() {
                    let (cmin, cmax, h) = check(child, order, min_keys, false);
                    match child_height {
                        None => child_height = Some(h),
                        Some(prev) => assert_eq!(prev, h, "subtrees differ in height (unbalanced)"),
                    }
                    if subtree_min.is_none() {
                        subtree_min = Some(cmin.clone());
                    }
                    // Separator i - 1 sits between child i - 1 and child i. It
                    // routes: everything left of it is below it, everything in
                    // and under the right child is at or above it. (After a
                    // delete the separator may be strictly below the right min,
                    // so this is `<=`, not equality.)
                    if i > 0 {
                        let sep = &internal.keys[i - 1];
                        assert!(
                            last_max.as_ref().is_some_and(|m| m < sep),
                            "left subtree max not below separator"
                        );
                        assert!(sep <= &cmin, "separator above right subtree's min key");
                    }
                    last_max = Some(cmax);
                }
                let height = child_height.map_or(1, |h| h + 1);
                // Internal nodes always carry at least two children, so both
                // accumulators are populated by the loop above.
                match (subtree_min, last_max) {
                    (Some(min), Some(max)) => (min, max, height),
                    _ => panic!("internal node with no children"),
                }
            }
        }
    }

    /// `check` entry point: derives `min_keys` from `order` and treats the tree
    /// root as exempt from the minimum-occupancy rule.
    fn check_tree<K: Ord + Clone + core::fmt::Debug, V>(tree: &BPlusTree<K, V>) {
        let _bounds = check(&tree.root, tree.order, tree.min_keys(), true);
    }

    /// Walk the leaves left to right and collect every entry key in order.
    fn collect_keys<K: Clone, V>(node: &Node<K, V>, out: &mut Vec<K>) {
        match node {
            Node::Leaf(leaf) => out.extend(leaf.keys.iter().cloned()),
            Node::Internal(internal) => {
                for child in &internal.children {
                    collect_keys(child, out);
                }
            }
        }
    }

    #[test]
    fn test_get_empty_returns_none() {
        let tree: BPlusTree<u32, u32> = BPlusTree::new();
        assert_eq!(tree.get(&0), None);
        assert!(tree.is_empty());
        assert_eq!(tree.height(), 1);
    }

    #[test]
    fn test_insert_duplicate_key_replaces_value() {
        let mut tree = BPlusTree::new();
        assert_eq!(tree.insert(1_u32, "a"), None);
        assert_eq!(tree.insert(1, "b"), Some("a"));
        assert_eq!(tree.get(&1), Some(&"b"));
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn test_insert_many_splits_and_stays_balanced() {
        let mut tree = BPlusTree::with_order(4);
        for k in 0..256_u32 {
            assert_eq!(tree.insert(k, k * 10), None);
        }
        check_tree(&tree);
        assert!(
            tree.height() > 1,
            "tree should have split into multiple levels"
        );
        for k in 0..256_u32 {
            assert_eq!(tree.get(&k), Some(&(k * 10)));
        }
        assert_eq!(tree.get(&256), None);
    }

    #[test]
    fn test_insert_reverse_order_keeps_keys_sorted() {
        let mut tree = BPlusTree::with_order(3);
        for k in (0..100_u32).rev() {
            assert_eq!(tree.insert(k, k), None);
        }
        let mut keys = Vec::new();
        collect_keys(&tree.root, &mut keys);
        assert_eq!(keys.len(), 100);
        assert!(keys.windows(2).all(|w| w[0] < w[1]), "leaf order broken");
    }

    proptest! {
        #[test]
        fn prop_matches_reference_map(
            order in 3_usize..8,
            ops in prop::collection::vec((0_u32..200, 0_u32..1_000_000), 0..400),
        ) {
            use std::collections::BTreeMap;

            let mut tree = BPlusTree::with_order(order);
            let mut reference = BTreeMap::new();
            for (k, v) in ops {
                let got = tree.insert(k, v);
                let want = reference.insert(k, v);
                prop_assert_eq!(got, want);
            }

            prop_assert_eq!(tree.len(), reference.len());
            // The structural check assumes a populated tree; the empty tree's
            // root is a legitimately empty leaf, which `check` would reject.
            if !tree.is_empty() {
                check_tree(&tree);
            }

            // Every key in the reference is found with the same value.
            for (k, v) in &reference {
                prop_assert_eq!(tree.get(k), Some(v));
            }
            // A key the reference lacks is absent from the tree too.
            for k in 0_u32..200 {
                if !reference.contains_key(&k) {
                    prop_assert_eq!(tree.get(&k), None);
                }
            }

            // The leaves, read left to right, are exactly the sorted key set.
            let mut keys = Vec::new();
            collect_keys(&tree.root, &mut keys);
            let expected: Vec<u32> = reference.keys().copied().collect();
            prop_assert_eq!(keys, expected);
        }
    }

    #[test]
    fn test_remove_absent_key_returns_none() {
        let mut tree = BPlusTree::new();
        assert_eq!(tree.insert(1_u32, "a"), None);
        assert_eq!(tree.remove(&2), None);
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn test_remove_present_key_returns_value() {
        let mut tree = BPlusTree::new();
        assert_eq!(tree.insert(1_u32, "a"), None);
        assert_eq!(tree.insert(2, "b"), None);
        assert_eq!(tree.remove(&1), Some("a"));
        assert_eq!(tree.get(&1), None);
        assert_eq!(tree.get(&2), Some(&"b"));
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn test_remove_all_empties_tree_and_collapses_root() {
        let mut tree = BPlusTree::with_order(4);
        for k in 0..200_u32 {
            let _ = tree.insert(k, k);
        }
        assert!(tree.height() > 1);
        // Remove in an order unrelated to insertion to drive merges/borrows.
        for k in (0..200_u32).step_by(2) {
            assert_eq!(tree.remove(&k), Some(k));
        }
        for k in (1..200_u32).step_by(2) {
            assert_eq!(tree.remove(&k), Some(k));
        }
        assert!(tree.is_empty());
        assert_eq!(tree.height(), 1, "root should collapse back to a leaf");
    }

    #[test]
    fn test_remove_keeps_tree_balanced() {
        let mut tree = BPlusTree::with_order(3);
        for k in 0..500_u32 {
            let _ = tree.insert(k, k);
        }
        for k in (0..500_u32).filter(|k| k % 3 == 0) {
            assert_eq!(tree.remove(&k), Some(k));
        }
        check_tree(&tree);
        for k in 0..500_u32 {
            assert_eq!(tree.get(&k), if k % 3 == 0 { None } else { Some(&k) });
        }
    }

    #[test]
    fn test_iter_empty_yields_nothing() {
        let tree: BPlusTree<u32, u32> = BPlusTree::new();
        assert_eq!(tree.iter().count(), 0);
        assert_eq!(tree.iter().next_back(), None);
        assert_eq!(tree.range(..).count(), 0);
    }

    #[test]
    fn test_iter_forward_and_reverse() {
        let mut tree = BPlusTree::with_order(4);
        for k in 0..50_u32 {
            let _ = tree.insert(k, k * 10);
        }
        let fwd: Vec<_> = tree.iter().map(|(&k, &v)| (k, v)).collect();
        let expected: Vec<_> = (0..50_u32).map(|k| (k, k * 10)).collect();
        assert_eq!(fwd, expected);

        let rev: Vec<_> = tree.iter().rev().map(|(&k, _)| k).collect();
        let expected_rev: Vec<_> = (0..50_u32).rev().collect();
        assert_eq!(rev, expected_rev);
    }

    #[test]
    fn test_iter_from_both_ends_meets_in_middle() {
        let mut tree = BPlusTree::with_order(3);
        for k in 0..9_u32 {
            let _ = tree.insert(k, k);
        }
        let mut it = tree.iter();
        let mut seq = Vec::new();
        let mut take_front = true;
        loop {
            let item = if take_front {
                it.next()
            } else {
                it.next_back()
            };
            match item {
                Some((&k, _)) => seq.push(k),
                None => break,
            }
            take_front = !take_front;
        }
        // Pulled alternately from each end, the entries interleave and meet.
        assert_eq!(seq, vec![0, 8, 1, 7, 2, 6, 3, 5, 4]);
    }

    #[test]
    fn test_range_bounds() {
        let mut tree = BPlusTree::with_order(4);
        for k in 0..20_u32 {
            let _ = tree.insert(k, k);
        }
        let collect = |it: Iter<'_, u32, u32>| it.map(|(&k, _)| k).collect::<Vec<_>>();
        assert_eq!(collect(tree.range(5..10)), vec![5, 6, 7, 8, 9]);
        assert_eq!(collect(tree.range(5..=10)), vec![5, 6, 7, 8, 9, 10]);
        assert_eq!(collect(tree.range(..3)), vec![0, 1, 2]);
        assert_eq!(collect(tree.range(17..)), vec![17, 18, 19]);
        assert_eq!(collect(tree.range(100..200)), Vec::<u32>::new());
        // A bound that falls between existing keys.
        let mut sparse = BPlusTree::with_order(3);
        for k in [0_u32, 10, 20, 30, 40] {
            let _ = sparse.insert(k, k);
        }
        assert_eq!(collect(sparse.range(5..35)), vec![10, 20, 30]);
    }

    proptest! {
        /// Forward iteration, reverse iteration, and arbitrary ranges all match
        /// `BTreeMap` over the same data.
        #[test]
        fn prop_iter_and_range_match_reference(
            order in 3_usize..8,
            keys in prop::collection::vec(0_u32..200, 0..300),
            lo in 0_u32..200,
            hi in 0_u32..200,
        ) {
            use std::collections::BTreeMap;

            let mut tree = BPlusTree::with_order(order);
            let mut reference = BTreeMap::new();
            for k in keys {
                let _ = tree.insert(k, k.wrapping_mul(7));
                let _ = reference.insert(k, k.wrapping_mul(7));
            }

            let tree_fwd: Vec<_> = tree.iter().map(|(&k, &v)| (k, v)).collect();
            let ref_fwd: Vec<_> = reference.iter().map(|(&k, &v)| (k, v)).collect();
            prop_assert_eq!(&tree_fwd, &ref_fwd);

            let tree_rev: Vec<_> = tree.iter().rev().map(|(&k, &v)| (k, v)).collect();
            let ref_rev: Vec<_> = reference.iter().rev().map(|(&k, &v)| (k, v)).collect();
            prop_assert_eq!(tree_rev, ref_rev);

            let (lo, hi) = (lo.min(hi), lo.max(hi));
            let tree_range: Vec<_> = tree.range(lo..hi).map(|(&k, _)| k).collect();
            let ref_range: Vec<_> = reference.range(lo..hi).map(|(&k, _)| k).collect();
            prop_assert_eq!(tree_range, ref_range);

            let tree_incl: Vec<_> = tree.range(lo..=hi).rev().map(|(&k, _)| k).collect();
            let ref_incl: Vec<_> = reference.range(lo..=hi).rev().map(|(&k, _)| k).collect();
            prop_assert_eq!(tree_incl, ref_incl);
        }

        /// A mixed insert/remove workload tracks `BTreeMap` exactly, and the tree
        /// stays a valid, balanced, minimally-occupied B+tree throughout.
        #[test]
        fn prop_insert_remove_matches_reference(
            order in 3_usize..8,
            ops in prop::collection::vec((any::<bool>(), 0_u32..150), 0..600),
        ) {
            use std::collections::BTreeMap;

            let mut tree = BPlusTree::with_order(order);
            let mut reference = BTreeMap::new();
            for (is_insert, k) in ops {
                if is_insert {
                    prop_assert_eq!(tree.insert(k, k), reference.insert(k, k));
                } else {
                    prop_assert_eq!(tree.remove(&k), reference.remove(&k));
                }
                prop_assert_eq!(tree.len(), reference.len());
                if !tree.is_empty() {
                    check_tree(&tree);
                }
            }

            let mut keys = Vec::new();
            collect_keys(&tree.root, &mut keys);
            let expected: Vec<u32> = reference.keys().copied().collect();
            prop_assert_eq!(keys, expected);
        }
    }
}
