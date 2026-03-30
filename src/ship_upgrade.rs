use crate::resource_minter::ResourceKey;
use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Map, Symbol, Vec};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum number of upgrade modules installable on a single ship.
pub const MAX_MODULES: u32 = 5;
/// Maximum cumulative mass before an upgrade is rejected.
pub const MAX_MASS: u32 = 100;
/// Maximum upgrades allowed in a single batch transaction.
pub const MAX_BATCH_UPGRADES: u32 = 2;

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ShipUpgradeError {
    NotInitialized     = 200,
    AlreadyInitialized = 201,
    InsufficientResources = 202,
    UnknownComponent   = 203,
    /// Invariant violated: module cap or mass limit exceeded.
    InvariantViolation = 204,
    BatchTooLarge      = 205,
}

// ── Data Types ────────────────────────────────────────────────────────────────

/// Live on-chain upgrade stats for a ship.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ShipState {
    pub ship_id: u64,
    /// Number of installed upgrade modules.
    pub module_count: u32,
    /// Cumulative mass of all installed components.
    pub total_mass: u32,
    /// Total scanner power bonus from upgrades.
    pub scanner_bonus: u32,
    /// Total hull strength bonus from upgrades.
    pub hull_bonus: u32,
}

/// Per-component upgrade blueprint: cost and stat bonuses.
#[contracttype]
#[derive(Clone, Debug)]
pub struct UpgradeBlueprint {
    /// Asset symbol of the resource to burn (matches `ResourceKey::ResourceBalance` asset_id).
    pub asset_id: Symbol,
    /// Amount of that resource to consume per upgrade.
    pub resource_cost: u32,
    /// Mass added by this component (contributes to `total_mass`).
    pub mass: u32,
    /// Scanner power bonus granted by this component.
    pub scanner_bonus: u32,
    /// Hull strength bonus granted by this component.
    pub hull_bonus: u32,
}

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
enum UpgradeDataKey {
    /// Admin address for protected init call.
    Admin,
    /// Map<Symbol, UpgradeBlueprint> of all registered component blueprints.
    Config,
    /// Current upgrade stats for a ship keyed by ship_id.
    ShipState(u64),
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Initialise the upgrade subsystem with a component blueprint map.
/// Admin-only; reverts with `AlreadyInitialized` if called again.
pub fn init_upgrade_config(
    env: &Env,
    admin: &Address,
    blueprints: Map<Symbol, UpgradeBlueprint>,
) -> Result<(), ShipUpgradeError> {
    if env.storage().instance().has(&UpgradeDataKey::Admin) {
        return Err(ShipUpgradeError::AlreadyInitialized);
    }
    admin.require_auth();
    env.storage().instance().set(&UpgradeDataKey::Admin, admin);
    env.storage().instance().set(&UpgradeDataKey::Config, &blueprints);
    Ok(())
}

/// Apply a single component upgrade to `ship_id`.
///
/// Steps:
/// 1. Require `player` authorisation.
/// 2. Look up `component` in the blueprint config.
/// 3. Burn `blueprint.resource_cost` from the player's harvested resource balance.
/// 4. Compute new `ShipState` using saturating arithmetic (overflow-safe).
/// 5. Validate invariants (module cap, mass limit).
/// 6. Persist updated state.
/// 7. Emit `ShipUpgraded` event with before/after stats.
pub fn apply_upgrade(
    env: &Env,
    player: &Address,
    ship_id: u64,
    component: Symbol,
) -> Result<ShipState, ShipUpgradeError> {
    player.require_auth();
    apply_upgrade_inner(env, player, ship_id, component)
}

/// Inner upgrade logic — no auth check; called from `apply_upgrade` (single)
/// and `batch_upgrade` (which does one top-level auth then calls this).
fn apply_upgrade_inner(
    env: &Env,
    player: &Address,
    ship_id: u64,
    component: Symbol,
) -> Result<ShipState, ShipUpgradeError> {
    let blueprints: Map<Symbol, UpgradeBlueprint> = env
        .storage()
        .instance()
        .get(&UpgradeDataKey::Config)
        .ok_or(ShipUpgradeError::NotInitialized)?;

    let blueprint = blueprints
        .get(component.clone())
        .ok_or(ShipUpgradeError::UnknownComponent)?;

    // Burn resource: deduct from ResourceMinter's balance for this player + asset.
    let res_key = ResourceKey::ResourceBalance(player.clone(), blueprint.asset_id.clone());
    let balance: u32 = env.storage().instance().get(&res_key).unwrap_or(0);
    if balance < blueprint.resource_cost {
        return Err(ShipUpgradeError::InsufficientResources);
    }
    env.storage()
        .instance()
        .set(&res_key, &(balance - blueprint.resource_cost));

    // Load before-state (default to zero stats if first upgrade for this ship).
    let before: ShipState = env
        .storage()
        .persistent()
        .get(&UpgradeDataKey::ShipState(ship_id))
        .unwrap_or(ShipState {
            ship_id,
            module_count: 0,
            total_mass: 0,
            scanner_bonus: 0,
            hull_bonus: 0,
        });

    // Build after-state with saturating arithmetic to prevent overflow exploits.
    let after = ShipState {
        ship_id,
        module_count: before.module_count.saturating_add(1),
        total_mass:   before.total_mass.saturating_add(blueprint.mass),
        scanner_bonus: before.scanner_bonus.saturating_add(blueprint.scanner_bonus),
        hull_bonus:    before.hull_bonus.saturating_add(blueprint.hull_bonus),
    };

    // Validate invariants before committing — revert if violated.
    validate_invariants(&after)?;

    // Persist updated state.
    env.storage()
        .persistent()
        .set(&UpgradeDataKey::ShipState(ship_id), &after);

    // Emit ShipUpgraded event carrying before/after snapshots and component name.
    env.events().publish(
        (symbol_short!("ship_upg"), ship_id, player.clone()),
        (before.clone(), after.clone(), component),
    );

    Ok(after)
}

/// Apply up to `MAX_BATCH_UPGRADES` (2) component upgrades in one transaction.
/// Requires player auth once at the top level; inner calls skip re-auth.
/// Fails atomically: if any upgrade fails the entire batch is reverted.
pub fn batch_upgrade(
    env: &Env,
    player: &Address,
    ship_id: u64,
    components: Vec<Symbol>,
) -> Result<Vec<ShipState>, ShipUpgradeError> {
    if components.len() > MAX_BATCH_UPGRADES {
        return Err(ShipUpgradeError::BatchTooLarge);
    }

    player.require_auth();

    let mut results: Vec<ShipState> = soroban_sdk::vec![env];
    for component in components.iter() {
        let state = apply_upgrade_inner(env, player, ship_id, component)?;
        results.push_back(state);
    }
    Ok(results)
}

/// Validate that a `ShipState` respects all hard invariants.
///
/// Invariants:
/// - `module_count` ≤ `MAX_MODULES` (5)
/// - `total_mass`   ≤ `MAX_MASS`    (100)
///
/// Returns `Err(InvariantViolation)` on the first breach.
pub fn validate_invariants(ship: &ShipState) -> Result<(), ShipUpgradeError> {
    if ship.module_count > MAX_MODULES {
        return Err(ShipUpgradeError::InvariantViolation);
    }
    if ship.total_mass > MAX_MASS {
        return Err(ShipUpgradeError::InvariantViolation);
    }
    Ok(())
}

/// Read the current upgrade state of `ship_id`. Returns `None` if no upgrades
/// have been applied yet.
pub fn get_ship_state(env: &Env, ship_id: u64) -> Option<ShipState> {
    env.storage()
        .persistent()
        .get(&UpgradeDataKey::ShipState(ship_id))
}

/// Read the registered blueprint map. Returns `None` if not yet initialised.
pub fn get_upgrade_config(env: &Env) -> Option<Map<Symbol, UpgradeBlueprint>> {
    env.storage().instance().get(&UpgradeDataKey::Config)
}
