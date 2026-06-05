# index-db &mdash; API Reference

> Complete reference for every public item in `index-db`, with examples.
> **Status: pre-1.0.** Sections below describe the intended surface as it lands across the 0.x series (see [`dev/ROADMAP.md`](../dev/ROADMAP.md)).

## Table of Contents

- [Overview](#overview)
- [Ordered B+tree](#ordered-btree) _(planned)_
- [Range scans](#range-scans) _(planned)_
- [Paged layout](#paged-layout) _(planned)_
- [Concurrent access](#concurrent-access) _(planned)_
- [Bulk load](#bulk-load) _(planned)_
- [Pluggable key ordering](#pluggable-key-ordering) _(planned)_
- [Feature flags](#feature-flags)

---

## Overview

index-db is a B+tree indexing primitive for storage engines: ordered keys mapped to values, with fast point lookups and efficient range scans, laid out over paged storage rather than the heap.

---

### Ordered B+tree

_keys kept in sorted order; logarithmic point lookup, insert, and delete. Documented as this lands across the 0.x series._

### Range scans

_forward and reverse iteration over a key range, the operation B+trees exist for. Documented as this lands across the 0.x series._

### Paged layout

_internal and leaf nodes are fixed-size pages, persistable and cacheable via a pager. Documented as this lands across the 0.x series._

### Concurrent access

_latch coupling (crabbing) for many-reader / many-writer traversal without a global lock. Documented as this lands across the 0.x series._

### Bulk load

_build a balanced tree bottom-up from sorted input. Documented as this lands across the 0.x series._

### Pluggable key ordering

_byte-ordered by default; custom comparators supported. Documented as this lands across the 0.x series._

---

## Feature flags

| Feature | Default | Description |
|---------|---------|-------------|
| `std` | yes | Standard library. |
| `serde` | no | Serialization for public types. |

---

<sub>Copyright &copy; 2026 <strong>James Gober</strong>.</sub>
