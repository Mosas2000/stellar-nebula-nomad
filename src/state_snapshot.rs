use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, BytesN, Env, Symbol, Vec,
};

use crate::ship_nft::{DataKey as ShipDataKey, ShipNft};

// ─── Constants ────────────────────────────────────────────────────────────

/// Maximum snapshots allowed per session (burst limit).
pub const MAX_SNAPSHOTS_PER_SESSION: u32 = 5;

/// Snapshot TTL in ledger sequences (~7 days).
pub const SNAPSHOT_TTL: u32 = 604_800;

/// Maximum TTL ceiling for snapshots.
pub const SNAPSHOT_MAX_TTL: u32 = 3_110_400;

/// Interval between automatic snapshots (24 hours in seconds).
pub const AUTO_SNAPSHOT_INTERVAL: u64 = 86_400;

// ─── Storage Keys ─────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum SnapshotKey {
    /// Global auto-incrementing snapshot counter.
    SnapshotCounter,
    /// Snapshot data keyed by snapshot ID: `Snapshot(snapshot_id)`.
    Snapshot(u64),
    /// List of snapshot IDs for a ship: `ShipSnapshots(ship_id)`.
    ShipSnapshots(u64),
    /// Counter of snapshots taken in the current session per ship.
    SessionCount(u64),
    /// Timestamp of the last auto-snapshot for a ship.
    LastAutoSnapshot(u64),
}

// ─── Custom Errors ────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SnapshotError {
    /// Ship not found in storage.
    ShipNotFound = 1,
    /// Snapshot with the given ID does not exist.
    SnapshotNotFound = 2,
    /// Caller is not the owner of the ship.
    NotOwner = 3,
    /// Snapshot integrity check failed (hash mismatch).
    SnapshotInvalid = 4,
    /// Session snapshot limit exceeded.
    SessionLimitExceeded = 5,
    /// Auto-snapshot interval has not elapsed.
    TooSoon = 6,
    /// Snapshot is immutable and cannot be modified.
    SnapshotImmutable = 7,
}

// ─── Data Types ───────────────────────────────────────────────────────────

/// Compressed state snapshot capturing ship and resource data.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct StateSnapshot {
    pub snapshot_id: u64,
    pub ship_id: u64,
    pub owner: Address,
    /// Packed ship stats: hull, scanner_power, ship_type hash.
    pub ship_hull: u32,
    pub ship_scanner_power: u32,
    pub ship_type: Symbol,
    /// Resource state at time of snapshot.
    pub resource_balance: u64,
    /// Integrity hash derived from all captured fields.
    pub integrity_hash: BytesN<32>,
    pub created_at: u64,
    /// Immutable flag — historical snapshots cannot be overwritten.
    pub immutable: bool,
}

/// Result returned after a successful restore operation.
#[derive(Clone, Debug)]
#[contracttype]
pub struct RestoreResult {
    pub snapshot_id: u64,
    pub ship_id: u64,
    pub restored_at: u64,
}

// ─── Internal Helpers ─────────────────────────────────────────────────────

/// Fetch the next snapshot ID and increment the global counter.
fn next_snapshot_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&SnapshotKey::SnapshotCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage()
        .instance()
        .set(&SnapshotKey::SnapshotCounter, &next);
    next
}

/// Compute an integrity hash from snapshot fields using on-chain crypto.
fn compute_integrity_hash(
    env: &Env,
    ship_id: u64,
    hull: u32,
    scanner_power: u32,
    resource_balance: u64,
    created_at: u64,
) -> BytesN<32> {
    let mut payload = [0u8; 32];
    let ship_bytes = ship_id.to_be_bytes();
    let hull_bytes = hull.to_be_bytes();
    let scanner_bytes = scanner_power.to_be_bytes();
    let resource_bytes = resource_balance.to_be_bytes();
    let time_bytes = created_at.to_be_bytes();

    // Pack fields into a 32-byte payload for hashing.
    payload[0..8].copy_from_slice(&ship_bytes);
    payload[8..12].copy_from_slice(&hull_bytes);
    payload[12..16].copy_from_slice(&scanner_bytes);
    payload[16..24].copy_from_slice(&resource_bytes);
    payload[24..32].copy_from_slice(&time_bytes);

    env.crypto()
        .sha256(&soroban_sdk::Bytes::from_array(env, &payload))
        .to_bytes()
}

/// Verify snapshot integrity by recomputing the hash.
fn verify_integrity(env: &Env, snapshot: &StateSnapshot) -> bool {
    let expected = compute_integrity_hash(
        env,
        snapshot.ship_id,
        snapshot.ship_hull,
        snapshot.ship_scanner_power,
        snapshot.resource_balance,
        snapshot.created_at,
    );
    snapshot.integrity_hash == expected
}

/// Track session snapshot count and enforce burst limit.
fn check_session_limit(env: &Env, ship_id: u64) -> Result<(), SnapshotError> {
    let count: u32 = env
        .storage()
        .instance()
        .get(&SnapshotKey::SessionCount(ship_id))
        .unwrap_or(0);

    if count >= MAX_SNAPSHOTS_PER_SESSION {
        return Err(SnapshotError::SessionLimitExceeded);
    }

    env.storage()
        .instance()
        .set(&SnapshotKey::SessionCount(ship_id), &(count + 1));

    Ok(())
}

/// Add a snapshot ID to the ship's snapshot list.
fn add_snapshot_to_ship(env: &Env, ship_id: u64, snapshot_id: u64) {
    let key = SnapshotKey::ShipSnapshots(ship_id);
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    ids.push_back(snapshot_id);
    env.storage().persistent().set(&key, &ids);
}

// ─── Public API ───────────────────────────────────────────────────────────

/// Take a snapshot of the current ship and resource state.
///
/// Captures hull, scanner power, ship type, resource balance, and
/// computes an integrity hash. The snapshot is immutable once stored.
/// Emits `SnapshotTaken` event.
pub fn take_snapshot(
    env: &Env,
    caller: &Address,
    ship_id: u64,
) -> Result<StateSnapshot, SnapshotError> {
    caller.require_auth();

    // Enforce session burst limit.
    check_session_limit(env, ship_id)?;

    // Load ship data.
    let ship: ShipNft = env
        .storage()
        .persistent()
        .get(&ShipDataKey::Ship(ship_id))
        .ok_or(SnapshotError::ShipNotFound)?;

    // Verify ownership.
    if ship.owner != *caller {
        return Err(SnapshotError::NotOwner);
    }

    let now = env.ledger().timestamp();

    // Derive resource balance from energy storage (cross-module).
    let resource_balance: u64 = env
        .storage()
        .persistent()
        .get(&crate::energy_manager::EnergyKey::EnergyBalance(ship_id))
        .unwrap_or(0u32) as u64;

    let integrity_hash = compute_integrity_hash(
        env,
        ship_id,
        ship.hull,
        ship.scanner_power,
        resource_balance,
        now,
    );

    let snapshot_id = next_snapshot_id(env);

    let snapshot = StateSnapshot {
        snapshot_id,
        ship_id,
        owner: caller.clone(),
        ship_hull: ship.hull,
        ship_scanner_power: ship.scanner_power,
        ship_type: ship.ship_type.clone(),
        resource_balance,
        integrity_hash,
        created_at: now,
        immutable: true,
    };

    // Store the snapshot with TTL bump for cost efficiency.
    env.storage()
        .persistent()
        .set(&SnapshotKey::Snapshot(snapshot_id), &snapshot);
    env.storage().persistent().extend_ttl(
        &SnapshotKey::Snapshot(snapshot_id),
        SNAPSHOT_TTL,
        SNAPSHOT_MAX_TTL,
    );

    add_snapshot_to_ship(env, ship_id, snapshot_id);

    env.events().publish(
        (symbol_short!("snap"), symbol_short!("taken")),
        (snapshot_id, ship_id, caller.clone(), now),
    );

    Ok(snapshot)
}

/// Restore ship state from a previously taken snapshot.
///
/// Verifies ownership and snapshot integrity before applying.
/// Does not modify the original snapshot (immutable history).
/// Emits `StateRestored` event.
pub fn restore_from_snapshot(
    env: &Env,
    caller: &Address,
    snapshot_id: u64,
) -> Result<RestoreResult, SnapshotError> {
    caller.require_auth();

    // Load and verify snapshot.
    let snapshot: StateSnapshot = env
        .storage()
        .persistent()
        .get(&SnapshotKey::Snapshot(snapshot_id))
        .ok_or(SnapshotError::SnapshotNotFound)?;

    // Verify ownership.
    if snapshot.owner != *caller {
        return Err(SnapshotError::NotOwner);
    }

    // Integrity check.
    if !verify_integrity(env, &snapshot) {
        return Err(SnapshotError::SnapshotInvalid);
    }

    // Load the current ship to restore into.
    let mut ship: ShipNft = env
        .storage()
        .persistent()
        .get(&ShipDataKey::Ship(snapshot.ship_id))
        .ok_or(SnapshotError::ShipNotFound)?;

    // Verify current ownership still matches.
    if ship.owner != *caller {
        return Err(SnapshotError::NotOwner);
    }

    // Apply snapshot state to ship.
    ship.hull = snapshot.ship_hull;
    ship.scanner_power = snapshot.ship_scanner_power;

    env.storage()
        .persistent()
        .set(&ShipDataKey::Ship(snapshot.ship_id), &ship);

    // Restore energy/resource balance.
    env.storage().persistent().set(
        &crate::energy_manager::EnergyKey::EnergyBalance(snapshot.ship_id),
        &(snapshot.resource_balance as u32),
    );

    let now = env.ledger().timestamp();

    let result = RestoreResult {
        snapshot_id,
        ship_id: snapshot.ship_id,
        restored_at: now,
    };

    env.events().publish(
        (symbol_short!("snap"), symbol_short!("restore")),
        (snapshot_id, snapshot.ship_id, caller.clone(), now),
    );

    Ok(result)
}

/// Get a snapshot by ID.
pub fn get_snapshot(env: &Env, snapshot_id: u64) -> Result<StateSnapshot, SnapshotError> {
    env.storage()
        .persistent()
        .get(&SnapshotKey::Snapshot(snapshot_id))
        .ok_or(SnapshotError::SnapshotNotFound)
}

/// Get all snapshot IDs for a ship.
pub fn get_ship_snapshots(env: &Env, ship_id: u64) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&SnapshotKey::ShipSnapshots(ship_id))
        .unwrap_or_else(|| Vec::new(env))
}

/// Trigger an automatic daily snapshot if the interval has elapsed.
///
/// Can be called by anyone to keep snapshots current.
/// Returns the new snapshot or `TooSoon` if the interval hasn't passed.
pub fn auto_snapshot(
    env: &Env,
    caller: &Address,
    ship_id: u64,
) -> Result<StateSnapshot, SnapshotError> {
    let now = env.ledger().timestamp();

    let last: u64 = env
        .storage()
        .instance()
        .get(&SnapshotKey::LastAutoSnapshot(ship_id))
        .unwrap_or(0);

    if now.saturating_sub(last) < AUTO_SNAPSHOT_INTERVAL {
        return Err(SnapshotError::TooSoon);
    }

    let snapshot = take_snapshot(env, caller, ship_id)?;

    env.storage()
        .instance()
        .set(&SnapshotKey::LastAutoSnapshot(ship_id), &now);

    Ok(snapshot)
}

/// Reset the per-ship session snapshot counter.
///
/// Called at session start to refresh the burst quota.
pub fn reset_session_count(env: &Env, ship_id: u64) {
    env.storage()
        .instance()
        .set(&SnapshotKey::SessionCount(ship_id), &0u32);
}
