//! B+tree node representation.
//!
//! A node is one of two shapes. A [`Leaf`] holds the key/value entries, kept in
//! two parallel arrays so a search touches only the keys and never pays to load
//! the values it skips past. An [`Internal`] node holds separator keys and the
//! [`NodeId`]s of its children: `n` separators fan out to `n + 1` children, and
//! the separators partition the key space so a lookup descends to exactly one
//! child at each level.
//!
//! Children are referenced by id, not owned inline. A node names its children by
//! [`NodeId`] and the [`NodeStore`](crate::store::NodeStore) resolves an id to
//! the node. This is the seam that lets one tree algorithm run over different
//! backends: an in-memory slab today, a paged store later. The id is small, so a
//! node's own footprint stays compact and cache-friendly.

use alloc::vec::Vec;

use crate::store::NodeId;

/// A B+tree node: either a leaf carrying entries or an internal routing node.
pub(crate) enum Node<K, V> {
    /// A leaf holding key/value entries.
    Leaf(Leaf<K, V>),
    /// An internal node holding separators and child ids.
    Internal(Internal<K>),
}

/// A leaf node: sorted keys paired with their values in parallel arrays.
pub(crate) struct Leaf<K, V> {
    /// Entry keys, sorted ascending.
    pub(crate) keys: Vec<K>,
    /// Entry values, positionally aligned with `keys`.
    pub(crate) vals: Vec<V>,
}

/// An internal node: sorted separator keys and the child ids they partition.
///
/// Invariant: `children.len() == keys.len() + 1`. Every key in the subtree under
/// `children[i]` is `< keys[i]`, and every key under `children[i + 1]` is
/// `>= keys[i]`.
pub(crate) struct Internal<K> {
    /// Separator keys, sorted ascending.
    pub(crate) keys: Vec<K>,
    /// Child ids, one more than the number of separators.
    pub(crate) children: Vec<NodeId>,
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

    /// Borrow this node as a leaf, or `None` if it is internal.
    #[inline]
    pub(crate) fn as_leaf(&self) -> Option<&Leaf<K, V>> {
        match self {
            Node::Leaf(leaf) => Some(leaf),
            Node::Internal(_) => None,
        }
    }
}

impl<K: Ord, V> Leaf<K, V> {
    /// Look up `key` within this leaf by binary search over the sorted keys.
    #[inline]
    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        match self.keys.binary_search(key) {
            Ok(i) => Some(&self.vals[i]),
            Err(_) => None,
        }
    }

    /// Remove `key`'s entry from this leaf, returning its value if present.
    pub(crate) fn remove(&mut self, key: &K) -> Option<V> {
        match self.keys.binary_search(key) {
            Ok(i) => {
                let _removed_key = self.keys.remove(i);
                Some(self.vals.remove(i))
            }
            Err(_) => None,
        }
    }
}

impl<K: Ord> Internal<K> {
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
