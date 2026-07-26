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
}
