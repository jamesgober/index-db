# index-db &mdash; API Reference

> Complete reference for every public item in `index-db`, with examples.
> **Status: pre-1.0.** This documents the surface shipped in `v0.2.0`; the
> remaining sections of the roadmap (deletion, range scans, concurrency) extend
> it across the 0.x series. See [`dev/ROADMAP.md`](../dev/ROADMAP.md).

## Table of Contents

- [Overview](#overview)
- [Installation](#installation)
- [`BPlusTree`](#bplustree)
  - [`BPlusTree::new`](#bplustreenew)
  - [`BPlusTree::insert`](#bplustreeinsert)
  - [`BPlusTree::get`](#bplustreeget)
  - [`BPlusTree::contains_key`](#bplustreecontains_key)
  - [`BPlusTree::len`](#bplustreelen)
  - [`BPlusTree::is_empty`](#bplustreeis_empty)
  - [`BPlusTree::height`](#bplustreeheight)
  - [`BPlusTree::clear`](#bplustreeclear)
  - [`Default`](#default)
- [Type parameters and bounds](#type-parameters-and-bounds)
- [Complexity](#complexity)
- [Feature flags](#feature-flags)

---

## Overview

index-db is a B+tree indexing primitive: an ordered map that keeps its keys
sorted across a tree of fixed-fan-out nodes. The single public type is
[`BPlusTree`](#bplustree). Keys live in sorted order, so a point lookup is one
binary search per level and the tree's height grows only with the logarithm of
the entry count.

The node layout — sorted keys packed into fixed-capacity arrays, internal nodes
routing to their children — is the structure a storage engine persists as an
on-disk index. This release keeps the tree in memory; the layout is the durable
one a pager will later back.

`v0.2.0` covers the B+tree core: search, insert, and node splitting. Deletion,
forward and reverse range scans, and latch-coupled concurrent access arrive in
later 0.x releases.

---

## Installation

```toml
[dependencies]
index-db = "0.2"
```

The crate is `no_std`-compatible. It uses `alloc` internally, so the only thing
the default `std` feature adds today is the standard prelude; disable it for a
`no_std` target:

```toml
[dependencies]
index-db = { version = "0.2", default-features = false }
```

---

## `BPlusTree`

```rust
pub struct BPlusTree<K, V> { /* private */ }
```

An ordered map backed by a B+tree. Construct one with
[`new`](#bplustreenew) or [`Default`](#default), fill it with
[`insert`](#bplustreeinsert), and read it back with [`get`](#bplustreeget) or
[`contains_key`](#bplustreecontains_key).

The tree owns its keys and values. A key may map to exactly one value;
re-inserting an existing key replaces the value and hands the old one back.

```rust
use index_db::BPlusTree;

let mut index: BPlusTree<u32, String> = BPlusTree::new();
index.insert(1, "one".to_string());
index.insert(2, "two".to_string());

assert_eq!(index.get(&1).map(String::as_str), Some("one"));
assert_eq!(index.len(), 2);
```

---

### `BPlusTree::new`

```rust
pub fn new() -> BPlusTree<K, V>
```

Create an empty tree with the default node fan-out.

No parameters. The fan-out (the maximum number of children per node) is fixed
internally at a cache-conscious default; a fresh tree is a single empty leaf, so
it allocates nothing for its nodes until the first insert.

**Returns:** an empty `BPlusTree<K, V>`.

```rust
use index_db::BPlusTree;

let index: BPlusTree<u64, &str> = BPlusTree::new();
assert!(index.is_empty());
assert_eq!(index.len(), 0);
```

A tree built up from many inserts stays balanced automatically:

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
for k in 0..10_000_u32 {
    index.insert(k, k * 2);
}
assert_eq!(index.get(&9_999), Some(&19_998));
```

---

### `BPlusTree::insert`

```rust
pub fn insert(&mut self, key: K, value: V) -> Option<V>
```

Insert `key` with `value`.

**Parameters:**

- `key` — the key to store. If it is already present, its value is replaced.
- `value` — the value to associate with `key`.

**Returns:** `Some(old_value)` if the key was already present (the displaced
value), or `None` if this is a new key.

Inserting may split a full node and, at the top, grow the tree one level taller.
Both happen transparently; every leaf stays at the same depth.

A new key returns `None`:

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
assert_eq!(index.insert(10_u32, "ten"), None);
assert_eq!(index.len(), 1);
```

Re-inserting an existing key replaces and returns the old value, and does not
change the length:

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
index.insert(10_u32, "ten");
assert_eq!(index.insert(10, "TEN"), Some("ten"));
assert_eq!(index.get(&10), Some(&"TEN"));
assert_eq!(index.len(), 1);
```

Keys can arrive in any order; the tree keeps them sorted internally:

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
for k in [5_u32, 1, 9, 3, 7, 2, 8, 4, 6] {
    index.insert(k, k);
}
assert_eq!(index.len(), 9);
assert_eq!(index.get(&1), Some(&1));
assert_eq!(index.get(&9), Some(&9));
```

---

### `BPlusTree::get`

```rust
pub fn get(&self, key: &K) -> Option<&V>
```

Look up the value stored under `key`.

**Parameters:**

- `key` — a reference to the key to find.

**Returns:** `Some(&value)` if the key is present, or `None` if it is absent.

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
index.insert("alpha", 1);
index.insert("beta", 2);

assert_eq!(index.get(&"alpha"), Some(&1));
assert_eq!(index.get(&"gamma"), None);
```

The returned reference borrows the tree, so it can be read without copying the
value out:

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
index.insert(1_u32, vec![10, 20, 30]);

if let Some(values) = index.get(&1) {
    assert_eq!(values.len(), 3);
    assert_eq!(values[0], 10);
}
```

---

### `BPlusTree::contains_key`

```rust
pub fn contains_key(&self, key: &K) -> bool
```

Test whether the tree holds an entry for `key`. Equivalent to
`get(key).is_some()`, but clearer at the call site when the value is not needed.

**Parameters:**

- `key` — a reference to the key to test.

**Returns:** `true` if the key is present, `false` otherwise.

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
index.insert(42_u32, "answer");

assert!(index.contains_key(&42));
assert!(!index.contains_key(&7));
```

---

### `BPlusTree::len`

```rust
pub fn len(&self) -> usize
```

The number of entries in the tree.

No parameters.

**Returns:** the entry count. Replacing an existing key does not change it.

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
assert_eq!(index.len(), 0);
index.insert(1_u32, "a");
index.insert(2, "b");
index.insert(1, "c"); // replaces, does not add
assert_eq!(index.len(), 2);
```

---

### `BPlusTree::is_empty`

```rust
pub fn is_empty(&self) -> bool
```

Whether the tree holds no entries. Equivalent to `len() == 0`.

No parameters.

**Returns:** `true` if there are no entries, `false` otherwise.

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
assert!(index.is_empty());
index.insert(1_u32, "a");
assert!(!index.is_empty());
```

---

### `BPlusTree::height`

```rust
pub fn height(&self) -> usize
```

The height of the tree in levels. A tree whose root is a leaf has height one;
each level of internal nodes above the leaves adds one more. Because the tree is
balanced, this is the number of nodes visited on any root-to-leaf path — the cost
of a point lookup measured in node visits.

No parameters.

**Returns:** the height in levels (always at least one).

This is an observability hook: it lets you watch the tree grow and confirm it
stays shallow.

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
assert_eq!(index.height(), 1); // a single empty leaf

for k in 0..100_000_u32 {
    index.insert(k, k);
}
// A hundred thousand keys still stand only a few levels tall.
assert!(index.height() >= 2);
assert!(index.height() <= 5);
```

---

### `BPlusTree::clear`

```rust
pub fn clear(&mut self)
```

Remove every entry, returning the tree to its empty state. The tree is reusable
afterward.

No parameters. No return value.

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
for k in 0..1_000_u32 {
    index.insert(k, k);
}
index.clear();

assert!(index.is_empty());
assert_eq!(index.get(&0), None);

// Still usable.
index.insert(1, 1);
assert_eq!(index.get(&1), Some(&1));
```

---

### `Default`

```rust
impl<K, V> Default for BPlusTree<K, V>
```

`BPlusTree::default()` is identical to [`new`](#bplustreenew): an empty tree with
the default fan-out.

```rust
use index_db::BPlusTree;

let index: BPlusTree<u32, u32> = BPlusTree::default();
assert!(index.is_empty());
```

---

## Type parameters and bounds

`BPlusTree<K, V>` is generic over the key type `K` and value type `V`.

- [`get`](#bplustreeget), [`contains_key`](#bplustreecontains_key) require
  `K: Ord` — lookups need a total order to navigate the tree.
- [`insert`](#bplustreeinsert) additionally requires `K: Clone`. A B+tree copies
  a separator key up into the parent when a leaf splits, so the key type must be
  cloneable. Keys such as integers and short strings clone cheaply.
- The structural methods ([`new`](#bplustreenew), [`len`](#bplustreelen),
  [`is_empty`](#bplustreeis_empty), [`height`](#bplustreeheight),
  [`clear`](#bplustreeclear), `Default`) place no bound on `K` or `V`.

Values (`V`) are never required to be `Ord`, `Clone`, or anything else.

---

## Complexity

For a tree of `n` entries with node fan-out `b`:

| Operation | Time | Allocations |
|-----------|------|-------------|
| `get` / `contains_key` | `O(log n)` | none |
| `insert` | `O(log n)` | amortized; only on a node split |
| `len` / `is_empty` / `height` | `O(1)` / `O(log n)` | none |
| `clear` | `O(n)` (drops entries) | none |

A lookup performs one binary search per level — at most `log_b(n)` levels, each a
search over up to `b - 1` keys. Lookups touch only keys, never values, and
allocate nothing.

---

## Feature flags

| Feature | Default | Description |
|---------|---------|-------------|
| `std` | yes | Use the standard library. With it disabled the crate is `no_std` (it always relies on `alloc`). |
| `serde` | no | Reserved for serialization of public types; not yet wired to a public surface. |

---

<sub>Copyright &copy; 2026 <strong>James Gober</strong>.</sub>
