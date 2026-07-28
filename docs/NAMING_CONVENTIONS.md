# Naming conventions

The Rust API follows the standard conventions enforced by `rustfmt` and
Clippy:

- modules, functions, variables, and fields use `snake_case`;
- structs, enums, traits, and enum variants use `UpperCamelCase`;
- constants use `SCREAMING_SNAKE_CASE`;
- acronyms are treated as words in type and variant names (`Ttl`, `Nft`,
  `Api`), while established protocol constants may retain uppercase acronyms;
- error types use the owning domain followed by `Error`;
- storage keys describe the stored value rather than the implementation.

Public contract names are part of the ABI. A naming-only ABI change must retain
the numeric discriminants of error variants and be called out in release notes.

For example:

```rust
pub enum CacheTtlError {
    CacheExpired = 1,
    EntryNotFound = 2,
    InvalidTtl = 3,
}

pub fn get_remaining_ttl(/* ... */) {
    // ...
}
```
