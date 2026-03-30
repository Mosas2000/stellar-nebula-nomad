use soroban_sdk::{contracterror, contracttype, symbol_short, Bytes, BytesN, Env, Vec};

/// Maximum number of seeds to keep in the entropy pool.
const MAX_ENTROPY_POOL_SIZE: u32 = 10;

#[derive(Clone)]
#[contracttype]
pub enum OracleKey {
    /// Entropy pool: last N seeds.
    EntropyPool,
    /// Previous block hash fallback.
    PreviousHash,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum OracleError {
    /// The provided seed failed validation.
    SeedInvalid = 1,
}

/// Generate a hybrid random seed by combining ledger sequence, timestamp,
/// and network ID via SHA-256.
///
/// The seed is constructed as:
/// ```text
/// SHA-256(ledger_sequence || timestamp || network_id || entropy_pool_hash)
/// ```
///
/// This provides anti-manipulation through multi-block mixing: each new
/// seed incorporates the accumulated entropy from previous blocks.
pub fn request_random_seed(env: &Env) -> BytesN<32> {
    let mut input = Bytes::new(env);

    // Mix ledger sequence
    input.append(&Bytes::from_slice(
        env,
        &env.ledger().sequence().to_be_bytes(),
    ));

    // Mix timestamp
    input.append(&Bytes::from_slice(
        env,
        &env.ledger().timestamp().to_be_bytes(),
    ));

    // Mix network ID
    let network_id = env.ledger().network_id();
    let network_bytes: Bytes = network_id.into();
    input.append(&network_bytes);

    // Mix in entropy pool if available
    let pool: Vec<BytesN<32>> = env
        .storage()
        .instance()
        .get(&OracleKey::EntropyPool)
        .unwrap_or_else(|| Vec::new(env));

    for i in 0..pool.len() {
        if let Some(prev_seed) = pool.get(i) {
            let prev_bytes: Bytes = prev_seed.into();
            input.append(&prev_bytes);
        }
    }

    // SHA-256 hash to produce the seed
    let seed: BytesN<32> = env.crypto().sha256(&input).into();

    // Add to entropy pool (circular buffer)
    let mut new_pool = Vec::new(env);
    let start = if pool.len() >= MAX_ENTROPY_POOL_SIZE {
        pool.len() - MAX_ENTROPY_POOL_SIZE + 1
    } else {
        0
    };
    for i in start..pool.len() {
        if let Some(s) = pool.get(i) {
            new_pool.push_back(s);
        }
    }
    new_pool.push_back(seed.clone());

    env.storage()
        .instance()
        .set(&OracleKey::EntropyPool, &new_pool);

    // Store as previous hash for fallback
    env.storage()
        .instance()
        .set(&OracleKey::PreviousHash, &seed);

    // Emit RandomSeedGenerated event
    env.events()
        .publish((symbol_short!("rng"), symbol_short!("seed")), seed.clone());

    seed
}

/// Validate a seed and fall back to the previous block hash if invalid.
///
/// A seed is considered invalid if it is all zeros.
/// Returns `Ok(seed)` if valid, or falls back to the stored previous hash.
/// If no fallback is available, returns `OracleError::SeedInvalid`.
pub fn verify_and_fallback(env: &Env, seed: &BytesN<32>) -> Result<BytesN<32>, OracleError> {
    // Check if seed is all zeros (invalid)
    if is_zero_seed(seed) {
        // Attempt fallback to previous hash
        let fallback: Option<BytesN<32>> = env.storage().instance().get(&OracleKey::PreviousHash);

        match fallback {
            Some(prev) => {
                env.events().publish(
                    (symbol_short!("rng"), symbol_short!("fallbck")),
                    prev.clone(),
                );
                Ok(prev)
            }
            None => Err(OracleError::SeedInvalid),
        }
    } else {
        Ok(seed.clone())
    }
}

/// Check if a BytesN<32> is all zeros.
fn is_zero_seed(seed: &BytesN<32>) -> bool {
    let bytes: Bytes = seed.clone().into();
    for i in 0..32u32 {
        if bytes.get(i).unwrap_or(0) != 0 {
            return false;
        }
    }
    true
}

/// Get the current entropy pool.
pub fn get_entropy_pool(env: &Env) -> Vec<BytesN<32>> {
    env.storage()
        .instance()
        .get(&OracleKey::EntropyPool)
        .unwrap_or_else(|| Vec::new(env))
}
