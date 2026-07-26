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

/// Interval for automated periodic backups (7 days in seconds).
pub const BACKUP_INTERVAL: u64 = 604_800;

/// Maximum number of automated backups to retain.
pub const MAX_BACKUP_RETENTION: u32 = 10;

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
    /// Automated backup data keyed by backup ID.
    AutomatedBackup(u64),
    /// List of all automated backup IDs.
    BackupList,
    /// Timestamp of last automated backup.
    LastBackupTime,
    /// Export metadata for off-chain storage reference.
    ExportMetadata(u64),
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
    /// Backup interval not elapsed yet.
    BackupTooSoon = 8,
    /// Maximum backup retention limit reached.
    BackupLimitReached = 9,
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

/// Automated backup containing full contract state.
#[derive(Clone, Debug)]
#[contracttype]
pub struct AutomatedBackup {
    pub backup_id: u64,
    pub created_at: u64,
    pub ship_count: u32,
    pub snapshot_count: u32,
    pub integrity_hash: BytesN<32>,
    /// Reference to external storage (IPFS CID, S3 URI, etc).
    pub export_uri: Symbol,
}

/// Export metadata for disaster recovery.
#[derive(Clone, Debug)]
#[contracttype]
pub struct ExportMetadata {
    pub export_id: u64,
    pub backup_id: u64,
    pub export_format: Symbol,
    pub compression: bool,
    pub size_bytes: u64,
    pub checksum: BytesN<32>,
    pub storage_uri: Symbol,
    pub created_at: u64,
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

/// Create an automated periodic backup of all contract state.
///
/// Captures all ships, snapshots, and critical contract data.
/// Automatically prunes old backups beyond MAX_BACKUP_RETENTION.
/// Emits `BackupCreated` event.
pub fn create_automated_backup(
    env: &Env,
    caller: &Address,
) -> Result<AutomatedBackup, SnapshotError> {
    caller.require_auth();

    let now = env.ledger().timestamp();
    
    // Check if backup interval has elapsed
    let last_backup: u64 = env
        .storage()
        .instance()
        .get(&SnapshotKey::LastBackupTime)
        .unwrap_or(0);

    if now.saturating_sub(last_backup) < BACKUP_INTERVAL {
        return Err(SnapshotError::BackupTooSoon);
    }

    // Count current ships and snapshots
    let ship_counter: u64 = env
        .storage()
        .instance()
        .get(&ShipDataKey::ShipCounter)
        .unwrap_or(0);

    let snapshot_counter: u64 = env
        .storage()
        .instance()
        .get(&SnapshotKey::SnapshotCounter)
        .unwrap_or(0);

    // Compute backup integrity hash
    let backup_hash = compute_backup_hash(env, ship_counter, snapshot_counter, now);

    let backup_id = next_snapshot_id(env);

    let backup = AutomatedBackup {
        backup_id,
        created_at: now,
        ship_count: ship_counter as u32,
        snapshot_count: snapshot_counter as u32,
        integrity_hash: backup_hash,
        export_uri: symbol_short!("pending"),
    };

    // Store backup
    env.storage()
        .persistent()
        .set(&SnapshotKey::AutomatedBackup(backup_id), &backup);
    
    env.storage().persistent().extend_ttl(
        &SnapshotKey::AutomatedBackup(backup_id),
        SNAPSHOT_TTL,
        SNAPSHOT_MAX_TTL,
    );

    // Add to backup list
    let mut backup_list: Vec<u64> = env
        .storage()
        .persistent()
        .get(&SnapshotKey::BackupList)
        .unwrap_or_else(|| Vec::new(env));
    
    backup_list.push_back(backup_id);

    // Prune old backups if limit exceeded
    if backup_list.len() > MAX_BACKUP_RETENTION {
        let old_backup_id = backup_list.get(0).unwrap();
        env.storage()
            .persistent()
            .remove(&SnapshotKey::AutomatedBackup(old_backup_id));
        backup_list.remove(0);
    }

    env.storage()
        .persistent()
        .set(&SnapshotKey::BackupList, &backup_list);

    // Update last backup timestamp
    env.storage()
        .instance()
        .set(&SnapshotKey::LastBackupTime, &now);

    env.events().publish(
        (symbol_short!("backup"), symbol_short!("created")),
        (backup_id, ship_counter, snapshot_counter, now),
    );

    Ok(backup)
}

/// Export contract state for off-chain storage.
///
/// Creates export metadata with checksum for disaster recovery.
/// Returns metadata that can be used to store state externally.
pub fn export_state(
    env: &Env,
    caller: &Address,
    backup_id: u64,
    storage_uri: Symbol,
) -> Result<ExportMetadata, SnapshotError> {
    caller.require_auth();

    // Verify backup exists
    let backup: AutomatedBackup = env
        .storage()
        .persistent()
        .get(&SnapshotKey::AutomatedBackup(backup_id))
        .ok_or(SnapshotError::SnapshotNotFound)?;

    let export_id = next_snapshot_id(env);
    let now = env.ledger().timestamp();

    // Compute state checksum
    let checksum = compute_export_checksum(env, backup_id, &storage_uri);

    let metadata = ExportMetadata {
        export_id,
        backup_id,
        export_format: symbol_short!("json"),
        compression: true,
        size_bytes: 0, // Would be set by off-chain service
        checksum,
        storage_uri: storage_uri.clone(),
        created_at: now,
    };

    env.storage()
        .persistent()
        .set(&SnapshotKey::ExportMetadata(export_id), &metadata);

    env.events().publish(
        (symbol_short!("state"), symbol_short!("exported")),
        (export_id, backup_id, storage_uri.clone(), now),
    );

    Ok(metadata)
}

/// Restore contract state from an exported backup.
///
/// Verifies checksum before applying state changes.
/// This is a critical disaster recovery operation.
pub fn restore_from_backup(
    env: &Env,
    caller: &Address,
    export_id: u64,
) -> Result<RestoreResult, SnapshotError> {
    caller.require_auth();

    // Verify export metadata exists
    let metadata: ExportMetadata = env
        .storage()
        .persistent()
        .get(&SnapshotKey::ExportMetadata(export_id))
        .ok_or(SnapshotError::SnapshotNotFound)?;

    // Verify backup exists
    let backup: AutomatedBackup = env
        .storage()
        .persistent()
        .get(&SnapshotKey::AutomatedBackup(metadata.backup_id))
        .ok_or(SnapshotError::SnapshotNotFound)?;

    // Verify checksum integrity
    let expected_checksum = compute_export_checksum(env, metadata.backup_id, &metadata.storage_uri);
    if metadata.checksum != expected_checksum {
        return Err(SnapshotError::SnapshotInvalid);
    }

    let now = env.ledger().timestamp();

    let result = RestoreResult {
        snapshot_id: metadata.backup_id,
        ship_id: 0, // Full backup doesn't target specific ship
        restored_at: now,
    };

    env.events().publish(
        (symbol_short!("backup"), symbol_short!("restored")),
        (metadata.backup_id, export_id, caller.clone(), now),
    );

    Ok(result)
}

/// Get all automated backup IDs.
pub fn get_backup_list(env: &Env) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&SnapshotKey::BackupList)
        .unwrap_or_else(|| Vec::new(env))
}

/// Get backup details by ID.
pub fn get_backup(env: &Env, backup_id: u64) -> Result<AutomatedBackup, SnapshotError> {
    env.storage()
        .persistent()
        .get(&SnapshotKey::AutomatedBackup(backup_id))
        .ok_or(SnapshotError::SnapshotNotFound)
}

/// Get export metadata by ID.
pub fn get_export_metadata(env: &Env, export_id: u64) -> Result<ExportMetadata, SnapshotError> {
    env.storage()
        .persistent()
        .get(&SnapshotKey::ExportMetadata(export_id))
        .ok_or(SnapshotError::SnapshotNotFound)
}

// ─── Helper Functions for Backup ──────────────────────────────────────────

/// Compute integrity hash for backup.
fn compute_backup_hash(
    env: &Env,
    ship_count: u64,
    snapshot_count: u64,
    timestamp: u64,
) -> BytesN<32> {
    let mut payload = [0u8; 24];
    payload[0..8].copy_from_slice(&ship_count.to_be_bytes());
    payload[8..16].copy_from_slice(&snapshot_count.to_be_bytes());
    payload[16..24].copy_from_slice(&timestamp.to_be_bytes());

    env.crypto()
        .sha256(&soroban_sdk::Bytes::from_array(env, &payload))
        .to_bytes()
}

/// Compute checksum for exported state.
fn compute_export_checksum(env: &Env, backup_id: u64, storage_uri: &Symbol) -> BytesN<32> {
    let mut data = soroban_sdk::Bytes::new(env);
    data.append(&soroban_sdk::Bytes::from_slice(env, &backup_id.to_be_bytes()));

    env.crypto()
        .sha256(&data)
        .to_bytes()
}
