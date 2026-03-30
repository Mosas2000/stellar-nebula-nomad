use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Vec};

/// Stability percentage below which travel is refused.
pub const MIN_STABLE_PCT: u32 = 10;

/// Maximum portals allowed per single register_portal_batch call.
pub const MAX_PORTALS_PER_TX: u32 = 8;

/// Portal stability decays by this many percentage points per day.
pub const DECAY_RATE_PER_DAY: u32 = 2;

/// Seconds in one day.
const SECONDS_PER_DAY: u64 = 86_400;

/// Base travel cost in resource units.
pub const BASE_TRAVEL_COST: i128 = 100;

// ─── Storage Keys ──────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum PortalKey {
    /// Auto-incrementing portal ID counter.
    Counter,
    /// Portal data keyed by portal ID.
    Portal(u64),
    /// Authorized registrar address.
    Admin,
}

// ─── Errors ─────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PortalError {
    /// Portal stability is below threshold — travel refused.
    PortalUnstable = 1,
    /// Caller is not the portal owner or admin.
    NotAuthorized = 2,
    /// Portal does not exist.
    PortalNotFound = 3,
    /// Batch size exceeded MAX_PORTALS_PER_TX.
    BatchTooLarge = 4,
    /// Source and target nebula IDs must differ.
    SameNebula = 5,
}

// ─── Data Types ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct Portal {
    pub id: u64,
    pub owner: Address,
    pub source_nebula: u64,
    pub target_nebula: u64,
    /// Stability at creation time (always 100).
    pub initial_stability: u32,
    pub created_at: u64,
    /// Timestamp of the last stability refresh.
    pub last_refreshed: u64,
    pub level: u32,
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Compute current stability accounting for decay since the portal was
/// last refreshed.
fn current_stability(portal: &Portal, now: u64) -> u32 {
    let elapsed_secs = now.saturating_sub(portal.last_refreshed);
    let elapsed_days = (elapsed_secs / SECONDS_PER_DAY) as u32;
    let decay = DECAY_RATE_PER_DAY.saturating_mul(elapsed_days);
    portal.initial_stability.saturating_sub(decay)
}

/// Travel cost scales inversely with stability: unstable portals cost more.
fn travel_cost(stability_pct: u32) -> i128 {
    if stability_pct == 0 {
        return i128::MAX;
    }
    // cost = BASE * (100 / stability)
    BASE_TRAVEL_COST * 100 / stability_pct as i128
}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Initialize the portal registry with an admin address.
pub fn initialize_portal_registry(env: &Env, admin: &Address) {
    admin.require_auth();
    env.storage().instance().set(&PortalKey::Admin, admin);
    env.storage().instance().set(&PortalKey::Counter, &0u64);
}

/// Register a new portal between two nebulae (owner-only).
///
/// Returns the newly assigned portal ID.
pub fn register_portal(
    env: &Env,
    owner: &Address,
    source_nebula: u64,
    target_nebula: u64,
) -> Result<u64, PortalError> {
    owner.require_auth();

    if source_nebula == target_nebula {
        return Err(PortalError::SameNebula);
    }

    let portal_id: u64 = env
        .storage()
        .instance()
        .get(&PortalKey::Counter)
        .unwrap_or(0);

    let now = env.ledger().timestamp();
    let portal = Portal {
        id: portal_id,
        owner: owner.clone(),
        source_nebula,
        target_nebula,
        initial_stability: 100,
        created_at: now,
        last_refreshed: now,
        level: 1,
    };

    env.storage()
        .persistent()
        .set(&PortalKey::Portal(portal_id), &portal);
    env.storage()
        .instance()
        .set(&PortalKey::Counter, &(portal_id + 1));

    env.events().publish(
        (symbol_short!("portal"), symbol_short!("regist")),
        (owner.clone(), portal_id, source_nebula, target_nebula, now),
    );

    Ok(portal_id)
}

/// Register up to [`MAX_PORTALS_PER_TX`] portals in one transaction.
///
/// `connections` is a list of `(source_nebula, target_nebula)` pairs.
pub fn register_portal_batch(
    env: &Env,
    owner: &Address,
    connections: &Vec<(u64, u64)>,
) -> Result<Vec<u64>, PortalError> {
    owner.require_auth();

    if connections.len() > MAX_PORTALS_PER_TX {
        return Err(PortalError::BatchTooLarge);
    }

    let mut ids: Vec<u64> = Vec::new(env);
    for pair in connections.iter() {
        let id = register_portal(env, owner, pair.0, pair.1)?;
        ids.push_back(id);
    }

    Ok(ids)
}

/// Return the current stability percentage and travel cost for a portal.
pub fn query_portal_status(
    env: &Env,
    portal_id: u64,
) -> Result<(u32, i128), PortalError> {
    let portal: Portal = env
        .storage()
        .persistent()
        .get(&PortalKey::Portal(portal_id))
        .ok_or(PortalError::PortalNotFound)?;

    let now = env.ledger().timestamp();
    let stability = current_stability(&portal, now);
    let cost = travel_cost(stability);

    Ok((stability, cost))
}

/// Refresh (re-stabilize) a portal to 100 % stability.
///
/// Only the portal owner may call this.
pub fn refresh_portal(env: &Env, owner: &Address, portal_id: u64) -> Result<(), PortalError> {
    owner.require_auth();

    let mut portal: Portal = env
        .storage()
        .persistent()
        .get(&PortalKey::Portal(portal_id))
        .ok_or(PortalError::PortalNotFound)?;

    if portal.owner != *owner {
        return Err(PortalError::NotAuthorized);
    }

    portal.initial_stability = 100;
    portal.last_refreshed = env.ledger().timestamp();
    env.storage()
        .persistent()
        .set(&PortalKey::Portal(portal_id), &portal);

    Ok(())
}

/// Attempt travel through a portal. Errors if stability < [`MIN_STABLE_PCT`].
///
/// Returns travel cost that should be charged by the calling contract.
pub fn travel_through_portal(env: &Env, portal_id: u64) -> Result<i128, PortalError> {
    let portal: Portal = env
        .storage()
        .persistent()
        .get(&PortalKey::Portal(portal_id))
        .ok_or(PortalError::PortalNotFound)?;

    let now = env.ledger().timestamp();
    let stability = current_stability(&portal, now);

    if stability < MIN_STABLE_PCT {
        return Err(PortalError::PortalUnstable);
    }

    Ok(travel_cost(stability))
}

/// Retrieve raw portal data.
pub fn get_portal(env: &Env, portal_id: u64) -> Result<Portal, PortalError> {
    env.storage()
        .persistent()
        .get(&PortalKey::Portal(portal_id))
        .ok_or(PortalError::PortalNotFound)
}
