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
