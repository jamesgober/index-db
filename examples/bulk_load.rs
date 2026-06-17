//! Bulk-load an index from sorted data, then read it back. Building bottom-up
//! from sorted input is much faster than inserting one entry at a time, and it
//! packs the tree densely.
//!
//! Run with `cargo run --example bulk_load --release`.

use index_db::BPlusTree;

fn main() {
    // Pretend this came from a sorted scan of some upstream store.
    const N: u64 = 1_000_000;
    let sorted = (0..N).map(|k| (k, k.wrapping_mul(2_654_435_761)));

    let index = BPlusTree::from_sorted(sorted);

    println!("bulk-loaded {} entries", index.len());
    println!("tree height: {} levels", index.height());

    // Spot-check a few keys.
    for k in [0_u64, 1, N / 2, N - 1] {
        let expected = k.wrapping_mul(2_654_435_761);
        assert_eq!(index.get(&k), Some(&expected));
        println!("  {k} -> {expected}");
    }

    // A range scan over the freshly built tree.
    let window: Vec<_> = index.range(100..105).map(|(&k, _)| k).collect();
    println!("range [100, 105): {window:?}");
}
