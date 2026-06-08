//! Basic usage: build an index, look keys up, and overwrite a value.
//!
//! Run with `cargo run --example basic`.

use index_db::BPlusTree;

fn main() {
    // An index from user id to display name.
    let mut users: BPlusTree<u32, String> = BPlusTree::new();

    users.insert(1001, "ada".to_string());
    users.insert(1002, "grace".to_string());
    users.insert(1003, "alan".to_string());

    // Point lookups.
    println!("1002 -> {:?}", users.get(&1002).map(String::as_str));
    println!("9999 -> {:?}", users.get(&9999).map(String::as_str));

    // Membership without reading the value.
    println!("has 1001? {}", users.contains_key(&1001));

    // Overwriting an existing key returns the previous value.
    let previous = users.insert(1003, "turing".to_string());
    println!(
        "1003 was {previous:?}, now {:?}",
        users.get(&1003).map(String::as_str)
    );

    println!("{} users, tree height {}", users.len(), users.height());
}
