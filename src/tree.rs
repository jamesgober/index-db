//! The [`BPlusTree`] public type: an ordered map laid out as a B+tree.

use alloc::vec::Vec;
use core::ops::RangeBounds;

use crate::iter::Iter;
use crate::node::Node;
use crate::ops;
use crate::store::{InMemoryStore, NodeId, NodeStore};

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
/// height grows with the logarithm of the entry count. Beyond point access it
/// supports ordered iteration and range scans, forward and in reverse.
///
/// The tree does not own its nodes directly. It addresses them by id through an
/// internal node store, and the store the nodes live in is a seam: today they
/// sit in a heap slab (a fast, single-threaded, in-process ordered map), and the
/// same algorithm can later run over a page-backed store without change. That
/// seam is internal — the public surface is just this one type.
///
/// `K` must be [`Ord`]; [`insert`](BPlusTree::insert) and
/// [`remove`](BPlusTree::remove) additionally need [`Clone`], because the tree
/// copies separator keys between nodes as it splits and rebalances.
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
    /// Id of the root node within `store`.
    root: NodeId,
    /// Backend the nodes live in.
    store: InMemoryStore<K, V>,
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
        let mut store = InMemoryStore::new();
        let root = store.alloc(Node::empty_leaf());
        BPlusTree {
            root,
            store,
            order: order.max(MIN_ORDER),
            len: 0,
        }
    }
}

impl<K: Ord + Clone, V> BPlusTree<K, V> {
    /// Build a tree in bulk from entries already sorted by key.
    ///
    /// When the input is sorted strictly ascending by key, the tree is built
    /// bottom-up — leaves packed and balanced in one pass — which is much faster
    /// than inserting one entry at a time. If the input is *not* strictly
    /// ascending (out of order or with duplicate keys), it falls back to ordinary
    /// insertion, so the result is always a correct tree; only the fast path
    /// requires sorted, unique keys. On the fallback path a later duplicate key
    /// overwrites an earlier one.
    ///
    /// # Examples
    ///
    /// ```
    /// use index_db::BPlusTree;
    ///
    /// // Sorted input takes the fast bottom-up path.
    /// let index = BPlusTree::from_sorted((0..1_000_u32).map(|k| (k, k * k)));
    /// assert_eq!(index.len(), 1_000);
    /// assert_eq!(index.get(&30), Some(&900));
    /// assert_eq!(index.get(&999), Some(&998_001));
    /// ```
    #[must_use]
    pub fn from_sorted<I: IntoIterator<Item = (K, V)>>(entries: I) -> Self {
        Self::from_sorted_with_order(entries, DEFAULT_ORDER)
    }

    /// [`from_sorted`](Self::from_sorted) with an explicit fan-out, for tests.
    #[must_use]
    pub(crate) fn from_sorted_with_order<I>(entries: I, order: usize) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
    {
        let order = order.max(MIN_ORDER);
        let entries: Vec<(K, V)> = entries.into_iter().collect();
        let ascending = entries.windows(2).all(|w| w[0].0 < w[1].0);
        if ascending {
            let mut store = InMemoryStore::new();
            let (root, len) = ops::bulk_load(&mut store, entries, order);
            BPlusTree {
                root,
                store,
                order,
                len,
            }
        } else {
            let mut tree = Self::with_order(order);
            for (key, value) in entries {
                let _previous = tree.insert(key, value);
            }
            tree
        }
    }
}

impl<K, V> BPlusTree<K, V> {
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
}

impl<K, V> BPlusTree<K, V> {
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
        let mut id = self.root;
        while let Node::Internal(internal) = self.store.get(id) {
            height += 1;
            id = internal.children[0];
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
        self.root = self.store.reset();
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
        Iter::full(&self.store, self.root)
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
        ops::get(&self.store, self.root, key)
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
        Iter::range(
            &self.store,
            self.root,
            range.start_bound(),
            range.end_bound(),
        )
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
        let (replaced, root) = ops::insert(&mut self.store, self.root, key, value, self.order);
        self.root = root;
        if replaced.is_none() {
            self.len = self.len.saturating_add(1);
        }
        replaced
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
        let min_keys = self.min_keys();
        let (removed, root) = ops::remove(&mut self.store, self.root, key, min_keys);
        self.root = root;
        if removed.is_some() {
            self.len -= 1;
        }
        removed
    }

    /// The minimum number of keys a non-root node must hold: a node is at least
    /// half full, so two under-full siblings always fit in one node on merge.
    #[inline]
    fn min_keys(&self) -> usize {
        self.order.div_ceil(2) - 1
    }
}

impl<K, V> Default for BPlusTree<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, K, V> IntoIterator for &'a BPlusTree<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, reason = "test assertions")]
mod tests {
    use alloc::vec::Vec;

    use proptest::prelude::*;

    use super::*;

    /// Recursively verify the structural invariants of the subtree at `id`,
    /// returning its `(min_key, max_key, height)` for the parent to check
    /// separators against. Panics with a description on the first violation.
    fn check<K: Ord + Clone + core::fmt::Debug, V>(
        store: &InMemoryStore<K, V>,
        id: NodeId,
        order: usize,
        min_keys: usize,
        is_root: bool,
    ) -> (K, K, usize) {
        match store.get(id) {
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
                    let (cmin, cmax, h) = check(store, *child, order, min_keys, false);
                    match child_height {
                        None => child_height = Some(h),
                        Some(prev) => assert_eq!(prev, h, "subtrees differ in height (unbalanced)"),
                    }
                    if subtree_min.is_none() {
                        subtree_min = Some(cmin.clone());
                    }
                    // Routing separator: max(left) < sep <= min(right). After a
                    // delete the separator may sit strictly below the right min,
                    // so this is `<=`, not equality.
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
                match (subtree_min, last_max) {
                    (Some(min), Some(max)) => (min, max, height),
                    _ => panic!("internal node with no children"),
                }
            }
        }
    }

    /// `check` entry point: treats the tree root as exempt from minimum occupancy.
    fn check_tree<K: Ord + Clone + core::fmt::Debug, V>(tree: &BPlusTree<K, V>) {
        let _bounds = check(&tree.store, tree.root, tree.order, tree.min_keys(), true);
    }

    /// Collect every entry key left to right across the leaves.
    fn collect_keys<K: Clone, V>(store: &InMemoryStore<K, V>, id: NodeId, out: &mut Vec<K>) {
        match store.get(id) {
            Node::Leaf(leaf) => out.extend(leaf.keys.iter().cloned()),
            Node::Internal(internal) => {
                for child in &internal.children {
                    collect_keys(store, *child, out);
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
        collect_keys(&tree.store, tree.root, &mut keys);
        assert_eq!(keys.len(), 100);
        assert!(keys.windows(2).all(|w| w[0] < w[1]), "leaf order broken");
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
    fn test_bulk_load_builds_balanced_tree() {
        for &n in &[0_u32, 1, 2, 5, 63, 64, 65, 1_000] {
            let tree = BPlusTree::from_sorted_with_order((0..n).map(|k| (k, k * 2)), 5);
            assert_eq!(tree.len(), n as usize);
            if n > 0 {
                check_tree(&tree);
            }
            for k in 0..n {
                assert_eq!(tree.get(&k), Some(&(k * 2)));
            }
            let keys: Vec<_> = tree.iter().map(|(&k, _)| k).collect();
            assert_eq!(keys, (0..n).collect::<Vec<_>>());
        }
    }

    #[test]
    fn test_bulk_load_unsorted_falls_back() {
        let tree = BPlusTree::from_sorted_with_order([(3_u32, 3), (1, 1), (2, 2), (1, 9)], 4);
        assert_eq!(tree.len(), 3);
        assert_eq!(tree.get(&1), Some(&9)); // last write wins on the fallback path
        let keys: Vec<_> = tree.iter().map(|(&k, _)| k).collect();
        assert_eq!(keys, vec![1, 2, 3]);
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
        let mut sparse = BPlusTree::with_order(3);
        for k in [0_u32, 10, 20, 30, 40] {
            let _ = sparse.insert(k, k);
        }
        assert_eq!(collect(sparse.range(5..35)), vec![10, 20, 30]);
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
                prop_assert_eq!(tree.insert(k, v), reference.insert(k, v));
            }

            prop_assert_eq!(tree.len(), reference.len());
            if !tree.is_empty() {
                check_tree(&tree);
            }
            for (k, v) in &reference {
                prop_assert_eq!(tree.get(k), Some(v));
            }
            for k in 0_u32..200 {
                if !reference.contains_key(&k) {
                    prop_assert_eq!(tree.get(&k), None);
                }
            }
            let mut keys = Vec::new();
            collect_keys(&tree.store, tree.root, &mut keys);
            let expected: Vec<u32> = reference.keys().copied().collect();
            prop_assert_eq!(keys, expected);
        }

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
            collect_keys(&tree.store, tree.root, &mut keys);
            let expected: Vec<u32> = reference.keys().copied().collect();
            prop_assert_eq!(keys, expected);
        }

        #[test]
        fn prop_bulk_load_matches_inserts(
            order in 3_usize..8,
            keys in prop::collection::btree_set(0_u32..1_000, 0..400),
        ) {
            let sorted: Vec<(u32, u32)> = keys.iter().map(|&k| (k, k)).collect();
            let bulk = BPlusTree::from_sorted_with_order(sorted.iter().copied(), order);
            prop_assert_eq!(bulk.len(), keys.len());
            if !bulk.is_empty() {
                check_tree(&bulk);
            }
            let bulk_keys: Vec<_> = bulk.iter().map(|(&k, _)| k).collect();
            let expected: Vec<u32> = keys.iter().copied().collect();
            prop_assert_eq!(bulk_keys, expected);
        }
    }
}
