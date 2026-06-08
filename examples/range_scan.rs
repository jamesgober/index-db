//! Range scans: the operation a B+tree exists for. Build an index over a time
//! series and pull bounded windows out of it, forward and in reverse.
//!
//! Run with `cargo run --example range_scan`.

use index_db::BPlusTree;

fn main() {
    // Event id -> a small payload, inserted out of order.
    let mut events: BPlusTree<u64, &str> = BPlusTree::new();
    for (id, label) in [
        (1003, "login"),
        (1001, "boot"),
        (1007, "flush"),
        (1002, "connect"),
        (1005, "write"),
        (1004, "read"),
        (1006, "commit"),
    ] {
        let _previous = events.insert(id, label);
    }

    // A half-open window [1002, 1005).
    println!("window [1002, 1005):");
    for (id, label) in events.range(1002..1005) {
        println!("  {id}: {label}");
    }

    // Everything from 1005 onward, newest first.
    println!("from 1005, reversed:");
    for (id, label) in events.range(1005..).rev() {
        println!("  {id}: {label}");
    }

    // Drop an event and confirm the index stays ordered.
    let removed = events.remove(&1004);
    println!("removed 1004 -> {removed:?}");
    let remaining: Vec<_> = events.iter().map(|(&id, _)| id).collect();
    println!("remaining ids in order: {remaining:?}");
}
