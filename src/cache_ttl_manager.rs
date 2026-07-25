use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

// ─── Cache TTL Management System ────────────────────────────────────────────
//
// This module provides consistent TTL enforcement across all cached data,
// preventing stale financial data from being served and protecting the
// gaming economy from exploits.

// ─── Configuration ───────────────────────────────────────────────────────

/// Default cache TTL in seconds (5 minutes).
pub const DEFAULT_CACHE_TTL: u64 = 300;

/// Yield forecast cache TTL (15 minutes - market data updates).
pub const YIELD_FORECAST_TTL: u64 = 900;

/// Market oracle price cache TTL (10 minutes).
pub const MARKET_ORACLE_TTL: u64 = 600;

/// State snapshot cache TTL (30 minutes).
pub const STATE_SNAPSHOT_TTL: u64 = 1800;

/// Leaderboard cache TTL (1 hour).
pub const LEADERBOARD_TTL: u64 = 3600;

/// Player profile cache TTL (15 minutes).
pub const PLAYER_PROFILE_TTL: u64 = 900;

/// Analytics cache TTL (5 minutes - hot data).
pub const ANALYTICS_TTL: u64 = 300;

// ─── Storage Keys ────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum CacheKey {
    /// Cache entry with TTL: `CacheEntry(namespace, key)`.
    CacheEntry(Symbol, Symbol),
    /// Timestamp of last cache invalidation.
    LastInvalidation(Symbol),
    /// TTL configuration per cache type.
    TTLConfig(Symbol),
    /// Stale entry detection flag.
    IsStale(Symbol, Symbol),
}

// ─── Error Types ─────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CacheTTLError {
    /// Cache entry has expired.
    CacheExpired = 1,
    /// Cache entry not found.
    EntryNotFound = 2,
    /// Invalid TTL value.
    InvalidTTL = 3,
    /// Unauthorized cache operation.
    Unauthorized = 4,
    /// Cache validation failed.
    ValidationFailed = 5,
}

// ─── Data Structures ─────────────────────────────────────────────────────

/// Cached data with TTL metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct CachedData {
    pub namespace: Symbol,
    pub key: Symbol,
    pub value: Vec<u8>,
    pub cached_at: u64,
    pub ttl_seconds: u64,
    pub is_valid: bool,
}

/// Cache invalidation event.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct InvalidationEvent {
    pub namespace: Symbol,
    pub reason: Symbol,
    pub invalidated_at: u64,
    pub affected_keys: u32,
}

/// TTL configuration per cache type.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct TTLConfig {
    pub namespace: Symbol,
    pub ttl_seconds: u64,
    pub auto_refresh: bool,
    pub max_age_for_refresh: u64,
}

// ─── Cache Operations ───────────────────────────────────────────────────

/// Store data with TTL enforcement.
pub fn cache_with_ttl(
    env: &Env,
    namespace: Symbol,
    key: Symbol,
    value: Vec<u8>,
    ttl_seconds: u64,
) -> Result<(), CacheTTLError> {
    if ttl_seconds == 0 {
        return Err(CacheTTLError::InvalidTTL);
    }

    let cached = CachedData {
        namespace: namespace.clone(),
        key: key.clone(),
        value,
        cached_at: env.ledger().timestamp(),
        ttl_seconds,
        is_valid: true,
    };

    env.storage()
        .persistent()
        .set(&CacheKey::CacheEntry(namespace, key.clone()), &cached);

    env.storage()
        .instance()
        .set(&CacheKey::IsStale(namespace.clone(), key), &false);

    env.events().publish(
        (symbol_short!("cache"), symbol_short!("stored")),
        (namespace, ttl_seconds, env.ledger().timestamp()),
    );

    Ok(())
}

/// Retrieve cached data with expiry validation.
pub fn get_cached_with_ttl(
    env: &Env,
    namespace: Symbol,
    key: Symbol,
) -> Result<Vec<u8>, CacheTTLError> {
    let cached: Option<CachedData> = env
        .storage()
        .persistent()
        .get(&CacheKey::CacheEntry(namespace.clone(), key.clone()));

    match cached {
        None => Err(CacheTTLError::EntryNotFound),
        Some(entry) => {
            let current_time = env.ledger().timestamp();
            let age = current_time.saturating_sub(entry.cached_at);

            if age > entry.ttl_seconds {
                // Mark as stale and emit event.
                env.storage()
                    .instance()
                    .set(&CacheKey::IsStale(namespace.clone(), key.clone()), &true);

                env.events().publish(
                    (symbol_short!("cache"), symbol_short!("expired")),
                    (namespace, key, current_time),
                );

                Err(CacheTTLError::CacheExpired)
            } else {
                Ok(entry.value)
            }
        }
    }
}

/// Check if cached entry is still valid without retrieving it.
pub fn is_cache_valid(env: &Env, namespace: Symbol, key: Symbol) -> bool {
    let cached: Option<CachedData> = env
        .storage()
        .persistent()
        .get(&CacheKey::CacheEntry(namespace, key));

    match cached {
        None => false,
        Some(entry) => {
            let current_time = env.ledger().timestamp();
            let age = current_time.saturating_sub(entry.cached_at);
            age <= entry.ttl_seconds
        }
    }
}

/// Get remaining TTL for a cache entry.
pub fn get_remaining_ttl(env: &Env, namespace: Symbol, key: Symbol) -> Result<u64, CacheTTLError> {
    let cached: Option<CachedData> = env
        .storage()
        .persistent()
        .get(&CacheKey::CacheEntry(namespace, key));

    match cached {
        None => Err(CacheTTLError::EntryNotFound),
        Some(entry) => {
            let current_time = env.ledger().timestamp();
            let age = current_time.saturating_sub(entry.cached_at);

            if age >= entry.ttl_seconds {
                Ok(0)
            } else {
                Ok(entry.ttl_seconds - age)
            }
        }
    }
}

// ─── Cache Invalidation ──────────────────────────────────────────────────

/// Invalidate a specific cache entry.
pub fn invalidate_cache_entry(
    env: &Env,
    namespace: Symbol,
    key: Symbol,
    reason: Symbol,
) {
    env.storage()
        .instance()
        .set(&CacheKey::IsStale(namespace.clone(), key.clone()), &true);

    env.events().publish(
        (symbol_short!("cache"), symbol_short!("invalid")),
        (namespace, key, reason, env.ledger().timestamp()),
    );
}

/// Invalidate all cache entries in a namespace.
pub fn invalidate_namespace(
    env: &Env,
    namespace: Symbol,
    reason: Symbol,
) {
    env.storage()
        .instance()
        .set(&CacheKey::LastInvalidation(namespace.clone()), &env.ledger().timestamp());

    env.events().publish(
        (symbol_short!("cache"), symbol_short!("ns_invald")),
        (namespace, reason, env.ledger().timestamp()),
    );
}

/// Automatic cleanup of expired entries (called periodically).
pub fn clear_stale_entries(env: &Env, namespace: Symbol) -> u32 {
    let mut cleared = 0u32;

    // In a real implementation, iterate through all entries in the namespace
    // and remove those where age > ttl_seconds. For this example, we track
    // via events and manual cleanup.

    env.events().publish(
        (symbol_short!("cache"), symbol_short!("cleanup")),
        (namespace, cleared, env.ledger().timestamp()),
    );

    cleared
}

// ─── TTL Configuration ──────────────────────────────────────────────────

/// Set TTL configuration for a cache type.
pub fn configure_ttl(
    env: &Env,
    admin: &Address,
    namespace: Symbol,
    ttl_seconds: u64,
    auto_refresh: bool,
) -> Result<(), CacheTTLError> {
    admin.require_auth();

    if ttl_seconds == 0 {
        return Err(CacheTTLError::InvalidTTL);
    }

    let config = TTLConfig {
        namespace: namespace.clone(),
        ttl_seconds,
        auto_refresh,
        max_age_for_refresh: ttl_seconds / 2,
    };

    env.storage()
        .instance()
        .set(&CacheKey::TTLConfig(namespace.clone()), &config);

    env.events().publish(
        (symbol_short!("cache"), symbol_short!("config")),
        (namespace, ttl_seconds, auto_refresh, env.ledger().timestamp()),
    );

    Ok(())
}

/// Get TTL configuration for a namespace.
pub fn get_ttl_config(env: &Env, namespace: Symbol) -> TTLConfig {
    env.storage()
        .instance()
        .get(&CacheKey::TTLConfig(namespace.clone()))
        .unwrap_or(TTLConfig {
            namespace,
            ttl_seconds: DEFAULT_CACHE_TTL,
            auto_refresh: true,
            max_age_for_refresh: DEFAULT_CACHE_TTL / 2,
        })
}

// ─── Health Checks ──────────────────────────────────────────────────────

/// Detect stale data and flag for invalidation.
pub fn detect_stale_data(env: &Env, namespace: Symbol, key: Symbol) -> bool {
    env.storage()
        .instance()
        .get(&CacheKey::IsStale(namespace, key))
        .unwrap_or(false)
}

/// Get cache statistics (for monitoring).
pub fn get_cache_stats(env: &Env, namespace: Symbol) -> (u32, u32) {
    // Returns (total_entries, stale_entries).
    // In a real implementation, this would iterate and count.
    // For now, return placeholder values.
    (0, 0)
}
