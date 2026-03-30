use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum FarmError {
    LockNotMet = 1,
    InsufficientBalance = 2,
    InvalidPool = 3,
    WhaleCapExceeded = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FarmPool {
    pub id: u64,
    pub owner: Address,
    pub amount: i128,
    pub lock_period: u32,
    pub start_time: u64,
    pub last_harvest: u64,
}

const SECONDS_IN_YEAR: u64 = 31_536_000;
const BASE_APY_BPS: i128 = 1500; // 15% Base APY
const WHALE_CAP: i128 = 1_000_000_000_000; // 1M essence example cap

pub fn deposit_to_pool(env: Env, owner: Address, amount: i128, lock_period: u32) -> Result<u64, FarmError> {
    owner.require_auth();

    if amount > WHALE_CAP {
        return Err(FarmError::WhaleCapExceeded);
    }

    let mut pool_id = env.storage().instance().get::<_, u64>(&symbol_short!("next_pid")).unwrap_or(0);
    
    let pool = FarmPool {
        id: pool_id,
        owner: owner.clone(),
        amount,
        lock_period,
        start_time: env.ledger().timestamp(),
        last_harvest: env.ledger().timestamp(),
    };

    env.storage().persistent().set(&pool_id, &pool);
    env.storage().instance().set(&symbol_short!("next_pid"), &(pool_id + 1));

    env.events().publish(
        (symbol_short!("farm"), symbol_short!("deposit")),
        (owner, amount, lock_period, pool_id),
    );

    Ok(pool_id)
}

pub fn harvest_farm_rewards(env: Env, owner: Address, pool_id: u64) -> Result<i128, FarmError> {
    owner.require_auth();

    let mut pool: FarmPool = env.storage().persistent().get(&pool_id).ok_or(FarmError::InvalidPool)?;
    
    if pool.owner != owner {
        return Err(FarmError::InvalidPool);
    }

    let now = env.ledger().timestamp();
    let elapsed = now.saturating_sub(pool.last_harvest);
    
    if elapsed == 0 {
        return Ok(0);
    }

    // Time-weighted reward calculation (simple linear APY for MVP)
    // Reward = Amount * (APY / 10000) * (Elapsed / SECONDS_IN_YEAR)
    let reward = (pool.amount * BASE_APY_BPS * elapsed as i128) / (10000 * SECONDS_IN_YEAR as i128);

    pool.last_harvest = now;
    env.storage().persistent().set(&pool_id, &pool);

    env.events().publish(
        (symbol_short!("farm"), symbol_short!("harvest")),
        (owner, reward, pool_id),
    );

    Ok(reward)
}

pub fn withdraw_from_pool(env: Env, owner: Address, pool_id: u64) -> Result<i128, FarmError> {
    owner.require_auth();

    let pool: FarmPool = env.storage().persistent().get(&pool_id).ok_or(FarmError::InvalidPool)?;
    
    if pool.owner != owner {
        return Err(FarmError::InvalidPool);
    }

    let now = env.ledger().timestamp();
    if now < pool.start_time + pool.lock_period as u64 {
        return Err(FarmError::LockNotMet);
    }

    // Harvest remaining rewards first
    let reward = harvest_farm_rewards(env.clone(), owner.clone(), pool_id)?;
    
    env.storage().persistent().remove(&pool_id);

    Ok(pool.amount + reward)
}
