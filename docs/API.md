# index-db &mdash; API Reference

> Complete reference for every public item in `index-db`, with examples.
> **Status: 1.0 — stable, API frozen until 2.0.** The public surface below is
> stable. The only change planned for the 1.x line is an additive, defaulted store
> type parameter when the page-backed, concurrent backend lands. See
> [`dev/ROADMAP.md`](../dev/ROADMAP.md).

## Table of Contents

- [Overview](#overview)
- [Installation](#installation)
- [`BPlusTree`](#bplustree)
  - [`BPlusTree::new`](#bplustreenew)
  - [`BPlusTree::from_sorted`](#bplustreefrom_sorted)
  - [`BPlusTree::insert`](#bplustreeinsert)
  - [`BPlusTree::get`](#bplustreeget)
  - [`BPlusTree::contains_key`](#bplustreecontains_key)
  - [`BPlusTree::remove`](#bplustreeremove)
  - [`BPlusTree::iter`](#bplustreeiter)
  - [`BPlusTree::range`](#bplustreerange)
  - [`BPlusTree::len`](#bplustreelen)
  - [`BPlusTree::is_empty`](#bplustreeis_empty)
  - [`BPlusTree::height`](#bplustreeheight)
  - [`BPlusTree::clear`](#bplustreeclear)
  - [`Default`](#default)
  - [`IntoIterator`](#intoiterator)
- [`Iter`](#iter)
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

As of `v1.0.0` the in-memory ordered-map surface is stable and frozen until 2.0:
search, insert, delete (with merge and redistribute), ordered iteration, forward
and reverse range scans, and bulk construction from sorted input. The tree is
`Sync`, so any number of threads may read it concurrently. A page-backed,
concurrent (write-side) backend arrives in a 1.x release; node access already
runs through an internal storage seam so it is additive.

---

## Installation

```toml
[dependencies]
index-db = "1"
```

The crate is `no_std`-compatible. It uses `alloc` internally, so the only thing
the default `std` feature adds today is the standard prelude; disable it for a
`no_std` target:

```toml
[dependencies]
index-db = { version = "1", default-features = false }
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

### `BPlusTree::from_sorted`

```rust
pub fn from_sorted<I: IntoIterator<Item = (K, V)>>(entries: I) -> BPlusTree<K, V>
```

Build a tree in bulk from entries already sorted by key.

**Parameters:**

- `entries` — an iterator of `(key, value)` pairs. When the keys are strictly
  ascending and unique, the tree is built bottom-up in a single fast pass, packed
  densely. Otherwise it falls back to inserting one entry at a time, so the result
  is always a correct tree; on that path a later duplicate key overwrites an
  earlier one.

**Returns:** the populated tree.

Use this when the data is already in order — loading from a sorted file, a range
scan of another store, or `BTreeMap` keys. It is much faster than repeated
`insert` and avoids the incremental splitting those would cause.

```rust
use index_db::BPlusTree;

// Sorted input takes the fast bottom-up path.
let index = BPlusTree::from_sorted((0..1_000_u32).map(|k| (k, k * k)));
assert_eq!(index.len(), 1_000);
assert_eq!(index.get(&30), Some(&900));
assert_eq!(index.get(&999), Some(&998_001));
```

The keys of a `BTreeMap` are already sorted, so they bulk-load directly:

```rust
use std::collections::BTreeMap;
use index_db::BPlusTree;

let mut source = BTreeMap::new();
source.insert(10_u32, "a");
source.insert(20, "b");
source.insert(30, "c");

let index = BPlusTree::from_sorted(source);
assert_eq!(index.get(&20), Some(&"b"));
assert_eq!(index.len(), 3);
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

### `BPlusTree::remove`

```rust
pub fn remove(&mut self, key: &K) -> Option<V>
```

Remove `key`, returning its value.

**Parameters:**

- `key` — a reference to the key to remove.

**Returns:** `Some(value)` if the key was present (the removed value), or `None`
if the tree held no such key.

Removing keeps the tree balanced. A node left below half full borrows an entry
from a sibling or merges with one, and when the root drops to a single child the
tree collapses a level. Every leaf stays at the same depth, so lookups, scans,
and further deletes remain logarithmic.

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
index.insert(1_u32, "a");
index.insert(2, "b");

assert_eq!(index.remove(&1), Some("a")); // returns the removed value
assert_eq!(index.remove(&1), None);       // already gone
assert_eq!(index.len(), 1);
```

Deleting every key returns the tree to a single empty leaf, ready for reuse:

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
for k in 0..1_000_u32 {
    index.insert(k, k);
}
for k in 0..1_000_u32 {
    assert_eq!(index.remove(&k), Some(k));
}
assert!(index.is_empty());
assert_eq!(index.height(), 1);
```

---

### `BPlusTree::iter`

```rust
pub fn iter(&self) -> Iter<'_, K, V>
```

Iterate over every entry in ascending key order.

No parameters.

**Returns:** an [`Iter`](#iter) yielding `(&K, &V)`. It is a
[`DoubleEndedIterator`], so `.rev()` walks the entries in descending order and
the iterator can be driven from both ends at once.

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
index.insert(2_u32, "b");
index.insert(1, "a");
index.insert(3, "c");

let entries: Vec<_> = index.iter().map(|(&k, &v)| (k, v)).collect();
assert_eq!(entries, vec![(1, "a"), (2, "b"), (3, "c")]);
```

Descending order with `.rev()`:

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
for k in 0..5_u32 {
    index.insert(k, k);
}
let keys: Vec<_> = index.iter().rev().map(|(&k, _)| k).collect();
assert_eq!(keys, vec![4, 3, 2, 1, 0]);
```

`&tree` is also iterable directly (see [`IntoIterator`](#intoiterator)):

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
index.insert(1_u32, 10);
index.insert(2, 20);

let mut total = 0;
for (_, &v) in &index {
    total += v;
}
assert_eq!(total, 30);
```

---

### `BPlusTree::range`

```rust
pub fn range<R: RangeBounds<K>>(&self, range: R) -> Iter<'_, K, V>
```

Iterate over the entries whose keys fall in `range`, in ascending key order.

**Parameters:**

- `range` — any standard range expression over the key order: `a..b`, `a..=b`,
  `..b`, `a..`, or `..`.

**Returns:** an [`Iter`](#iter) over the matching entries. Like
[`iter`](#bplustreeiter) it is double-ended, so a range can be scanned forward or
in reverse.

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
for k in 0..10_u32 {
    index.insert(k, k);
}

// Half-open range.
let a: Vec<_> = index.range(3..7).map(|(&k, _)| k).collect();
assert_eq!(a, vec![3, 4, 5, 6]);

// Inclusive range.
let b: Vec<_> = index.range(3..=7).map(|(&k, _)| k).collect();
assert_eq!(b, vec![3, 4, 5, 6, 7]);

// Open-ended.
let c: Vec<_> = index.range(8..).map(|(&k, _)| k).collect();
assert_eq!(c, vec![8, 9]);
```

A range scanned in reverse, and an empty range:

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
for k in 0..10_u32 {
    index.insert(k, k);
}
let rev: Vec<_> = index.range(2..=5).rev().map(|(&k, _)| k).collect();
assert_eq!(rev, vec![5, 4, 3, 2]);

let empty: Vec<_> = index.range(100..200).map(|(&k, _)| k).collect();
assert!(empty.is_empty());
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

### `IntoIterator`

```rust
impl<'a, K, V> IntoIterator for &'a BPlusTree<K, V>
```

A shared reference to a tree iterates over its entries, so `for (k, v) in &tree`
works and the tree is reusable afterward. Equivalent to
[`iter`](#bplustreeiter); the item type is `(&K, &V)` and the iterator is
[`Iter`](#iter).

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
index.insert(1_u32, "a");
index.insert(2, "b");

let collected: Vec<_> = (&index).into_iter().map(|(&k, _)| k).collect();
assert_eq!(collected, vec![1, 2]);
```

---

## `Iter`

```rust
pub struct Iter<'a, K, V> { /* private */ }
```

The iterator returned by [`BPlusTree::iter`](#bplustreeiter) and
[`BPlusTree::range`](#bplustreerange). It yields `(&'a K, &'a V)` in ascending key
order and implements both `Iterator` and `DoubleEndedIterator`, so it composes
with the standard iterator adapters (`map`, `filter`, `take`, `rev`, ...) and can
be consumed from either end.

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();
for k in 0..6_u32 {
    index.insert(k, k);
}

// Standard adapters apply.
let evens: Vec<_> = index.iter().filter(|(&k, _)| k % 2 == 0).map(|(&k, _)| k).collect();
assert_eq!(evens, vec![0, 2, 4]);

// Front and back at once.
let mut it = index.range(1..5);
assert_eq!(it.next().map(|(&k, _)| k), Some(1));
assert_eq!(it.next_back().map(|(&k, _)| k), Some(4));
```

---

## Type parameters and bounds

`BPlusTree<K, V>` is generic over the key type `K` and value type `V`.

- [`get`](#bplustreeget), [`contains_key`](#bplustreecontains_key), and
  [`range`](#bplustreerange) require `K: Ord` — navigating and bounding the tree
  needs a total order.
- [`insert`](#bplustreeinsert), [`remove`](#bplustreeremove), and
  [`from_sorted`](#bplustreefrom_sorted) additionally require `K: Clone`. A B+tree
  copies a separator key up into the parent when a leaf splits, and rewrites
  separators when nodes borrow on delete, so the key type must be cloneable. Keys
  such as integers and short strings clone cheaply.
- The structural methods ([`new`](#bplustreenew), [`len`](#bplustreelen),
  [`is_empty`](#bplustreeis_empty), [`height`](#bplustreeheight),
  [`clear`](#bplustreeclear), [`iter`](#bplustreeiter), `Default`) place no bound
  on `K` or `V`.

Values (`V`) are never required to be `Ord`, `Clone`, or anything else.

---

## Complexity

For a tree of `n` entries with node fan-out `b`:

| Operation | Time | Allocations |
|-----------|------|-------------|
| `get` / `contains_key` | `O(log n)` | none |
| `insert` | `O(log n)` | amortized; only on a node split |
| `remove` | `O(log n)` | none |
| `from_sorted` (sorted input) | `O(n)` | one pass, no per-entry splitting |
| `iter` / `range` | `O(log n)` to start, then `O(1)` per entry | one path stack per cursor |
| `len` / `is_empty` / `height` | `O(1)` / `O(1)` / `O(log n)` | none |
| `clear` | `O(n)` (drops entries) | none |

A lookup performs one binary search per level — at most `log_b(n)` levels, each a
search over up to `b - 1` keys. Lookups touch only keys, never values, and
allocate nothing. A range scan seeks to its start in `O(log n)` and then yields
each entry in amortized `O(1)`.

---

## Feature flags

| Feature | Default | Description |
|---------|---------|-------------|
| `std` | yes | Use the standard library. With it disabled the crate is `no_std` (it always relies on `alloc`). |

index-db has no runtime dependencies.

---

<sub>Copyright &copy; 2026 <strong>James Gober</strong>.</sub>
