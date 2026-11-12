<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br>
    <b>index-db</b>
    <br>
    <sub><sup>B+TREE INDEXING PRIMITIVE</sup></sub>
</h1>

<div align="center">
    <a href="https://crates.io/crates/index-db"><img alt="Crates.io" src="https://img.shields.io/crates/v/index-db"></a>
    <a href="https://crates.io/crates/index-db" alt="Download index-db"><img alt="Crates.io Downloads" src="https://img.shields.io/crates/d/index-db?color=%230099ff"></a>
    <a href="https://docs.rs/index-db" title="index-db Documentation"><img alt="docs.rs" src="https://img.shields.io/docsrs/index-db"></a>
    <a href="https://github.com/jamesgober/index-db/actions"><img alt="GitHub CI" src="https://github.com/jamesgober/index-db/actions/workflows/ci.yml/badge.svg"></a>
    <a href="https://github.com/rust-lang/rfcs/blob/master/text/2495-min-rust-version.md" title="MSRV"><img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.85%2B-blue"></a>
</div>

<br>

<div align="left">
    <p>
        <strong>index-db</strong> is a <b>B+tree indexing primitive</b> for storage engines: an <b>in-memory ordered map</b> with fast point lookups, <b>efficient range scans</b> forward and in reverse, and bulk construction from sorted input.
    </p>
    <p>
        The tree lives on the heap and is not persisted. It is <code>Sync</code>, so any number of threads can read it at once; writes take <code>&amp;mut self</code>, so concurrent writers need a lock around the whole tree (for example <code>RwLock&lt;BPlusTree&lt;K, V&gt;&gt;</code>). Nodes are reached through an internal storage seam, the groundwork for a planned page-backed backend over <code>page-db</code> with persistence and latch-coupled concurrent writers; that backend is not part of 1.0.
    </p>
    <br>
    <hr>
    <p>
        <strong>MSRV is 1.85+</strong> (Rust 2024 edition). In-memory ordered B+tree. Range scans. Concurrent reads.
    </p>
    <blockquote>
        <strong>Status: 1.0, stable, API frozen until 2.0.</strong> <code>v1.0</code> is the stable in-memory ordered map (ordered storage, point lookups, deletion, forward and reverse range scans, and bulk construction from sorted input), hardened with adversarial, large-scale, and sustained soak tests. The tree is <code>Sync</code>, so any number of threads may read it at once. Paging, persistence, and concurrent writers are planned, not shipped: a page-backed backend over <code>page-db</code> is on the roadmap and will be additive. See <a href="./dev/ROADMAP.md"><code>dev/ROADMAP.md</code></a>.
    </blockquote>
</div>

<hr>
<br>

<h2>What it does</h2>

**Available now (`v1.0`):**

- **Ordered B+tree**: keys kept in sorted order; logarithmic point lookup, insert, and delete
- **Self-balancing**: nodes split on insert and borrow or merge on delete, so the tree stays balanced at every depth on its own
- **Range scans**: forward and reverse iteration over a key range, the operation B+trees exist for
- **Bulk load**: build a balanced tree bottom-up from sorted input in one fast pass
- **Concurrent reads**: the tree is `Sync`, so any number of threads can read it at once; writers need `&mut` access
- **Storage seam**: nodes are addressed by id through an internal node store, so the same algorithm can later run over a paged, persistent backend
- **`no_std` support**: depends only on `alloc`; the `std` feature is optional

**On the roadmap, not in 1.0** (see [`dev/ROADMAP.md`](./dev/ROADMAP.md)):

- **Page-backed, persistent, concurrent backend**: nodes as `page-db` pages, persisted and cached by the pager, with latch coupling (crabbing) over the pager's frame guards so many readers and writers can traverse at once. Until it ships, the tree is in memory only and is lost when the process exits.

<br>
<hr>
<br>

## Installation

```toml
[dependencies]
index-db = "1"
```

<br>

## Quick Start

```rust
use index_db::BPlusTree;

let mut index = BPlusTree::new();

// Insert key/value pairs in any order; the tree keeps them sorted.
index.insert(3_u32, "three");
index.insert(1, "one");
index.insert(2, "two");

// Point lookups return a reference to the value, or None.
assert_eq!(index.get(&2), Some(&"two"));
assert_eq!(index.get(&9), None);

// Re-inserting a key replaces and returns the old value.
assert_eq!(index.insert(1, "ONE"), Some("one"));

// Scan a key range in order (and in reverse).
let keys: Vec<_> = index.range(1..3).map(|(&k, _)| k).collect();
assert_eq!(keys, vec![1, 2]);

// Remove returns the value that was there.
assert_eq!(index.remove(&2), Some("two"));
assert_eq!(index.len(), 2);
```

Build a large index in one pass from sorted data:

```rust
use index_db::BPlusTree;

let index = BPlusTree::from_sorted((0..1_000_u32).map(|k| (k, k * k)));
assert_eq!(index.get(&30), Some(&900));
assert_eq!(index.len(), 1_000);
```

<br>

## API Overview

For the complete reference with examples for every method, see [`docs/API.md`](./docs/API.md). For benchmark baselines, see [`docs/PERFORMANCE.md`](./docs/PERFORMANCE.md).

| Method | Purpose |
|--------|---------|
| [`BPlusTree::new`](./docs/API.md#bplustreenew) | Create an empty tree |
| [`from_sorted`](./docs/API.md#bplustreefrom_sorted) | Bulk-build from sorted entries |
| [`insert`](./docs/API.md#bplustreeinsert) | Insert or replace a key's value |
| [`get`](./docs/API.md#bplustreeget) / [`contains_key`](./docs/API.md#bplustreecontains_key) | Point lookup / membership test |
| [`remove`](./docs/API.md#bplustreeremove) | Delete a key, returning its value |
| [`iter`](./docs/API.md#bplustreeiter) / [`range`](./docs/API.md#bplustreerange) | Ordered iteration / range scan, forward or reverse |
| [`len`](./docs/API.md#bplustreelen) / [`is_empty`](./docs/API.md#bplustreeis_empty) / [`height`](./docs/API.md#bplustreeheight) | Size and shape |
| [`clear`](./docs/API.md#bplustreeclear) | Remove every entry |

<br>
<hr>
<br>

## Where It Fits

`index-db` is the ordered-index layer. Today it is an in-memory map; related crates:

- [`page-db`](https://github.com/jamesgober/page-db): the paged storage substrate the planned page-backed backend will use (not a dependency of 1.0)
- [`lock-db`](https://github.com/jamesgober/lock-db): range locks for protecting scanned key ranges in a storage engine
- [`lsm-db`](https://github.com/jamesgober/lsm-db): an LSM storage engine, the write-optimized alternative to a B-tree index

It has no runtime dependencies, so it builds and tests standalone.

<br>

## Cross-Platform Support

Linux (x86_64, aarch64), macOS (x86_64, Apple Silicon), and Windows (x86_64) are first-class and verified by the CI matrix.

<br>

## Contributing

See [`CONTRIBUTING.md`](./CONTRIBUTING.md) and [`dev/DIRECTIVES.md`](./dev/DIRECTIVES.md). Before a PR: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` must be clean.

<br>

<div id="license">
    <h2>License</h2>
    <p>Licensed under either of</p>
    <ul>
        <li><b>Apache License, Version 2.0</b> &mdash; <a href="./LICENSE-APACHE">LICENSE-APACHE</a></li>
        <li><b>MIT License</b> &mdash; <a href="./LICENSE-MIT">LICENSE-MIT</a></li>
    </ul>
    <p>at your option.</p>
</div>

<div align="center">
  <h2></h2>
  <sup>COPYRIGHT <small>&copy;</small> 2026 <strong>JAMES GOBER.</strong></sup>
</div>
