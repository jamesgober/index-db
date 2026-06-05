# Contributing to index-db

Thanks for your interest in index-db. This document is the short version; the
full engineering standards and the definition of done live in
[`dev/DIRECTIVES.md`](./dev/DIRECTIVES.md), and the current phase plan is in
[`dev/ROADMAP.md`](./dev/ROADMAP.md).

## Before you open a PR

All of these must be clean:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps --all-features   # with RUSTDOCFLAGS="-D warnings"
```

- Hot-path changes require a `criterion` benchmark with before/after numbers.
- Correctness-critical paths require property tests (`proptest`), and any
  lock-free or shared-state path requires a `loom` model check.
- Every public item carries rustdoc with a runnable `# Examples` block.
- Commits are imperative and lowercase (`add page header crc`, not
  `Added CRC.`).

## Licensing

By contributing you agree your work is dual-licensed under
**Apache-2.0 OR MIT**, matching the project.
