//! Advanced trading — limit orders and trading history — Issue #141
//!
//! Provides a limit-order book (buy/sell at a specified price) and a
//! ring-buffered trading history for price charts and portfolio tracking.
//! Stop-loss orders are modelled as sell-side limit orders and executed
//! by an off-chain keeper that calls `cancel_limit_order` + market sell.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Map, Symbol, Vec};

use crate::reentrancy_guard::{with_guard, ReentrancyError};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum open orders per trader.
pub const MAX_ORDERS_PER_TRADER: u32 = 50;
/// Maximum records kept in the global trading history ring buffer.
pub const MAX_HISTORY: u32 = 50;

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum TradingKey {
    /// Order counter.
    OrderCounter,
    /// Open order by order_id.
    Order(u64),
    /// Open order IDs per trader.
    TraderOrders(Address),
    /// Trading history ring buffer (flat vec, newest first).
    History,
}

// ── Types ──────────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
#[contracttype]
pub enum OrderSide {
    Buy,
    Sell,
}

/// A limit order placed by a trader.
#[derive(Clone)]
#[contracttype]
pub struct LimitOrder {
    pub id: u64,
    pub trader: Address,
    pub side: OrderSide,
    /// Resource symbol being traded.
    pub resource: Symbol,
    /// Quantity in resource units.
    pub quantity: i128,
    /// Limit price per unit in stroops.
    pub limit_price: i128,
    /// Timestamp when the order was placed.
    pub placed_at: u64,
    /// True if this is a stop-loss order.
    pub is_stop_loss: bool,
}

/// A completed trade recorded in history.
#[derive(Clone)]
#[contracttype]
pub struct TradeRecord {
    pub order_id: u64,
    pub trader: Address,
    pub side: OrderSide,
    pub resource: Symbol,
    pub quantity: i128,
    pub price: i128,
    pub executed_at: u64,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TradingError {
    InvalidOrder = 1,
    OrderNotFound = 2,
    NotOrderOwner = 3,
    OrderCapReached = 4,
    InvalidPrice = 5,
    InvalidQuantity = 6,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn next_order_id(env: &Env) -> u64 {
    let n: u64 = env
        .storage()
        .instance()
        .get(&TradingKey::OrderCounter)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&TradingKey::OrderCounter, &(n + 1));
    n + 1
}

// ── Functions ─────────────────────────────────────────────────────────────────

/// Place a limit order (buy or sell). Emits `OrderPlaced`.
///
/// Returns the new order ID.
pub fn place_limit_order(
    env: &Env,
    trader: &Address,
    mut order: LimitOrder,
) -> Result<u64, TradingError> {
    trader.require_auth();

    if order.limit_price <= 0 {
        return Err(TradingError::InvalidPrice);
    }
    if order.quantity <= 0 {
        return Err(TradingError::InvalidQuantity);
    }

    // Enforce per-trader open order cap
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&TradingKey::TraderOrders(trader.clone()))
        .unwrap_or_else(|| Vec::new(env));

    if ids.len() >= MAX_ORDERS_PER_TRADER {
        return Err(TradingError::OrderCapReached);
    }

    let id = next_order_id(env);
    order.id = id;
    order.trader = trader.clone();
    order.placed_at = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&TradingKey::Order(id), &order);

    ids.push_back(id);
    env.storage()
        .persistent()
        .set(&TradingKey::TraderOrders(trader.clone()), &ids);

    env.events().publish(
        (symbol_short!("trade"), symbol_short!("placed")),
        (trader.clone(), id, order.limit_price, order.quantity),
    );

    Ok(id)
}

/// Cancel an open limit order (owner only). Emits `OrderCancelled`.
pub fn cancel_limit_order(env: &Env, trader: &Address, order_id: u64) -> Result<(), TradingError> {
    trader.require_auth();

    let order: LimitOrder = env
        .storage()
        .persistent()
        .get(&TradingKey::Order(order_id))
        .ok_or(TradingError::OrderNotFound)?;

    if &order.trader != trader {
        return Err(TradingError::NotOrderOwner);
    }

    env.storage()
        .persistent()
        .remove(&TradingKey::Order(order_id));

    // Remove from trader's order list
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&TradingKey::TraderOrders(trader.clone()))
        .unwrap_or_else(|| Vec::new(env));
    let mut new_ids: Vec<u64> = Vec::new(env);
    for i in 0..ids.len() {
        let oid = ids.get(i).unwrap();
        if oid != order_id {
            new_ids.push_back(oid);
        }
    }
    env.storage()
        .persistent()
        .set(&TradingKey::TraderOrders(trader.clone()), &new_ids);

    env.events().publish(
        (symbol_short!("trade"), symbol_short!("cancel")),
        (trader.clone(), order_id),
    );

    Ok(())
}

/// Get a limit order by ID.
pub fn get_limit_order(env: &Env, order_id: u64) -> Option<LimitOrder> {
    env.storage().persistent().get(&TradingKey::Order(order_id))
}

/// Get all open order IDs for a trader and return the order structs.
pub fn get_trader_orders(env: &Env, trader: &Address) -> Vec<LimitOrder> {
    let ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&TradingKey::TraderOrders(trader.clone()))
        .unwrap_or_else(|| Vec::new(env));

    let mut orders: Vec<LimitOrder> = Vec::new(env);
    for i in 0..ids.len() {
        let oid = ids.get(i).unwrap();
        if let Some(order) = env.storage().persistent().get(&TradingKey::Order(oid)) {
            orders.push_back(order);
        }
    }
    orders
}

/// Record a completed trade in the history ring buffer. Emits `TradeExecuted`.
pub fn record_trade(env: &Env, caller: &Address, trade: TradeRecord) -> Result<(), TradingError> {
    caller.require_auth();

    let mut history: Vec<TradeRecord> = env
        .storage()
        .persistent()
        .get(&TradingKey::History)
        .unwrap_or_else(|| Vec::new(env));

    // Keep only the most recent MAX_HISTORY records (trim oldest)
    if history.len() >= MAX_HISTORY {
        let mut trimmed: Vec<TradeRecord> = Vec::new(env);
        let start = history.len() - MAX_HISTORY + 1;
        for i in start..history.len() {
            trimmed.push_back(history.get(i).unwrap());
        }
        history = trimmed;
    }

    history.push_back(trade.clone());
    env.storage()
        .persistent()
        .set(&TradingKey::History, &history);

    env.events().publish(
        (symbol_short!("trade"), symbol_short!("exec")),
        (caller.clone(), trade.order_id, trade.price, trade.quantity),
    );

    Ok(())
}

/// Return the full trading history (up to `MAX_HISTORY` records).
pub fn get_trading_history(env: &Env) -> Vec<TradeRecord> {
    env.storage()
        .persistent()
        .get(&TradingKey::History)
        .unwrap_or_else(|| Vec::new(env))
}

// ═══════════════════════════════════════════════════════════════════════════════
//  AMM — Automated Market Maker (Issue #189)
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum slippage allowed for swaps (in basis points: 100 = 1%).
pub const MAX_SLIPPAGE_BPS: u32 = 500;
/// Maximum number of hops in a multi-pool swap route.
pub const AMM_MAX_ROUTE_HOPS: u32 = 3;
/// Fee charged on each swap, in basis points (30 = 0.3%).
pub const SWAP_FEE_BPS: u32 = 30;

#[derive(Clone)]
#[contracttype]
pub enum AmmKey {
    /// Liquidity pool by pool_id.
    Pool(u64),
    /// Pool counter.
    PoolCounter,
    /// LP token balance for (pool_id, provider).
    LpBalance(u64, Address),
    /// Total LP token supply for a pool.
    LpTotalSupply(u64),
    /// List of pool IDs.
    PoolList,
}

#[contracterror]
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u32)]
pub enum AmmError {
    PoolNotFound = 100,
    PoolAlreadyExists = 101,
    InsufficientLiquidity = 102,
    InvalidAmount = 103,
    SlippageExceeded = 104,
    InvalidRoute = 105,
    InsufficientLpTokens = 106,
    ZeroLiquidity = 107,
    /// A guarded section was re-entered (Issue #238).
    Reentrancy = 108,
}

impl From<ReentrancyError> for AmmError {
    fn from(_: ReentrancyError) -> Self {
        AmmError::Reentrancy
    }
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct LiquidityPool {
    pub pool_id: u64,
    /// Resource symbol pair (e.g. resource_a, resource_b).
    pub resource_a: Symbol,
    pub resource_b: Symbol,
    /// Reserve balances.
    pub reserve_a: i128,
    pub reserve_b: i128,
    /// Total LP token supply.
    pub lp_total_supply: i128,
    /// Fee basis points (default SWAP_FEE_BPS).
    pub fee_bps: u32,
    /// Timestamp of pool creation.
    pub created_at: u64,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct LiquidityProvider {
    pub pool_id: u64,
    pub provider: Address,
    pub lp_balance: i128,
}

// ── AMM Helpers ───────────────────────────────────────────────────────────────

fn next_pool_id(env: &Env) -> u64 {
    let n: u64 = env
        .storage()
        .instance()
        .get(&AmmKey::PoolCounter)
        .unwrap_or(0);
    env.storage().instance().set(&AmmKey::PoolCounter, &(n + 1));
    n + 1
}

/// Constant product formula: k = reserve_a * reserve_b
fn constant_product(reserve_a: i128, reserve_b: i128) -> i128 {
    reserve_a * reserve_b
}

/// Calculate output amount for a given input using constant product AMM.
/// amount_in * reserve_out / (reserve_in + amount_in)
fn get_output_amount(amount_in: i128, reserve_in: i128, reserve_out: i128, fee_bps: u32) -> i128 {
    let fee = amount_in * (fee_bps as i128) / 10000;
    let amount_in_after_fee = amount_in - fee;
    amount_in_after_fee * reserve_out / (reserve_in + amount_in_after_fee)
}

/// Calculate the LP token share for a liquidity deposit.
fn calculate_lp_mint(
    amount_a: i128,
    amount_b: i128,
    reserve_a: i128,
    reserve_b: i128,
    total_supply: i128,
) -> i128 {
    if total_supply == 0 {
        // First deposit: geometric mean
        let sqrt = |x: i128| -> i128 {
            let mut r = x;
            let mut s = x / 2 + 1;
            while s < r {
                r = s;
                s = (s + x / s) / 2;
            }
            r
        };
        sqrt(amount_a * amount_b)
    } else {
        let share_a = amount_a * total_supply / reserve_a;
        let share_b = amount_b * total_supply / reserve_b;
        if share_a < share_b {
            share_a
        } else {
            share_b
        }
    }
}

// ── AMM Public API ────────────────────────────────────────────────────────────

/// Create a new liquidity pool for a resource pair.
/// Returns the pool_id.
pub fn create_pool(
    env: &Env,
    creator: &Address,
    resource_a: Symbol,
    resource_b: Symbol,
) -> Result<u64, AmmError> {
    creator.require_auth();

    if resource_a == resource_b {
        return Err(AmmError::InvalidAmount);
    }

    // Check if pool already exists for this pair (both orderings)
    let pools: Vec<u64> = env
        .storage()
        .instance()
        .get(&AmmKey::PoolList)
        .unwrap_or_else(|| Vec::new(env));

    for i in 0..pools.len() {
        if let Some(pid) = pools.get(i) {
            if let Some(pool) = env
                .storage()
                .persistent()
                .get::<_, LiquidityPool>(&AmmKey::Pool(pid))
            {
                if (pool.resource_a == resource_a && pool.resource_b == resource_b)
                    || (pool.resource_a == resource_b && pool.resource_b == resource_a)
                {
                    return Err(AmmError::PoolAlreadyExists);
                }
            }
        }
    }

    let pool_id = next_pool_id(env);
    let pool = LiquidityPool {
        pool_id,
        resource_a: resource_a.clone(),
        resource_b: resource_b.clone(),
        reserve_a: 0,
        reserve_b: 0,
        lp_total_supply: 0,
        fee_bps: SWAP_FEE_BPS,
        created_at: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&AmmKey::Pool(pool_id), &pool);

    let mut pool_list = pools;
    pool_list.push_back(pool_id);
    env.storage().instance().set(&AmmKey::PoolList, &pool_list);

    env.events().publish(
        (symbol_short!("amm"), symbol_short!("pool_c")),
        (pool_id, resource_a, resource_b),
    );

    Ok(pool_id)
}

/// Add liquidity to a pool. Provider receives LP tokens proportional to their share.
/// Returns (lp_tokens_minted, pool after state).
pub fn add_liquidity(
    env: &Env,
    provider: &Address,
    pool_id: u64,
    amount_a: i128,
    amount_b: i128,
) -> Result<(i128, LiquidityPool), AmmError> {
    provider.require_auth();

    if amount_a <= 0 || amount_b <= 0 {
        return Err(AmmError::InvalidAmount);
    }

    let key = AmmKey::Pool(pool_id);
    let mut pool: LiquidityPool = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(AmmError::PoolNotFound)?;

    let lp_mint = calculate_lp_mint(
        amount_a,
        amount_b,
        pool.reserve_a,
        pool.reserve_b,
        pool.lp_total_supply,
    );

    if lp_mint <= 0 {
        return Err(AmmError::ZeroLiquidity);
    }

    // Update reserves
    pool.reserve_a = pool
        .reserve_a
        .checked_add(amount_a)
        .ok_or(AmmError::InvalidAmount)?;
    pool.reserve_b = pool
        .reserve_b
        .checked_add(amount_b)
        .ok_or(AmmError::InvalidAmount)?;
    pool.lp_total_supply = pool
        .lp_total_supply
        .checked_add(lp_mint)
        .ok_or(AmmError::InvalidAmount)?;

    env.storage().persistent().set(&key, &pool);

    // Mint LP tokens to provider
    let lp_key = AmmKey::LpBalance(pool_id, provider.clone());
    let current_balance: i128 = env.storage().persistent().get(&lp_key).unwrap_or(0);
    env.storage()
        .persistent()
        .set(&lp_key, &(current_balance + lp_mint));

    // Update total supply key
    env.storage()
        .persistent()
        .set(&AmmKey::LpTotalSupply(pool_id), &pool.lp_total_supply);

    env.events().publish(
        (symbol_short!("amm"), symbol_short!("liq_add")),
        (pool_id, provider.clone(), amount_a, amount_b, lp_mint),
    );

    Ok((lp_mint, pool))
}

/// Remove liquidity by burning LP tokens. Provider receives proportional reserves.
pub fn remove_liquidity(
    env: &Env,
    provider: &Address,
    pool_id: u64,
    lp_amount: i128,
) -> Result<(i128, i128), AmmError> {
    provider.require_auth();

    if lp_amount <= 0 {
        return Err(AmmError::InvalidAmount);
    }

    let key = AmmKey::Pool(pool_id);
    let mut pool: LiquidityPool = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(AmmError::PoolNotFound)?;

    let lp_key = AmmKey::LpBalance(pool_id, provider.clone());
    let current_balance: i128 = env.storage().persistent().get(&lp_key).unwrap_or(0);

    if current_balance < lp_amount {
        return Err(AmmError::InsufficientLpTokens);
    }

    if pool.lp_total_supply <= 0 {
        return Err(AmmError::ZeroLiquidity);
    }

    // Calculate share of reserves
    let share_a = lp_amount * pool.reserve_a / pool.lp_total_supply;
    let share_b = lp_amount * pool.reserve_b / pool.lp_total_supply;

    // Update reserves
    pool.reserve_a = pool
        .reserve_a
        .checked_sub(share_a)
        .ok_or(AmmError::InsufficientLiquidity)?;
    pool.reserve_b = pool
        .reserve_b
        .checked_sub(share_b)
        .ok_or(AmmError::InsufficientLiquidity)?;
    pool.lp_total_supply = pool
        .lp_total_supply
        .checked_sub(lp_amount)
        .ok_or(AmmError::InvalidAmount)?;

    env.storage().persistent().set(&key, &pool);

    // Burn LP tokens
    env.storage()
        .persistent()
        .set(&lp_key, &(current_balance - lp_amount));
    env.storage()
        .persistent()
        .set(&AmmKey::LpTotalSupply(pool_id), &pool.lp_total_supply);

    env.events().publish(
        (symbol_short!("amm"), symbol_short!("liq_rem")),
        (pool_id, provider.clone(), lp_amount, share_a, share_b),
    );

    Ok((share_a, share_b))
}

/// Swap an exact input amount for an output. Supports multi-hop routing via `route`.
///
/// `route` is a vec of pool_ids that form a chain: resource_in -> pool[0] -> ... -> pool[n] -> resource_out.
/// For a single-pool swap, route contains exactly one pool_id.
/// # Security
/// * Holds a reentrancy guard across the full multi-hop reserve update
///   sequence so a nested call cannot observe or trade against a
///   partially-updated pool mid-route (Issue #238).
pub fn swap_exact_input(
    env: &Env,
    trader: &Address,
    resource_in: Symbol,
    amount_in: i128,
    min_amount_out: i128,
    route: Vec<u64>,
) -> Result<i128, AmmError> {
    trader.require_auth();

    if amount_in <= 0 {
        return Err(AmmError::InvalidAmount);
    }
    if route.is_empty() || route.len() > AMM_MAX_ROUTE_HOPS {
        return Err(AmmError::InvalidRoute);
    }

    with_guard(env, || {
        let mut current_amount = amount_in;
        let resource_in_clone = resource_in.clone();
        let mut current_resource = resource_in;

        for i in 0..route.len() {
            let pool_id = route.get(i).ok_or(AmmError::InvalidRoute)?;
            let key = AmmKey::Pool(pool_id);
            let pool: LiquidityPool = env
                .storage()
                .persistent()
                .get(&key)
                .ok_or(AmmError::PoolNotFound)?;

            let (reserve_in, reserve_out) = if pool.resource_a == current_resource {
                (pool.reserve_a, pool.reserve_b)
            } else if pool.resource_b == current_resource {
                (pool.reserve_b, pool.reserve_a)
            } else {
                return Err(AmmError::InvalidRoute);
            };

            if reserve_in <= 0 || reserve_out <= 0 {
                return Err(AmmError::InsufficientLiquidity);
            }

            let output = get_output_amount(current_amount, reserve_in, reserve_out, pool.fee_bps);
            if output <= 0 {
                return Err(AmmError::InsufficientLiquidity);
            }

            // Update pool reserves
            let mut updated_pool = pool;
            if updated_pool.resource_a == current_resource {
                updated_pool.reserve_a = updated_pool
                    .reserve_a
                    .checked_add(current_amount)
                    .ok_or(AmmError::InvalidAmount)?;
                updated_pool.reserve_b = updated_pool
                    .reserve_b
                    .checked_sub(output)
                    .ok_or(AmmError::InsufficientLiquidity)?;
                current_resource = updated_pool.resource_b.clone();
            } else {
                updated_pool.reserve_b = updated_pool
                    .reserve_b
                    .checked_add(current_amount)
                    .ok_or(AmmError::InvalidAmount)?;
                updated_pool.reserve_a = updated_pool
                    .reserve_a
                    .checked_sub(output)
                    .ok_or(AmmError::InsufficientLiquidity)?;
                current_resource = updated_pool.resource_a.clone();
            }

            env.storage()
                .persistent()
                .set(&AmmKey::Pool(pool_id), &updated_pool);

            current_amount = output;
        }

        if current_amount < min_amount_out {
            return Err(AmmError::SlippageExceeded);
        }

        env.events().publish(
            (symbol_short!("amm"), symbol_short!("swap")),
            (
                trader.clone(),
                resource_in_clone,
                amount_in,
                current_resource.clone(),
                current_amount,
            ),
        );

        Ok(current_amount)
    })
}

/// Get pool details.
pub fn get_pool(env: &Env, pool_id: u64) -> Option<LiquidityPool> {
    env.storage().persistent().get(&AmmKey::Pool(pool_id))
}

/// Get LP balance for a provider in a pool.
pub fn get_lp_balance(env: &Env, pool_id: u64, provider: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&AmmKey::LpBalance(pool_id, provider.clone()))
        .unwrap_or(0)
}

/// Get all pool IDs.
pub fn get_all_pools(env: &Env) -> Vec<u64> {
    env.storage()
        .instance()
        .get(&AmmKey::PoolList)
        .unwrap_or_else(|| Vec::new(env))
}

/// Quote the output amount for a given input without executing the swap.
pub fn quote_swap(
    env: &Env,
    pool_id: u64,
    resource_in: Symbol,
    amount_in: i128,
) -> Result<i128, AmmError> {
    if amount_in <= 0 {
        return Err(AmmError::InvalidAmount);
    }

    let pool: LiquidityPool = env
        .storage()
        .persistent()
        .get(&AmmKey::Pool(pool_id))
        .ok_or(AmmError::PoolNotFound)?;

    let (reserve_in, reserve_out) = if pool.resource_a == resource_in {
        (pool.reserve_a, pool.reserve_b)
    } else if pool.resource_b == resource_in {
        (pool.reserve_b, pool.reserve_a)
    } else {
        return Err(AmmError::InvalidRoute);
    };

    if reserve_in <= 0 || reserve_out <= 0 {
        return Err(AmmError::InsufficientLiquidity);
    }

    Ok(get_output_amount(
        amount_in,
        reserve_in,
        reserve_out,
        pool.fee_bps,
    ))
}

// ── Tests (Issue #238: reentrancy safety) ───────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, testutils::Address as _};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    fn make_env() -> (Env, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(Stub, ());
        (env, contract_id)
    }

    fn seed_pool(env: &Env, provider: &Address, resource_a: Symbol, resource_b: Symbol) -> u64 {
        let pool_id = create_pool(env, provider, resource_a, resource_b).unwrap();
        add_liquidity(env, provider, pool_id, 10_000, 10_000).unwrap();
        pool_id
    }

    #[test]
    fn test_swap_exact_input_rejected_while_guard_held() {
        // Simulates a reentrant callback attempting to re-enter
        // swap_exact_input while a prior invocation's guard is held.
        let (env, contract_id) = make_env();
        let provider = Address::generate(&env);
        let trader = Address::generate(&env);
        let resource_a = Symbol::new(&env, "stdust");
        let resource_b = Symbol::new(&env, "drmatt");

        env.as_contract(&contract_id, || {
            let pool_id = seed_pool(&env, &provider, resource_a.clone(), resource_b.clone());
            let mut route = Vec::new(&env);
            route.push_back(pool_id);

            crate::reentrancy_guard::acquire(&env).expect("lock should be free");
            let result = swap_exact_input(&env, &trader, resource_a.clone(), 100, 0, route.clone());
            assert_eq!(result, Err(AmmError::Reentrancy));
            crate::reentrancy_guard::release(&env);

            // Once released, the swap succeeds normally.
            let out = swap_exact_input(&env, &trader, resource_a, 100, 0, route)
                .expect("swap should succeed once unlocked");
            assert!(out > 0);
        });
    }

    #[test]
    fn test_swap_exact_input_respects_slippage() {
        let (env, contract_id) = make_env();
        let provider = Address::generate(&env);
        let trader = Address::generate(&env);
        let resource_a = Symbol::new(&env, "stdust");
        let resource_b = Symbol::new(&env, "drmatt");

        env.as_contract(&contract_id, || {
            let pool_id = seed_pool(&env, &provider, resource_a.clone(), resource_b.clone());
            let mut route = Vec::new(&env);
            route.push_back(pool_id);

            let result = swap_exact_input(&env, &trader, resource_a, 100, i128::MAX, route);
            assert_eq!(result, Err(AmmError::SlippageExceeded));
        });
    }
}
