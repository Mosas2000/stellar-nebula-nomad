use soroban_sdk::{contracttype, contracterror, symbol_short, Address, Env, Symbol};

/// Default essence bonus distributed to the referrer after the new nomad's first scan.
/// Overridable at runtime via `set_reward_config`.
pub const DEFAULT_ESSENCE_REWARD: i128 = 100;
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
    /// Lifetime essence rewarded per referrer address.
    LifetimeRewards(Address),
    /// Global reward pool balance (essence held for distribution).
    RewardPool,
    /// Reward configuration: fixed amount per claim.
    RewardConfig,
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

/// Admin-configurable reward settings.
#[derive(Clone)]
#[contracttype]
pub struct RewardConfig {
    /// Fixed essence amount paid out per successful referral claim.
    pub reward_per_claim: i128,
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
    InsufficientRewardPool = 7,
}

// ─── Admin Functions ──────────────────────────────────────────────────────────

/// Deposit `amount` essence into the global reward pool.
///
/// Only callable by an authorised admin address. The pool balance must be
/// positive before referral rewards can be claimed.
pub fn fund_reward_pool(env: &Env, admin: Address, amount: i128) -> Result<i128, ReferralError> {
    admin.require_auth();

    let current: i128 = env
        .storage()
        .instance()
        .get(&ReferralKey::RewardPool)
        .unwrap_or(0i128);
    let new_balance = current + amount;
    env.storage()
        .instance()
        .set(&ReferralKey::RewardPool, &new_balance);

    env.events().publish(
        (symbol_short!("referral"), symbol_short!("funded")),
        (admin, amount, new_balance),
    );

    Ok(new_balance)
}

/// Update the per-claim reward amount.
///
/// Only callable by an authorised admin address.
pub fn set_reward_config(
    env: &Env,
    admin: Address,
    reward_per_claim: i128,
) -> Result<(), ReferralError> {
    admin.require_auth();

    env.storage()
        .instance()
        .set(&ReferralKey::RewardConfig, &RewardConfig { reward_per_claim });

    env.events().publish(
        (symbol_short!("referral"), symbol_short!("cfg")),
        (admin, reward_per_claim),
    );

    Ok(())
}

// ─── View Helpers ─────────────────────────────────────────────────────────────

/// Return the current reward pool balance.
pub fn get_reward_pool_balance(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&ReferralKey::RewardPool)
        .unwrap_or(0i128)
}

/// Return lifetime essence rewards earned by `referrer`.
pub fn get_lifetime_rewards(env: &Env, referrer: Address) -> i128 {
    env.storage()
        .persistent()
        .get(&ReferralKey::LifetimeRewards(referrer))
        .unwrap_or(0i128)
}

// ─── Core Functions ───────────────────────────────────────────────────────────

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
/// per referrer. Deducts from the global reward pool and accumulates
/// `lifetime_rewards` for the referrer. Emits `RewardClaimed`.
/// Returns the essence amount awarded.
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

    // Resolve configured reward amount (falls back to default if admin hasn't set it).
    let reward_amount: i128 = env
        .storage()
        .instance()
        .get::<ReferralKey, RewardConfig>(&ReferralKey::RewardConfig)
        .map(|c| c.reward_per_claim)
        .unwrap_or(DEFAULT_ESSENCE_REWARD);

    // Verify the pool can cover the payout.
    let pool: i128 = env
        .storage()
        .instance()
        .get(&ReferralKey::RewardPool)
        .unwrap_or(0i128);
    if pool < reward_amount {
        return Err(ReferralError::InsufficientRewardPool);
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

    // Deduct from pool.
    env.storage()
        .instance()
        .set(&ReferralKey::RewardPool, &(pool - reward_amount));

    // Accumulate lifetime rewards for the referrer.
    let lifetime_key = ReferralKey::LifetimeRewards(referrer.clone());
    let lifetime: i128 = env
        .storage()
        .persistent()
        .get(&lifetime_key)
        .unwrap_or(0i128);
    env.storage()
        .persistent()
        .set(&lifetime_key, &(lifetime + reward_amount));

    referral.claimed = true;
    env.storage()
        .persistent()
        .set(&ReferralKey::Referral(new_nomad.clone()), &referral);

    env.events().publish(
        (symbol_short!("referral"), symbol_short!("claimed")),
        (referrer, new_nomad, reward_amount),
    );

    Ok(reward_amount)
}

/// Retrieve a referral record by the new nomad's address.
pub fn get_referral(env: &Env, new_nomad: Address) -> Result<Referral, ReferralError> {
    env.storage()
        .persistent()
        .get(&ReferralKey::Referral(new_nomad))
        .ok_or(ReferralError::ReferralNotFound)
}

// ═══════════════════════════════════════════════════════════════════════════════
// REFERRAL PROGRAM V2 (#279)
// ═══════════════════════════════════════════════════════════════════════════════

/// Additional storage keys for V2 features.
#[derive(Clone)]
#[contracttype]
pub enum ReferralV2Key {
    /// Multi-tier reward config per tier level.
    TierConfig(u32),
    /// Referral count per referrer (for tier calculation).
    ReferralCount(Address),
    /// Referrer analytics: total earned, total referrals, conversion rate.
    ReferrerAnalytics(Address),
    /// Fraud flags per address.
    FraudFlag(Address),
    /// IP/device fingerprint tracking for fraud detection.
    FingerprintHash(u64),
    /// Global analytics.
    TotalReferrals,
    TotalRewardsDistributed,
}

/// Tier configuration for multi-tier rewards.
#[derive(Clone)]
#[contracttype]
pub struct TierConfig {
    /// Minimum referrals to qualify for this tier.
    pub min_referrals: u32,
    /// Reward multiplier in basis points (10000 = 1x, 15000 = 1.5x).
    pub multiplier_bps: u32,
    /// Tier name for display.
    pub tier_level: u32,
}

/// Referrer analytics record.
#[derive(Clone)]
#[contracttype]
pub struct ReferrerAnalytics {
    pub total_referrals: u32,
    pub successful_referrals: u32,
    pub total_essence_earned: i128,
    pub current_tier: u32,
    pub joined_at: u64,
    pub last_referral_at: u64,
}

/// Fraud detection flags.
#[derive(Clone)]
#[contracttype]
pub struct FraudRecord {
    pub address: Address,
    pub flag_count: u32,
    pub is_blocked: bool,
    pub reason: Symbol,
}

/// Additional errors for V2.
#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ReferralV2Error {
    /// Player is blocked due to fraud detection.
    PlayerBlocked = 10,
    /// Referral velocity too high — possible fraud.
    VelocityTooHigh = 11,
    /// Tier not found.
    TierNotFound = 12,
}

const VELOCITY_WINDOW: u64 = 3_600; // 1 hour
const MAX_REFERRALS_PER_HOUR: u32 = 5;

/// Initialize multi-tier reward tiers. Call once at contract startup.
pub fn init_tiers(env: &Env) {
    let tiers = [
        TierConfig { min_referrals: 0, multiplier_bps: 10_000, tier_level: 0 },   // Bronze: 1x
        TierConfig { min_referrals: 5, multiplier_bps: 12_000, tier_level: 1 },    // Silver: 1.2x
        TierConfig { min_referrals: 20, multiplier_bps: 15_000, tier_level: 2 },   // Gold: 1.5x
        TierConfig { min_referrals: 50, multiplier_bps: 20_000, tier_level: 3 },   // Platinum: 2x
        TierConfig { min_referrals: 100, multiplier_bps: 25_000, tier_level: 4 },  // Diamond: 2.5x
    ];

    for tier in tiers.iter() {
        env.storage()
            .instance()
            .set(&ReferralV2Key::TierConfig(tier.tier_level), tier);
    }
}

/// Get the tier config for a given tier level.
pub fn get_tier_config(env: &Env, tier_level: u32) -> Option<TierConfig> {
    env.storage()
        .instance()
        .get(&ReferralV2Key::TierConfig(tier_level))
}

/// Calculate the referrer's current tier based on their referral count.
pub fn calculate_tier(env: &Env, referrer: &Address) -> u32 {
    let count: u32 = env
        .storage()
        .persistent()
        .get(&ReferralV2Key::ReferralCount(referrer.clone()))
        .unwrap_or(0);

    // Find the highest tier the referrer qualifies for.
    let mut best_tier = 0u32;
    for level in 0..5 {
        if let Some(tier) = get_tier_config(env, level) {
            if count >= tier.min_referrals {
                best_tier = tier.tier_level;
            }
        }
    }
    best_tier
}

/// Register a referral with fraud detection and multi-tier rewards.
pub fn register_referral_v2(
    env: &Env,
    referrer: Address,
    new_nomad: Address,
    fingerprint: Option<u64>,
) -> Result<u64, ReferralError> {
    // Fraud checks.
    check_fraud(env, &referrer, &new_nomad, fingerprint)?;

    // Check velocity.
    check_velocity(env, &referrer)?;

    // Use the original registration logic.
    let id = register_referral(env, referrer.clone(), new_nomad)?;

    // Increment referrer count.
    let count: u32 = env
        .storage()
        .persistent()
        .get(&ReferralV2Key::ReferralCount(referrer.clone()))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&ReferralV2Key::ReferralCount(referrer.clone()), &(count + 1));

    // Update analytics.
    let mut analytics: ReferrerAnalytics = env
        .storage()
        .persistent()
        .get(&ReferralV2Key::ReferrerAnalytics(referrer.clone()))
        .unwrap_or(ReferrerAnalytics {
            total_referrals: 0,
            successful_referrals: 0,
            total_essence_earned: 0,
            current_tier: 0,
            joined_at: env.ledger().timestamp(),
            last_referral_at: 0,
        });
    analytics.total_referrals += 1;
    analytics.current_tier = calculate_tier(env, &referrer);
    analytics.last_referral_at = env.ledger().timestamp();
    env.storage()
        .persistent()
        .set(&ReferralV2Key::ReferrerAnalytics(referrer.clone()), &analytics);

    // Update global count.
    let total: u64 = env
        .storage()
        .instance()
        .get(&ReferralV2Key::TotalReferrals)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&ReferralV2Key::TotalReferrals, &(total + 1));

    Ok(id)
}

/// Claim referral reward with tier multiplier.
pub fn claim_referral_reward_v2(
    env: &Env,
    referrer: Address,
    new_nomad: Address,
) -> Result<i128, ReferralError> {
    // Check if blocked by fraud detection.
    let fraud: FraudRecord = env
        .storage()
        .persistent()
        .get(&ReferralV2Key::FraudFlag(referrer.clone()))
        .unwrap_or(FraudRecord {
            address: referrer.clone(),
            flag_count: 0,
            is_blocked: false,
            reason: symbol_short!("clean"),
        });

    if fraud.is_blocked {
        return Err(ReferralError::InsufficientRewardPool);
    }

    // Claim base reward using existing logic.
    let base_reward = claim_referral_reward(env, referrer.clone(), new_nomad)?;

    // Apply tier multiplier.
    let tier = calculate_tier(env, &referrer);
    let multiplier: u32 = get_tier_config(env, tier)
        .map(|t| t.multiplier_bps)
        .unwrap_or(10_000);

    let bonus = base_reward * (multiplier as i128 - 10_000) / 10_000;

    // Update analytics.
    if let Some(mut analytics) = env
        .storage()
        .persistent()
        .get::<_, ReferrerAnalytics>(&ReferralV2Key::ReferrerAnalytics(referrer.clone()))
    {
        analytics.successful_referrals += 1;
        analytics.total_essence_earned += base_reward + bonus;
        analytics.current_tier = tier;
        env.storage()
            .persistent()
            .set(&ReferralV2Key::ReferrerAnalytics(referrer.clone()), &analytics);
    }

    // Update global rewards.
    let total: i128 = env
        .storage()
        .instance()
        .get(&ReferralV2Key::TotalRewardsDistributed)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&ReferralV2Key::TotalRewardsDistributed, &(total + base_reward + bonus));

    Ok(base_reward + bonus)
}

/// Get referrer analytics.
pub fn get_referrer_analytics(env: &Env, referrer: &Address) -> Option<ReferrerAnalytics> {
    env.storage()
        .persistent()
        .get(&ReferralV2Key::ReferrerAnalytics(referrer.clone()))
}

/// Get global referral statistics.
pub fn get_global_stats(env: &Env) -> (u64, i128) {
    let total_referrals: u64 = env
        .storage()
        .instance()
        .get(&ReferralV2Key::TotalReferrals)
        .unwrap_or(0);
    let total_rewards: i128 = env
        .storage()
        .instance()
        .get(&ReferralV2Key::TotalRewardsDistributed)
        .unwrap_or(0);
    (total_referrals, total_rewards)
}

// ─── Fraud Prevention ─────────────────────────────────────────────────────

/// Check for fraud indicators before registering a referral.
fn check_fraud(
    env: &Env,
    referrer: &Address,
    new_nomad: &Address,
    fingerprint: Option<u64>,
) -> Result<(), ReferralError> {
    // Check if referrer is blocked.
    let fraud: FraudRecord = env
        .storage()
        .persistent()
        .get(&ReferralV2Key::FraudFlag(referrer.clone()))
        .unwrap_or(FraudRecord {
            address: referrer.clone(),
            flag_count: 0,
            is_blocked: false,
            reason: symbol_short!("clean"),
        });

    if fraud.is_blocked {
        return Err(ReferralError::InsufficientRewardPool);
    }

    // Check fingerprint collision (same device referring multiple accounts).
    if let Some(fp) = fingerprint {
        let existing: Option<Address> = env
            .storage()
            .temporary()
            .get(&ReferralV2Key::FingerprintHash(fp));
        if let Some(existing_addr) = existing {
            if existing_addr != *referrer {
                flag_fraud(env, referrer, symbol_short!("fp_collis"));
            }
        } else {
            env.storage()
                .temporary()
                .set(&ReferralV2Key::FingerprintHash(fp), referrer);
        }
    }

    Ok(())
}

/// Check referral velocity (too many referrals in a short time).
fn check_velocity(env: &Env, referrer: &Address) -> Result<(), ReferralError> {
    let now = env.ledger().timestamp();
    let window_key = ReferralKey::DailyClaims(referrer.clone(), now / VELOCITY_WINDOW);
    let count: u32 = env.storage().temporary().get(&window_key).unwrap_or(0);

    if count >= MAX_REFERRALS_PER_HOUR {
        flag_fraud(env, referrer, symbol_short!("velocity"));
        return Err(ReferralError::DailyClaimCapReached);
    }

    env.storage().temporary().set(&window_key, &(count + 1));
    Ok(())
}

/// Flag an address for fraud.
fn flag_fraud(env: &Env, address: &Address, reason: Symbol) {
    let mut fraud: FraudRecord = env
        .storage()
        .persistent()
        .get(&ReferralV2Key::FraudFlag(address.clone()))
        .unwrap_or(FraudRecord {
            address: address.clone(),
            flag_count: 0,
            is_blocked: false,
            reason: symbol_short!("clean"),
        });

    fraud.flag_count += 1;
    fraud.reason = reason;

    // Block after 3 flags.
    if fraud.flag_count >= 3 {
        fraud.is_blocked = true;
    }

    env.storage()
        .persistent()
        .set(&ReferralV2Key::FraudFlag(address.clone()), &fraud);

    env.events().publish(
        (symbol_short!("referral"), symbol_short!("fraud")),
        (address.clone(), fraud.flag_count, fraud.is_blocked),
    );
}

/// Admin: manually block a referrer for fraud.
pub fn admin_block_referrer(
    env: &Env,
    admin: Address,
    target: Address,
    reason: Symbol,
) -> Result<(), ReferralError> {
    admin.require_auth();

    let fraud = FraudRecord {
        address: target.clone(),
        flag_count: 99,
        is_blocked: true,
        reason,
    };

    env.storage()
        .persistent()
        .set(&ReferralV2Key::FraudFlag(target), &fraud);

    Ok(())
}
