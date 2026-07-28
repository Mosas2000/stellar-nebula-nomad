# Dead-code audit

The audit searches production Rust sources for explicit dead-code suppressions,
unreferenced public helpers, stale imports, and duplicate test utilities.

Removed in this pass:

- `dex_integration::get_offer`, which had no caller or root-contract entry
  point;
- `treasure_vault::set_min_lock_duration`, which had no caller and bypassed
  the authorization expected by its “admin function” comment.

The test-helper corpus remains intentional. It is exported behind
`cfg(any(test, feature = "fuzz"))` and is exercised by `tests/fuzz.rs` when the
`fuzz` feature is enabled.

Recommended repeatable checks:

```sh
cargo clippy --all-targets --all-features -- -D warnings
rg 'allow\\(dead_code\\)' src
cargo machete
```

`cargo machete` audits unused Cargo dependencies and should be run whenever
dependencies change.
