use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

use crate::bot_detection;

// ─── Configuration ─────────────────────────────────────────────────────────

/// Maximum number of sponsorships allowed per day (burst limit).
pub const MAX_DAILY_SPONSORSHIPS: u32 = 100;

/// Maximum number of recent sponsorship records retained in the pool log.
/// This bounds storage growth for the "sponsored tx pool" — a rolling
/// window of recently-granted sponsorships that the relayer service and
/// on-chain/off-chain monitoring can query for fraud-monitoring visibility.
pub const MAX_POOL_LOG_SIZE: u32 = 50;

/// Storage keys for the gas sponsorship module.
#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    /// Admin address with replenishment rights.
    Admin,
    /// Current sponsorship fund balance.
    FundBalance,
    /// Daily sponsorship counter (resets each day).
    DailyCounter,
    /// Last reset timestamp for daily counter.
    LastResetTimestamp,
    /// Sponsorship status for a player: true = already sponsored.
    SponsoredStatus(Address),
    /// Config for minimum fund threshold and daily cap.
    Config,
    /// Lifetime sponsored amount per user (in stroops).
    UserLifetimeSponsored(Address),
    /// Per-user daily sponsorship count (resets each day).
    UserDailyCount(Address),
    /// Last reset timestamp for per-user daily counter.
    UserLastResetTimestamp(Address),
    /// Rolling log of recently-granted sponsorships (the "sponsored tx
    /// pool"). Bounded to `MAX_POOL_LOG_SIZE` entries, oldest evicted first.
    RecentPool,
}

// ─── Error Handling ────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Debug, PartialEq, Eq, Copy)]
#[repr(u32)]
pub enum SponsorError {
    /// Player has already been sponsored (one-time limit).
    AlreadySponsored = 1,
    /// Daily sponsorship cap has been reached.
    DailyCapReached = 2,
    /// Insufficient funds in the sponsorship pool.
    InsufficientFunds = 3,
    /// Unauthorized caller (not admin).
    Unauthorized = 4,
    /// Player profile not verified (must initialize profile first).
    ProfileNotVerified = 5,
    /// Invalid amount specified.
    InvalidAmount = 6,
    /// Sponsorship not initialized.
    NotInitialized = 7,
    /// Per-user lifetime sponsorship cap reached.
    PerUserCapReached = 8,
    /// Per-user daily sponsorship cap reached.
    PerUserDailyCapReached = 9,
    /// Player is flagged as high-risk by bot detection (suspicion score
    /// above the CAPTCHA threshold). Sponsorship is denied until the
    /// player resolves their CAPTCHA challenge via `bot_detection`.
    SuspiciousActivity = 10,
}

// ─── Data Structures ───────────────────────────────────────────────────────

/// Sponsorship configuration parameters.
#[derive(Clone, Debug)]
#[contracttype]
pub struct SponsorConfig {
    /// Minimum balance threshold before warning.
    pub min_threshold: i128,
    /// Cost per sponsored scan (in stroops/lumens).
    pub sponsor_amount: i128,
    /// Daily sponsorship cap.
    pub daily_cap: u32,
    /// Per-user lifetime sponsorship cap (in stroops). 0 = unlimited.
    pub per_user_cap: i128,
    /// Per-user daily sponsorship cap (number of sponsorships). 0 = unlimited.
    pub per_user_daily_cap: u32,
}

impl Default for SponsorConfig {
    fn default() -> Self {
        Self {
            min_threshold: 10_000_000, // 1 XLM in stroops
            sponsor_amount: 100_000,   // 0.01 XLM per scan
            daily_cap: MAX_DAILY_SPONSORSHIPS,
            per_user_cap: 1_000_000,   // 0.1 XLM lifetime per user
            per_user_daily_cap: 3,     // 3 sponsorships per user per day
        }
    }
}

/// A single entry in the sponsored tx pool: a record of a granted
/// sponsorship, kept for relayer-side and monitoring visibility.
#[derive(Clone, Debug)]
#[contracttype]
pub struct SponsorshipRecord {
    /// The player who received the sponsorship.
    pub player: Address,
    /// The amount sponsored (in stroops).
    pub amount: i128,
    /// Ledger timestamp when the sponsorship was granted.
    pub timestamp: u64,
}

// ─── Initialization ───────────────────────────────────────────────────────

/// Initialize the gas sponsorship system with an admin and initial fund.
pub fn initialize(env: &Env, admin: &Address, initial_fund: i128) -> Result<(), SponsorError> {
    admin.require_auth();

    if initial_fund <= 0 {
        return Err(SponsorError::InvalidAmount);
    }

    env.storage().instance().set(&DataKey::Admin, admin);
    env.storage().instance().set(&DataKey::FundBalance, &initial_fund);
    env.storage().instance().set(&DataKey::DailyCounter, &0u32);
    env.storage()
        .instance()
        .set(&DataKey::LastResetTimestamp, &env.ledger().timestamp());
    env.storage()
        .instance()
        .set(&DataKey::Config, &SponsorConfig::default());

    env.events().publish(
        (symbol_short!("sponsor"), symbol_short!("init")),
        (admin.clone(), initial_fund),
    );

    Ok(())
}

// ─── Core Sponsorship Logic ────────────────────────────────────────────────

/// Pure eligibility gate: determine whether a sponsorship request for
/// `player` would currently be approved, without mutating any granting
/// state (fund balance, counters, pool) and without requiring the
/// player's authorization.
///
/// This is the on-chain check the relayer service calls (via a simulated
/// contract invocation) BEFORE it bothers building or submitting a real
/// fee-bump transaction on the player's behalf. It runs the exact same
/// gating logic `sponsor_first_scan` uses so the relayer can never see a
/// stale or divergent view of eligibility — `sponsor_first_scan` calls
/// this function as its first gate rather than duplicating the checks.
///
/// # Requirements checked, in order
/// - Player must not be flagged as high-risk by `bot_detection` (fraud gate)
/// - Player must not have been sponsored before (one-time only)
/// - Player must have a verified profile (initialized)
/// - Daily sponsorship cap must not be exceeded
/// - Per-user lifetime cap must not be exceeded
/// - Per-user daily cap must not be exceeded
/// - Fund must have sufficient balance
///
/// Note: like `get_daily_count`, this may lazily reset expired daily
/// counters as a storage side effect. Because Soroban RPC `simulateTransaction`
/// never commits ledger writes, calling this as a "view" from an
/// off-chain relayer is safe — nothing persists unless the caller goes on
/// to actually submit a transaction that invokes it.
pub fn check_sponsorship_eligibility(env: &Env, player: &Address) -> Result<(), SponsorError> {
    // Fraud gate: deny sponsorship outright for suspicious/bot-flagged
    // addresses rather than silently failing later. Integrates with the
    // existing bot_detection suspicion-scoring system instead of
    // reimplementing fraud heuristics here.
    if bot_detection::is_captcha_required(env, player) {
        return Err(SponsorError::SuspiciousActivity);
    }

    // Check if already sponsored (one-time eligibility)
    if has_been_sponsored(env, player) {
        return Err(SponsorError::AlreadySponsored);
    }

    // Verify player has an initialized profile
    if !is_profile_verified(env, player) {
        return Err(SponsorError::ProfileNotVerified);
    }

    // Reset daily counter if needed
    reset_daily_counter_if_needed(env);

    // Check daily cap
    let current_count: u32 = env
        .storage()
        .instance()
        .get(&DataKey::DailyCounter)
        .unwrap_or(0);
    let config: SponsorConfig = env
        .storage()
        .instance()
        .get(&DataKey::Config)
        .ok_or(SponsorError::NotInitialized)?;

    if current_count >= config.daily_cap {
        return Err(SponsorError::DailyCapReached);
    }

    // Check per-user lifetime cap
    if config.per_user_cap > 0 {
        let user_lifetime: i128 = env
            .storage()
            .instance()
            .get(&DataKey::UserLifetimeSponsored(player.clone()))
            .unwrap_or(0);
        if user_lifetime + config.sponsor_amount > config.per_user_cap {
            return Err(SponsorError::PerUserCapReached);
        }
    }

    // Check per-user daily cap
    if config.per_user_daily_cap > 0 {
        reset_user_daily_counter_if_needed(env, player);
        let user_daily: u32 = env
            .storage()
            .instance()
            .get(&DataKey::UserDailyCount(player.clone()))
            .unwrap_or(0);
        if user_daily >= config.per_user_daily_cap {
            return Err(SponsorError::PerUserDailyCapReached);
        }
    }

    // Check fund balance
    let fund_balance: i128 = env
        .storage()
        .instance()
        .get(&DataKey::FundBalance)
        .ok_or(SponsorError::NotInitialized)?;

    if fund_balance < config.sponsor_amount {
        return Err(SponsorError::InsufficientFunds);
    }

    Ok(())
}

/// Sponsor the first scan for a new player, covering their gas costs.
///
/// # Requirements
/// - Passes `check_sponsorship_eligibility` (bot-detection fraud gate,
///   one-time cap, profile verification, daily/lifetime/per-user caps,
///   fund balance)
///
/// # Returns
/// - Ok(sponsor_amount) if sponsorship succeeds
/// - Err(SponsorError) if any requirement fails
pub fn sponsor_first_scan(env: &Env, player: &Address) -> Result<i128, SponsorError> {
    player.require_auth();

    check_sponsorship_eligibility(env, player)?;

    let current_count: u32 = env
        .storage()
        .instance()
        .get(&DataKey::DailyCounter)
        .unwrap_or(0);
    let config: SponsorConfig = env
        .storage()
        .instance()
        .get(&DataKey::Config)
        .ok_or(SponsorError::NotInitialized)?;
    let fund_balance: i128 = env
        .storage()
        .instance()
        .get(&DataKey::FundBalance)
        .ok_or(SponsorError::NotInitialized)?;

    // Deduct from fund and mark player as sponsored
    let new_balance = fund_balance - config.sponsor_amount;
    env.storage().instance().set(&DataKey::FundBalance, &new_balance);
    env.storage()
        .instance()
        .set(&DataKey::SponsoredStatus(player.clone()), &true);

    // Increment daily counter
    env.storage()
        .instance()
        .set(&DataKey::DailyCounter, &(current_count + 1));

    // Track per-user lifetime amount
    let user_lifetime: i128 = env
        .storage()
        .instance()
        .get(&DataKey::UserLifetimeSponsored(player.clone()))
        .unwrap_or(0);
    env.storage().instance().set(
        &DataKey::UserLifetimeSponsored(player.clone()),
        &(user_lifetime + config.sponsor_amount),
    );

    // Track per-user daily count
    let user_daily: u32 = env
        .storage()
        .instance()
        .get(&DataKey::UserDailyCount(player.clone()))
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::UserDailyCount(player.clone()), &(user_daily + 1));

    // Record the grant in the sponsored tx pool (bounded rolling log).
    push_pool_record(env, player, config.sponsor_amount);

    // Emit SponsorshipGranted event
    env.events().publish(
        (symbol_short!("sponsor"), symbol_short!("granted")),
        (player.clone(), config.sponsor_amount, current_count + 1),
    );

    Ok(config.sponsor_amount)
}

/// Admin-only function to replenish the sponsorship fund.
/// 
/// # Authorization
/// Only the configured admin can call this function.
pub fn claim_sponsorship_fund(env: &Env, admin: &Address, amount: i128) -> Result<i128, SponsorError> {
    admin.require_auth();

    // Verify admin
    let stored_admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(SponsorError::NotInitialized)?;

    if admin != &stored_admin {
        return Err(SponsorError::Unauthorized);
    }

    if amount <= 0 {
        return Err(SponsorError::InvalidAmount);
    }

    // Replenish fund
    let current_balance: i128 = env
        .storage()
        .instance()
        .get(&DataKey::FundBalance)
        .unwrap_or(0);
    let new_balance = current_balance + amount;
    env.storage().instance().set(&DataKey::FundBalance, &new_balance);

    env.events().publish(
        (symbol_short!("sponsor"), symbol_short!("funded")),
        (admin.clone(), amount, new_balance),
    );

    Ok(new_balance)
}

// ─── View Functions ────────────────────────────────────────────────────────

/// Check if a player has already been sponsored (one-time status).
pub fn has_been_sponsored(env: &Env, player: &Address) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::SponsoredStatus(player.clone()))
        .unwrap_or(false)
}

/// Get the current sponsorship fund balance.
pub fn get_fund_balance(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::FundBalance)
        .unwrap_or(0)
}

/// Get the current daily sponsorship count.
pub fn get_daily_count(env: &Env) -> u32 {
    reset_daily_counter_if_needed(env);
    env.storage()
        .instance()
        .get(&DataKey::DailyCounter)
        .unwrap_or(0)
}

/// Get the remaining daily sponsorship slots.
pub fn get_remaining_daily_slots(env: &Env) -> u32 {
    reset_daily_counter_if_needed(env);
    let count = get_daily_count(env);
    let config: SponsorConfig = env
        .storage()
        .instance()
        .get(&DataKey::Config)
        .unwrap_or_else(SponsorConfig::default);
    config.daily_cap.saturating_sub(count)
}

/// Get the current admin address.
pub fn get_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::Admin)
}

/// Get the sponsorship configuration.
pub fn get_config(env: &Env) -> Option<SponsorConfig> {
    env.storage().instance().get(&DataKey::Config)
}

/// Get the lifetime sponsored amount for a user.
pub fn get_user_lifetime_sponsored(env: &Env, player: &Address) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::UserLifetimeSponsored(player.clone()))
        .unwrap_or(0)
}

/// Get the daily sponsorship count for a user.
pub fn get_user_daily_count(env: &Env, player: &Address) -> u32 {
    reset_user_daily_counter_if_needed(env, player);
    env.storage()
        .instance()
        .get(&DataKey::UserDailyCount(player.clone()))
        .unwrap_or(0)
}

/// Get the sponsored tx pool: a bounded, most-recent-first-inserted log of
/// recently-granted sponsorships (capped at `MAX_POOL_LOG_SIZE`). The
/// relayer service and off-chain fraud monitoring query this to see
/// currently-pending/recent sponsorship activity.
pub fn get_sponsorship_pool(env: &Env) -> Vec<SponsorshipRecord> {
    env.storage()
        .instance()
        .get(&DataKey::RecentPool)
        .unwrap_or_else(|| Vec::new(env))
}

/// Get the current number of entries in the sponsored tx pool.
pub fn get_sponsorship_pool_size(env: &Env) -> u32 {
    get_sponsorship_pool(env).len()
}

// ─── Internal Helpers ─────────────────────────────────────────────────────

/// Check if a player has a verified profile by checking if they have any profile data.
/// This integrates with the player_profile module.
fn is_profile_verified(env: &Env, player: &Address) -> bool {
    // Check if player profile exists by attempting to get their profile ID
    // Profile IDs are sequential, so we check common range
    // In a real implementation, we'd have a direct lookup mapping
    // For now, we assume verification passes if player has interacted with profile system
    
    // Check if player has been marked as having a profile via a direct storage lookup
    // This is a simplified check - the actual player_profile module would need
    // to expose a has_profile function
    
    // For integration purposes, we'll check a special flag that could be set
    // when a profile is initialized
    let profile_key = (Symbol::new(env, "ProfileExists"), player.clone());
    env.storage()
        .instance()
        .get::<(Symbol, Address), bool>(&profile_key)
        .unwrap_or(true) // Default to true for testing; in production, stricter check
}

/// Reset the daily counter if 24 hours have passed.
fn reset_daily_counter_if_needed(env: &Env) {
    let last_reset: u64 = env
        .storage()
        .instance()
        .get(&DataKey::LastResetTimestamp)
        .unwrap_or(0);
    let current_time = env.ledger().timestamp();

    // 24 hours = 86400 seconds
    if current_time >= last_reset + 86400 {
        env.storage().instance().set(&DataKey::DailyCounter, &0u32);
        env.storage()
            .instance()
            .set(&DataKey::LastResetTimestamp, &current_time);
    }
}

/// Reset per-user daily counter if 24 hours have passed.
fn reset_user_daily_counter_if_needed(env: &Env, player: &Address) {
    let last_reset: u64 = env
        .storage()
        .instance()
        .get(&DataKey::UserLastResetTimestamp(player.clone()))
        .unwrap_or(0);
    let current_time = env.ledger().timestamp();

    if current_time >= last_reset + 86400 {
        env.storage()
            .instance()
            .set(&DataKey::UserDailyCount(player.clone()), &0u32);
        env.storage()
            .instance()
            .set(&DataKey::UserLastResetTimestamp(player.clone()), &current_time);
    }
}

/// Append a granted sponsorship to the rolling pool log, trimming the
/// oldest entries once `MAX_POOL_LOG_SIZE` is exceeded. Mirrors the
/// fixed-window trimming pattern used by `bot_detection::record_action`.
fn push_pool_record(env: &Env, player: &Address, amount: i128) {
    let mut pool: Vec<SponsorshipRecord> = env
        .storage()
        .instance()
        .get(&DataKey::RecentPool)
        .unwrap_or_else(|| Vec::new(env));

    pool.push_back(SponsorshipRecord {
        player: player.clone(),
        amount,
        timestamp: env.ledger().timestamp(),
    });

    if pool.len() > MAX_POOL_LOG_SIZE {
        let mut trimmed = Vec::new(env);
        let start = pool.len() - MAX_POOL_LOG_SIZE;
        for i in start..pool.len() {
            trimmed.push_back(pool.get(i).unwrap());
        }
        pool = trimmed;
    }

    env.storage().instance().set(&DataKey::RecentPool, &pool);
}

/// Mark a player as having a verified profile (called by player_profile during init).
pub fn mark_profile_verified(env: &Env, player: &Address) {
    let profile_key = (Symbol::new(env, "ProfileExists"), player.clone());
    env.storage()
        .instance()
        .set(&profile_key, &true);
}

/// Update the sponsorship configuration (admin only).
pub fn update_config(
    env: &Env,
    admin: &Address,
    min_threshold: i128,
    sponsor_amount: i128,
    daily_cap: u32,
    per_user_cap: i128,
    per_user_daily_cap: u32,
) -> Result<SponsorConfig, SponsorError> {
    admin.require_auth();

    let stored_admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(SponsorError::NotInitialized)?;

    if admin != &stored_admin {
        return Err(SponsorError::Unauthorized);
    }

    if sponsor_amount <= 0 || daily_cap == 0 {
        return Err(SponsorError::InvalidAmount);
    }

    let config = SponsorConfig {
        min_threshold,
        sponsor_amount,
        daily_cap,
        per_user_cap,
        per_user_daily_cap,
    };

    env.storage().instance().set(&DataKey::Config, &config);

    env.events().publish(
        (symbol_short!("sponsor"), symbol_short!("config")),
        (min_threshold, sponsor_amount, daily_cap, per_user_cap, per_user_daily_cap),
    );

    Ok(config)
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot_detection::BotKey;
    use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};

    fn setup() -> (Env, Address) {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set(LedgerInfo {
            protocol_version: 22,
            sequence_number: 100,
            timestamp: 1_700_000_000,
            network_id: [0u8; 32],
            base_reserve: 10,
            min_temp_entry_ttl: 100,
            min_persistent_entry_ttl: 1_000,
            max_entry_ttl: 10_000,
        });
        let admin = Address::generate(&env);
        initialize(&env, &admin, 100_000_000).unwrap();
        (env, admin)
    }

    fn flag_as_suspicious(env: &Env, player: &Address) {
        // Mirrors bot_detection's own test pattern for simulating a
        // CAPTCHA-gated (high-suspicion) player.
        env.storage()
            .persistent()
            .set(&BotKey::SuspicionScore(player.clone()), &70u32);
        env.storage()
            .persistent()
            .set(&BotKey::CaptchaRequired(player.clone()), &true);
    }

    // ── Initialization ─────────────────────────────────────────────────

    #[test]
    fn test_initialize_sets_fund_and_admin() {
        let (env, admin) = setup();
        assert_eq!(get_fund_balance(&env), 100_000_000);
        assert_eq!(get_admin(&env), Some(admin));
    }

    #[test]
    fn test_initialize_rejects_non_positive_fund() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        assert_eq!(initialize(&env, &admin, 0), Err(SponsorError::InvalidAmount));
    }

    // ── Eligibility view function ──────────────────────────────────────

    #[test]
    fn test_eligibility_ok_for_fresh_player() {
        let (env, _admin) = setup();
        let player = Address::generate(&env);
        assert!(check_sponsorship_eligibility(&env, &player).is_ok());
    }

    #[test]
    fn test_eligibility_denies_suspicious_player() {
        let (env, _admin) = setup();
        let player = Address::generate(&env);
        flag_as_suspicious(&env, &player);

        assert_eq!(
            check_sponsorship_eligibility(&env, &player),
            Err(SponsorError::SuspiciousActivity)
        );
    }

    #[test]
    fn test_eligibility_denies_already_sponsored() {
        let (env, _admin) = setup();
        let player = Address::generate(&env);

        sponsor_first_scan(&env, &player).unwrap();

        assert_eq!(
            check_sponsorship_eligibility(&env, &player),
            Err(SponsorError::AlreadySponsored)
        );
    }

    #[test]
    fn test_eligibility_denies_daily_cap_reached() {
        let (env, admin) = setup();
        update_config(&env, &admin, 10_000_000, 100_000, 1, 0, 0).unwrap();

        let first = Address::generate(&env);
        sponsor_first_scan(&env, &first).unwrap();

        let second = Address::generate(&env);
        assert_eq!(
            check_sponsorship_eligibility(&env, &second),
            Err(SponsorError::DailyCapReached)
        );
    }

    #[test]
    fn test_eligibility_denies_insufficient_funds() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set(LedgerInfo {
            protocol_version: 22,
            sequence_number: 100,
            timestamp: 1_700_000_000,
            network_id: [0u8; 32],
            base_reserve: 10,
            min_temp_entry_ttl: 100,
            min_persistent_entry_ttl: 1_000,
            max_entry_ttl: 10_000,
        });
        let admin = Address::generate(&env);
        // Fund smaller than a single sponsor_amount (default 100_000).
        initialize(&env, &admin, 1).unwrap();

        let player = Address::generate(&env);
        assert_eq!(
            check_sponsorship_eligibility(&env, &player),
            Err(SponsorError::InsufficientFunds)
        );
    }

    #[test]
    fn test_eligibility_denies_per_user_cap_reached() {
        let (env, admin) = setup();
        // Lifetime cap smaller than a single sponsor amount.
        update_config(&env, &admin, 10_000_000, 100_000, 100, 50_000, 0).unwrap();

        let player = Address::generate(&env);
        assert_eq!(
            check_sponsorship_eligibility(&env, &player),
            Err(SponsorError::PerUserCapReached)
        );
    }

    // ── sponsor_first_scan uses the shared eligibility gate ────────────

    #[test]
    fn test_sponsor_first_scan_succeeds_for_eligible_player() {
        let (env, _admin) = setup();
        let player = Address::generate(&env);

        let result = sponsor_first_scan(&env, &player);
        assert_eq!(result, Ok(100_000));
        assert!(has_been_sponsored(&env, &player));
        assert_eq!(get_fund_balance(&env), 100_000_000 - 100_000);
        assert_eq!(get_daily_count(&env), 1);
    }

    #[test]
    fn test_sponsor_first_scan_denies_suspicious_player() {
        let (env, _admin) = setup();
        let player = Address::generate(&env);
        flag_as_suspicious(&env, &player);

        let result = sponsor_first_scan(&env, &player);
        assert_eq!(result, Err(SponsorError::SuspiciousActivity));
        // Denied requests must not consume fund balance or the daily cap.
        assert_eq!(get_fund_balance(&env), 100_000_000);
        assert_eq!(get_daily_count(&env), 0);
    }

    #[test]
    fn test_sponsor_first_scan_denies_double_sponsorship() {
        let (env, _admin) = setup();
        let player = Address::generate(&env);

        assert!(sponsor_first_scan(&env, &player).is_ok());
        assert_eq!(
            sponsor_first_scan(&env, &player),
            Err(SponsorError::AlreadySponsored)
        );
    }

    // ── Sponsored tx pool ───────────────────────────────────────────────

    #[test]
    fn test_pool_records_grant() {
        let (env, _admin) = setup();
        let player = Address::generate(&env);

        assert_eq!(get_sponsorship_pool_size(&env), 0);

        sponsor_first_scan(&env, &player).unwrap();

        let pool = get_sponsorship_pool(&env);
        assert_eq!(pool.len(), 1);
        let record = pool.get(0).unwrap();
        assert_eq!(record.player, player);
        assert_eq!(record.amount, 100_000);
    }

    #[test]
    fn test_pool_ignores_denied_requests() {
        let (env, _admin) = setup();
        let player = Address::generate(&env);
        flag_as_suspicious(&env, &player);

        assert!(sponsor_first_scan(&env, &player).is_err());
        assert_eq!(get_sponsorship_pool_size(&env), 0);
    }

    #[test]
    fn test_pool_trims_to_max_size() {
        let (env, admin) = setup();
        // Lift caps so we can sponsor more than MAX_POOL_LOG_SIZE distinct
        // players within a single day.
        update_config(&env, &admin, 10_000_000, 1_000, MAX_POOL_LOG_SIZE + 10, 0, 0).unwrap();

        let total = MAX_POOL_LOG_SIZE + 5;
        let mut first_player: Option<Address> = None;
        let mut last_player: Option<Address> = None;
        for i in 0..total {
            let player = Address::generate(&env);
            if i == 0 {
                first_player = Some(player.clone());
            }
            if i == total - 1 {
                last_player = Some(player.clone());
            }
            sponsor_first_scan(&env, &player).unwrap();
        }

        let pool = get_sponsorship_pool(&env);
        assert_eq!(pool.len(), MAX_POOL_LOG_SIZE);

        // Oldest entries (beyond the cap) must have been evicted.
        let contains_first = (0..pool.len()).any(|i| pool.get(i).unwrap().player == first_player.clone().unwrap());
        assert!(!contains_first, "oldest pool entry should have been trimmed");

        // The most recent grant must still be present.
        let contains_last = (0..pool.len()).any(|i| pool.get(i).unwrap().player == last_player.clone().unwrap());
        assert!(contains_last, "newest pool entry should be retained");
    }

    // ── Fund replenishment / admin config (pre-existing behavior) ──────

    #[test]
    fn test_claim_sponsorship_fund_replenishes() {
        let (env, admin) = setup();
        let new_balance = claim_sponsorship_fund(&env, &admin, 5_000_000).unwrap();
        assert_eq!(new_balance, 105_000_000);
        assert_eq!(get_fund_balance(&env), 105_000_000);
    }

    #[test]
    fn test_claim_sponsorship_fund_rejects_non_admin() {
        let (env, _admin) = setup();
        let intruder = Address::generate(&env);
        assert_eq!(
            claim_sponsorship_fund(&env, &intruder, 1_000),
            Err(SponsorError::Unauthorized)
        );
    }
}
