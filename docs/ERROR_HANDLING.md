# Error handling standard

Contract modules keep their Soroban `#[contracterror]` enums because those
numeric values are part of the public ABI. New and migrated errors also
implement `StandardContractError`, which supplies one consistent descriptor:

- `module`: a stable namespace; `(module, code)` is the unique error identity.
- `code`: the existing ABI-safe `u32` discriminant.
- `kind`: validation, authorization, not found, conflict, resource limit, or
  internal.
- `retryable`: whether an unchanged request may succeed if attempted later.

This creates a hierarchy that SDKs, logs, and user interfaces can consume
without renumbering deployed errors. Numeric codes only need to be unique
within a module. They must never be reused for a different meaning after
release.

## Adding an error

1. Add a documented variant to the module's `#[repr(u32)]` error enum.
2. Assign the next unused code in that enum; do not reorder or renumber old
   variants.
3. Add the variant to the module's `StandardContractError` match.
4. Classify caller mistakes as `Validation`, permission failures as
   `Authorization`, missing state as `NotFound`, state collisions as
   `Conflict`, bounded-capacity failures as `ResourceLimit`, and invariant or
   downstream failures as `Internal`.
5. Mark an error retryable only when time or transient capacity can resolve it.
6. Test its descriptor and the contract behavior that emits it.

The access-control, analytics, and batch-processing modules are the reference
implementations. Other modules can migrate incrementally without ABI changes.
