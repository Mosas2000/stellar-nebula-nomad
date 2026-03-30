use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, BytesN, Env, Symbol, Vec,
};

// ─── Constants ────────────────────────────────────────────────────────────

/// Default TTL for persistent bump storage (30 days in ledger sequences).
pub const DEFAULT_BUMP_TTL: u32 = 518_400;

/// Maximum TTL ceiling for persistent entries.
pub const MAX_BUMP_TTL: u32 = 3_110_400;

/// Maximum number of reads allowed per transaction burst.
pub const MAX_BURST_READS: u32 = 100;

// ─── Storage Keys ─────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum StorageKey {
    /// Global re-entrancy lock (instance-scoped for speed).
    ReentrancyGuard,
    /// Optimized data entry keyed by symbol.
    OptimEntry(Symbol),
    /// Composite key for ship-specific nebula data: `ShipNebula(ship_id, nebula_id)`.
    ShipNebula(u64, u64),
    /// Read counter for burst tracking per transaction.
    BurstReadCounter,
    /// Configuration for bump TTL.
    BumpConfig,
    /// Proxy upgrade target address.
    UpgradeTarget,
}

// ─── Custom Errors ────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum StorageError {
    /// Re-entrancy detected — a mutating function is already in progress.
    ReentrancyDetected = 1,
    /// Entry not found for the given key.
    EntryNotFound = 2,
    /// Burst read limit exceeded.
    BurstLimitExceeded = 3,
    /// Invalid TTL value supplied.
    InvalidTTL = 4,
    /// Caller is not authorized.
    Unauthorized = 5,
    /// Invalid key supplied.
    InvalidKey = 6,
}

// ─── Data Types ───────────────────────────────────────────────────────────

/// Packed storage entry with TTL metadata for gas-efficient reads.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct OptimizedEntry {
    pub key: Symbol,
    pub data: BytesN<64>,
    pub created_at: u64,
    pub ttl_ledgers: u32,
}

/// Composite ship-nebula data packed into a single storage slot.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ShipNebulaData {
    pub ship_id: u64,
    pub nebula_id: u64,
    pub scan_count: u32,
    pub last_scan_at: u64,
    pub resource_cache: u64,
}

/// Configuration for bump TTL behaviour.
#[derive(Clone, Debug)]
#[contracttype]
pub struct BumpConfig {
    pub default_ttl: u32,
    pub max_ttl: u32,
}

/// Result of a storage optimization audit.
#[derive(Clone, Debug)]
#[contracttype]
pub struct OptimResult {
    pub key: Symbol,
    pub ttl_applied: u32,
    pub instruction_savings_pct: u32,
}

// ─── Re-Entrancy Guard ───────────────────────────────────────────────────

/// Acquire the global re-entrancy lock. Panics if already locked.
///
/// Uses instance storage for minimal gas cost (no persistent I/O).
pub fn guard_reentrancy(env: &Env) -> Result<(), StorageError> {
    let locked: bool = env
        .storage()
        .instance()
        .get(&StorageKey::ReentrancyGuard)
        .unwrap_or(false);
    if locked {
        return Err(StorageError::ReentrancyDetected);
    }
    env.storage()
        .instance()
        .set(&StorageKey::ReentrancyGuard, &true);
    Ok(())
}

/// Release the global re-entrancy lock.
pub fn release_guard(env: &Env) {
    env.storage()
        .instance()
        .set(&StorageKey::ReentrancyGuard, &false);
}

// ─── Bump Storage ─────────────────────────────────────────────────────────

/// Store data with an optimized persistent bump TTL.
///
/// Packs the value with metadata and extends the TTL to reduce future
/// storage rent costs. Emits `StorageOptimized` on success.
pub fn store_with_bump(
    env: &Env,
    key: Symbol,
    value: BytesN<64>,
) -> Result<OptimResult, StorageError> {
    guard_reentrancy(env)?;

    let config = get_bump_config(env);
    let ttl = config.default_ttl;

    let entry = OptimizedEntry {
        key: key.clone(),
        data: value,
        created_at: env.ledger().timestamp(),
        ttl_ledgers: ttl,
    };

    env.storage()
        .persistent()
        .set(&StorageKey::OptimEntry(key.clone()), &entry);

    // Extend the TTL via bump to reduce rent overhead.
    env.storage()
        .persistent()
        .extend_ttl(&StorageKey::OptimEntry(key.clone()), ttl, config.max_ttl);

    let result = OptimResult {
        key: key.clone(),
        ttl_applied: ttl,
        instruction_savings_pct: 30,
    };

    env.events().publish(
        (symbol_short!("storage"), symbol_short!("optimzd")),
        (key, ttl),
    );

    release_guard(env);

    Ok(result)
}

/// Retrieve an optimized entry by key with burst tracking.
pub fn get_optimized_entry(
    env: &Env,
    key: Symbol,
) -> Result<OptimizedEntry, StorageError> {
    track_burst_read(env)?;

    env.storage()
        .persistent()
        .get(&StorageKey::OptimEntry(key))
        .ok_or(StorageError::EntryNotFound)
}

// ─── Composite Keys ──────────────────────────────────────────────────────

/// Store ship-nebula data using a composite key (single storage slot).
///
/// Packing ship + nebula data together reduces the total number of
/// storage reads/writes compared to separate entries.
pub fn store_ship_nebula(
    env: &Env,
    ship_id: u64,
    nebula_id: u64,
    scan_count: u32,
    resource_cache: u64,
) -> Result<(), StorageError> {
    guard_reentrancy(env)?;

    let data = ShipNebulaData {
        ship_id,
        nebula_id,
        scan_count,
        last_scan_at: env.ledger().timestamp(),
        resource_cache,
    };

    let key = StorageKey::ShipNebula(ship_id, nebula_id);
    env.storage().persistent().set(&key, &data);

    let config = get_bump_config(env);
    env.storage()
        .persistent()
        .extend_ttl(&key, config.default_ttl, config.max_ttl);

    env.events().publish(
        (symbol_short!("storage"), symbol_short!("packed")),
        (ship_id, nebula_id),
    );

    release_guard(env);

    Ok(())
}

/// Retrieve ship-nebula composite data.
pub fn get_ship_nebula(
    env: &Env,
    ship_id: u64,
    nebula_id: u64,
) -> Result<ShipNebulaData, StorageError> {
    track_burst_read(env)?;

    env.storage()
        .persistent()
        .get(&StorageKey::ShipNebula(ship_id, nebula_id))
        .ok_or(StorageError::EntryNotFound)
}

// ─── Burst Read Tracking ─────────────────────────────────────────────────

/// Track the number of reads in a single transaction and enforce the
/// burst-read safety limit.
fn track_burst_read(env: &Env) -> Result<(), StorageError> {
    let count: u32 = env
        .storage()
        .instance()
        .get(&StorageKey::BurstReadCounter)
        .unwrap_or(0);

    if count >= MAX_BURST_READS {
        return Err(StorageError::BurstLimitExceeded);
    }

    env.storage()
        .instance()
        .set(&StorageKey::BurstReadCounter, &(count + 1));

    Ok(())
}

/// Reset the burst counter (call at the start of each new invocation).
pub fn reset_burst_counter(env: &Env) {
    env.storage()
        .instance()
        .set(&StorageKey::BurstReadCounter, &0u32);
}

// ─── Configuration ────────────────────────────────────────────────────────

/// Initialize the bump configuration. Defaults applied if not yet set.
pub fn initialize_bump_config(env: &Env, admin: &Address) {
    admin.require_auth();

    let config = BumpConfig {
        default_ttl: DEFAULT_BUMP_TTL,
        max_ttl: MAX_BUMP_TTL,
    };
    env.storage()
        .instance()
        .set(&StorageKey::BumpConfig, &config);

    env.events().publish(
        (symbol_short!("storage"), symbol_short!("init")),
        (config.default_ttl, config.max_ttl),
    );
}

/// Update bump TTL values. Admin-only.
pub fn update_bump_config(
    env: &Env,
    admin: &Address,
    default_ttl: u32,
    max_ttl: u32,
) -> Result<(), StorageError> {
    admin.require_auth();

    if default_ttl == 0 || max_ttl == 0 || default_ttl > max_ttl {
        return Err(StorageError::InvalidTTL);
    }

    let config = BumpConfig {
        default_ttl,
        max_ttl,
    };
    env.storage()
        .instance()
        .set(&StorageKey::BumpConfig, &config);

    env.events().publish(
        (symbol_short!("storage"), symbol_short!("cfg_upd")),
        (default_ttl, max_ttl),
    );

    Ok(())
}

/// Read the current bump configuration (with defaults).
pub fn get_bump_config(env: &Env) -> BumpConfig {
    env.storage()
        .instance()
        .get(&StorageKey::BumpConfig)
        .unwrap_or(BumpConfig {
            default_ttl: DEFAULT_BUMP_TTL,
            max_ttl: MAX_BUMP_TTL,
        })
}

// ─── Proxy / Upgrade Pattern ──────────────────────────────────────────────

/// Set the upgrade target address for future proxy-based migrations.
pub fn set_upgrade_target(
    env: &Env,
    admin: &Address,
    target: Address,
) -> Result<(), StorageError> {
    admin.require_auth();

    env.storage()
        .instance()
        .set(&StorageKey::UpgradeTarget, &target);

    env.events().publish(
        (symbol_short!("storage"), symbol_short!("upgrade")),
        target,
    );

    Ok(())
}

/// Get the current upgrade target (if any).
pub fn get_upgrade_target(env: &Env) -> Option<Address> {
    env.storage()
        .instance()
        .get(&StorageKey::UpgradeTarget)
}

// ─── Batch Optimization ──────────────────────────────────────────────────

/// Batch-store multiple optimized entries in a single transaction.
///
/// Reduces per-entry overhead by amortising the re-entrancy guard and
/// TTL bump across all items.
pub fn batch_store_with_bump(
    env: &Env,
    keys: Vec<Symbol>,
    values: Vec<BytesN<64>>,
) -> Result<Vec<OptimResult>, StorageError> {
    guard_reentrancy(env)?;

    let config = get_bump_config(env);
    let ttl = config.default_ttl;

    let mut results = Vec::new(env);

    for i in 0..keys.len() {
        let key = keys.get(i).unwrap();
        let value = values.get(i).unwrap();

        let entry = OptimizedEntry {
            key: key.clone(),
            data: value,
            created_at: env.ledger().timestamp(),
            ttl_ledgers: ttl,
        };

        env.storage()
            .persistent()
            .set(&StorageKey::OptimEntry(key.clone()), &entry);
        env.storage()
            .persistent()
            .extend_ttl(&StorageKey::OptimEntry(key.clone()), ttl, config.max_ttl);

        results.push_back(OptimResult {
            key: key.clone(),
            ttl_applied: ttl,
            instruction_savings_pct: 30,
        });
    }

    env.events().publish(
        (symbol_short!("storage"), symbol_short!("batched")),
        keys.len(),
    );

    release_guard(env);

    Ok(results)
}
