use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env};

/// Default minimum lock duration: 7 days in seconds.
pub const DEFAULT_MIN_LOCK_DURATION: u64 = 604_800;

/// Bonus multiplier: 10% bonus on locked amount (in basis points).
const BONUS_BPS: u64 = 1_000;
const BPS_DENOM: u64 = 10_000;

#[derive(Clone)]
#[contracttype]
pub enum VaultKey {
    /// Vault data keyed by vault_id.
    Vault(u64),
    /// Auto-incrementing vault counter.
    VaultCounter,
    /// Minimum lock duration in seconds (configurable).
    MinLockDuration,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VaultError {
    /// Vault not found.
    VaultNotFound = 1,
    /// Caller is not the vault owner.
    NotOwner = 2,
    /// Vault is still within its lock period.
    StillLocked = 3,
    /// Vault has already been claimed.
    AlreadyClaimed = 4,
    /// Deposit amount must be positive.
    InvalidAmount = 5,
    /// A checked arithmetic operation overflowed (Issue #239).
    ArithmeticOverflow = 6,
}

/// A time-locked treasure vault.
#[derive(Clone)]
#[contracttype]
pub struct TreasureVault {
    pub vault_id: u64,
    pub owner: Address,
    pub ship_id: u64,
    pub amount: u64,
    pub lock_until: u64,
    pub bonus_multiplier: u64,
    pub claimed: bool,
}

fn next_vault_id(env: &Env) -> Result<u64, VaultError> {
    let current: u64 = env
        .storage()
        .instance()
        .get(&VaultKey::VaultCounter)
        .unwrap_or(0);
    let next = current
        .checked_add(1)
        .ok_or(VaultError::ArithmeticOverflow)?;
    env.storage().instance().set(&VaultKey::VaultCounter, &next);
    Ok(next)
}

/// Bonus yield for a claimed vault: `amount * bonus_multiplier / BPS_DENOM`,
/// computed with checked arithmetic so an extreme `amount` reports overflow
/// instead of silently wrapping (Issue #239).
fn calculate_bonus_payout(amount: u64, bonus_multiplier: u64) -> Option<u64> {
    let bonus = amount
        .checked_mul(bonus_multiplier)?
        .checked_div(BPS_DENOM)?;
    amount.checked_add(bonus)
}

fn get_min_lock_duration(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&VaultKey::MinLockDuration)
        .unwrap_or(DEFAULT_MIN_LOCK_DURATION)
}

/// Deposit resources into a time-locked treasure vault.
///
/// The vault locks the specified `amount` until `lock_until`, which is
/// calculated as the current timestamp plus the minimum lock duration.
/// A bonus multiplier is applied at claim time.
pub fn deposit_treasure(
    env: &Env,
    owner: &Address,
    ship_id: u64,
    amount: u64,
) -> Result<TreasureVault, VaultError> {
    owner.require_auth();

    if amount == 0 {
        return Err(VaultError::InvalidAmount);
    }

    let min_lock = get_min_lock_duration(env);
    let lock_until = env
        .ledger()
        .timestamp()
        .checked_add(min_lock)
        .ok_or(VaultError::ArithmeticOverflow)?;
    let vault_id = next_vault_id(env)?;

    let vault = TreasureVault {
        vault_id,
        owner: owner.clone(),
        ship_id,
        amount,
        lock_until,
        bonus_multiplier: BONUS_BPS,
        claimed: false,
    };

    env.storage()
        .instance()
        .set(&VaultKey::Vault(vault_id), &vault);

    // Emit VaultDeposited event
    env.events().publish(
        (symbol_short!("vault"), symbol_short!("deposit")),
        (vault_id, owner.clone(), ship_id, amount, lock_until),
    );

    Ok(vault)
}

/// Claim a treasure vault after its lock period has expired.
///
/// Returns the original amount plus bonus yield.
/// The bonus is calculated as: `amount * bonus_multiplier / 10_000`.
pub fn claim_treasure(env: &Env, owner: &Address, vault_id: u64) -> Result<u64, VaultError> {
    owner.require_auth();

    let mut vault: TreasureVault = env
        .storage()
        .instance()
        .get(&VaultKey::Vault(vault_id))
        .ok_or(VaultError::VaultNotFound)?;

    if vault.owner != *owner {
        return Err(VaultError::NotOwner);
    }

    if vault.claimed {
        return Err(VaultError::AlreadyClaimed);
    }

    let now = env.ledger().timestamp();
    if now < vault.lock_until {
        return Err(VaultError::StillLocked);
    }

    // Calculate bonus yield (checked: Issue #239)
    let total_payout = calculate_bonus_payout(vault.amount, vault.bonus_multiplier)
        .ok_or(VaultError::ArithmeticOverflow)?;

    vault.claimed = true;
    env.storage()
        .instance()
        .set(&VaultKey::Vault(vault_id), &vault);

    // Emit VaultClaimed event
    env.events().publish(
        (symbol_short!("vault"), symbol_short!("claimed")),
        (vault_id, owner.clone(), total_payout),
    );

    Ok(total_payout)
}

/// Read a vault by ID.
pub fn get_vault(env: &Env, vault_id: u64) -> Option<TreasureVault> {
    env.storage().instance().get(&VaultKey::Vault(vault_id))
}

// ── Tests (Issue #239: arithmetic safety) ───────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
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

    #[test]
    fn test_calculate_bonus_payout_matches_default_bonus() {
        // BONUS_BPS = 1_000 (10%): 100 + 10% = 110.
        assert_eq!(calculate_bonus_payout(100, BONUS_BPS), Some(110));
    }

    #[test]
    fn test_calculate_bonus_payout_overflow_reported_not_wrapped() {
        assert_eq!(calculate_bonus_payout(u64::MAX, BONUS_BPS), None);
    }

    proptest! {
        /// The payout is always >= the original amount for any non-overflowing
        /// input, and the helper never panics across the full u64 domain.
        #[test]
        fn bonus_payout_never_below_principal(amount in 0u64..=(u64::MAX / 10_000)) {
            if let Some(payout) = calculate_bonus_payout(amount, BONUS_BPS) {
                prop_assert!(payout >= amount);
            }
        }

        #[test]
        fn bonus_payout_never_panics(amount in any::<u64>(), multiplier in any::<u64>()) {
            let _ = calculate_bonus_payout(amount, multiplier);
        }
    }

    #[test]
    fn test_deposit_then_claim_before_unlock_is_still_locked() {
        let (env, contract_id) = make_env();
        let owner = Address::generate(&env);

        env.as_contract(&contract_id, || {
            let vault = deposit_treasure(&env, &owner, 1, 500).unwrap();
            let result = claim_treasure(&env, &owner, vault.vault_id);
            assert_eq!(result, Err(VaultError::StillLocked));
        });
    }

    #[test]
    fn test_vault_ids_increment_safely() {
        let (env, contract_id) = make_env();
        let owner = Address::generate(&env);

        env.as_contract(&contract_id, || {
            let v1 = deposit_treasure(&env, &owner, 1, 100).unwrap();
            let v2 = deposit_treasure(&env, &owner, 1, 100).unwrap();
            assert_eq!(v2.vault_id, v1.vault_id + 1);
        });
    }
}
