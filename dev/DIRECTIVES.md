# index-db -- Engineering Directives

> Engineering standards and the definition of done for this project. Read alongside `REPS.md` (root, authoritative) and `dev/ROADMAP.md` (current phase). If anything here conflicts with `REPS.md`, `REPS.md` wins.

---

## 0. Philosophy

This library is built and maintained to a production standard and treated as a flagship piece of work. Plan the full path, then build one verified step at a time. "Good enough" is treated as a defect.

---

## 1. What this is

index-db is a B+tree indexing primitive for storage engines: ordered keys mapped to values, with fast point lookups and efficient range scans, laid out over paged storage rather than the heap. Nodes are pages, so the tree persists and caches through a pager (it pairs with `page-db`), and it supports concurrent access via latch coupling (crabbing) so many readers and writers can traverse at once without locking the whole tree.

---

## 2. Engineering law (non-negotiable)

- **Performance** -- peak is the baseline; borrow over clone; no steady-state hot-path allocation; no "faster" claim without `criterion` numbers.
- **Concurrency** -- correctness under contention is proven with `loom`, not assumed.
- **Correctness** -- the invariants in section 4 are covered by property tests.
- **Security** -- all untrusted input validated; every allocation bounded; library code never panics on hostile input; parse/recovery paths fuzzed.
- **Architecture** -- SOLID, KISS, YAGNI; one responsibility; trait seams are the extension points.
- **Cross-platform** -- Linux/macOS/Windows first-class, verified by CI.
- **Error handling** -- every fallible path returns `Result`; errors are never silently swallowed.
- **Production-ready** -- no commented-out code, no stray `println!`/`dbg!`; every public item has rustdoc with a runnable example.

---

## 3. Definition of done

1. Compiles clean on Linux/macOS/Windows, stable and MSRV.
2. `fmt`, `clippy -D warnings`, `test --all-features`, `cargo doc -D warnings` clean.
3. `cargo audit` + `cargo deny check` pass.
4. No `unwrap`/`expect`/`todo!`/`dbg!` in shipping code; `unsafe` only with `// SAFETY:`.
5. A Tier-1 API exists and headlines the docs.
6. Property tests cover every section-4 invariant; `loom` covers every concurrent path.
7. Hot-path changes carry benchmarks; no regression over 5%.
8. Docs and `CHANGELOG.md` updated.

---

## 4. Project-specific invariants

- The B+tree invariants (sorted keys, balanced height, correct sibling links) hold after every insert/delete - property-tested against a reference ordered map.
- Range scans return exactly the keys in range, in order, with no duplicates or omissions across node boundaries.
- Latch coupling is the concurrency core; the crabbing protocol carries a `loom` model check for deadlock- and corruption-freedom.

Per-phase exit criteria in `dev/ROADMAP.md` encode these.

---

## 5. Integration points

- `page-db`: B+tree nodes are pages allocated and cached by the pager
- `lock-db`: range locks protect scanned key ranges
- storage engines: the secondary-index and ordered-access structure above the pager
- `lsm-db`: a B-tree alternative to the LSM index for read-heavy workloads

<sub>Copyright &copy; 2026 <strong>James Gober</strong>.</sub>
