# Module architecture

Nebula Nomad is organized as a set of bounded gameplay and infrastructure
modules exposed through the root contract facade in `src/lib.rs`.

## Gameplay domains

- Exploration modules generate nebulae, constellations, missions, and
  environmental state.
- Progression modules manage achievements, badges, battle passes, difficulty,
  player energy, and seasonal content.
- Social modules own alliances, clans, communication, gifting, and reputation.
- Economy modules own resources, recipes, crafting, trading, bounties, and DEX
  integration.

## Infrastructure domains

- Access-control and emergency modules protect privileged operations.
- Audit, analytics, metrics, and event modules expose observable state changes.
- Cache, storage, migration, and versioning modules manage lifecycle and
  upgrade compatibility.
- Batch and configuration modules bound work and delay sensitive changes.

## Dependency direction

Feature modules may depend on shared types and infrastructure utilities.
Infrastructure modules must not call high-level gameplay modules. The root
contract facade coordinates cross-domain operations and remains the only place
that should expose unrelated domains through one public interface.

## Module documentation standard

Every Rust module starts with `//!` documentation explaining its responsibility.
Public functions that mutate state should document authorization, storage
effects, emitted events, errors, and a short usage example where the call
sequence is not obvious.

```rust
//! Player energy balances, spending, and regeneration.

/// Spends energy after authenticating the player.
///
/// Returns an error when the balance is insufficient.
pub fn spend_energy(/* ... */) {
    // ...
}
```
