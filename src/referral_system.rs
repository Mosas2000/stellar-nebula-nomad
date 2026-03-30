use soroban_sdk::{contracttype, contracterror, symbol_short, Address, Env};

/// Essence bonus distributed to the referrer after the new nomad's first scan.
pub const ESSENCE_REWARD: i128 = 100;
/// Maximum number of rewards a referrer may claim in a single calendar day.
pub const MAX_DAILY_CLAIMS: u32 = 10;
/// Seconds in one day — used to derive the current day bucket.
const SECS_PER_DAY: u64 = 86_400;

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum ReferralKey {
    /// Referral record keyed by the new nomad's address (prevents duplicates).
    Referral(Address),
    /// Global auto-increment counter for referral IDs.
    ReferralCount,
    /// Daily claim counter: (referrer, day_number) → u32.
    DailyClaims(Address, u64),
}

// ─── Data Types ───────────────────────────────────────────────────────────────

/// On-chain referral record linking a referrer to a newly onboarded nomad.
#[derive(Clone)]
#[contracttype]
pub struct Referral {
    pub id: u64,
    pub referrer: Address,
    pub new_nomad: Address,
    pub registered_at: u64,
    /// True once the referrer has claimed the reward.
    pub claimed: bool,
    /// True once the new nomad has completed their first scan.
    pub first_scan_done: bool,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ReferralError {
    AlreadyReferred = 1,
    SelfReferral = 2,
    ReferralNotFound = 3,
    AlreadyClaimed = 4,
    FirstScanNotDone = 5,
    DailyClaimCapReached = 6,
}

// ─── Functions ────────────────────────────────────────────────────────────────

/// Record a referral from `referrer` for `new_nomad`.
///
/// Prevents self-referrals and duplicate registrations. Emits
/// `ReferralRegistered`. Returns the new referral ID.
pub fn register_referral(
    env: &Env,
    referrer: Address,
    new_nomad: Address,
) -> Result<u64, ReferralError> {
    referrer.require_auth();

    if referrer == new_nomad {
        return Err(ReferralError::SelfReferral);
    }

    if env
        .storage()
        .persistent()
        .has(&ReferralKey::Referral(new_nomad.clone()))
    {
        return Err(ReferralError::AlreadyReferred);
    }

    let id: u64 = env
        .storage()
        .instance()
        .get(&ReferralKey::ReferralCount)
        .unwrap_or(0u64)
        + 1;
    env.storage()
        .instance()
        .set(&ReferralKey::ReferralCount, &id);

    let referral = Referral {
        id,
        referrer: referrer.clone(),
        new_nomad: new_nomad.clone(),
        registered_at: env.ledger().timestamp(),
        claimed: false,
        first_scan_done: false,
    };

    env.storage()
        .persistent()
        .set(&ReferralKey::Referral(new_nomad.clone()), &referral);

    env.events().publish(
        (symbol_short!("referral"), symbol_short!("register")),
        (referrer, new_nomad, id),
    );

    Ok(id)
}

/// Mark that `nomad` has completed their first scan, unlocking the referral reward.
///
/// Called by the scan flow after a successful `scan_nebula`. The nomad
/// must authorize this call.
pub fn mark_first_scan(env: &Env, nomad: Address) -> Result<(), ReferralError> {
    nomad.require_auth();

    let mut referral: Referral = env
        .storage()
        .persistent()
        .get(&ReferralKey::Referral(nomad.clone()))
        .ok_or(ReferralError::ReferralNotFound)?;

    referral.first_scan_done = true;
    env.storage()
        .persistent()
        .set(&ReferralKey::Referral(nomad), &referral);

    Ok(())
}

/// Distribute the essence bonus to the referrer.
///
/// One-time claim per referral. Enforces a daily cap of `MAX_DAILY_CLAIMS`
/// per referrer. Emits `RewardClaimed`. Returns the essence amount awarded.
pub fn claim_referral_reward(
    env: &Env,
    referrer: Address,
    new_nomad: Address,
) -> Result<i128, ReferralError> {
    referrer.require_auth();

    let mut referral: Referral = env
        .storage()
        .persistent()
        .get(&ReferralKey::Referral(new_nomad.clone()))
        .ok_or(ReferralError::ReferralNotFound)?;

    if !referral.first_scan_done {
        return Err(ReferralError::FirstScanNotDone);
    }

    if referral.claimed {
        return Err(ReferralError::AlreadyClaimed);
    }

    // Enforce daily claim cap using temporary storage keyed by day bucket.
    let day = env.ledger().timestamp() / SECS_PER_DAY;
    let daily_key = ReferralKey::DailyClaims(referrer.clone(), day);
    let daily_count: u32 = env.storage().temporary().get(&daily_key).unwrap_or(0u32);
    if daily_count >= MAX_DAILY_CLAIMS {
        return Err(ReferralError::DailyClaimCapReached);
    }
    env.storage()
        .temporary()
        .set(&daily_key, &(daily_count + 1));

    referral.claimed = true;
    env.storage()
        .persistent()
        .set(&ReferralKey::Referral(new_nomad.clone()), &referral);

    env.events().publish(
        (symbol_short!("referral"), symbol_short!("claimed")),
        (referrer, new_nomad, ESSENCE_REWARD),
    );

    Ok(ESSENCE_REWARD)
}

/// Retrieve a referral record by the new nomad's address.
pub fn get_referral(env: &Env, new_nomad: Address) -> Result<Referral, ReferralError> {
    env.storage()
        .persistent()
        .get(&ReferralKey::Referral(new_nomad))
        .ok_or(ReferralError::ReferralNotFound)
}
