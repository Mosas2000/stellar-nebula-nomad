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

fn next_vault_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&VaultKey::VaultCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage().instance().set(&VaultKey::VaultCounter, &next);
    next
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
    let lock_until = env.ledger().timestamp() + min_lock;
    let vault_id = next_vault_id(env);

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

    // Calculate bonus yield
    let bonus = vault.amount * vault.bonus_multiplier / BPS_DENOM;
    let total_payout = vault.amount + bonus;

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

/// Set the minimum lock duration (admin function).
#[allow(dead_code)]
pub fn set_min_lock_duration(env: &Env, duration: u64) {
    env.storage()
        .instance()
        .set(&VaultKey::MinLockDuration, &duration);
}
