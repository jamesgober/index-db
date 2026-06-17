# index-db &mdash; Performance

> Benchmark baselines for the in-memory `BPlusTree`, captured at the `v0.6.0`
> alpha. Numbers are single-machine and single-threaded; treat them as the shape
> of the cost, not exact constants. The benchmark sources live in
> [`benches/index_bench.rs`](../benches/index_bench.rs).

## Method

All figures come from [Criterion](https://github.com/bheisler/criterion.rs):

```bash
cargo bench
```

Each case builds a tree of `n` entries keyed `u64 -> u64` and measures one
operation against it. Lookups sweep the key space so the access pattern is not a
single hot key. Trees are built with the default fan-out (64 children per node).

**Environment.** Windows 11 x86_64, Rust stable (release profile: fat LTO, one
codegen unit). The same commands run on Linux (WSL2) and through the CI matrix.
Absolute timings move with the host and with system load; the relationships
between sizes are the stable part.

## Point lookups — `get`

A lookup is one binary search per level, descending child ids through the node
store. Cost grows with the tree's height (logarithmic in `n`) and, more visibly,
with how far the working set spills out of cache.

| Entries | Tree height | `get` |
|--------:|:-----------:|------:|
| 1,000 | 2 | ~12 ns |
| 100,000 | 3 | ~35 ns |
| 1,000,000 | 4 | ~80 ns |

The growth from 12 ns to 80 ns across three orders of magnitude is cache
behaviour, not algorithmic cost — the height only moves from 2 to 4. Reaching a
child by id (rather than inline) adds about one memory indirection per level,
which is the dominant cost at the largest size and the price of the storage seam
that a page-backed backend will reuse.

## Inserts and removes

| Operation | 1,000 entries | 100,000 entries |
|-----------|--------------:|----------------:|
| `insert` (amortized, build from empty) | ~20 ns | ~44 ns |
| `remove` (amortized, delete all) | ~38 ns | ~62 ns |

Both are logarithmic per operation. Inserts allocate only when a node splits;
removes never allocate, but rebalancing (borrow or merge) touches a sibling, so a
delete does a little more pointer-chasing than an insert at the same size.

## Range scans

A scan seeks to its lower bound in `O(log n)` and then yields each entry in
amortized constant time, walking the leaves through an explicit cursor. The cost
tracks the *window width*, not the tree size.

| Window | 10,000-entry tree | 1,000,000-entry tree |
|--------|------------------:|---------------------:|
| 1,000 entries | ~0.9 µs | ~2.4 µs |

Roughly a nanosecond per entry yielded. The difference between the two tree sizes
is the seek and cache state, not the scan itself — a 1,000-entry window costs
about the same wherever it sits.

## Bulk load

`from_sorted` builds the tree bottom-up in a single pass over sorted input,
packing leaves and laying the internal levels above them. It avoids the
incremental node splitting that a million `insert` calls would cause and is the
right entry point when the data is already ordered (a sorted file, a `BTreeMap`,
a range scan of another store).

## Interpreting these numbers

- **Lookups, inserts, removes are `O(log n)`** with a small constant; the visible
  growth at scale is cache, not the algorithm.
- **Scans are `O(log n)` to start, then `O(1)` per entry**, allocation-free per
  step.
- **The id-addressed node store costs ~1 indirection per level** versus inline
  children. That is a deliberate trade: the page-backed backend is id-addressed by
  nature, so paying it now keeps that backend additive. Compared with `v0.3.0`'s
  inline layout, `get` at a million entries moved from ~52 ns to ~80 ns; scans and
  the write path are essentially unchanged.

## Reproducing

```bash
cargo bench                       # all benchmarks
cargo bench --bench index_bench -- get   # just the lookup group
```

Criterion writes HTML reports under `target/criterion/`.
