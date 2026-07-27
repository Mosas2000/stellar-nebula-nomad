// ============================================================
// resource_minter.rs — Fix #175: Rate-limited resource minting
// Branch: security/rate-limiting
// ============================================================
//
// Changes vs baseline:
//   • Import and call check_rate_limit(Operation::ResourceMinting)
//     at the top of mint_resource() before any state mutation.
//   • RateLimitHit events are emitted inside check_rate_limit.

#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, log, symbol_short, Address, Env, String,
    Symbol,
};

use crate::nebula_gen::{NebulaError as NebulaGenError, NebulaGen};
use crate::rate_limiter::{check_rate_limit, Operation, RateLimitError};

pub type AssetId = ResourceType;

#[contracttype]
#[derive(Clone)]
pub enum ResourceKey {
    ResourceBalance(Address, Symbol),
}

pub fn resource_type_to_symbol(rt: &ResourceType) -> Symbol {
    match rt {
        ResourceType::StellarDust => symbol_short!("stdust"),
        ResourceType::DarkMatter => symbol_short!("drmatt"),
        ResourceType::ExoticMatter => symbol_short!("exomat"),
    }
}

// ── Resource types ────────────────────────────────────────────
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourceType {
    StellarDust,
    DarkMatter,
    ExoticMatter,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceRecord {
    pub owner: Address,
    pub resource_type: ResourceType,
    pub amount: u64,
    pub minted_at: u64,
}

#[contracttype]
pub enum MinterKey {
    Balance(Address, ResourceType),
    TotalSupply(ResourceType),
    /// Cumulative amount ever minted, never decremented. Together with the
    /// burn counters in [`crate::token_burning`] this gives the deflation rate
    /// (Issue #281): `burned / ever_minted`.
    TotalMinted(ResourceType),
}

// ── Error ─────────────────────────────────────────────────────
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MinterError {
    /// Amount must be > 0.
    InvalidAmount = 200,
    /// Caller exceeded the minting rate limit (DoS prevention).
    RateLimitExceeded = 201,
    /// No nebula layout found for this ship (must scan first).
    NoLayoutForShip = 202,
    /// The specified anomaly index does not contain a resource.
    NoResourceAtAnomaly = 203,
    /// A checked arithmetic operation overflowed (Issue #239).
    ArithmeticOverflow = 204,
    /// The account holds less than the requested debit amount (Issue #281).
    InsufficientBalance = 205,
}

impl From<RateLimitError> for MinterError {
    fn from(_: RateLimitError) -> Self {
        MinterError::RateLimitExceeded
    }
}

// ── Contract ─────────────────────────────────────────────────
#[contract]
pub struct ResourceMinterContract;

#[contractimpl]
impl ResourceMinterContract {
    /// Mint `amount` units of `resource_type` for `caller`.
    ///
    /// Rate-limited to prevent spam (Issue #175).
    pub fn mint_resource(
        env: &Env,
        caller: Address,
        ship_id: u64,
        anomaly_index: u32,
        resource_type: ResourceType,
        amount: u64,
    ) -> Result<ResourceRecord, MinterError> {
        // ── Auth ───────────────────────────────────────────────
        caller.require_auth();

        // ── Rate limit check (Issue #175) ──────────────────────
        check_rate_limit(env, &caller, Operation::ResourceMinting).map_err(MinterError::from)?;

        // ── Basic validation ───────────────────────────────────
        if amount == 0 {
            return Err(MinterError::InvalidAmount);
        }

        // ── Confirm anomaly exists for this ship ───────────────
        NebulaGen::has_anomaly(env.clone(), ship_id, anomaly_index).map_err(|e| match e {
            NebulaGenError::LayoutNotFound => MinterError::NoLayoutForShip,
            NebulaGenError::AnomalyOutOfBounds => MinterError::NoResourceAtAnomaly,
            _ => MinterError::NoLayoutForShip,
        })?;

        // ── Update balances (checked: Issue #239) ──────────────
        let balance_key = MinterKey::Balance(caller.clone(), resource_type.clone());
        let current: u64 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        let new_balance = current
            .checked_add(amount)
            .ok_or(MinterError::ArithmeticOverflow)?;
        env.storage().persistent().set(&balance_key, &new_balance);

        let supply_key = MinterKey::TotalSupply(resource_type.clone());
        let supply: u64 = env.storage().persistent().get(&supply_key).unwrap_or(0);
        let new_supply = supply
            .checked_add(amount)
            .ok_or(MinterError::ArithmeticOverflow)?;
        env.storage().persistent().set(&supply_key, &new_supply);

        // ── Cumulative mint counter (Issue #281) ───────────────
        // Unlike TotalSupply this is monotonic — burning reduces supply but
        // never the historical mint total, which is the denominator of the
        // deflation rate.
        let minted_key = MinterKey::TotalMinted(resource_type.clone());
        let minted: u64 = env.storage().persistent().get(&minted_key).unwrap_or(0);
        let new_minted = minted
            .checked_add(amount)
            .ok_or(MinterError::ArithmeticOverflow)?;
        env.storage().persistent().set(&minted_key, &new_minted);

        let record = ResourceRecord {
            owner: caller.clone(),
            resource_type: resource_type.clone(),
            amount,
            minted_at: env.ledger().timestamp(),
        };

        // ── Emit event ─────────────────────────────────────────
        env.events().publish(
            (symbol_short!("Minter"), symbol_short!("minted")),
            (caller, resource_type, amount),
        );

        Ok(record)
    }

    /// Query the balance of `owner` for `resource_type`.
    pub fn balance(env: &Env, owner: Address, resource_type: ResourceType) -> u64 {
        env.storage()
            .persistent()
            .get(&MinterKey::Balance(owner, resource_type))
            .unwrap_or(0)
    }

    /// Total supply of a given resource type.
    pub fn total_supply(env: &Env, resource_type: ResourceType) -> u64 {
        env.storage()
            .persistent()
            .get(&MinterKey::TotalSupply(resource_type))
            .unwrap_or(0)
    }

    /// Cumulative amount ever minted for `resource_type`, ignoring burns.
    pub fn total_minted(env: &Env, resource_type: ResourceType) -> u64 {
        self::total_minted(env, &resource_type)
    }
}

// ─────────────────────────────────────────────────────────────
// Supply-reducing primitives (Issue #281)
// ─────────────────────────────────────────────────────────────
//
// `token_burning` is the only intended caller. Keeping these here rather than
// in the burning module means the balance and supply ledgers stay owned by a
// single module, so every credit and debit goes through the same checked
// arithmetic.

/// Read a holder's balance without going through the contract entrypoint.
pub fn balance_of(env: &Env, owner: &Address, resource_type: &ResourceType) -> u64 {
    env.storage()
        .persistent()
        .get(&MinterKey::Balance(owner.clone(), resource_type.clone()))
        .unwrap_or(0)
}

/// Cumulative amount ever minted for `resource_type`.
pub fn total_minted(env: &Env, resource_type: &ResourceType) -> u64 {
    env.storage()
        .persistent()
        .get(&MinterKey::TotalMinted(resource_type.clone()))
        .unwrap_or(0)
}

/// Current circulating supply for `resource_type`.
pub fn circulating_supply(env: &Env, resource_type: &ResourceType) -> u64 {
    env.storage()
        .persistent()
        .get(&MinterKey::TotalSupply(resource_type.clone()))
        .unwrap_or(0)
}

/// Debit `amount` from `owner`'s balance.
///
/// Does **not** require auth — the caller is responsible for having authorized
/// the holder. Returns the new balance, or `InsufficientBalance` if the debit
/// would go negative (checked, never wrapping).
pub fn debit_balance(
    env: &Env,
    owner: &Address,
    resource_type: &ResourceType,
    amount: u64,
) -> Result<u64, MinterError> {
    if amount == 0 {
        return Err(MinterError::InvalidAmount);
    }

    let key = MinterKey::Balance(owner.clone(), resource_type.clone());
    let current: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    let new_balance = current
        .checked_sub(amount)
        .ok_or(MinterError::InsufficientBalance)?;
    env.storage().persistent().set(&key, &new_balance);

    Ok(new_balance)
}

/// Reduce the circulating supply of `resource_type` by `amount`.
///
/// Returns the new supply. Underflow is treated as `InsufficientBalance`: it
/// would mean the supply ledger disagreed with the sum of balances, and
/// silently saturating there would hide the inconsistency.
pub fn reduce_supply(
    env: &Env,
    resource_type: &ResourceType,
    amount: u64,
) -> Result<u64, MinterError> {
    let key = MinterKey::TotalSupply(resource_type.clone());
    let supply: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    let new_supply = supply
        .checked_sub(amount)
        .ok_or(MinterError::InsufficientBalance)?;
    env.storage().persistent().set(&key, &new_supply);

    Ok(new_supply)
}

/// Move `amount` between two holders without touching supply counters.
///
/// A transfer is not a mint or a burn, so neither `TotalSupply` nor
/// `TotalMinted` changes — only the two balances do.
pub fn move_balance(
    env: &Env,
    from: &Address,
    to: &Address,
    resource_type: &ResourceType,
    amount: u64,
) -> Result<u64, MinterError> {
    if amount == 0 {
        return Err(MinterError::InvalidAmount);
    }
    if from == to {
        return Ok(balance_of(env, from, resource_type));
    }

    let from_key = MinterKey::Balance(from.clone(), resource_type.clone());
    let from_balance: u64 = env.storage().persistent().get(&from_key).unwrap_or(0);
    let remaining = from_balance
        .checked_sub(amount)
        .ok_or(MinterError::InsufficientBalance)?;

    let to_key = MinterKey::Balance(to.clone(), resource_type.clone());
    let to_balance: u64 = env.storage().persistent().get(&to_key).unwrap_or(0);
    let credited = to_balance
        .checked_add(amount)
        .ok_or(MinterError::ArithmeticOverflow)?;

    env.storage().persistent().set(&from_key, &remaining);
    env.storage().persistent().set(&to_key, &credited);

    Ok(credited)
}

/// Credit `amount` to `owner` without auth or rate limiting.
///
/// Test- and reward-path helper: used by the burn tests to seed balances and by
/// reward subsystems that grant resources they have already authorized.
pub fn credit_balance(
    env: &Env,
    owner: &Address,
    resource_type: &ResourceType,
    amount: u64,
) -> Result<u64, MinterError> {
    if amount == 0 {
        return Err(MinterError::InvalidAmount);
    }

    let key = MinterKey::Balance(owner.clone(), resource_type.clone());
    let current: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    let new_balance = current
        .checked_add(amount)
        .ok_or(MinterError::ArithmeticOverflow)?;
    env.storage().persistent().set(&key, &new_balance);

    let supply_key = MinterKey::TotalSupply(resource_type.clone());
    let supply: u64 = env.storage().persistent().get(&supply_key).unwrap_or(0);
    env.storage().persistent().set(
        &supply_key,
        &supply
            .checked_add(amount)
            .ok_or(MinterError::ArithmeticOverflow)?,
    );

    let minted_key = MinterKey::TotalMinted(resource_type.clone());
    let minted: u64 = env.storage().persistent().get(&minted_key).unwrap_or(0);
    env.storage().persistent().set(
        &minted_key,
        &minted
            .checked_add(amount)
            .ok_or(MinterError::ArithmeticOverflow)?,
    );

    Ok(new_balance)
}

// ─────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn make_env() -> Env {
        let env = Env::default();
        env.mock_all_auths();
        env
    }

    // ── Arithmetic safety (Issue #239) ──────────────────────────
    //
    // `mint_resource` credits balances via `current.checked_add(amount)`
    // (see above). These property tests pin down that operation's
    // overflow behavior directly, independent of the full mint flow.
    mod arithmetic_safety {
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn checked_credit_never_wraps(current in any::<u64>(), amount in any::<u64>()) {
                match current.checked_add(amount) {
                    Some(sum) => {
                        prop_assert!(sum >= current);
                        prop_assert!(sum >= amount);
                    }
                    None => {
                        // Only reports overflow when the true (unbounded) sum
                        // would actually exceed u64::MAX — never a false positive.
                        prop_assert!(current as u128 + amount as u128 > u64::MAX as u128);
                    }
                }
            }
        }

        #[test]
        fn checked_credit_detects_overflow_at_max_balance() {
            assert_eq!(u64::MAX.checked_add(1), None);
            assert_eq!((u64::MAX - 1).checked_add(1), Some(u64::MAX));
        }
    }

    #[test]
    fn test_mint_zero_amount_rejected() {
        let env = make_env();
        let caller = Address::generate(&env);
        let result =
            ResourceMinterContract::mint_resource(&env, caller, 1, 0, ResourceType::StellarDust, 0);
        assert_eq!(result, Err(MinterError::InvalidAmount));
    }

    #[test]
    fn test_rate_limit_enforced_on_minting() {
        let env = make_env();
        let caller = Address::generate(&env);

        // Use up the default ResourceMinting limit (10 / 60 s)
        // We expect the first 10 to fail with NoLayoutForShip (no layout),
        // but RateLimitExceeded must fire on the 11th.
        for _ in 0..10 {
            let _ = ResourceMinterContract::mint_resource(
                &env,
                caller.clone(),
                1,
                0,
                ResourceType::StellarDust,
                1,
            );
        }
        let result = ResourceMinterContract::mint_resource(
            &env,
            caller.clone(),
            1,
            0,
            ResourceType::StellarDust,
            1,
        );
        assert_eq!(result, Err(MinterError::RateLimitExceeded));
    }

    // ── Supply-reducing primitives (Issue #281) ─────────────────
    //
    // These touch the balance and supply ledgers, so — unlike the pure
    // arithmetic tests above — they must run inside a contract invocation.
    mod supply_primitives {
        use super::*;
        use soroban_sdk::contractimpl;

        #[contract]
        struct Stub;
        #[contractimpl]
        impl Stub {}

        fn in_contract<T>(f: impl FnOnce(&Env) -> T) -> T {
            let env = make_env();
            let contract = env.register(Stub, ());
            env.as_contract(&contract, || f(&env))
        }

        #[test]
        fn credit_updates_balance_supply_and_mint_total() {
            in_contract(|env| {
                let holder = Address::generate(env);
                let rt = ResourceType::DarkMatter;

                assert_eq!(credit_balance(env, &holder, &rt, 400).unwrap(), 400);
                assert_eq!(balance_of(env, &holder, &rt), 400);
                assert_eq!(circulating_supply(env, &rt), 400);
                assert_eq!(total_minted(env, &rt), 400);
            });
        }

        #[test]
        fn debit_reduces_balance_but_not_the_mint_total() {
            in_contract(|env| {
                let holder = Address::generate(env);
                let rt = ResourceType::StellarDust;
                credit_balance(env, &holder, &rt, 100).unwrap();

                assert_eq!(debit_balance(env, &holder, &rt, 40).unwrap(), 60);
                assert_eq!(reduce_supply(env, &rt, 40).unwrap(), 60);
                assert_eq!(
                    total_minted(env, &rt),
                    100,
                    "the historical mint total must be monotonic"
                );
            });
        }

        #[test]
        fn debit_beyond_balance_is_rejected_without_wrapping() {
            in_contract(|env| {
                let holder = Address::generate(env);
                let rt = ResourceType::StellarDust;
                credit_balance(env, &holder, &rt, 10).unwrap();

                assert_eq!(
                    debit_balance(env, &holder, &rt, 11),
                    Err(MinterError::InsufficientBalance)
                );
                assert_eq!(balance_of(env, &holder, &rt), 10, "balance is unchanged");
            });
        }

        #[test]
        fn reduce_supply_beyond_supply_is_rejected() {
            in_contract(|env| {
                assert_eq!(
                    reduce_supply(env, &ResourceType::ExoticMatter, 1),
                    Err(MinterError::InsufficientBalance)
                );
            });
        }

        #[test]
        fn zero_amount_transfers_are_rejected() {
            in_contract(|env| {
                let holder = Address::generate(env);
                let rt = ResourceType::StellarDust;

                assert_eq!(
                    credit_balance(env, &holder, &rt, 0),
                    Err(MinterError::InvalidAmount)
                );
                assert_eq!(
                    debit_balance(env, &holder, &rt, 0),
                    Err(MinterError::InvalidAmount)
                );
            });
        }

        #[test]
        fn credit_detects_balance_overflow() {
            in_contract(|env| {
                let holder = Address::generate(env);
                let rt = ResourceType::StellarDust;
                credit_balance(env, &holder, &rt, u64::MAX).unwrap();

                assert_eq!(
                    credit_balance(env, &holder, &rt, 1),
                    Err(MinterError::ArithmeticOverflow)
                );
            });
        }

        #[test]
        fn balances_are_tracked_per_resource_type() {
            in_contract(|env| {
                let holder = Address::generate(env);
                credit_balance(env, &holder, &ResourceType::StellarDust, 10).unwrap();
                credit_balance(env, &holder, &ResourceType::DarkMatter, 20).unwrap();

                assert_eq!(balance_of(env, &holder, &ResourceType::StellarDust), 10);
                assert_eq!(balance_of(env, &holder, &ResourceType::DarkMatter), 20);
                assert_eq!(balance_of(env, &holder, &ResourceType::ExoticMatter), 0);
            });
        }

        #[test]
        fn move_balance_shifts_holdings_without_changing_supply() {
            in_contract(|env| {
                let from = Address::generate(env);
                let to = Address::generate(env);
                let rt = ResourceType::DarkMatter;
                credit_balance(env, &from, &rt, 500).unwrap();

                assert_eq!(move_balance(env, &from, &to, &rt, 200).unwrap(), 200);
                assert_eq!(balance_of(env, &from, &rt), 300);
                assert_eq!(balance_of(env, &to, &rt), 200);
                assert_eq!(circulating_supply(env, &rt), 500, "a move is not a mint");
                assert_eq!(total_minted(env, &rt), 500);
            });
        }

        #[test]
        fn move_balance_rejects_an_underfunded_sender() {
            in_contract(|env| {
                let from = Address::generate(env);
                let to = Address::generate(env);
                let rt = ResourceType::DarkMatter;
                credit_balance(env, &from, &rt, 10).unwrap();

                assert_eq!(
                    move_balance(env, &from, &to, &rt, 11),
                    Err(MinterError::InsufficientBalance)
                );
                assert_eq!(balance_of(env, &from, &rt), 10);
                assert_eq!(balance_of(env, &to, &rt), 0);
            });
        }

        #[test]
        fn move_balance_to_self_is_a_no_op() {
            in_contract(|env| {
                let holder = Address::generate(env);
                let rt = ResourceType::DarkMatter;
                credit_balance(env, &holder, &rt, 100).unwrap();

                assert_eq!(move_balance(env, &holder, &holder, &rt, 50).unwrap(), 100);
                assert_eq!(balance_of(env, &holder, &rt), 100);
            });
        }
    }
}
