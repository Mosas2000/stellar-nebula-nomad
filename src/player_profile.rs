use soroban_sdk::{contracttype, contracterror, symbol_short, Address, Env, Vec};

use crate::input_validation;

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
    /// Consecutive daily-login days (Issue #280). Authoritative streak value —
    /// [`crate::daily_rewards`] owns the calendar, the profile owns the streak.
    pub login_streak: u32,
    /// Best login streak ever achieved.
    pub longest_login_streak: u32,
    /// Day index (`timestamp / 86_400`) of the most recent recorded login.
    pub last_login_day: u64,
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
    /// A balance-modifying operation would have wrapped.
    ArithmeticOverflow = 5,
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
        login_streak: 0,
        longest_login_streak: 0,
        last_login_day: 0,
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

/// Retrieve a player profile by owner address.
pub fn get_profile_by_owner(env: &Env, owner: &Address) -> Result<PlayerProfile, ProfileError> {
    let profile_id: u64 = env
        .storage()
        .persistent()
        .get(&ProfileKey::OwnerProfile(owner.clone()))
        .ok_or(ProfileError::ProfileNotFound)?;

    get_profile(env, profile_id)
}

/// Mark an achievement flag on a profile.
pub fn mark_achievement_unlocked(
    env: &Env,
    profile_id: u64,
    achievement_id: u64,
) -> Result<(), ProfileError> {
    let mut profile = get_profile(env, profile_id)?;
    if achievement_id > 0 && achievement_id <= 32 {
        profile.achievement_flags |= 1u32 << ((achievement_id - 1) as u32);
    }

    env.storage()
        .persistent()
        .set(&ProfileKey::Profile(profile_id), &profile);

    Ok(())
}

// ─── Reward Crediting (Issue #280) ────────────────────────────────────────────

/// Credit `amount` essence to a profile without requiring the owner's auth.
///
/// Intended for reward-granting subsystems (daily logins, quests) that have
/// already established the caller's right to the payout. Rejects negative
/// amounts so it can never be used as a debit path, and uses checked
/// arithmetic per the crate-wide overflow policy.
pub fn credit_essence(env: &Env, profile_id: u64, amount: i128) -> Result<i128, ProfileError> {
    if amount < 0 {
        return Err(ProfileError::Unauthorized);
    }

    let mut profile = get_profile(env, profile_id)?;
    profile.essence_earned = profile
        .essence_earned
        .checked_add(amount)
        .ok_or(ProfileError::ArithmeticOverflow)?;
    profile.last_updated = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&ProfileKey::Profile(profile_id), &profile);

    env.events().publish(
        (symbol_short!("profile"), symbol_short!("credited")),
        (profile_id, amount, profile.essence_earned),
    );

    Ok(profile.essence_earned)
}

/// Record a daily login against a profile.
///
/// `login_day` is a `timestamp / 86_400` day index and `streak` the streak the
/// caller computed for that day; the profile stores both so any subsystem can
/// read the streak without re-deriving it. Stale writes (a `login_day` at or
/// before the one already stored) are ignored rather than rejected, keeping the
/// call idempotent for retried transactions.
pub fn record_login(
    env: &Env,
    profile_id: u64,
    login_day: u64,
    streak: u32,
) -> Result<u32, ProfileError> {
    let mut profile = get_profile(env, profile_id)?;

    if login_day <= profile.last_login_day && profile.last_login_day != 0 {
        return Ok(profile.login_streak);
    }

    profile.login_streak = streak;
    profile.longest_login_streak = profile.longest_login_streak.max(streak);
    profile.last_login_day = login_day;
    profile.last_updated = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&ProfileKey::Profile(profile_id), &profile);

    env.events().publish(
        (symbol_short!("profile"), symbol_short!("login")),
        (profile_id, login_day, streak),
    );

    Ok(streak)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{contract, contractimpl};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    /// Storage is only reachable inside a contract invocation, so each test
    /// body runs through `Env::as_contract`.
    fn with_profile<T>(f: impl FnOnce(&Env, u64) -> T) -> T {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register(Stub, ());
        let owner = Address::generate(&env);

        let id = env.as_contract(&contract, || {
            initialize_profile(&env, owner.clone()).unwrap()
        });
        env.as_contract(&contract, || f(&env, id))
    }

    #[test]
    fn new_profile_starts_with_no_login_history() {
        with_profile(|env, id| {
            let profile = get_profile(env, id).unwrap();
            assert_eq!(profile.login_streak, 0);
            assert_eq!(profile.longest_login_streak, 0);
            assert_eq!(profile.last_login_day, 0);
        });
    }

    #[test]
    fn credit_essence_accumulates() {
        with_profile(|env, id| {
            assert_eq!(credit_essence(env, id, 50).unwrap(), 50);
            assert_eq!(credit_essence(env, id, 25).unwrap(), 75);
            assert_eq!(get_profile(env, id).unwrap().essence_earned, 75);
        });
    }

    #[test]
    fn credit_essence_rejects_negative_amounts() {
        with_profile(|env, id| {
            assert_eq!(credit_essence(env, id, -1), Err(ProfileError::Unauthorized));
            assert_eq!(get_profile(env, id).unwrap().essence_earned, 0);
        });
    }

    #[test]
    fn credit_essence_detects_overflow() {
        with_profile(|env, id| {
            credit_essence(env, id, i128::MAX).unwrap();
            assert_eq!(
                credit_essence(env, id, 1),
                Err(ProfileError::ArithmeticOverflow)
            );
            // The failed credit left the balance untouched.
            assert_eq!(get_profile(env, id).unwrap().essence_earned, i128::MAX);
        });
    }

    #[test]
    fn credit_essence_requires_an_existing_profile() {
        with_profile(|env, _id| {
            assert_eq!(
                credit_essence(env, 9_999, 10),
                Err(ProfileError::ProfileNotFound)
            );
        });
    }

    #[test]
    fn record_login_tracks_streak_and_best() {
        with_profile(|env, id| {
            record_login(env, id, 10, 1).unwrap();
            record_login(env, id, 11, 2).unwrap();
            record_login(env, id, 12, 3).unwrap();
            assert_eq!(get_profile(env, id).unwrap().longest_login_streak, 3);

            // A broken streak lowers the current value but not the best.
            record_login(env, id, 20, 1).unwrap();
            let profile = get_profile(env, id).unwrap();
            assert_eq!(profile.login_streak, 1);
            assert_eq!(profile.longest_login_streak, 3);
            assert_eq!(profile.last_login_day, 20);
        });
    }

    #[test]
    fn record_login_ignores_stale_days() {
        with_profile(|env, id| {
            record_login(env, id, 10, 5).unwrap();

            // Replaying an older day must not rewind the profile.
            assert_eq!(record_login(env, id, 9, 1).unwrap(), 5);
            let profile = get_profile(env, id).unwrap();
            assert_eq!(profile.login_streak, 5);
            assert_eq!(profile.last_login_day, 10);
        });
    }
}
