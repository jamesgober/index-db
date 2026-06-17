//! The B+tree algorithm, written against the [`Store`] seam.
//!
//! Every function here speaks only `NodeId`s and a `&mut S` / `&S`: it never
//! assumes where a node lives, only that the store can resolve, allocate, and
//! reclaim nodes by id. That is what lets the same search, insert, delete, and
//! bulk-load logic drive any backend.
//!
//! Because a node is reached through the store rather than owned inline, an
//! operation that touches two nodes (a node and its child, or two siblings)
//! touches them one at a time: peek the first to learn what it needs, release
//! that borrow, then act on the second. The store serialises access by id, so
//! there is never a second mutable borrow to reconcile.

use alloc::vec::Vec;

use crate::node::{Internal, Leaf, Node};
use crate::store::{NodeId, NodeStore as Store};

/// Outcome of inserting into a subtree, reported to the caller one level up.
enum Insert<K, V> {
    /// The key already existed; this is the value it displaced.
    Replaced(V),
    /// A new entry was stored and the node still fits within `order`.
    Done,
    /// The node split; promote `sep` and link the new right node `right`.
    Split { sep: K, right: NodeId },
}

/// Look up `key`, descending child ids until the owning leaf is reached.
#[inline]
pub(crate) fn get<'a, K: Ord + 'a, V: 'a, S: Store<K, V>>(
    store: &'a S,
    root: NodeId,
    key: &K,
) -> Option<&'a V> {
    let mut id = root;
    loop {
        match store.get(id) {
            Node::Leaf(leaf) => return leaf.get(key),
            Node::Internal(internal) => id = internal.children[internal.child_index(key)],
        }
    }
}

/// Insert `key`/`value`, returning the displaced value (if any) and the tree's
/// root, which changes only when the tree grows a new level.
pub(crate) fn insert<K: Ord + Clone, V, S: Store<K, V>>(
    store: &mut S,
    root: NodeId,
    key: K,
    value: V,
    order: usize,
) -> (Option<V>, NodeId) {
    match insert_into(store, root, key, value, order) {
        Insert::Replaced(old) => (Some(old), root),
        Insert::Done => (None, root),
        Insert::Split { sep, right } => {
            let mut keys = Vec::with_capacity(order);
            keys.push(sep);
            let mut children = Vec::with_capacity(order + 1);
            children.push(root);
            children.push(right);
            let new_root = store.alloc(Node::Internal(Internal { keys, children }));
            (None, new_root)
        }
    }
}

/// Insert into the subtree at `id`, returning the outcome for the parent.
fn insert_into<K: Ord + Clone, V, S: Store<K, V>>(
    store: &mut S,
    id: NodeId,
    key: K,
    value: V,
    order: usize,
) -> Insert<K, V> {
    let descend = match store.get(id) {
        Node::Internal(internal) => {
            let idx = internal.child_index(&key);
            Some((internal.children[idx], idx))
        }
        Node::Leaf(_) => None,
    };
    match descend {
        Some((child, idx)) => match insert_into(store, child, key, value, order) {
            Insert::Split { sep, right } => absorb_split(store, id, idx, sep, right, order),
            settled => settled,
        },
        None => insert_leaf(store, id, key, value, order),
    }
}

/// Insert into the leaf at `id`, splitting it if the entry overflows `order`.
fn insert_leaf<K: Ord + Clone, V, S: Store<K, V>>(
    store: &mut S,
    id: NodeId,
    key: K,
    value: V,
    order: usize,
) -> Insert<K, V> {
    /// Result computed while the leaf is borrowed; the split's alloc happens
    /// after the borrow is released.
    enum Step<K, V> {
        Replaced(V),
        Done,
        Split { keys: Vec<K>, vals: Vec<V>, sep: K },
    }

    let step = if let Node::Leaf(leaf) = store.get_mut(id) {
        match leaf.keys.binary_search(&key) {
            Ok(i) => Step::Replaced(core::mem::replace(&mut leaf.vals[i], value)),
            Err(i) => {
                leaf.keys.insert(i, key);
                leaf.vals.insert(i, value);
                if leaf.keys.len() < order {
                    Step::Done
                } else {
                    // Copy the right half's first key up as the separator: the
                    // entry it names stays in the leaf where lookups find it.
                    let mid = leaf.keys.len() / 2;
                    let keys = leaf.keys.split_off(mid);
                    let vals = leaf.vals.split_off(mid);
                    let sep = keys[0].clone();
                    Step::Split { keys, vals, sep }
                }
            }
        }
    } else {
        // Unreachable: `id` was peeked as a leaf before this call.
        Step::Done
    };

    match step {
        Step::Replaced(old) => Insert::Replaced(old),
        Step::Done => Insert::Done,
        Step::Split { keys, vals, sep } => {
            let right = store.alloc(Node::Leaf(Leaf { keys, vals }));
            Insert::Split { sep, right }
        }
    }
}

/// Absorb a child's split into the internal node at `id`: insert the promoted
/// separator at `idx` and the new child after it, splitting this node if that
/// overflows `order`.
fn absorb_split<K: Ord + Clone, V, S: Store<K, V>>(
    store: &mut S,
    id: NodeId,
    idx: usize,
    sep: K,
    right: NodeId,
    order: usize,
) -> Insert<K, V> {
    enum Step<K> {
        Done,
        Split {
            keys: Vec<K>,
            children: Vec<NodeId>,
            sep: K,
        },
    }

    let step = if let Node::Internal(internal) = store.get_mut(id) {
        internal.keys.insert(idx, sep);
        internal.children.insert(idx + 1, right);
        if internal.keys.len() < order {
            Step::Done
        } else {
            // Move the median up: an internal separator routes only, so it need
            // not be kept in either half.
            let mid = internal.keys.len() / 2;
            let children = internal.children.split_off(mid + 1);
            let keys = internal.keys.split_off(mid + 1);
            let sep = internal.keys.remove(mid);
            Step::Split {
                keys,
                children,
                sep,
            }
        }
    } else {
        Step::Done
    };

    match step {
        Step::Done => Insert::Done,
        Step::Split {
            keys,
            children,
            sep,
        } => {
            let right = store.alloc(Node::Internal(Internal { keys, children }));
            Insert::Split { sep, right }
        }
    }
}

/// Remove `key`, returning its value (if present) and the tree's root, which
/// changes only when the tree collapses a level.
pub(crate) fn remove<K: Ord + Clone, V, S: Store<K, V>>(
    store: &mut S,
    root: NodeId,
    key: &K,
    min_keys: usize,
) -> (Option<V>, NodeId) {
    let removed = remove_from(store, root, key, min_keys);
    let new_root = if removed.is_some() {
        collapse_root(store, root)
    } else {
        root
    };
    (removed, new_root)
}

/// Remove `key` from the subtree at `id`, restoring an under-full child's
/// minimum occupancy on the way back up.
fn remove_from<K: Ord + Clone, V, S: Store<K, V>>(
    store: &mut S,
    id: NodeId,
    key: &K,
    min_keys: usize,
) -> Option<V> {
    let descend = match store.get(id) {
        Node::Internal(internal) => {
            let idx = internal.child_index(key);
            Some((internal.children[idx], idx))
        }
        Node::Leaf(_) => None,
    };
    match descend {
        None => {
            if let Node::Leaf(leaf) = store.get_mut(id) {
                leaf.remove(key)
            } else {
                None
            }
        }
        Some((child, idx)) => {
            let removed = remove_from(store, child, key, min_keys);
            if removed.is_some() && store.get(child).keys_len() < min_keys {
                rebalance(store, id, idx, min_keys);
            }
            removed
        }
    }
}

/// When the root is an internal node left with a single child, that child
/// becomes the new root and the tree loses a level. Returns the (possibly new)
/// root id.
fn collapse_root<K, V, S: Store<K, V>>(store: &mut S, root: NodeId) -> NodeId {
    let only_child = match store.get(root) {
        Node::Internal(internal) if internal.children.len() == 1 => Some(internal.children[0]),
        _ => None,
    };
    match only_child {
        Some(child) => {
            let _old_root = store.reclaim(root);
            child
        }
        None => root,
    }
}

/// Restore the minimum occupancy of the under-full child at `idx`: borrow from a
/// sibling with one to spare, else merge with a sibling.
fn rebalance<K: Ord + Clone, V, S: Store<K, V>>(
    store: &mut S,
    parent: NodeId,
    idx: usize,
    min_keys: usize,
) {
    let (left, child, right) = match store.get(parent) {
        Node::Internal(internal) => {
            let n = internal.children.len();
            let left = if idx > 0 {
                Some(internal.children[idx - 1])
            } else {
                None
            };
            let right = if idx + 1 < n {
                Some(internal.children[idx + 1])
            } else {
                None
            };
            (left, internal.children[idx], right)
        }
        Node::Leaf(_) => return,
    };

    if let Some(sibling) = left {
        if store.get(sibling).keys_len() > min_keys {
            borrow_from_left(store, parent, idx, sibling, child);
            return;
        }
    }
    if let Some(sibling) = right {
        if store.get(sibling).keys_len() > min_keys {
            borrow_from_right(store, parent, idx, child, sibling);
            return;
        }
    }
    if let Some(sibling) = left {
        merge(store, parent, idx - 1, sibling, child);
    } else if let Some(sibling) = right {
        merge(store, parent, idx, child, sibling);
    }
}

/// Move one entry from the left sibling into the front of the child, updating
/// the separator between them.
fn borrow_from_left<K: Ord + Clone, V, S: Store<K, V>>(
    store: &mut S,
    parent: NodeId,
    idx: usize,
    left: NodeId,
    child: NodeId,
) {
    if store.get(child).as_leaf().is_some() {
        let moved = if let Node::Leaf(node) = store.get_mut(left) {
            match (node.keys.pop(), node.vals.pop()) {
                (Some(k), Some(v)) => Some((k, v)),
                _ => None,
            }
        } else {
            None
        };
        if let Some((k, v)) = moved {
            if let Node::Leaf(node) = store.get_mut(child) {
                node.keys.insert(0, k.clone());
                node.vals.insert(0, v);
            }
            if let Node::Internal(node) = store.get_mut(parent) {
                node.keys[idx - 1] = k;
            }
        }
    } else {
        let sep = match store.get(parent) {
            Node::Internal(node) => node.keys[idx - 1].clone(),
            Node::Leaf(_) => return,
        };
        let (moved_child, new_sep) = if let Node::Internal(node) = store.get_mut(left) {
            (node.children.pop(), node.keys.pop())
        } else {
            (None, None)
        };
        if let Node::Internal(node) = store.get_mut(child) {
            node.keys.insert(0, sep);
            if let Some(moved) = moved_child {
                node.children.insert(0, moved);
            }
        }
        if let Some(ns) = new_sep {
            if let Node::Internal(node) = store.get_mut(parent) {
                node.keys[idx - 1] = ns;
            }
        }
    }
}

/// Move one entry from the right sibling into the back of the child, updating
/// the separator between them.
fn borrow_from_right<K: Ord + Clone, V, S: Store<K, V>>(
    store: &mut S,
    parent: NodeId,
    idx: usize,
    child: NodeId,
    right: NodeId,
) {
    if store.get(child).as_leaf().is_some() {
        let moved = if let Node::Leaf(node) = store.get_mut(right) {
            if node.keys.is_empty() {
                None
            } else {
                Some((node.keys.remove(0), node.vals.remove(0)))
            }
        } else {
            None
        };
        if let Some((k, v)) = moved {
            if let Node::Leaf(node) = store.get_mut(child) {
                node.keys.push(k);
                node.vals.push(v);
            }
            let new_sep = match store.get(right) {
                Node::Leaf(node) => node.keys.first().cloned(),
                Node::Internal(_) => None,
            };
            if let Some(ns) = new_sep {
                if let Node::Internal(node) = store.get_mut(parent) {
                    node.keys[idx] = ns;
                }
            }
        }
    } else {
        let sep = match store.get(parent) {
            Node::Internal(node) => node.keys[idx].clone(),
            Node::Leaf(_) => return,
        };
        let moved_child = if let Node::Internal(node) = store.get_mut(right) {
            if node.children.is_empty() {
                None
            } else {
                Some(node.children.remove(0))
            }
        } else {
            None
        };
        if let Node::Internal(node) = store.get_mut(child) {
            node.keys.push(sep);
            if let Some(moved) = moved_child {
                node.children.push(moved);
            }
        }
        let new_sep = if let Node::Internal(node) = store.get_mut(right) {
            if node.keys.is_empty() {
                None
            } else {
                Some(node.keys.remove(0))
            }
        } else {
            None
        };
        if let Some(ns) = new_sep {
            if let Node::Internal(node) = store.get_mut(parent) {
                node.keys[idx] = ns;
            }
        }
    }
}

/// Merge the child at `sep_idx + 1` into the child at `sep_idx`, consuming the
/// separator between them. This node loses one separator and one child, and the
/// merged-away node is reclaimed.
fn merge<K: Ord + Clone, V, S: Store<K, V>>(
    store: &mut S,
    parent: NodeId,
    sep_idx: usize,
    left: NodeId,
    right: NodeId,
) {
    let sep = if let Node::Internal(node) = store.get_mut(parent) {
        let sep = node.keys.remove(sep_idx);
        let _removed_child = node.children.remove(sep_idx + 1);
        sep
    } else {
        return;
    };
    let right_node = store.reclaim(right);
    match (store.get_mut(left), right_node) {
        (Node::Leaf(node), Node::Leaf(mut right)) => {
            node.keys.append(&mut right.keys);
            node.vals.append(&mut right.vals);
        }
        (Node::Internal(node), Node::Internal(mut right)) => {
            node.keys.push(sep);
            node.keys.append(&mut right.keys);
            node.children.append(&mut right.children);
        }
        // Siblings in a balanced tree are always the same kind.
        _ => {}
    }
}

/// Build a balanced tree bottom-up from `entries`, which must be sorted strictly
/// ascending by key. Returns the root id and the entry count.
///
/// Nodes are filled as evenly as the order allows, so every node lands within
/// the legal occupancy range and the result is a valid, balanced B+tree.
pub(crate) fn bulk_load<K: Clone, V, S: Store<K, V>>(
    store: &mut S,
    entries: Vec<(K, V)>,
    order: usize,
) -> (NodeId, usize) {
    let len = entries.len();
    if len == 0 {
        return (store.alloc(Node::empty_leaf()), 0);
    }

    // Build the leaf level: split entries into evenly sized leaves.
    let mut level: Vec<(NodeId, K)> = Vec::new();
    let mut entries = entries.into_iter();
    for size in even_chunks(len, order - 1) {
        let mut keys = Vec::with_capacity(size);
        let mut vals = Vec::with_capacity(size);
        for _ in 0..size {
            if let Some((k, v)) = entries.next() {
                keys.push(k);
                vals.push(v);
            }
        }
        let min = keys[0].clone();
        let id = store.alloc(Node::Leaf(Leaf { keys, vals }));
        level.push((id, min));
    }

    // Build internal levels until a single root remains.
    while level.len() > 1 {
        let count = level.len();
        let mut children = level.into_iter();
        let mut parents: Vec<(NodeId, K)> = Vec::new();
        for size in even_chunks(count, order) {
            let mut keys = Vec::with_capacity(size.saturating_sub(1));
            let mut ids = Vec::with_capacity(size);
            let mut min: Option<K> = None;
            for _ in 0..size {
                if let Some((id, child_min)) = children.next() {
                    // The first child's subtree min becomes this node's min; each
                    // later child's min becomes the separator to its left.
                    if min.is_none() {
                        min = Some(child_min);
                    } else {
                        keys.push(child_min);
                    }
                    ids.push(id);
                }
            }
            let parent = store.alloc(Node::Internal(Internal {
                keys,
                children: ids,
            }));
            if let Some(min) = min {
                parents.push((parent, min));
            }
        }
        level = parents;
    }

    (level[0].0, len)
}

/// Split `total` items into evenly sized chunks of at most `max` each: the
/// chunk count is the minimum needed, and the items are spread so no chunk falls
/// below the others by more than one. Returns each chunk's size.
fn even_chunks(total: usize, max: usize) -> Vec<usize> {
    if total == 0 {
        return Vec::new();
    }
    let chunks = total.div_ceil(max);
    let base = total / chunks;
    let remainder = total % chunks;
    let mut sizes = Vec::with_capacity(chunks);
    for i in 0..chunks {
        sizes.push(if i < remainder { base + 1 } else { base });
    }
    sizes
}
