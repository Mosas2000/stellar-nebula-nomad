use soroban_sdk::{contracttype, contracterror, symbol_short, Address, Env, Vec};

/// Maximum number of stat updates allowed in a single batch transaction.
pub const MAX_BATCH_SIZE: u32 = 5;

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum ProfileKey {
    /// Individual profile data keyed by profile ID.
    Profile(u64),
    /// Maps an owner address to their profile ID (prevents duplicates).
    OwnerProfile(Address),
    /// Global auto-increment counter for profile IDs.
    ProfileCount,
}

// ─── Data Types ───────────────────────────────────────────────────────────────

/// On-chain player profile tracking nomad journey progress.
#[derive(Clone)]
#[contracttype]
pub struct PlayerProfile {
    pub id: u64,
    pub owner: Address,
    pub total_scans: u32,
    pub essence_earned: i128,
    /// ID of the first ship linked to this profile.
    pub ship_id: u64,
    /// Bitmask of unlocked achievement flags for future NFT badges.
    pub achievement_flags: u32,
    pub created_at: u64,
    pub last_updated: u64,
}

/// Single entry for a batch progress update.
#[derive(Clone)]
#[contracttype]
pub struct ProgressUpdate {
    pub profile_id: u64,
    pub scan_count: u32,
    pub essence: i128,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ProfileError {
    ProfileNotFound = 1,
    ProfileAlreadyExists = 2,
    Unauthorized = 3,
    BatchTooLarge = 4,
}

// ─── Functions ────────────────────────────────────────────────────────────────

/// Create a new player profile for `owner`.
///
/// Derives a profile ID from the global counter. Emits `NomadJoined`.
/// Returns the new profile ID.
pub fn initialize_profile(env: &Env, owner: Address) -> Result<u64, ProfileError> {
    owner.require_auth();

    if env
        .storage()
        .persistent()
        .has(&ProfileKey::OwnerProfile(owner.clone()))
    {
        return Err(ProfileError::ProfileAlreadyExists);
    }

    let id: u64 = env
        .storage()
        .instance()
        .get(&ProfileKey::ProfileCount)
        .unwrap_or(0u64)
        + 1;
    env.storage().instance().set(&ProfileKey::ProfileCount, &id);

    let timestamp = env.ledger().timestamp();
    let profile = PlayerProfile {
        id,
        owner: owner.clone(),
        total_scans: 0,
        essence_earned: 0,
        ship_id: id,
        achievement_flags: 0,
        created_at: timestamp,
        last_updated: timestamp,
    };

    env.storage()
        .persistent()
        .set(&ProfileKey::Profile(id), &profile);
    env.storage()
        .persistent()
        .set(&ProfileKey::OwnerProfile(owner.clone()), &id);

    env.events().publish(
        (symbol_short!("nomad"), symbol_short!("joined")),
        (owner, id),
    );

    Ok(id)
}

/// Atomically update scan stats and essence after a successful harvest.
///
/// Caller must be the profile owner. Emits `ProfileUpdated`.
pub fn update_progress(
    env: &Env,
    caller: Address,
    profile_id: u64,
    scan_count: u32,
    essence: i128,
) -> Result<(), ProfileError> {
    caller.require_auth();

    let mut profile: PlayerProfile = env
        .storage()
        .persistent()
        .get(&ProfileKey::Profile(profile_id))
        .ok_or(ProfileError::ProfileNotFound)?;

    if profile.owner != caller {
        return Err(ProfileError::Unauthorized);
    }

    profile.total_scans += scan_count;
    profile.essence_earned += essence;
    profile.last_updated = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&ProfileKey::Profile(profile_id), &profile);

    env.events().publish(
        (symbol_short!("profile"), symbol_short!("updated")),
        (caller, profile_id, profile.total_scans, profile.essence_earned),
    );

    Ok(())
}

/// Apply up to `MAX_BATCH_SIZE` stat updates in a single transaction.
///
/// Useful for multi-scan runs. Each update is validated for ownership.
/// Emits `ProfileUpdated` for every entry in the batch.
pub fn batch_update_progress(
    env: &Env,
    caller: Address,
    updates: Vec<ProgressUpdate>,
) -> Result<(), ProfileError> {
    caller.require_auth();

    if updates.len() > MAX_BATCH_SIZE {
        return Err(ProfileError::BatchTooLarge);
    }

    for i in 0..updates.len() {
        let update = updates.get(i).unwrap();

        let mut profile: PlayerProfile = env
            .storage()
            .persistent()
            .get(&ProfileKey::Profile(update.profile_id))
            .ok_or(ProfileError::ProfileNotFound)?;

        if profile.owner != caller {
            return Err(ProfileError::Unauthorized);
        }

        profile.total_scans += update.scan_count;
        profile.essence_earned += update.essence;
        profile.last_updated = env.ledger().timestamp();

        env.storage()
            .persistent()
            .set(&ProfileKey::Profile(update.profile_id), &profile);

        env.events().publish(
            (symbol_short!("profile"), symbol_short!("updated")),
            (
                caller.clone(),
                update.profile_id,
                profile.total_scans,
                profile.essence_earned,
            ),
        );
    }

    Ok(())
}

/// Retrieve a player profile by ID. Returns `ProfileNotFound` if absent.
pub fn get_profile(env: &Env, profile_id: u64) -> Result<PlayerProfile, ProfileError> {
    env.storage()
        .persistent()
        .get(&ProfileKey::Profile(profile_id))
        .ok_or(ProfileError::ProfileNotFound)
}
