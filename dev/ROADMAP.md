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

## v0.3.0 -- delete + merge/redistribute + forward & reverse range scans

Exit criteria:
- [ ] New surface tested; hot paths benchmarked.

---

## v0.4.0 -- concurrent access via latch coupling + bulk load + feature freeze

Exit criteria:
- [ ] No `todo!`/`unimplemented!`. Feature freeze declared.

---

## v0.5.0 -- adversarial key distributions + concurrent-traversal stress + API freeze

Exit criteria:
- [ ] Public API frozen (recorded here). `cargo audit` + `cargo deny` clean.

---

## v0.6.0 -> v1.0.0 -- Alpha / Beta / RC / Stable

Integrate against real consumers, broaden testing, capture final benchmarks, then freeze the public API until 2.0 and publish.

---

## Out of scope for 1.0

- The page store itself - `page-db` owns pages and caching.
- Locking - `lock-db` owns concurrency control; index-db uses latches only for tree structure.
- LSM / column storage - different engines (`lsm-db`, CORD).
