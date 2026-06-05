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
        <strong>index-db</strong> is a <b>B+tree indexing primitive</b> for storage engines: ordered keys mapped to values, with fast point lookups and <b>efficient range scans</b>, laid out over <b>paged storage</b> rather than the heap.
    </p>
    <p>
        Nodes are pages, so the tree persists and caches through a pager (it pairs with <code>page-db</code>), and it supports <b>concurrent access</b> via latch coupling (crabbing) so many readers and writers can traverse at once without locking the whole tree.
    </p>
    <br>
    <hr>
    <p>
        <strong>MSRV is 1.85+</strong> (Rust 2024 edition). Ordered B+tree. Range scans. Latch-coupled concurrent access.
    </p>
    <blockquote>
        <strong>Status: pre-1.0, in active development.</strong> This is the <code>v0.1.0</code> scaffold &mdash; structure, tooling, and CI gates are in place; the implementation lands across the 0.x series per <a href="./dev/ROADMAP.md"><code>dev/ROADMAP.md</code></a>. The public API is frozen at <code>1.0.0</code>.
    </blockquote>
</div>

<hr>
<br>

<h2>What it does</h2>

- **Ordered B+tree** &mdash; keys kept in sorted order; logarithmic point lookup, insert, and delete
- **Range scans** &mdash; forward and reverse iteration over a key range, the operation B+trees exist for
- **Paged layout** &mdash; internal and leaf nodes are fixed-size pages, persistable and cacheable via a pager
- **Concurrent access** &mdash; latch coupling (crabbing) for many-reader / many-writer traversal without a global lock
- **Bulk load** &mdash; build a balanced tree bottom-up from sorted input
- **Pluggable key ordering** &mdash; byte-ordered by default; custom comparators supported

<br>
<hr>
<br>

## Installation

```toml
[dependencies]
index-db = "0.1"
```

<br>

## API Overview

For the complete reference, see [`docs/API.md`](./docs/API.md).

- [`Ordered B+tree`](./docs/API.md)
- [`Range scans`](./docs/API.md)
- [`Paged layout`](./docs/API.md)
- [`Concurrent access`](./docs/API.md)

<br>
<hr>
<br>

## Where It Fits

`index-db` is the ordered-index layer. It builds on / pairs with:

- [`page-db`](https://github.com/jamesgober/page-db) &mdash; B+tree nodes are pages allocated and cached by the pager
- [`lock-db`](https://github.com/jamesgober/lock-db) &mdash; range locks protect scanned key ranges
- storage engines &mdash; the secondary-index and ordered-access structure above the pager
- [`lsm-db`](https://github.com/jamesgober/lsm-db) &mdash; a B-tree alternative to the LSM index for read-heavy workloads

It has no first-party dependencies, so it builds and tests standalone today.

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
