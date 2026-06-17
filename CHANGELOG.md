# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---


## [Unreleased]

## [1.0.0] - 2026-06-08

First stable release: the in-memory ordered B+tree. The public API is frozen
until 2.0.

### Changed

- Removed the inert `serde` feature and its optional dependency. It gated no code
  (there was no `Serialize`/`Deserialize` implementation) and serde is not
  index-db's persistence path — the page-backed backend is. index-db now has no
  runtime dependencies.

### Stable surface (frozen until 2.0)

- `BPlusTree<K, V>`: `new`, `from_sorted`, `insert`, `get`, `contains_key`,
  `remove`, `iter`, `range`, `len`, `is_empty`, `height`, `clear`.
- Trait impls: `Default`, `IntoIterator for &BPlusTree`.
- `Iter<'a, K, V>`: `Iterator` + `DoubleEndedIterator`, item `(&K, &V)`.

### Notes

- The page-backed, concurrent (write-side) backend — a `PageStore` behind the
  storage seam, with latch coupling over `page-db`'s page guards — will arrive in
  a 1.x release. The seam makes it an additive, non-breaking change. The in-memory
  tree is `Sync`, so concurrent reads work today.

## [0.6.0] - 2026-06-08

Alpha. The API stays frozen; this release broadens coverage to a sustained,
consumer-shaped workload and records the benchmark baselines.

### Added

- `tests/soak.rs`: a long, read-heavy mixed workload (point lookups, range scans,
  inserts, deletes) over an evolving working set — 200,000 deterministic
  operations checked against a `BTreeMap` reference per operation and in full at
  checkpoints, then drained to an empty tree. The shape a storage engine drives a
  secondary index with.
- `docs/PERFORMANCE.md`: benchmark baselines for point lookups, inserts, removes,
  range scans, and bulk load, with method and interpretation.

## [0.5.0] - 2026-06-08

Hardening and the public API freeze. No new public surface — this release adds
adversarial and large-scale stress tests and locks the API ahead of 1.0.

### Added

- Adversarial unit and property tests over pathological key distributions:
  ascending / descending / zigzag inserts against opposite-order and middle-out
  deletes, clustered keys, repeated overwrites, and a small-order adversarial
  property test, all checking the structural invariants after every delete.
- `tests/stress.rs`: large scattered insert/delete (50k), sustained
  insert/delete churn, bulk-load then heavy delete, adversarial string keys, and
  a concurrent-reader test that runs eight threads traversing a shared tree at
  once (the in-memory tree is `Sync`).

### Notes

- **The public API is frozen** as of this release: `BPlusTree::{new, from_sorted,
  insert, get, contains_key, remove, iter, range, len, is_empty, height, clear}`,
  the `Default` and `IntoIterator for &BPlusTree` impls, and the `Iter` iterator.
  The only change planned before 1.0 is an additive, defaulted store type
  parameter when the page-backed backend lands.

## [0.4.0] - 2026-06-08

Bulk construction and an internal storage seam. The in-memory ordered-map surface
is now feature-frozen. Node access was routed through an internal node store so a
page-backed, concurrent backend can be added later without changing the tree
algorithm.

### Added

- `BPlusTree::from_sorted` — build a tree bottom-up from entries already sorted by
  key, much faster than inserting one at a time. Unsorted or duplicate input falls
  back to ordinary insertion, so the result is always correct.
- `examples/bulk_load.rs`, and benchmarks for range scans and bulk removal.

### Changed

- Internal: nodes are addressed by id through a `NodeStore` seam rather than owned
  inline, so the same search / insert / delete / bulk-load logic can drive a
  different backend. The in-memory backend is the only one today. This is an
  internal refactor; the public API is unchanged.
- The id indirection costs roughly one extra cache miss per level on point
  lookups, so `get` is slower at large sizes than `v0.3.0` (about 12 ns at a
  thousand entries, 79 ns at a million, against 10 ns / 52 ns before); range scans
  and the other operations are essentially unchanged. The seam is the price of the
  page-backed backend, which is id-addressed by nature.

## [0.3.0] - 2026-06-08

Deletion and range scans, completing the ordered-map surface. Entries can be
removed with the tree rebalancing itself to stay balanced, and the contents can
be iterated or scanned over a key range, forward or in reverse.

### Added

- `BPlusTree::remove` — delete a key, returning its value. Under-full nodes
  borrow from a sibling or merge, and the root collapses a level when it drops to
  a single child; every leaf stays at the same depth.
- `BPlusTree::iter` — a double-ended iterator over all entries in ascending key
  order. Use `.rev()` for descending order.
- `BPlusTree::range` — a double-ended iterator over the entries in a key range,
  accepting any standard range expression (`a..b`, `a..=b`, `..b`, `a..`, `..`).
- `Iter`, the iterator type returned by `iter` and `range`, implementing
  `Iterator` and `DoubleEndedIterator` over `(&K, &V)`.
- `impl IntoIterator for &BPlusTree`, so `for (k, v) in &tree` works.
- Property tests for deletion (mixed insert/remove against `BTreeMap`, with the
  balance and minimum-occupancy invariants checked after every operation) and for
  iteration and range scans (forward, reverse, and arbitrary bounds vs
  `BTreeMap`).
- Benchmarks for range scans and bulk removal; an `examples/range_scan.rs`.

## [0.2.0] - 2026-06-08

The B+tree core: ordered storage, point lookups, and automatic node splitting. A
key/value map laid out as a tree of fixed-fan-out nodes that stays balanced as it
grows. Deletion, range scans, and concurrent access follow in later 0.x releases.

### Added

- `BPlusTree<K, V>`, an ordered map backed by a B+tree, with `new`, `insert`,
  `get`, `contains_key`, `len`, `is_empty`, `height`, `clear`, and a `Default`
  impl. `insert` returns the displaced value when a key is overwritten.
- Automatic leaf and internal node splitting with median promotion; the tree
  grows a level at the root and keeps every leaf at the same depth.
- `no_std` support: the crate relies only on `alloc`, with `std` optional.
- Property tests cross-checking the tree against `std::collections::BTreeMap`
  and verifying the structural invariants (balance, sorted keys, correct
  separators) after arbitrary insert sequences.
- Criterion benchmarks for point lookup and insert (`benches/index_bench.rs`).
- Runnable examples: `examples/basic.rs` and `examples/bulk_lookup.rs`.
- `docs/API.md` documents every public item with parameters and examples.

## [0.1.0] - 2026-06-05

Initial scaffold and repository bootstrap. No domain logic yet &mdash; this release establishes the structure, tooling, and quality gates the implementation will be built on.

### Added

- `Cargo.toml` with crate metadata, Rust 2024 edition, MSRV 1.85, dual `Apache-2.0 OR MIT` license.
- `README.md`, `docs/API.md`, `CONTRIBUTING.md`, and a documentation skeleton.
- `dev/DIRECTIVES.md` and `dev/ROADMAP.md` (committed engineering standards + plan).
- `REPS.md` compliance baseline; `deny.toml`, `clippy.toml`, `rustfmt.toml`.
- `.github/workflows/ci.yml` (Node 24 actions; fmt, clippy, test, doc, audit, deny) and `.github/FUNDING.yml`.

<!-- LINKS -->
[Unreleased]: https://github.com/jamesgober/index-db/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/jamesgober/index-db/compare/v0.6.0...v1.0.0
[0.6.0]: https://github.com/jamesgober/index-db/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/jamesgober/index-db/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/jamesgober/index-db/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/jamesgober/index-db/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/jamesgober/index-db/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jamesgober/index-db/releases/tag/v0.1.0
