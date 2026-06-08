//! Build a large index and confirm every key reads back, while the tree stays
//! shallow. Shows how the height grows only with the logarithm of the size.
//!
//! Run with `cargo run --example bulk_lookup --release`.

use index_db::BPlusTree;

fn main() {
    let mut index = BPlusTree::new();

    // Insert keys out of order so the tree must sort and split as it grows.
    const N: u32 = 1_000_000;
    let mut k = 0_u32;
    for _ in 0..N {
        k = k.wrapping_mul(2_654_435_761).wrapping_add(1);
        let _previous = index.insert(k, k);
    }

    println!("inserted {} keys", index.len());
    println!("tree height: {} levels", index.height());

    // Every key we put in reads back with its value.
    let mut k = 0_u32;
    let mut found = 0_u64;
    for _ in 0..N {
        k = k.wrapping_mul(2_654_435_761).wrapping_add(1);
        if index.get(&k) == Some(&k) {
            found += 1;
        }
    }
    println!("verified {found} of {N} lookups");
}
