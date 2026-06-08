//! # index-db
//!
//! A B+tree indexing primitive for storage engines: ordered keys mapped to
//! values, with fast point lookups, laid out as a tree of fixed-fan-out nodes
//! rather than on the heap one entry at a time.
//!
//! The one public type is [`BPlusTree`], an ordered map. Keys are kept sorted
//! across the tree, so a lookup is a binary search at each level and the height
//! grows only with the logarithm of the entry count. The node layout — sorted
//! keys packed into fixed-capacity arrays, internal nodes routing to children —
//! is the same structure a storage engine persists as an on-disk index; this
//! release keeps the tree in memory.
//!
//! ## Example
//!
//! ```
//! use index_db::BPlusTree;
//!
//! let mut index = BPlusTree::new();
//! index.insert(42_u32, "answer");
//! index.insert(7, "lucky");
//!
//! assert_eq!(index.get(&42), Some(&"answer"));
//! assert_eq!(index.get(&7), Some(&"lucky"));
//! assert_eq!(index.get(&13), None);
//! assert_eq!(index.len(), 2);
//! ```
//!
//! ## Scope
//!
//! This is the `v0.2.0` release: the B+tree core — search, insert, and node
//! splitting. Deletion, merge/redistribute, and forward and reverse range scans
//! land in `v0.3.0`; latch-coupled concurrent access in `v0.4.0`. See
//! `dev/ROADMAP.md`.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]
#![deny(unused_must_use)]
#![deny(unused_results)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::print_stdout)]
#![deny(clippy::print_stderr)]
#![deny(clippy::dbg_macro)]
#![forbid(unsafe_code)]

extern crate alloc;

mod node;
mod tree;

pub use crate::tree::BPlusTree;
