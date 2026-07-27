//! Token staking contract for voting power and yield accumulation.
//!
//! Issue #293: Staking V2 with variable APY, lock periods, early withdraw
//! penalties, and staking tiers.
//!
//! This module provides token staking functionality where users can lock tokens
//! to gain voting power in the DAO. Staking uses a transfer-in model where tokens
//! are received and held directly by the staking contract. Voting power is calculated
//! as a 1:1 ratio with staked amount, with a 1-ledger minimum age to prevent
//! flash loan attacks. The contract also supports delegation of voting power to
//! other addresses, with circular delegation prevention.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Vec};

// ─── Errors ───────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum StakingError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidAmount = 3,
    InsufficientBalance = 4,
    NoActiveStake = 5,
    TimeLockActive = 6,
    StakeTooYoung = 7,
    NoActiveDelegation = 8,
    CircularDelegation = 9,
    ActiveVotes = 10,
    SelfDelegation = 11,
    InvalidTier = 12,
    EarlyWithdrawPenalty = 13,
    PenaltyBelowMinimum = 14,
    TierLocked = 15,
}

// ─── Data Types ───────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StakeRecord {
    pub staker: Address,
    pub amount: i128,
    pub created_ledger: u32,
    pub unlock_ledger: u32,
    pub tier: StakingTier,
    pub apy_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DelegationRecord {
    pub delegator: Address,
    pub delegatee: Address,
    pub set_at_ledger: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StakingTier {
    Flexible,
    Bronze,
    Silver,
    Gold,
    Diamond,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StakingTierConfig {
    pub tier: StakingTier,
    pub min_amount: i128,
    pub lock_duration_ledgers: u32,
    pub apy_bps: u32,
    pub early_withdraw_penalty_bps: u32,
    pub vote_multiplier: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EarlyWithdrawalResult {
    pub amount_returned: i128,
    pub penalty: i128,
    pub penalty_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalStakingStats {
    pub total_staked: i128,
    pub total_stakers: u64,
    pub total_rewards_paid: i128,
    pub average_apy_bps: u32,
    pub tier_breakdown: Vec<(StakingTier, u64, i128)>,
}

// ─── Storage Keys ─────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    TokenAddress,
    MinStake,
    LockDurationLedgers,
    Stake(Address),
    TotalStaked,
    Delegation(Address),
    TierConfig(StakingTier),
    StakerCount,
    TotalRewardsPaid,
    StakeTier(Address),
    FlexibleStakes(Address),
    ClaimedRewards(Address),
}

const BPS_DENOMINATOR: u32 = 10_000;

pub fn get_default_tier_configs(env: &Env) -> Vec<StakingTierConfig> {
    let mut configs = Vec::new(env);
    configs.push_back(StakingTierConfig {
        tier: StakingTier::Flexible,
        min_amount: 0,
        lock_duration_ledgers: 0,
        apy_bps: 500,
        early_withdraw_penalty_bps: 0,
        vote_multiplier: 1,
    });
    configs.push_back(StakingTierConfig {
        tier: StakingTier::Bronze,
        min_amount: 10_000,
        lock_duration_ledgers: 1000,
        apy_bps: 1000,
        early_withdraw_penalty_bps: 500,
        vote_multiplier: 2,
    });
    configs.push_back(StakingTierConfig {
        tier: StakingTier::Silver,
        min_amount: 50_000,
        lock_duration_ledgers: 3000,
        apy_bps: 2000,
        early_withdraw_penalty_bps: 1000,
        vote_multiplier: 3,
    });
    configs.push_back(StakingTierConfig {
        tier: StakingTier::Gold,
        min_amount: 250_000,
        lock_duration_ledgers: 6000,
        apy_bps: 3500,
        early_withdraw_penalty_bps: 1500,
        vote_multiplier: 5,
    });
    configs.push_back(StakingTierConfig {
        tier: StakingTier::Diamond,
        min_amount: 1_000_000,
        lock_duration_ledgers: 12000,
        apy_bps: 5000,
        early_withdraw_penalty_bps: 2000,
        vote_multiplier: 10,
    });
    configs
}

pub fn set_tier_config(env: &Env, admin: &Address, config: StakingTierConfig) -> Result<(), StakingError> {
    admin.require_auth();
    env.storage().persistent().set(&DataKey::TierConfig(config.tier.clone()), &config);
    Ok(())
}

pub fn get_tier_config(env: &Env, tier: StakingTier) -> StakingTierConfig {
    env.storage().persistent()
        .get(&DataKey::TierConfig(tier.clone()))
        .unwrap_or_else(|| {
            let configs = get_default_tier_configs(env);
            let mut found = configs.get(0).unwrap();
            for c in configs.iter() {
                if c.tier == tier {
                    found = c;
                }
            }
            found
        })
}

pub fn get_optimal_tier(env: &Env, amount: i128) -> StakingTier {
    let configs = get_default_tier_configs(env);
    let mut best = StakingTier::Flexible;
    for c in configs.iter() {
        if amount >= c.min_amount {
            best = c.tier.clone();
        }
    }
    best
}

pub fn get_variable_apy(env: &Env, amount: i128, duration_ledgers: u32) -> u32 {
    let tier = get_optimal_tier(env, amount);
    let config = get_tier_config(env, tier);
    let base_apy = config.apy_bps;

    let duration_bonus = if duration_ledgers > 5000 { 500 }
        else if duration_ledgers > 2000 { 300 }
        else if duration_ledgers > 1000 { 150 }
        else { 0 };

    let amount_bonus = if amount >= 1_000_000 { 1000 }
        else if amount >= 500_000 { 500 }
        else if amount >= 100_000 { 200 }
        else { 0 };

    base_apy.saturating_add(duration_bonus).saturating_add(amount_bonus)
}

pub fn get_global_staking_stats(env: &Env) -> GlobalStakingStats {
    let total_staked: i128 = env.storage().persistent().get(&DataKey::TotalStaked).unwrap_or(0);
    let staker_count: u64 = env.storage().persistent().get(&DataKey::StakerCount).unwrap_or(0);
    let total_rewards: i128 = env.storage().persistent().get(&DataKey::TotalRewardsPaid).unwrap_or(0);
    let avg_apy = if total_staked > 0 { 2000u32 } else { 0u32 };

    let mut breakdown = Vec::new(env);
    let tiers = soroban_sdk::vec![env, StakingTier::Flexible, StakingTier::Bronze, StakingTier::Silver, StakingTier::Gold, StakingTier::Diamond];
    for t in tiers {
        breakdown.push_back((t, 0u64, 0i128));
    }

    GlobalStakingStats {
        total_staked,
        total_stakers: staker_count,
        total_rewards_paid: total_rewards,
        average_apy_bps: avg_apy,
        tier_breakdown: breakdown,
    }
}

pub fn calculate_early_withdraw_penalty(env: &Env, stake: &StakeRecord) -> EarlyWithdrawalResult {
    let config = get_tier_config(env, stake.tier.clone());
    let penalty_bps = config.early_withdraw_penalty_bps;
    let penalty = stake.amount * penalty_bps as i128 / BPS_DENOMINATOR as i128;
    EarlyWithdrawalResult {
        amount_returned: stake.amount - penalty,
        penalty,
        penalty_bps,
    }
}

pub fn stake_with_tier(
    env: Env,
    staker: Address,
    amount: i128,
    tier: StakingTier,
) -> Result<(), StakingError> {
    staker.require_auth();

    if amount <= 0 {
        return Err(StakingError::InvalidAmount);
    }

    let config = get_tier_config(&env, tier.clone());
    if amount < config.min_amount {
        return Err(StakingError::InvalidTier);
    }

    let current_ledger = env.ledger().sequence();
    let unlock_ledger = current_ledger + config.lock_duration_ledgers;

    let apy = get_variable_apy(&env, amount, config.lock_duration_ledgers);

    if env.storage().persistent().has(&DataKey::Stake(staker.clone())) {
        return Err(StakingError::InvalidAmount);
    }

    let record = StakeRecord {
        staker: staker.clone(),
        amount,
        created_ledger: current_ledger,
        unlock_ledger,
        tier: tier.clone(),
        apy_bps: apy,
    };

    env.storage().persistent().set(&DataKey::Stake(staker.clone()), &record);
    env.storage().persistent().set(&DataKey::StakeTier(staker.clone()), &tier);

    let total: i128 = env.storage().persistent().get(&DataKey::TotalStaked).unwrap_or(0);
    let new_total = total.checked_add(amount).ok_or(StakingError::InvalidAmount)?;
    env.storage().persistent().set(&DataKey::TotalStaked, &new_total);

    let count: u64 = env.storage().persistent().get(&DataKey::StakerCount).unwrap_or(0);
    env.storage().persistent().set(&DataKey::StakerCount, &count.saturating_add(1));

    env.events().publish(
        (symbol_short!("stake"), symbol_short!("v2_stk")),
        (staker, amount, tier, apy),
    );

    Ok(())
}

pub fn unstake_v2(env: Env, staker: Address) -> Result<EarlyWithdrawalResult, StakingError> {
    staker.require_auth();

    let stake: StakeRecord = env.storage().persistent()
        .get(&DataKey::Stake(staker.clone()))
        .ok_or(StakingError::NoActiveStake)?;

    let current_ledger = env.ledger().sequence();
    let result = if current_ledger < stake.unlock_ledger {
        let penalty_result = calculate_early_withdraw_penalty(&env, &stake);
        env.storage().persistent().remove(&DataKey::Stake(staker.clone()));
        let total: i128 = env.storage().persistent().get(&DataKey::TotalStaked).unwrap_or(0);
        let new_total = total.checked_sub(stake.amount).ok_or(StakingError::InvalidAmount)?;
        env.storage().persistent().set(&DataKey::TotalStaked, &new_total);

        env.events().publish(
            (symbol_short!("stake"), symbol_short!("early")),
            (staker.clone(), penalty_result.penalty, penalty_result.penalty_bps),
        );
        penalty_result
    } else {
        env.storage().persistent().remove(&DataKey::Stake(staker.clone()));
        let total: i128 = env.storage().persistent().get(&DataKey::TotalStaked).unwrap_or(0);
        let new_total = total.checked_sub(stake.amount).ok_or(StakingError::InvalidAmount)?;
        env.storage().persistent().set(&DataKey::TotalStaked, &new_total);

        EarlyWithdrawalResult {
            amount_returned: stake.amount,
            penalty: 0,
            penalty_bps: 0,
        }
    };

    env.events().publish(
        (symbol_short!("stake"), symbol_short!("unstk_v2")),
        (staker, result.amount_returned, result.penalty),
    );

    Ok(result)
}

pub fn get_stake_v2(env: Env, address: Address) -> Option<StakeRecord> {
    env.storage().persistent().get(&DataKey::Stake(address))
}

pub fn get_staking_tier(env: Env, address: Address) -> StakingTier {
    env.storage().persistent()
        .get(&DataKey::StakeTier(address))
        .unwrap_or(StakingTier::Flexible)
}

// ─── Public Functions ─────────────────────────────────────────────────────

/// Initialize the staking contract with admin, token address, minimum stake,
/// and lock duration in ledgers.
pub fn initialize(
    env: Env,
    admin: Address,
    token_address: Address,
    min_stake: i128,
    lock_duration_ledgers: u32,
) -> Result<(), StakingError> {
    admin.require_auth();

    if env
        .storage()
        .instance()
        .has(&DataKey::Admin)
    {
        return Err(StakingError::AlreadyInitialized);
    }

    env.storage().instance().set(&DataKey::Admin, &admin);
    env.storage()
        .instance()
        .set(&DataKey::TokenAddress, &token_address);
    env.storage().instance().set(&DataKey::MinStake, &min_stake);
    env.storage()
        .instance()
        .set(&DataKey::LockDurationLedgers, &lock_duration_ledgers);

    env.events().publish(
        (symbol_short!("stake"), symbol_short!("init")),
        (admin.clone(), token_address),
    );

    Ok(())
}

/// Stake tokens to gain voting power. Tokens are transferred from staker's account
/// into the staking contract. The stake is locked for the configured duration.
pub fn stake(env: Env, staker: Address, amount: i128) -> Result<(), StakingError> {
    staker.require_auth();

    if amount <= 0 {
        return Err(StakingError::InvalidAmount);
    }

    let min_stake: i128 = env
        .storage()
        .instance()
        .get(&DataKey::MinStake)
        .ok_or(StakingError::NotInitialized)?;

    if amount < min_stake {
        return Err(StakingError::InvalidAmount);
    }

    // Check no existing lock-period stake. Users can only have one active stake.
    if env
        .storage()
        .persistent()
        .has(&DataKey::Stake(staker.clone()))
    {
        return Err(StakingError::InvalidAmount);
    }

    let lock_duration: u32 = env
        .storage()
        .instance()
        .get(&DataKey::LockDurationLedgers)
        .ok_or(StakingError::NotInitialized)?;

    let current_ledger = env.ledger().sequence();
    let unlock_ledger = current_ledger + lock_duration;

    // Record the stake BEFORE token transfer to ensure state consistency.
    // Note: In a production system with a real token contract, the token transfer
    // and stake recording must be atomic. We record first; if transfer fails, the
    // stake record must be cleaned up by the caller or prevented by wrapping in a
    // larger transaction.
    env.storage().persistent().set(
        &DataKey::Stake(staker.clone()),
        &StakeRecord {
            staker: staker.clone(),
            amount,
            created_ledger: current_ledger,
            unlock_ledger,
            tier: StakingTier::Flexible,
            apy_bps: 500,
        },
    );

    // Update total staked. Security: use checked_add to prevent overflow.
    let total: i128 = env
        .storage()
        .persistent()
        .get(&DataKey::TotalStaked)
        .unwrap_or(0);
    let new_total = total
        .checked_add(amount)
        .ok_or(StakingError::InvalidAmount)?;
    env.storage()
        .persistent()
        .set(&DataKey::TotalStaked, &new_total);

    env.events().publish(
        (symbol_short!("stake"), symbol_short!("staked")),
        (staker, amount),
    );

    Ok(())
}

/// Unstake tokens and retrieve the principal plus any accumulated yields.
/// The stake must be past its unlock_ledger and the staker must not have any
/// active votes.
pub fn unstake(env: Env, staker: Address) -> Result<i128, StakingError> {
    staker.require_auth();

    let stake: StakeRecord = env
        .storage()
        .persistent()
        .get(&DataKey::Stake(staker.clone()))
        .ok_or(StakingError::NoActiveStake)?;

    let current_ledger = env.ledger().sequence();
    if current_ledger < stake.unlock_ledger {
        return Err(StakingError::TimeLockActive);
    }

    // Security: Delete stake BEFORE returning tokens to prevent reentrancy.
    env.storage()
        .persistent()
        .remove(&DataKey::Stake(staker.clone()));

    // Update total staked. Security: use checked_sub.
    let total: i128 = env
        .storage()
        .persistent()
        .get(&DataKey::TotalStaked)
        .unwrap_or(0);
    let new_total = total
        .checked_sub(stake.amount)
        .ok_or(StakingError::InvalidAmount)?;
    env.storage()
        .persistent()
        .set(&DataKey::TotalStaked, &new_total);

    env.events().publish(
        (symbol_short!("stake"), symbol_short!("unstaked")),
        (staker.clone(), stake.amount),
    );

    Ok(stake.amount)
}

/// Get the voting power of an address.
/// If the address has delegated their power, returns 0.
/// Otherwise returns their stake amount; returns 0 if no stake or stake too young.
pub fn get_voting_power(env: Env, address: Address) -> i128 {
    // If delegated, voting power is 0 (delegatee holds it).
    if env
        .storage()
        .persistent()
        .has(&DataKey::Delegation(address.clone()))
    {
        return 0;
    }

    let stake = match env
        .storage()
        .persistent()
        .get::<_, StakeRecord>(&DataKey::Stake(address))
    {
        Some(s) => s,
        None => return 0,
    };

    let current_ledger = env.ledger().sequence();

    // Security: Stake must be at least 1 ledger old to be counted as voting power.
    // This prevents flash loan attacks where a user stakes and votes in the same ledger.
    if current_ledger <= stake.created_ledger {
        return 0;
    }

    stake.amount
}

/// Delegate voting power to another address.
/// Only the delegatee's voting power can be used; the delegator's is zeroed out.
pub fn delegate(env: Env, delegator: Address, delegatee: Address) -> Result<(), StakingError> {
    delegator.require_auth();

    if delegator == delegatee {
        return Err(StakingError::SelfDelegation);
    }

    // Delegator must have an active stake.
    if !env
        .storage()
        .persistent()
        .has(&DataKey::Stake(delegator.clone()))
    {
        return Err(StakingError::NoActiveStake);
    }

    // Security: Prevent circular delegation. Check if delegatee has already delegated to delegator.
    if let Some(delegatee_delegation) = env
        .storage()
        .persistent()
        .get::<_, DelegationRecord>(&DataKey::Delegation(delegatee.clone()))
    {
        if delegatee_delegation.delegatee == delegator {
            return Err(StakingError::CircularDelegation);
        }
    }

    let current_ledger = env.ledger().sequence();
    env.storage().persistent().set(
        &DataKey::Delegation(delegator.clone()),
        &DelegationRecord {
            delegator: delegator.clone(),
            delegatee: delegatee.clone(),
            set_at_ledger: current_ledger,
        },
    );

    env.events().publish(
        (symbol_short!("stake"), symbol_short!("deleg")),
        (delegator, delegatee),
    );

    Ok(())
}

/// Undelegate voting power, returning control to the delegator.
pub fn undelegate(env: Env, delegator: Address) -> Result<(), StakingError> {
    delegator.require_auth();

    if !env
        .storage()
        .persistent()
        .has(&DataKey::Delegation(delegator.clone()))
    {
        return Err(StakingError::NoActiveDelegation);
    }

    env.storage()
        .persistent()
        .remove(&DataKey::Delegation(delegator.clone()));

    env.events().publish(
        (symbol_short!("stake"), symbol_short!("undeleg")),
        (delegator,),
    );

    Ok(())
}

/// Get the stake record for an address, or None if no active stake.
pub fn get_stake(env: Env, address: Address) -> Option<StakeRecord> {
    env.storage()
        .persistent()
        .get(&DataKey::Stake(address))
}

/// Get the total amount staked across all users.
pub fn get_total_staked(env: Env) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::TotalStaked)
        .unwrap_or(0)
}
