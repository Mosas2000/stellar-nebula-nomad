use soroban_sdk::{contracterror, contracttype, symbol_short, Address, BytesN, Env, Vec};

/// Minimum number of stars required to form a valid constellation.
pub const MIN_STARS: u32 = 3;

/// Maximum patterns matched per `match_constellation` call.
pub const MAX_MATCH_BURST: u32 = 15;

// ─── Storage Keys ──────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum ConstellationKey {
    /// Auto-incrementing constellation ID.
    Counter,
    /// Constellation data by ID.
    Constellation(u64),
}

// ─── Errors ─────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ConstellationError {
    /// No known constellation matched the observed pattern.
    NoMatchFound = 1,
    /// Star vector is below the minimum count.
    TooFewStars = 2,
    /// Attempted to mutate an immutable historical record.
    ImmutableRecord = 3,
    /// Requested burst size exceeds MAX_MATCH_BURST.
    BurstTooLarge = 4,
}

// ─── Data Types ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct Constellation {
    pub id: u64,
    pub recorder: Address,
    /// Ordered list of star hashes (coordinate + spectral fingerprint).
    pub stars: Vec<BytesN<32>>,
    pub recorded_at: u64,
    /// Optional name (stored as first star's hash parity for on-chain cheapness).
    pub star_count: u32,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct MatchResult {
    pub constellation_id: u64,
    /// Number of stars that coincided with the query.
    pub matched_stars: u32,
    pub total_stars: u32,
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Count how many elements of `observed` appear in `known`.
///
/// Both slices are small (≤ 50 stars), so O(n*m) is acceptable within gas
/// limits for on-chain execution.
fn count_matching_stars(observed: &Vec<BytesN<32>>, known: &Vec<BytesN<32>>) -> u32 {
    let mut count = 0u32;
    for obs in observed.iter() {
        for kn in known.iter() {
            if obs == kn {
                count += 1;
                break;
            }
        }
    }
    count
}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Record a new constellation pattern on-chain.
///
/// Records are immutable after creation to preserve historical navigation data.
/// Returns the assigned constellation ID.
pub fn record_constellation(
    env: &Env,
    recorder: &Address,
    stars: &Vec<BytesN<32>>,
) -> Result<u64, ConstellationError> {
    recorder.require_auth();

    if stars.len() < MIN_STARS {
        return Err(ConstellationError::TooFewStars);
    }

    let id: u64 = env
        .storage()
        .instance()
        .get(&ConstellationKey::Counter)
        .unwrap_or(0);

    let now = env.ledger().timestamp();
    let star_count = stars.len();

    let constellation = Constellation {
        id,
        recorder: recorder.clone(),
        stars: stars.clone(),
        recorded_at: now,
        star_count,
    };

    env.storage()
        .persistent()
        .set(&ConstellationKey::Constellation(id), &constellation);
    env.storage()
        .instance()
        .set(&ConstellationKey::Counter, &(id + 1));

    env.events().publish(
        (symbol_short!("const"), symbol_short!("record")),
        (recorder.clone(), id, star_count, now),
    );

    Ok(id)
}

/// Find the known constellation with the highest star overlap with `observed`.
///
/// Returns the best [`MatchResult`], or [`ConstellationError::NoMatchFound`]
/// if no constellation shares at least one star with the query.
pub fn match_constellation(
    env: &Env,
    observed: &Vec<BytesN<32>>,
) -> Result<MatchResult, ConstellationError> {
    if observed.len() < MIN_STARS {
        return Err(ConstellationError::TooFewStars);
    }

    let total: u64 = env
        .storage()
        .instance()
        .get(&ConstellationKey::Counter)
        .unwrap_or(0);

    if total == 0 {
        return Err(ConstellationError::NoMatchFound);
    }

    // Cap the search to MAX_MATCH_BURST most-recently recorded constellations.
    let search_from = total.saturating_sub(MAX_MATCH_BURST as u64);

    let mut best_id: u64 = 0;
    let mut best_matches: u32 = 0;
    let mut best_total: u32 = 0;
    let mut found = false;

    for id in search_from..total {
        let constellation: Constellation = match env
            .storage()
            .persistent()
            .get(&ConstellationKey::Constellation(id))
        {
            Some(c) => c,
            None => continue,
        };

        let matched = count_matching_stars(observed, &constellation.stars);
        if matched > best_matches {
            best_matches = matched;
            best_id = id;
            best_total = constellation.star_count;
            found = true;
        }
    }

    if !found || best_matches == 0 {
        return Err(ConstellationError::NoMatchFound);
    }

    let result = MatchResult {
        constellation_id: best_id,
        matched_stars: best_matches,
        total_stars: best_total,
    };

    env.events().publish(
        (symbol_short!("const"), symbol_short!("matched")),
        (best_id, best_matches, best_total),
    );

    Ok(result)
}

/// Match multiple observed patterns in one call (burst mode).
///
/// Returns a list of the best [`MatchResult`] for each pattern.
/// Patterns that yield no match are silently skipped (caller should check
/// result length vs input length).
pub fn match_constellations_batch(
    env: &Env,
    patterns: &Vec<Vec<BytesN<32>>>,
) -> Result<Vec<MatchResult>, ConstellationError> {
    if patterns.len() > MAX_MATCH_BURST {
        return Err(ConstellationError::BurstTooLarge);
    }

    let mut results: Vec<MatchResult> = Vec::new(env);
    for pattern in patterns.iter() {
        if let Ok(r) = match_constellation(env, &pattern) {
            results.push_back(r);
        }
    }

    Ok(results)
}

/// Retrieve a stored constellation by ID.
pub fn get_constellation(
    env: &Env,
    constellation_id: u64,
) -> Result<Constellation, ConstellationError> {
    env.storage()
        .persistent()
        .get(&ConstellationKey::Constellation(constellation_id))
        .ok_or(ConstellationError::NoMatchFound)
}

/// Return the total number of recorded constellations.
pub fn get_constellation_count(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&ConstellationKey::Counter)
        .unwrap_or(0)
}
