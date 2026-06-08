# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---


## [Unreleased]

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
[Unreleased]: https://github.com/jamesgober/index-db/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/jamesgober/index-db/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jamesgober/index-db/releases/tag/v0.1.0
