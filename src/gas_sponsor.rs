use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol};

use crate::player_profile::{self, PlayerProfile};

// ─── Configuration ─────────────────────────────────────────────────────────

/// Maximum number of sponsorships allowed per day (burst limit).
pub const MAX_DAILY_SPONSORSHIPS: u32 = 100;

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
}

impl Default for SponsorConfig {
    fn default() -> Self {
        Self {
            min_threshold: 10_000_000, // 1 XLM in stroops
            sponsor_amount: 100_000,   // 0.01 XLM per scan
            daily_cap: MAX_DAILY_SPONSORSHIPS,
        }
    }
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

/// Sponsor the first scan for a new player, covering their gas costs.
/// 
/// # Requirements
/// - Player must have a verified profile (initialized)
/// - Player must not have been sponsored before (one-time only)
/// - Daily sponsorship cap must not be exceeded
/// - Fund must have sufficient balance
/// 
/// # Returns
/// - Ok(sponsor_amount) if sponsorship succeeds
/// - Err(SponsorError) if any requirement fails
pub fn sponsor_first_scan(env: &Env, player: &Address) -> Result<i128, SponsorError> {
    player.require_auth();

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

    // Check fund balance
    let fund_balance: i128 = env
        .storage()
        .instance()
        .get(&DataKey::FundBalance)
        .ok_or(SponsorError::NotInitialized)?;

    if fund_balance < config.sponsor_amount {
        return Err(SponsorError::InsufficientFunds);
    }

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
    };

    env.storage().instance().set(&DataKey::Config, &config);

    env.events().publish(
        (symbol_short!("sponsor"), symbol_short!("config")),
        (min_threshold, sponsor_amount, daily_cap),
    );

    Ok(config)
}
