use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, BytesN, Env, Vec, Map, String,
};

/// Default bounty expiry duration: 14 days in seconds.
pub const DEFAULT_BOUNTY_EXPIRY: u64 = 1_209_600;

/// Maximum active bounties.
pub const MAX_ACTIVE_BOUNTIES: u32 = 20;

/// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum BountyKey {
    /// Auto-incrementing bounty ID counter.
    Counter,
    /// Bounty data keyed by bounty ID.
    Bounty(u64),
    /// Admin address authorized to approve high-value bounties.
    Admin,
    /// Global bounty expiry duration (seconds).
    Expiry,
}

/// ─── Errors ─────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum BountyError {
    /// Bounty does not exist.
    BountyNotFound = 1,
    /// Bounty has expired.
    BountyExpired = 2,
    /// Caller is not the bounty poster.
    NotPoster = 3,
    /// Caller is not authorized (admin).
    NotAuthorized = 4,
    /// Maximum active bounties reached.
    TooManyActiveBounties = 5,
    /// Reward amount must be positive.
    InvalidReward = 6,
    /// Proof does not meet requirements.
    InvalidProof = 7,
    /// Bounty already claimed.
    AlreadyClaimed = 8,
}

/// ─── Data Types ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct Bounty {
    pub id: u64,
    pub poster: Address,
    pub description: String,
    pub reward: i128,
    pub expires_at: u64,
    pub claimed: bool,
    pub created_at: u64,
    pub claimer: Option<Address>,
    pub proof_hash: Option<BytesN<32>>,
}

/// ─── Public API ─────────────────────────────────────────────────────────────

/// Initialize bounty board with admin and default expiry.
pub fn initialize_bounty_board(env: &Env, admin: &Address) {
    admin.require_auth();
    env.storage().instance().set(&BountyKey::Admin, admin);
    env.storage()
        .instance()
        .set(&BountyKey::Expiry, &DEFAULT_BOUNTY_EXPIRY);
    env.storage()
        .instance()
        .set(&BountyKey::Counter, &0u64);
}

/// Set bounty expiry duration. Admin-only.
pub fn set_bounty_expiry(env: &Env, admin: &Address, expiry_seconds: u64) -> Result<(), BountyError> {
    admin.require_auth();
    if env.storage().instance().get::<BountyKey, Address>(&BountyKey::Admin) != Some(admin.clone()) {
        return Err(BountyError::NotAuthorized);
    }
    env.storage()
        .instance()
        .set(&BountyKey::Expiry, &expiry_seconds);
    Ok(())
}

/// Post a new bounty. High-value bounties require admin approval (placeholder).
pub fn post_bounty(
    env: &Env,
    poster: &Address,
    description: String,
    reward: i128,
) -> Result<Bounty, BountyError> {
    poster.require_auth();

    if reward <= 0 {
        return Err(BountyError::InvalidReward);
    }

    // Simple active-bounty limit check (could be more sophisticated).
    let counter: u64 = env
        .storage()
        .instance()
        .get(&BountyKey::Counter)
        .unwrap_or(0);
    if counter >= MAX_ACTIVE_BOUNTIES as u64 {
        return Err(BountyError::TooManyActiveBounties);
    }

    let next_id = counter + 1;
    env.storage()
        .instance()
        .set(&BountyKey::Counter, &next_id);

    let expiry_seconds: u64 = env
        .storage()
        .instance()
        .get(&BountyKey::Expiry)
        .unwrap_or(DEFAULT_BOUNTY_EXPIRY);
    let expires_at = env.ledger().timestamp() + expiry_seconds;

    let bounty = Bounty {
        id: next_id,
        poster: poster.clone(),
        description,
        reward,
        expires_at,
        claimed: false,
        created_at: env.ledger().timestamp(),
        claimer: None,
        proof_hash: None,
    };

    env.storage()
        .instance()
        .set(&BountyKey::Bounty(next_id), &bounty);

    env.events().publish(
        (symbol_short!("bounty"), symbol_short!("posted")),
        (
            poster.clone(),
            next_id,
            bounty.reward,
            bounty.expires_at,
            bounty.created_at,
        ),
    );

    Ok(bounty)
}

/// Claim a bounty by submitting proof (128-byte proof hash placeholder).
pub fn claim_bounty(
    env: &Env,
    claimer: &Address,
    bounty_id: u64,
    proof: BytesN<32>,
) -> Result<Bounty, BountyError> {
    claimer.require_auth();

    let mut bounty: Bounty = env
        .storage()
        .instance()
        .get(&BountyKey::Bounty(bounty_id))
        .ok_or(BountyError::BountyNotFound)?;

    if bounty.claimed {
        return Err(BountyError::AlreadyClaimed);
    }

    let now = env.ledger().timestamp();
    if now > bounty.expires_at {
        return Err(BountyError::BountyExpired);
    }

    // Placeholder proof validation: require non-empty proof.
    if proof == BytesN::from_array(&env, &[0; 32]) {
        return Err(BountyError::InvalidProof);
    }

    // In a real implementation, you would verify the proof against the task requirements.
    // Here we accept any non-zero proof.
    bounty.claimed = true;
    bounty.claimer = Some(claimer.clone());
    bounty.proof_hash = Some(proof);

    env.storage()
        .instance()
        .set(&BountyKey::Bounty(bounty_id), &bounty);

    // Emit BountyClaimed event; actual reward distribution would be handled via token interface.
    env.events().publish(
        (symbol_short!("bounty"), symbol_short!("claimed")),
        (
            claimer.clone(),
            bounty_id,
            bounty.reward,
            now,
        ),
    );

    Ok(bounty)
}

/// Get bounty by ID.
pub fn get_bounty(env: &Env, bounty_id: u64) -> Option<Bounty> {
    env.storage()
        .instance()
        .get(&BountyKey::Bounty(bounty_id))
}
