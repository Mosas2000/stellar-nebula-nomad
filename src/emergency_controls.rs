use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

/// Time delay required before an unpause can be executed (1 hour in seconds).
pub const UNPAUSE_DELAY: u64 = 3_600;

// ─── Storage Keys ─────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum EmergencyKey {
    /// Global pause flag.
    Paused,
    /// Authorized admin addresses (multi-sig set at deployment).
    Admins,
    /// Ledger timestamp after which unpause is permitted.
    UnpauseAt,
}

// ─── Errors ───────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum EmergencyError {
    /// Contract is currently paused — operation blocked.
    ContractPaused = 1,
    /// Caller is not an authorized admin.
    NotAdmin = 2,
    /// Contract is not paused; cannot unpause.
    NotPaused = 3,
    /// Time-delayed unpause window has not elapsed.
    UnpauseDelayNotMet = 4,
    /// Admin set has already been initialized.
    AlreadyInitialized = 5,
    /// Admin list must contain at least one address.
    EmptyAdminSet = 6,
}

// ─── Internal Helpers ────────────────────────────────────────────────────

fn is_admin(env: &Env, caller: &Address) -> bool {
    let admins: Vec<Address> = env
        .storage()
        .instance()
        .get(&EmergencyKey::Admins)
        .unwrap_or_else(|| Vec::new(env));

    for i in 0..admins.len() {
        if admins.get(i).unwrap() == *caller {
            return true;
        }
    }
    false
}

fn require_admin(env: &Env, caller: &Address) -> Result<(), EmergencyError> {
    if !is_admin(env, caller) {
        return Err(EmergencyError::NotAdmin);
    }
    Ok(())
}

// ─── Public API ──────────────────────────────────────────────────────────

/// Initialize the multi-sig admin set at deployment.
///
/// Must be called exactly once. `admins` must contain at least one address.
/// Each admin must authorize this call.
pub fn initialize_admins(
    env: &Env,
    admins: Vec<Address>,
) -> Result<(), EmergencyError> {
    if env.storage().instance().has(&EmergencyKey::Admins) {
        return Err(EmergencyError::AlreadyInitialized);
    }
    if admins.len() == 0 {
        return Err(EmergencyError::EmptyAdminSet);
    }

    for i in 0..admins.len() {
        admins.get(i).unwrap().require_auth();
    }

    env.storage().instance().set(&EmergencyKey::Admins, &admins);
    env.storage().instance().set(&EmergencyKey::Paused, &false);

    Ok(())
}

/// Check that the contract is not paused. Returns `ContractPaused` if paused.
///
/// Call this at the top of every mutating function to apply the pause guard.
pub fn require_not_paused(env: &Env) -> Result<(), EmergencyError> {
    let paused: bool = env
        .storage()
        .instance()
        .get(&EmergencyKey::Paused)
        .unwrap_or(false);
    if paused {
        return Err(EmergencyError::ContractPaused);
    }
    Ok(())
}

/// Instantly freeze all mutating contract functions. Admin-only.
///
/// Emits a `ContractPaused` event with the caller and timestamp.
pub fn pause_contract(env: &Env, admin: &Address) -> Result<(), EmergencyError> {
    admin.require_auth();
    require_admin(env, admin)?;

    env.storage().instance().set(&EmergencyKey::Paused, &true);

    env.events().publish(
        (symbol_short!("ctrl"), symbol_short!("paused")),
        (admin.clone(), env.ledger().timestamp()),
    );

    Ok(())
}

/// Schedule an unpause by recording the earliest allowed unpause timestamp.
///
/// The actual unpause is gated by `UNPAUSE_DELAY` (1 hour). Admin-only.
pub fn schedule_unpause(env: &Env, admin: &Address) -> Result<u64, EmergencyError> {
    admin.require_auth();
    require_admin(env, admin)?;

    let paused: bool = env
        .storage()
        .instance()
        .get(&EmergencyKey::Paused)
        .unwrap_or(false);
    if !paused {
        return Err(EmergencyError::NotPaused);
    }

    let unpause_at = env.ledger().timestamp() + UNPAUSE_DELAY;
    env.storage()
        .instance()
        .set(&EmergencyKey::UnpauseAt, &unpause_at);

    Ok(unpause_at)
}

/// Execute the scheduled unpause once the delay has elapsed. Admin-only.
///
/// Must call `schedule_unpause` first. Emits a `ContractUnpaused` event.
pub fn execute_unpause(env: &Env, admin: &Address) -> Result<(), EmergencyError> {
    admin.require_auth();
    require_admin(env, admin)?;

    let unpause_at: u64 = env
        .storage()
        .instance()
        .get(&EmergencyKey::UnpauseAt)
        .unwrap_or(u64::MAX);

    if env.ledger().timestamp() < unpause_at {
        return Err(EmergencyError::UnpauseDelayNotMet);
    }

    env.storage().instance().set(&EmergencyKey::Paused, &false);
    env.storage().instance().remove(&EmergencyKey::UnpauseAt);

    env.events().publish(
        (symbol_short!("ctrl"), symbol_short!("unpaused")),
        (admin.clone(), env.ledger().timestamp()),
    );

    Ok(())
}

/// Admin-only recovery of stuck funds/resources from the contract.
///
/// `resource` is the asset symbol identifying the token to recover.
/// Emits an `EmergencyWithdraw` event for on-chain auditability.
///
/// Note: actual token transfer is handled by the calling contract
/// via the Soroban token interface; this function records intent
/// and emits the event for indexer consumption.
pub fn emergency_withdraw(
    env: &Env,
    admin: &Address,
    resource: Symbol,
) -> Result<(), EmergencyError> {
    admin.require_auth();
    require_admin(env, admin)?;

    env.events().publish(
        (symbol_short!("ctrl"), symbol_short!("emrg_wd")),
        (admin.clone(), resource, env.ledger().timestamp()),
    );

    Ok(())
}

/// Returns `true` if the contract is currently paused.
pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&EmergencyKey::Paused)
        .unwrap_or(false)
}

/// Returns the current admin list.
pub fn get_admins(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&EmergencyKey::Admins)
        .unwrap_or_else(|| Vec::new(env))
}
