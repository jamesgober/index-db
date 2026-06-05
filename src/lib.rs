//! # index-db
//!
//! B+tree indexing primitive for Rust storage engines - ordered keys, range scans, and concurrent access over paged storage.
//!
//! Scaffold release (v0.1.0). The public surface is being designed across the
//! 0.x series and frozen at v1.0. See `docs/API.md` and `dev/ROADMAP.md`.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]
#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(1 + 1, 2);
    }
}
