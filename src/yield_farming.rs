use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum FarmError {
    LockNotMet = 1,
    InsufficientBalance = 2,
    InvalidPool = 3,
    WhaleCapExceeded = 4,
    /// A checked arithmetic operation overflowed (Issue #239).
    ArithmeticOverflow = 5,
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

/// Time-weighted linear APY reward for `amount` staked over `elapsed` seconds.
///
/// `reward = amount * BASE_APY_BPS * elapsed / (10_000 * SECONDS_IN_YEAR)`,
/// computed with checked arithmetic so a pathological `amount`/`elapsed` pair
/// overflows into `None` instead of silently wrapping (Issue #239).
fn calculate_farm_reward(amount: i128, elapsed: u64) -> Option<i128> {
    amount
        .checked_mul(BASE_APY_BPS)?
        .checked_mul(elapsed as i128)?
        .checked_div(10_000_i128.checked_mul(SECONDS_IN_YEAR as i128)?)
}

pub fn deposit_to_pool(
    env: Env,
    owner: Address,
    amount: i128,
    lock_period: u32,
) -> Result<u64, FarmError> {
    owner.require_auth();

    if amount > WHALE_CAP {
        return Err(FarmError::WhaleCapExceeded);
    }

    let pool_id = env
        .storage()
        .instance()
        .get::<_, u64>(&symbol_short!("next_pid"))
        .unwrap_or(0);

    let pool = FarmPool {
        id: pool_id,
        owner: owner.clone(),
        amount,
        lock_period,
        start_time: env.ledger().timestamp(),
        last_harvest: env.ledger().timestamp(),
    };

    env.storage().persistent().set(&pool_id, &pool);
    let next_pool_id = pool_id
        .checked_add(1)
        .ok_or(FarmError::ArithmeticOverflow)?;
    env.storage()
        .instance()
        .set(&symbol_short!("next_pid"), &next_pool_id);

    env.events().publish(
        (symbol_short!("farm"), symbol_short!("deposit")),
        (owner, amount, lock_period, pool_id),
    );

    Ok(pool_id)
}

pub fn harvest_farm_rewards(env: Env, owner: Address, pool_id: u64) -> Result<i128, FarmError> {
    owner.require_auth();

    let mut pool: FarmPool = env
        .storage()
        .persistent()
        .get(&pool_id)
        .ok_or(FarmError::InvalidPool)?;

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
    let reward =
        calculate_farm_reward(pool.amount, elapsed).ok_or(FarmError::ArithmeticOverflow)?;

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

    let pool: FarmPool = env
        .storage()
        .persistent()
        .get(&pool_id)
        .ok_or(FarmError::InvalidPool)?;

    if pool.owner != owner {
        return Err(FarmError::InvalidPool);
    }

    let now = env.ledger().timestamp();
    let unlock_at = pool
        .start_time
        .checked_add(pool.lock_period as u64)
        .ok_or(FarmError::ArithmeticOverflow)?;
    if now < unlock_at {
        return Err(FarmError::LockNotMet);
    }

    // Harvest remaining rewards first
    let reward = harvest_farm_rewards(env.clone(), owner.clone(), pool_id)?;

    env.storage().persistent().remove(&pool_id);

    pool.amount
        .checked_add(reward)
        .ok_or(FarmError::ArithmeticOverflow)
}

// ── Tests (Issue #239: arithmetic safety) ───────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use soroban_sdk::testutils::Address as _;

    fn make_env() -> Env {
        let env = Env::default();
        env.mock_all_auths();
        env
    }

    #[test]
    fn test_calculate_farm_reward_matches_worked_example() {
        // 15% APY, 100 staked, 1 full year elapsed => 15 units of reward.
        let reward = calculate_farm_reward(100, SECONDS_IN_YEAR).unwrap();
        assert_eq!(reward, 15);
    }

    #[test]
    fn test_calculate_farm_reward_zero_elapsed_is_zero() {
        assert_eq!(calculate_farm_reward(1_000_000, 0), Some(0));
    }

    #[test]
    fn test_calculate_farm_reward_overflow_reported_not_wrapped() {
        // i128::MAX * BASE_APY_BPS overflows the first checked_mul.
        assert_eq!(calculate_farm_reward(i128::MAX, SECONDS_IN_YEAR), None);
    }

    proptest! {
        /// The reward helper never panics for any amount within the whale
        /// cap and any realistic elapsed duration, and never returns a
        /// negative reward for a non-negative stake.
        #[test]
        fn farm_reward_never_panics_within_whale_cap(
            amount in 0i128..=WHALE_CAP,
            elapsed in 0u64..=(SECONDS_IN_YEAR * 100),
        ) {
            let reward = calculate_farm_reward(amount, elapsed);
            if let Some(r) = reward {
                prop_assert!(r >= 0);
            }
        }
    }

    #[test]
    fn test_deposit_withdraw_pool_id_increments_safely() {
        let env = make_env();
        let owner = Address::generate(&env);

        let id1 = deposit_to_pool(env.clone(), owner.clone(), 100, 0).unwrap();
        let id2 = deposit_to_pool(env.clone(), owner.clone(), 100, 0).unwrap();
        assert_eq!(id2, id1 + 1);
    }

    #[test]
    fn test_withdraw_before_lock_period_rejected() {
        let env = make_env();
        let owner = Address::generate(&env);

        let id = deposit_to_pool(env.clone(), owner.clone(), 100, 1_000).unwrap();
        let result = withdraw_from_pool(env.clone(), owner, id);
        assert_eq!(result, Err(FarmError::LockNotMet));
    }
}
