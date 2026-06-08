//! B+tree node representation.
//!
//! A node is one of two shapes. A [`Leaf`] holds the key/value entries, kept in
//! two parallel arrays so a search touches only the keys and never pays to load
//! the values it skips past. An [`Internal`] node holds separator keys and its
//! children stored inline: `n` separators fan out to `n + 1` children, and the
//! separators partition the key space so a lookup descends to exactly one child
//! at each level.
//!
//! Children live directly in the parent's array rather than behind individual
//! boxes: a [`Node`] is a fixed size (its `Vec`s are pointer triples), so the
//! children sit contiguously and a downward traversal reads them with good cache
//! locality and no per-child allocation.
//!
//! The arrays are sized to a fixed *order* (the maximum fan-out) so a node maps
//! cleanly onto a fixed-size page. A node overflows when an insert would push it
//! past `order - 1` keys; the overflow is resolved by a split, which is reported
//! back up to the parent as [`Insert::Split`] so the parent can absorb the new
//! separator and child.

use alloc::vec::Vec;

/// Outcome of inserting a key/value pair into a subtree.
///
/// Each node reports one of these to its parent so a structural change (a split)
/// can propagate exactly one level up the tree per call frame, rather than the
/// whole path being rewritten.
pub(crate) enum Insert<K, V> {
    /// The key already existed; this is the value it displaced.
    Replaced(V),
    /// A new entry was stored and the node still fits within `order`.
    Inserted,
    /// The node overflowed and split in two. `sep` is the separator key to
    /// promote into the parent, and `right` is the freshly created right half.
    Split {
        /// Separator key promoted to the parent.
        sep: K,
        /// The new right-hand node produced by the split.
        right: Node<K, V>,
    },
}

/// A B+tree node: either a leaf carrying entries or an internal routing node.
pub(crate) enum Node<K, V> {
    /// A leaf holding key/value entries.
    Leaf(Leaf<K, V>),
    /// An internal node holding separators and child links.
    Internal(Internal<K, V>),
}

/// A leaf node: sorted keys paired with their values in parallel arrays.
pub(crate) struct Leaf<K, V> {
    /// Entry keys, sorted ascending.
    pub(crate) keys: Vec<K>,
    /// Entry values, positionally aligned with `keys`.
    pub(crate) vals: Vec<V>,
}

/// An internal node: sorted separator keys and the children they partition.
///
/// Invariant: `children.len() == keys.len() + 1`. Every key in the subtree under
/// `children[i]` is `< keys[i]`, and every key under `children[i + 1]` is
/// `>= keys[i]`.
pub(crate) struct Internal<K, V> {
    /// Separator keys, sorted ascending.
    pub(crate) keys: Vec<K>,
    /// Children, one more than the number of separators.
    pub(crate) children: Vec<Node<K, V>>,
}

impl<K, V> Node<K, V> {
    /// Build an empty leaf node, the shape of a tree with no entries.
    #[inline]
    pub(crate) fn empty_leaf() -> Self {
        Node::Leaf(Leaf {
            keys: Vec::new(),
            vals: Vec::new(),
        })
    }

    /// The number of keys held in this node. For a leaf this is the entry count;
    /// for an internal node it is the separator count (one fewer than children).
    #[inline]
    pub(crate) fn keys_len(&self) -> usize {
        match self {
            Node::Leaf(leaf) => leaf.keys.len(),
            Node::Internal(internal) => internal.keys.len(),
        }
    }
}

impl<K: Ord, V> Node<K, V> {
    /// Look up `key`, descending child links until the owning leaf is reached.
    ///
    /// Iterative on purpose: the path length is the tree height, so a recursive
    /// form would only add call frames without simplifying the logic.
    #[inline]
    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        let mut node = self;
        loop {
            match node {
                Node::Leaf(leaf) => return leaf.get(key),
                Node::Internal(internal) => node = internal.child(key),
            }
        }
    }
}

impl<K: Ord + Clone, V> Node<K, V> {
    /// Insert `key`/`value` into this subtree, returning the outcome for the
    /// parent to act on.
    pub(crate) fn insert(&mut self, key: K, value: V, order: usize) -> Insert<K, V> {
        match self {
            Node::Leaf(leaf) => leaf.insert(key, value, order),
            Node::Internal(internal) => internal.insert(key, value, order),
        }
    }

    /// Remove `key` from this subtree, returning its value if it was present.
    ///
    /// A node that drops below `min_keys` after the removal is left under-full;
    /// its parent restores the minimum by borrowing from or merging with a
    /// sibling. The root is exempt and may shrink freely.
    pub(crate) fn remove(&mut self, key: &K, min_keys: usize) -> Option<V> {
        match self {
            Node::Leaf(leaf) => leaf.remove(key),
            Node::Internal(internal) => internal.remove(key, min_keys),
        }
    }
}

impl<K: Ord, V> Leaf<K, V> {
    /// Look up `key` within this leaf by binary search over the sorted keys.
    #[inline]
    fn get(&self, key: &K) -> Option<&V> {
        match self.keys.binary_search(key) {
            Ok(i) => Some(&self.vals[i]),
            Err(_) => None,
        }
    }

    /// Remove `key`'s entry from this leaf, returning its value if present.
    fn remove(&mut self, key: &K) -> Option<V> {
        match self.keys.binary_search(key) {
            Ok(i) => {
                let _removed_key = self.keys.remove(i);
                Some(self.vals.remove(i))
            }
            Err(_) => None,
        }
    }
}

impl<K: Ord + Clone, V> Leaf<K, V> {
    /// Insert into this leaf, splitting if the entry pushes it past `order`.
    fn insert(&mut self, key: K, value: V, order: usize) -> Insert<K, V> {
        match self.keys.binary_search(&key) {
            Ok(i) => Insert::Replaced(core::mem::replace(&mut self.vals[i], value)),
            Err(i) => {
                self.keys.insert(i, key);
                self.vals.insert(i, value);
                if self.keys.len() < order {
                    Insert::Inserted
                } else {
                    self.split()
                }
            }
        }
    }

    /// Split a full leaf in two, copying the first key of the right half up as
    /// the separator. In a B+tree the separator is *copied*, not moved, because
    /// the entry it names still lives in the leaf where lookups must find it.
    fn split(&mut self) -> Insert<K, V> {
        let mid = self.keys.len() / 2;
        let right_keys = self.keys.split_off(mid);
        let right_vals = self.vals.split_off(mid);
        let sep = right_keys[0].clone();
        let right = Node::Leaf(Leaf {
            keys: right_keys,
            vals: right_vals,
        });
        Insert::Split { sep, right }
    }
}

impl<K: Ord, V> Internal<K, V> {
    /// The child to descend into when searching for `key`.
    #[inline]
    fn child(&self, key: &K) -> &Node<K, V> {
        &self.children[self.child_index(key)]
    }

    /// The index of the child that owns `key`'s key range.
    ///
    /// A key equal to `keys[i]` belongs to the right child of that separator
    /// (`i + 1`), since separators mark the lower bound of the right subtree.
    #[inline]
    pub(crate) fn child_index(&self, key: &K) -> usize {
        match self.keys.binary_search(key) {
            Ok(i) => i + 1,
            Err(i) => i,
        }
    }
}

impl<K: Ord + Clone, V> Internal<K, V> {
    /// Route the insert to the owning child, then absorb a split if one bubbled
    /// up, splitting this node in turn if that pushes it past `order`.
    fn insert(&mut self, key: K, value: V, order: usize) -> Insert<K, V> {
        let idx = self.child_index(&key);
        match self.children[idx].insert(key, value, order) {
            Insert::Replaced(old) => Insert::Replaced(old),
            Insert::Inserted => Insert::Inserted,
            Insert::Split { sep, right } => {
                self.keys.insert(idx, sep);
                self.children.insert(idx + 1, right);
                if self.keys.len() < order {
                    Insert::Inserted
                } else {
                    self.split()
                }
            }
        }
    }

    /// Split a full internal node, promoting the median separator. Unlike a leaf
    /// split the median is *moved* up, not copied: an internal separator is pure
    /// routing information with no value attached, so it need not be retained.
    fn split(&mut self) -> Insert<K, V> {
        let mid = self.keys.len() / 2;
        let right_children = self.children.split_off(mid + 1);
        let right_keys = self.keys.split_off(mid + 1);
        let sep = self.keys.remove(mid);
        let right = Node::Internal(Internal {
            keys: right_keys,
            children: right_children,
        });
        Insert::Split { sep, right }
    }

    /// Route the removal to the owning child, then restore that child's minimum
    /// occupancy if the deletion left it under-full.
    fn remove(&mut self, key: &K, min_keys: usize) -> Option<V> {
        let idx = self.child_index(key);
        let removed = self.children[idx].remove(key, min_keys);
        if removed.is_some() && self.children[idx].keys_len() < min_keys {
            self.rebalance(idx, min_keys);
        }
        removed
    }

    /// Restore the minimum occupancy of the under-full child at `idx`. Borrow a
    /// single entry from a sibling that has one to spare; otherwise merge the
    /// child into a sibling, which removes one separator from this node and may
    /// in turn leave *this* node under-full for its own parent to fix.
    fn rebalance(&mut self, idx: usize, min_keys: usize) {
        if idx > 0 && self.children[idx - 1].keys_len() > min_keys {
            self.borrow_from_left(idx);
        } else if idx + 1 < self.children.len() && self.children[idx + 1].keys_len() > min_keys {
            self.borrow_from_right(idx);
        } else if idx > 0 {
            self.merge(idx - 1);
        } else {
            self.merge(idx);
        }
    }

    /// Move one entry from the left sibling (`idx - 1`) into the front of the
    /// child at `idx`, and update the separator between them.
    fn borrow_from_left(&mut self, idx: usize) {
        let sep = self.keys[idx - 1].clone();
        let (left_part, right_part) = self.children.split_at_mut(idx);
        let new_sep = match (&mut left_part[idx - 1], &mut right_part[0]) {
            (Node::Leaf(left), Node::Leaf(child)) => match (left.keys.pop(), left.vals.pop()) {
                (Some(k), Some(v)) => {
                    child.keys.insert(0, k.clone());
                    child.vals.insert(0, v);
                    Some(k)
                }
                _ => None,
            },
            (Node::Internal(left), Node::Internal(child)) => {
                child.keys.insert(0, sep);
                if let Some(moved) = left.children.pop() {
                    child.children.insert(0, moved);
                }
                left.keys.pop()
            }
            // Siblings in a balanced tree are always the same kind.
            _ => None,
        };
        if let Some(ns) = new_sep {
            self.keys[idx - 1] = ns;
        }
    }

    /// Move one entry from the right sibling (`idx + 1`) into the back of the
    /// child at `idx`, and update the separator between them.
    fn borrow_from_right(&mut self, idx: usize) {
        let sep = self.keys[idx].clone();
        let (left_part, right_part) = self.children.split_at_mut(idx + 1);
        let new_sep = match (&mut left_part[idx], &mut right_part[0]) {
            (Node::Leaf(child), Node::Leaf(right)) => {
                if right.keys.is_empty() {
                    None
                } else {
                    let k = right.keys.remove(0);
                    let v = right.vals.remove(0);
                    child.keys.push(k);
                    child.vals.push(v);
                    right.keys.first().cloned()
                }
            }
            (Node::Internal(child), Node::Internal(right)) => {
                child.keys.push(sep);
                if !right.children.is_empty() {
                    let moved = right.children.remove(0);
                    child.children.push(moved);
                }
                if right.keys.is_empty() {
                    None
                } else {
                    Some(right.keys.remove(0))
                }
            }
            // Siblings in a balanced tree are always the same kind.
            _ => None,
        };
        if let Some(ns) = new_sep {
            self.keys[idx] = ns;
        }
    }

    /// Merge the child at `i + 1` into the child at `i`, consuming the separator
    /// between them. The merged node holds both children's contents; this node
    /// loses one separator and one child.
    fn merge(&mut self, i: usize) {
        let right = self.children.remove(i + 1);
        let sep = self.keys.remove(i);
        match (&mut self.children[i], right) {
            (Node::Leaf(left), Node::Leaf(mut right)) => {
                left.keys.append(&mut right.keys);
                left.vals.append(&mut right.vals);
            }
            (Node::Internal(left), Node::Internal(mut right)) => {
                left.keys.push(sep);
                left.keys.append(&mut right.keys);
                left.children.append(&mut right.children);
            }
            // Siblings in a balanced tree are always the same kind.
            _ => {}
        }
    }
}
