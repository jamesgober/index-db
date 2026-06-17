//! Node storage: the seam between the B+tree algorithm and where its nodes live.
//!
//! The tree never owns its nodes directly. It names them by [`NodeId`] and asks
//! a [`NodeStore`] to resolve, allocate, and reclaim them. That indirection is
//! the whole point: the same insert / delete / search logic ([`crate::ops`])
//! runs unchanged over any backend that can store nodes by id.
//!
//! [`InMemoryStore`] is the backend shipped today — a slab of nodes on the heap
//! with a free list, single-threaded, allocation-light. A page-backed store over
//! `page-db`, where a node is a page and concurrency rides on the pager's frame
//! latches, is the planned second backend; because the algorithm already speaks
//! only `NodeId`, adding it is additive rather than a rewrite. The seam is kept
//! internal for now, so the public surface is just `BPlusTree`; the backend
//! becomes a type parameter when the second one lands.

use alloc::vec::Vec;

use crate::node::Node;

/// A handle to a node within a [`NodeStore`].
///
/// Opaque and `Copy`. A slab index for [`InMemoryStore`]; a different backend is
/// free to give it a different meaning.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct NodeId(pub(crate) u32);

/// The operations a backend provides to store a tree's nodes: resolve an id to a
/// node, allocate a new node, and reclaim a freed one.
pub(crate) trait NodeStore<K, V> {
    /// Store `node` and return a fresh id for it.
    fn alloc(&mut self, node: Node<K, V>) -> NodeId;

    /// Remove the node at `id`, returning it so its keys and values can be
    /// reused or dropped. The id may be handed back out by a later `alloc`.
    fn reclaim(&mut self, id: NodeId) -> Node<K, V>;

    /// Resolve `id` to its node.
    fn get(&self, id: NodeId) -> &Node<K, V>;

    /// Resolve `id` to its node for mutation.
    fn get_mut(&mut self, id: NodeId) -> &mut Node<K, V>;

    /// Drop every node and return a fresh empty-leaf root. Used to empty a tree
    /// without leaving its old nodes behind.
    fn reset(&mut self) -> NodeId;
}

/// In-memory node storage: a slab of nodes on the heap addressed by slab index,
/// with a free list so reclaimed slots are reused.
///
/// This is the backend behind [`BPlusTree`](crate::BPlusTree) today. It is
/// single-threaded and holds its nodes on the heap.
pub(crate) struct InMemoryStore<K, V> {
    /// All live and reclaimed node slots, addressed by [`NodeId`].
    slab: Vec<Node<K, V>>,
    /// Ids of reclaimed slots, available for reuse before the slab grows.
    free: Vec<NodeId>,
}

impl<K, V> InMemoryStore<K, V> {
    /// Create an empty store with no nodes.
    #[inline]
    pub(crate) fn new() -> Self {
        InMemoryStore {
            slab: Vec::new(),
            free: Vec::new(),
        }
    }
}

impl<K, V> NodeStore<K, V> for InMemoryStore<K, V> {
    fn alloc(&mut self, node: Node<K, V>) -> NodeId {
        if let Some(id) = self.free.pop() {
            self.slab[id.0 as usize] = node;
            id
        } else {
            let id = NodeId(self.slab.len() as u32);
            self.slab.push(node);
            id
        }
    }

    fn reclaim(&mut self, id: NodeId) -> Node<K, V> {
        // Leave a cheap empty leaf (no heap allocation) in the vacated slot and
        // record the id for reuse; the real node is handed back to the caller.
        let node = core::mem::replace(&mut self.slab[id.0 as usize], Node::empty_leaf());
        self.free.push(id);
        node
    }

    #[inline]
    fn get(&self, id: NodeId) -> &Node<K, V> {
        &self.slab[id.0 as usize]
    }

    #[inline]
    fn get_mut(&mut self, id: NodeId) -> &mut Node<K, V> {
        &mut self.slab[id.0 as usize]
    }

    fn reset(&mut self) -> NodeId {
        self.slab.clear();
        self.free.clear();
        self.alloc(Node::empty_leaf())
    }
}
