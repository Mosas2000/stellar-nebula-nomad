use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, Bytes, Env, Vec, Map,
};

/// Current contract version (starts at 1 at deployment).
pub const CURRENT_VERSION: u32 = 1;

/// Maximum records to migrate in a single batch.
pub const MIGRATION_BATCH_SIZE: u32 = 50;

/// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum VersioningKey {
    /// Global version counter.
    Version,
    /// Migration status for a given (old_version, new_version) pair.
    MigrationStatus(u32, u32),
    /// Temporary storage for batch migration state.
    MigrationBatch(u32),
    /// Per-address opt-in flag for automatic migration.
    AutoMigrate(Address),
}

/// ─── Errors ─────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VersioningError {
    /// Target version is not supported.
    IncompatibleVersion = 1,
    /// Migration already completed for this version pair.
    AlreadyMigrated = 2,
    /// Migration is still in progress.
    MigrationInProgress = 3,
    /// Batch size exceeds limit.
    BatchTooLarge = 4,
    /// Caller is not authorized to trigger migration.
    NotAuthorized = 5,
}

/// ─── Data Types ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct MigrationRecord {
    pub from_version: u32,
    pub to_version: u32,
    pub migrated_at: u64,
    pub record_count: u32,
}

/// ─── Public API ─────────────────────────────────────────────────────────────

/// Initialize the versioning system at deployment.
pub fn initialize_version(env: &Env) {
    if !env.storage().instance().has(&VersioningKey::Version) {
        env.storage()
            .instance()
            .set(&VersioningKey::Version, &CURRENT_VERSION);
    }
}

/// Get the current contract version.
pub fn get_version(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&VersioningKey::Version)
        .unwrap_or(CURRENT_VERSION)
}

/// Check compatibility for a given version against the current.
pub fn check_compatibility(env: &Env, version: u32) -> Result<(), VersioningError> {
    let current = get_version(env);
    if version > current {
        return Err(VersioningError::IncompatibleVersion);
    }
    Ok(())
}

/// Enable/disable automatic migration for a caller.
pub fn set_auto_migrate(env: &Env, caller: &Address, enabled: bool) {
    caller.require_auth();
    if enabled {
        env.storage()
            .instance()
            .set(&VersioningKey::AutoMigrate(caller.clone()), &true);
    } else {
        env.storage()
            .instance()
            .remove(&VersioningKey::AutoMigrate(caller.clone()));
    }
}

/// Trigger migration from old_version to new_version.
/// Supports batch migration up to MIGRATION_BATCH_SIZE records.
pub fn migrate_data(
    env: &Env,
    caller: &Address,
    old_version: u32,
    new_version: u32,
    batch: Vec<Bytes>,
) -> Result<MigrationRecord, VersioningError> {
    caller.require_auth();

    if (batch.len() as u32) > MIGRATION_BATCH_SIZE {
        return Err(VersioningError::BatchTooLarge);
    }

    let current = get_version(env);
    if new_version > current {
        return Err(VersioningError::IncompatibleVersion);
    }

    let status_key = VersioningKey::MigrationStatus(old_version, new_version);
    if env
        .storage()
        .instance()
        .get::<VersioningKey, MigrationRecord>(&status_key)
        .is_some()
    {
        return Err(VersioningError::AlreadyMigrated);
    }

    // Placeholder: actual migration logic would be contract-specific.
    // Here we simply record the migration.
    let record = MigrationRecord {
        from_version: old_version,
        to_version: new_version,
        migrated_at: env.ledger().timestamp(),
        record_count: batch.len() as u32,
    };

    env.storage()
        .instance()
        .set(&status_key, &record);

    env.events().publish(
        (symbol_short!("version"), symbol_short!("migrated")),
        (
            caller.clone(),
            old_version,
            new_version,
            record.record_count,
            record.migrated_at,
        ),
    );

    Ok(record)
}

/// Check if automatic migration is enabled for the caller.
pub fn is_auto_migrate_enabled(env: &Env, caller: &Address) -> bool {
    env.storage()
        .instance()
        .get(&VersioningKey::AutoMigrate(caller.clone()))
        .unwrap_or(false)
}

/// Get migration record for a version pair, if any.
pub fn get_migration_record(
    env: &Env,
    old_version: u32,
    new_version: u32,
) -> Option<MigrationRecord> {
    env.storage()
        .instance()
        .get(&VersioningKey::MigrationStatus(old_version, new_version))
}
