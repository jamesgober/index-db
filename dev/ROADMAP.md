# index-db -- Roadmap

> Path from scaffold to a stable 1.0. Hard parts are front-loaded; each phase has hard exit criteria.
>
> **Anti-deferral rule:** no listed hard task moves to a later phase unless this file records the move and the reason.

---

## v0.1.0 -- Scaffold (DONE)

Compiles, CI green, structure correct, no domain logic.

- [x] Manifest, README, CHANGELOG, REPS, dual license, CI, deny, clippy, rustfmt, FUNDING.
- [x] API surface sketched in `docs/API.md`.

---

## v0.2.0 -- B+tree core over pages: search / insert / node split (THE HARD PART, NOT DEFERRED) (DONE)

`BPlusTree<K, V>` with `new`/`insert`/`get`/`contains_key`/`len`/`is_empty`/`height`/`clear`.
Automatic leaf + internal split with median promotion; balanced at every depth.
Heap-backed nodes for now (page-db is unpublished and carries no path dep); the
node layout is the page-oriented one a pager will persist later.

Exit criteria:
- [x] Every public item has rustdoc + a runnable example.
- [x] Core invariants property-tested (balance, sorted keys, separators; cross-checked vs `BTreeMap`).

---

## v0.3.0 -- delete + merge/redistribute + forward & reverse range scans (DONE)

`remove` with sibling borrow / merge and root collapse; `iter`/`range` as a
double-ended iterator (`Iter`) over `(&K, &V)`, plus `IntoIterator for &tree`.
Routing-separator + minimum-occupancy invariants property-tested after every
delete; iteration/range cross-checked vs `BTreeMap` forward and reverse.

Exit criteria:
- [x] New surface tested; hot paths benchmarked (range scan + bulk remove).

---

## v0.4.0 -- bulk load + node-storage seam + feature freeze (DONE; latch coupling moved, see below)

`from_sorted` builds a balanced tree bottom-up from sorted input (with an
insertion fallback for unsorted/duplicate input). Node access was refactored to
run through an internal `NodeStore` seam (`InMemoryStore` the only backend), so
the concurrent backend is additive rather than a rewrite. Feature freeze on the
in-memory surface declared.

**Anti-deferral record — latch coupling moved to a later phase, with reason.**
The original plan put latch coupling on heap nodes (`Arc<RwLock<Node>>`). That is
wrong-layer work: it rebuilds `page-db`'s already-loom-checked buffer-pool latches
one level too high, on throwaway heap nodes, and taxes the in-memory hot path for
a capability the substrate already provides. The concurrent B+tree is the
*page-backed* one: a node is a page, traversal fetches pinned `PageGuard`s, and
crabbing rides on `page-db`'s frame latches. index-db owns only the crabbing
protocol. The v0.4.0 storage seam is what makes that purely additive. The
page-backed backend is therefore deferred to a dedicated phase below; it is
blocked on `page-db` being published (no path deps across portfolio crates).

Exit criteria:
- [x] No `todo!`/`unimplemented!`. Feature freeze declared.
- [x] Bulk load shipped and tested; node-storage seam in place.

---

## v0.5.0 -- adversarial key distributions + concurrent-traversal stress + API freeze (DONE)

Adversarial workloads added: ascending/descending/zigzag insert against
opposite-order and middle-out deletes, clustered keys, repeated overwrite, plus a
small-order adversarial property test — all checking the structural invariants
after every delete. Large-scale stress (`tests/stress.rs`): scattered
insert/delete at 50k, sustained churn, bulk-load-then-delete, adversarial string
keys. **Concurrent-traversal stress** for the in-memory tree means concurrent
*reads*: the tree is `Sync`, so `tests/stress.rs` runs eight threads doing
`get` / `iter` / `range` over a shared `&tree` at once (the test does not compile
unless `BPlusTree: Sync`). Write-side concurrency is the deferred page backend.

**Public API — FROZEN as of v0.5.0:**
- `BPlusTree<K, V>`: `new`, `from_sorted`, `insert`, `get`, `contains_key`,
  `remove`, `iter`, `range`, `len`, `is_empty`, `height`, `clear`.
- Trait impls: `Default`, `IntoIterator for &BPlusTree`.
- `Iter<'a, K, V>`: `Iterator` + `DoubleEndedIterator`, item `(&K, &V)`.

No public item is added or changed before 1.0 except a future additive `S` store
type parameter on `BPlusTree`/`Iter` when the page-backed backend lands (a
defaulted parameter, non-breaking).

Exit criteria:
- [x] Public API frozen (recorded here). `cargo audit` + `cargo deny` clean.

---

## v0.6.0 -> v1.0.0 -- Alpha / Beta / RC / Stable

Integrate against real consumers, broaden testing, capture final benchmarks, then freeze the public API until 2.0 and publish.

- **v0.6.0 -- Alpha (DONE).** API frozen; sustained consumer-shaped soak
  (`tests/soak.rs`, 200k mixed ops vs `BTreeMap`); benchmark baselines recorded
  (`docs/PERFORMANCE.md`). No new API.
- **v1.0.0 -- Stable (DONE).** Definition-of-Done audit passed; inert `serde`
  feature + optional dep removed (now zero runtime deps); public API frozen until
  2.0. Ships as the stable *in-memory* ordered B+tree. (Beta/RC soak content was
  already satisfied by the v0.5/v0.6 adversarial, stress, and soak suites, so the
  intermediate tags were folded into 1.0.)

**1.0 scoping decision (settled):** 1.0 ships as the stable in-memory tree. The
page-backed concurrent (write-side) backend lands additively in a 1.x release —
the storage seam makes it non-breaking. Concurrent reads work today (the tree is
`Sync`).

---

## 1.x -- page-backed store + latch coupling (additive)

A `PageStore` backend behind the v0.4.0 `NodeStore` seam: nodes are pages from
`page-db`, traversal fetches pinned guards, and concurrent access is latch
coupling (crabbing) over those guards. Carries a `loom` model check; frame-level
safety is covered by `page-db`. Added as a 1.x minor — a defaulted `S` store type
parameter on `BPlusTree`/`Iter`, non-breaking for existing `BPlusTree<K, V>` code.

---

## Out of scope for 1.0

- The page store itself - `page-db` owns pages and caching.
- Locking - `lock-db` owns concurrency control; index-db uses latches only for tree structure.
- LSM / column storage - different engines (`lsm-db`, CORD).
