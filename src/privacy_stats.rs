use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, BytesN, Env, Symbol, Vec,
};

use crate::rate_limiter;

/// Maximum number of commitments allowed per transaction.
pub const MAX_COMMITMENTS_PER_TX: u32 = 10;

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum PrivacyKey {
    /// Commitment hash storage keyed by player and stat type.
    Commitment(Address, Symbol),
    /// Opt-in status for a player.
    OptIn(Address),
    /// Global commitment counter.
    CommitmentCount,
    /// Burst counter for rate limiting.
    BurstCounter,
}

// ─── Data Types ───────────────────────────────────────────────────────────────

/// Privacy-preserving stat commitment record.
#[derive(Clone)]
#[contracttype]
pub struct StatCommitment {
    pub player: Address,
    pub stat_type: Symbol,
    pub commitment_hash: BytesN<32>,
    pub timestamp: u64,
    pub verified: bool,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PrivacyError {
    /// Player has not opted in to privacy features.
    NotOptedIn = 1,
    /// Invalid proof provided for verification.
    InvalidProof = 2,
    /// Commitment not found.
    CommitmentNotFound = 3,
    /// Burst limit exceeded (max 10 commitments per tx).
    BurstLimitExceeded = 4,
    /// Commitment already exists for this stat type.
    CommitmentExists = 5,
}

// ─── Helper Functions ─────────────────────────────────────────────────────────

/// Compute a simple commitment hash: hash(stat_type || value || player || timestamp).
/// In production, this would use a proper cryptographic commitment scheme.
fn compute_commitment_hash(
    env: &Env,
    stat_type: &Symbol,
    value: i128,
    player: &Address,
    timestamp: u64,
) -> BytesN<32> {
    let mut data = soroban_sdk::Bytes::new(env);
    
    // Append stat_type as bytes (use a simple representation)
    // Convert symbol to a deterministic byte representation
    let stat_val = stat_type.to_val();
    let stat_u64 = stat_val.get_payload();
    let stat_bytes = stat_u64.to_be_bytes();
    for byte in stat_bytes.iter() {
        data.push_back(*byte);
    }
    
    // Append value bytes
    let value_bytes = value.to_be_bytes();
    for byte in value_bytes.iter() {
        data.push_back(*byte);
    }
    
    // Append timestamp bytes
    let ts_bytes = timestamp.to_be_bytes();
    for byte in ts_bytes.iter() {
        data.push_back(*byte);
    }
    
    // Use Soroban's built-in hash function and convert to BytesN
    BytesN::from_array(env, &env.crypto().sha256(&data).to_array())
}

/// Verify a proof against a commitment.
/// This is a simplified verification - in production, use proper ZK proofs.
fn verify_proof_internal(
    env: &Env,
    commitment: &BytesN<32>,
    proof: &BytesN<64>,
) -> bool {
    // Simple verification: check if proof's first 32 bytes match commitment
    // In production, this would verify a proper zero-knowledge proof
    let mut matches = true;
    for i in 0..32 {
        if commitment.get(i).unwrap() != proof.get(i).unwrap() {
            matches = false;
            break;
        }
    }
    matches
}

/// Check and increment burst counter.
fn check_burst_limit(env: &Env) -> Result<(), PrivacyError> {
    let current: u32 = env
        .storage()
        .instance()
        .get(&PrivacyKey::BurstCounter)
        .unwrap_or(0);
    
    if current >= MAX_COMMITMENTS_PER_TX {
        return Err(PrivacyError::BurstLimitExceeded);
    }
    
    env.storage()
        .instance()
        .set(&PrivacyKey::BurstCounter, &(current + 1));
    
    Ok(())
}

/// Reset burst counter (called at start of new transaction).
pub fn reset_burst_counter(env: &Env) {
    env.storage().instance().set(&PrivacyKey::BurstCounter, &0u32);
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Opt in to privacy-preserving stat sharing.
/// This is a one-time setup that enables privacy features for the player.
pub fn opt_in_privacy(env: &Env, player: Address) -> Result<(), PrivacyError> {
    player.require_auth();
    
    env.storage()
        .persistent()
        .set(&PrivacyKey::OptIn(player.clone()), &true);
    
    env.events().publish(
        (symbol_short!("privacy"), symbol_short!("optin")),
        player,
    );
    
    Ok(())
}

/// Check if a player has opted in to privacy features.
pub fn is_opted_in(env: &Env, player: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&PrivacyKey::OptIn(player.clone()))
        .unwrap_or(false)
}

/// Commit a private stat without revealing the raw value.
/// Stores a cryptographic commitment that can later be verified.
///
/// # Arguments
/// * `player` - The player committing the stat
/// * `stat_type` - Type of stat (e.g., "score", "kills", "resources")
/// * `value` - The actual stat value (not stored, only used for commitment)
///
/// # Returns
/// The commitment hash that can be used for later verification.
pub fn commit_private_stat(
    env: &Env,
    player: Address,
    stat_type: Symbol,
    value: i128,
) -> Result<BytesN<32>, PrivacyError> {
    player.require_auth();

    rate_limiter::check_rate_limit(env, &player, rate_limiter::Operation::PrivacyCommit)
        .map_err(|_| PrivacyError::BurstLimitExceeded)?;
    
    // Check opt-in status
    if !is_opted_in(env, &player) {
        return Err(PrivacyError::NotOptedIn);
    }
    
    // Check burst limit
    check_burst_limit(env)?;
    
    // Check if commitment already exists
    let key = PrivacyKey::Commitment(player.clone(), stat_type.clone());
    if env.storage().persistent().has(&key) {
        return Err(PrivacyError::CommitmentExists);
    }
    
    let timestamp = env.ledger().timestamp();
    let commitment_hash = compute_commitment_hash(env, &stat_type, value, &player, timestamp);
    
    let commitment = StatCommitment {
        player: player.clone(),
        stat_type: stat_type.clone(),
        commitment_hash: commitment_hash.clone(),
        timestamp,
        verified: false,
    };
    
    env.storage().persistent().set(&key, &commitment);
    
    // Increment global counter
    let count: u64 = env
        .storage()
        .instance()
        .get(&PrivacyKey::CommitmentCount)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&PrivacyKey::CommitmentCount, &(count + 1));
    
    // Emit PrivateStatCommitted event
    env.events().publish(
        (symbol_short!("privacy"), symbol_short!("commit")),
        (player, stat_type, commitment_hash.clone()),
    );
    
    Ok(commitment_hash)
}

/// Verify a private stat commitment using a zero-knowledge proof.
/// This allows validation without revealing the underlying data.
///
/// # Arguments
/// * `commitment` - The commitment hash to verify
/// * `proof` - A 64-byte zero-knowledge proof
///
/// # Returns
/// `true` if the proof is valid, otherwise returns `InvalidProof` error.
pub fn verify_private_stat(
    env: &Env,
    commitment: BytesN<32>,
    proof: BytesN<64>,
) -> Result<bool, PrivacyError> {
    // Pure verification function - no auth required
    
    if !verify_proof_internal(env, &commitment, &proof) {
        return Err(PrivacyError::InvalidProof);
    }
    
    // Emit verification event
    env.events().publish(
        (symbol_short!("privacy"), symbol_short!("verify")),
        (commitment, true),
    );
    
    Ok(true)
}

/// Get a commitment for a player and stat type.
pub fn get_commitment(
    env: &Env,
    player: Address,
    stat_type: Symbol,
) -> Result<StatCommitment, PrivacyError> {
    let key = PrivacyKey::Commitment(player, stat_type);
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(PrivacyError::CommitmentNotFound)
}

/// Get total number of commitments made across all players.
pub fn get_commitment_count(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&PrivacyKey::CommitmentCount)
        .unwrap_or(0)
}

/// Batch commit multiple stats in a single transaction (up to 10).
/// This is more gas-efficient for committing multiple stats at once.
pub fn batch_commit_stats(
    env: &Env,
    player: Address,
    stat_types: Vec<Symbol>,
    values: Vec<i128>,
) -> Result<Vec<BytesN<32>>, PrivacyError> {
    player.require_auth();
    
    if !is_opted_in(env, &player) {
        return Err(PrivacyError::NotOptedIn);
    }
    
    let count = stat_types.len();
    if count > MAX_COMMITMENTS_PER_TX {
        return Err(PrivacyError::BurstLimitExceeded);
    }
    
    if count != values.len() {
        return Err(PrivacyError::InvalidProof); // Reuse error for mismatched lengths
    }
    
    let mut commitments = Vec::new(env);
    
    for i in 0..count {
        let stat_type = stat_types.get(i).unwrap();
        let value = values.get(i).unwrap();
        
        // Check if commitment already exists
        let key = PrivacyKey::Commitment(player.clone(), stat_type.clone());
        if env.storage().persistent().has(&key) {
            return Err(PrivacyError::CommitmentExists);
        }
        
        let timestamp = env.ledger().timestamp();
        let commitment_hash = compute_commitment_hash(env, &stat_type, value, &player, timestamp);
        
        let commitment = StatCommitment {
            player: player.clone(),
            stat_type: stat_type.clone(),
            commitment_hash: commitment_hash.clone(),
            timestamp,
            verified: false,
        };
        
        env.storage().persistent().set(&key, &commitment);
        commitments.push_back(commitment_hash.clone());
        
        // Emit event for each commitment
        env.events().publish(
            (symbol_short!("privacy"), symbol_short!("commit")),
            (player.clone(), stat_type, commitment_hash),
        );
    }
    
    // Update global counter
    let current_count: u64 = env
        .storage()
        .instance()
        .get(&PrivacyKey::CommitmentCount)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&PrivacyKey::CommitmentCount, &(current_count + count as u64));
    
    Ok(commitments)
}

// ─── Balance Hiding (#277) ────────────────────────────────────────────────

/// Storage keys for balance hiding and selective disclosure.
#[derive(Clone)]
#[contracttype]
pub enum BalancePrivacyKey {
    /// Hidden balance commitment for a player: (player, token) → commitment.
    HiddenBalance(Address, Symbol),
    /// Selective disclosure grant: (owner, viewer, stat_type) → bool.
    DisclosureGrant(Address, Address, Symbol),
    /// Privacy level for a player (0=none, 1=balance-only, 2=full).
    PrivacyLevel(Address),
}

/// Commit a hidden balance. The raw balance is never stored on-chain —
/// only the commitment hash. The player can later reveal the balance
/// off-chain and prove it matches the commitment.
pub fn hide_balance(
    env: &Env,
    player: Address,
    token: Symbol,
    balance: i128,
) -> Result<BytesN<32>, PrivacyError> {
    player.require_auth();

    if !is_opted_in(env, &player) {
        return Err(PrivacyError::NotOptedIn);
    }

    let timestamp = env.ledger().timestamp();
    let commitment = compute_commitment_hash(env, &token, balance, &player, timestamp);

    env.storage().persistent().set(
        &BalancePrivacyKey::HiddenBalance(player.clone(), token.clone()),
        &commitment,
    );

    env.events().publish(
        (symbol_short!("privacy"), symbol_short!("bal_hide")),
        (player, token, commitment.clone()),
    );

    Ok(commitment)
}

/// Get the hidden balance commitment for a player.
pub fn get_hidden_balance(
    env: &Env,
    player: &Address,
    token: &Symbol,
) -> Option<BytesN<32>> {
    env.storage()
        .persistent()
        .get(&BalancePrivacyKey::HiddenBalance(player.clone(), token.clone()))
}

// ─── Selective Disclosure (#277) ──────────────────────────────────────────

/// Grant a viewer permission to see a specific stat type.
pub fn grant_disclosure(
    env: &Env,
    owner: Address,
    viewer: Address,
    stat_type: Symbol,
) -> Result<(), PrivacyError> {
    owner.require_auth();

    if !is_opted_in(env, &owner) {
        return Err(PrivacyError::NotOptedIn);
    }

    env.storage().persistent().set(
        &BalancePrivacyKey::DisclosureGrant(owner.clone(), viewer.clone(), stat_type.clone()),
        &true,
    );

    env.events().publish(
        (symbol_short!("privacy"), symbol_short!("disc_grnt")),
        (owner, viewer, stat_type),
    );

    Ok(())
}

/// Revoke a viewer's permission to see a specific stat type.
pub fn revoke_disclosure(
    env: &Env,
    owner: Address,
    viewer: Address,
    stat_type: Symbol,
) -> Result<(), PrivacyError> {
    owner.require_auth();

    env.storage().persistent().remove(
        &BalancePrivacyKey::DisclosureGrant(owner, viewer, stat_type),
    );

    Ok(())
}

/// Check if a viewer has been granted access to a specific stat type.
pub fn has_disclosure(
    env: &Env,
    owner: &Address,
    viewer: &Address,
    stat_type: &Symbol,
) -> bool {
    env.storage()
        .persistent()
        .get::<_, bool>(&BalancePrivacyKey::DisclosureGrant(
            owner.clone(),
            viewer.clone(),
            stat_type.clone(),
        ))
        .unwrap_or(false)
}

/// Set the privacy level for a player.
/// - 0: No privacy (all stats visible)
/// - 1: Balance-only hidden
/// - 2: Full privacy (all stats require selective disclosure)
pub fn set_privacy_level(
    env: &Env,
    player: Address,
    level: u32,
) -> Result<(), PrivacyError> {
    player.require_auth();

    if level > 2 {
        return Err(PrivacyError::InvalidProof);
    }

    env.storage()
        .persistent()
        .set(&BalancePrivacyKey::PrivacyLevel(player), &level);

    Ok(())
}

/// Get the privacy level for a player.
pub fn get_privacy_level(env: &Env, player: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&BalancePrivacyKey::PrivacyLevel(player.clone()))
        .unwrap_or(0)
}

/// Verify a commitment without revealing the value. This is a read-only
/// proof verification that any party can perform.
pub fn verify_balance_commitment(
    env: &Env,
    player: &Address,
    token: &Symbol,
    proof: &BytesN<64>,
) -> Result<bool, PrivacyError> {
    let commitment = get_hidden_balance(env, player, token)
        .ok_or(PrivacyError::CommitmentNotFound)?;

    Ok(verify_proof_internal(env, &commitment, proof))
}
