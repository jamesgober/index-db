//! In-order iteration over a [`BPlusTree`](crate::BPlusTree).
//!
//! A [`Cursor`] is a position in the tree — a path of internal nodes from the
//! root down to a leaf, plus an index within that leaf. Stepping a cursor walks
//! the entries in key order: within a leaf it moves the index, and at a leaf
//! boundary it ascends the path to the next subtree and descends again. The
//! public [`Iter`] holds two cursors, one advancing from the front and one
//! retreating from the back, so it serves both forward and reverse scans; they
//! meet in the middle, at which point iteration is done.

use alloc::vec::Vec;
use core::ops::Bound;

use crate::node::{Internal, Leaf, Node};

/// The path from the root to a leaf: each internal node on the way down, paired
/// with the index of the child that was descended into.
type Path<'a, K, V> = Vec<(&'a Internal<K, V>, usize)>;

/// A position in the tree: the path of internal nodes descended from the root,
/// each paired with the child index taken, down to the current leaf and entry.
struct Cursor<'a, K, V> {
    path: Path<'a, K, V>,
    leaf: &'a Leaf<K, V>,
    idx: usize,
}

impl<'a, K, V> Cursor<'a, K, V> {
    /// The key/value at the current position.
    #[inline]
    fn current(&self) -> (&'a K, &'a V) {
        (&self.leaf.keys[self.idx], &self.leaf.vals[self.idx])
    }

    /// The key at the current position.
    #[inline]
    fn key(&self) -> &'a K {
        &self.leaf.keys[self.idx]
    }

    /// Whether two cursors point at the same entry. Leaves are compared by
    /// address, so this is exact and needs no ordering on `K`.
    #[inline]
    fn same_position(&self, other: &Self) -> bool {
        core::ptr::eq(self.leaf, other.leaf) && self.idx == other.idx
    }

    /// A cursor at the first (smallest) entry, or `None` if the tree is empty.
    fn first(root: &'a Node<K, V>) -> Option<Self> {
        let mut path = Vec::new();
        let mut node = root;
        loop {
            match node {
                Node::Internal(internal) => {
                    path.push((internal, 0));
                    node = &internal.children[0];
                }
                Node::Leaf(leaf) => {
                    return if leaf.keys.is_empty() {
                        None
                    } else {
                        Some(Cursor { path, leaf, idx: 0 })
                    };
                }
            }
        }
    }

    /// A cursor at the last (largest) entry, or `None` if the tree is empty.
    fn last(root: &'a Node<K, V>) -> Option<Self> {
        let mut path = Vec::new();
        let mut node = root;
        loop {
            match node {
                Node::Internal(internal) => {
                    let ci = internal.children.len() - 1;
                    path.push((internal, ci));
                    node = &internal.children[ci];
                }
                Node::Leaf(leaf) => {
                    return if leaf.keys.is_empty() {
                        None
                    } else {
                        Some(Cursor {
                            path,
                            leaf,
                            idx: leaf.keys.len() - 1,
                        })
                    };
                }
            }
        }
    }

    /// Advance to the next entry in key order. Returns `false` at the end.
    fn step_forward(&mut self) -> bool {
        if self.idx + 1 < self.leaf.keys.len() {
            self.idx += 1;
            return true;
        }
        loop {
            let (parent, ci) = match self.path.last() {
                Some(&frame) => frame,
                None => return false,
            };
            if ci + 1 < parent.children.len() {
                if let Some(frame) = self.path.last_mut() {
                    frame.1 = ci + 1;
                }
                self.descend_leftmost(&parent.children[ci + 1]);
                return true;
            }
            let _popped = self.path.pop();
        }
    }

    /// Retreat to the previous entry in key order. Returns `false` at the start.
    fn step_backward(&mut self) -> bool {
        if self.idx > 0 {
            self.idx -= 1;
            return true;
        }
        loop {
            let (parent, ci) = match self.path.last() {
                Some(&frame) => frame,
                None => return false,
            };
            if ci > 0 {
                if let Some(frame) = self.path.last_mut() {
                    frame.1 = ci - 1;
                }
                self.descend_rightmost(&parent.children[ci - 1]);
                return true;
            }
            let _popped = self.path.pop();
        }
    }

    /// Descend `node`'s leftmost path, pushing frames, and land on its first
    /// entry. `node` must belong to the tree this cursor walks.
    fn descend_leftmost(&mut self, node: &'a Node<K, V>) {
        let mut node = node;
        loop {
            match node {
                Node::Internal(internal) => {
                    self.path.push((internal, 0));
                    node = &internal.children[0];
                }
                Node::Leaf(leaf) => {
                    self.leaf = leaf;
                    self.idx = 0;
                    return;
                }
            }
        }
    }

    /// Descend `node`'s rightmost path, pushing frames, and land on its last
    /// entry. `node` must belong to the tree this cursor walks.
    fn descend_rightmost(&mut self, node: &'a Node<K, V>) {
        let mut node = node;
        loop {
            match node {
                Node::Internal(internal) => {
                    let ci = internal.children.len() - 1;
                    self.path.push((internal, ci));
                    node = &internal.children[ci];
                }
                Node::Leaf(leaf) => {
                    self.leaf = leaf;
                    self.idx = leaf.keys.len() - 1;
                    return;
                }
            }
        }
    }
}

impl<'a, K: Ord, V> Cursor<'a, K, V> {
    /// A cursor at the first entry satisfying the lower `bound`, or `None` if no
    /// entry does.
    fn lower_bound(root: &'a Node<K, V>, bound: Bound<&K>) -> Option<Self> {
        let probe = match bound {
            Bound::Unbounded => return Self::first(root),
            Bound::Included(b) | Bound::Excluded(b) => b,
        };
        let (path, leaf) = descend_to(root, probe);
        if leaf.keys.is_empty() {
            return None;
        }
        let idx = match leaf.keys.binary_search(probe) {
            Ok(i) => match bound {
                Bound::Excluded(_) => i + 1,
                _ => i,
            },
            Err(i) => i,
        };
        let mut cursor = Cursor {
            path,
            leaf,
            idx: leaf.keys.len() - 1,
        };
        if idx < leaf.keys.len() {
            cursor.idx = idx;
            Some(cursor)
        } else if cursor.step_forward() {
            Some(cursor)
        } else {
            None
        }
    }

    /// A cursor at the last entry satisfying the upper `bound`, or `None` if no
    /// entry does.
    fn upper_bound(root: &'a Node<K, V>, bound: Bound<&K>) -> Option<Self> {
        let probe = match bound {
            Bound::Unbounded => return Self::last(root),
            Bound::Included(b) | Bound::Excluded(b) => b,
        };
        let (path, leaf) = descend_to(root, probe);
        if leaf.keys.is_empty() {
            return None;
        }
        let idx = match leaf.keys.binary_search(probe) {
            Ok(i) => match bound {
                Bound::Excluded(_) => i.checked_sub(1),
                _ => Some(i),
            },
            Err(i) => i.checked_sub(1),
        };
        let mut cursor = Cursor { path, leaf, idx: 0 };
        match idx {
            Some(idx) => {
                cursor.idx = idx;
                Some(cursor)
            }
            None if cursor.step_backward() => Some(cursor),
            None => None,
        }
    }
}

/// Descend from `root` to the leaf whose key range covers `probe`, returning the
/// path taken and that leaf.
fn descend_to<'a, K: Ord, V>(root: &'a Node<K, V>, probe: &K) -> (Path<'a, K, V>, &'a Leaf<K, V>) {
    let mut path = Vec::new();
    let mut node = root;
    loop {
        match node {
            Node::Internal(internal) => {
                let ci = internal.child_index(probe);
                path.push((internal, ci));
                node = &internal.children[ci];
            }
            Node::Leaf(leaf) => return (path, leaf),
        }
    }
}

/// A forward-and-reverse iterator over a [`BPlusTree`](crate::BPlusTree)'s
/// entries in ascending key order, yielding `(&K, &V)`.
///
/// Returned by [`BPlusTree::iter`](crate::BPlusTree::iter) and
/// [`BPlusTree::range`](crate::BPlusTree::range). It is a
/// [`DoubleEndedIterator`], so a reverse scan is `iter.rev()` and a range can be
/// walked from either end.
///
/// # Examples
///
/// ```
/// use index_db::BPlusTree;
///
/// let mut index = BPlusTree::new();
/// for k in 0..5_u32 {
///     index.insert(k, k * k);
/// }
///
/// let forward: Vec<_> = index.iter().map(|(&k, &v)| (k, v)).collect();
/// assert_eq!(forward, vec![(0, 0), (1, 1), (2, 4), (3, 9), (4, 16)]);
///
/// let reverse: Vec<_> = index.iter().rev().map(|(&k, _)| k).collect();
/// assert_eq!(reverse, vec![4, 3, 2, 1, 0]);
/// ```
pub struct Iter<'a, K, V> {
    span: Option<Span<'a, K, V>>,
}

/// The not-yet-yielded slice of an iteration: cursors at its first and last
/// remaining entries. Once they coincide, one element is left.
struct Span<'a, K, V> {
    front: Cursor<'a, K, V>,
    back: Cursor<'a, K, V>,
}

impl<'a, K, V> Iter<'a, K, V> {
    /// An iterator over every entry in the tree.
    pub(crate) fn full(root: &'a Node<K, V>) -> Self {
        match (Cursor::first(root), Cursor::last(root)) {
            (Some(front), Some(back)) => Iter {
                span: Some(Span { front, back }),
            },
            _ => Iter { span: None },
        }
    }
}

impl<'a, K: Ord, V> Iter<'a, K, V> {
    /// An iterator over the entries whose keys fall within `[lower, upper]` as
    /// described by the two bounds.
    pub(crate) fn range(root: &'a Node<K, V>, lower: Bound<&K>, upper: Bound<&K>) -> Self {
        match (
            Cursor::lower_bound(root, lower),
            Cursor::upper_bound(root, upper),
        ) {
            (Some(front), Some(back)) if front.key() <= back.key() => Iter {
                span: Some(Span { front, back }),
            },
            _ => Iter { span: None },
        }
    }
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        let span = self.span.as_mut()?;
        let out = span.front.current();
        if span.front.same_position(&span.back) {
            self.span = None;
        } else {
            let _advanced = span.front.step_forward();
        }
        Some(out)
    }
}

impl<K, V> DoubleEndedIterator for Iter<'_, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let span = self.span.as_mut()?;
        let out = span.back.current();
        if span.front.same_position(&span.back) {
            self.span = None;
        } else {
            let _retreated = span.back.step_backward();
        }
        Some(out)
    }
}
