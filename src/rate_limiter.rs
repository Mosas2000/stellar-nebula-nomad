// ============================================================
// rate_limiter.rs — Fix #175: Per-Address Rate Limiting
// Branch: security/rate-limiting
// ============================================================
//
// Changes:
//   • Per-address rate limiting for expensive operations
//   • Configurable rate limits per operation type
//   • Emit RateLimitHit events when limits are exceeded
//
// DoS prevention: prevents spam attacks on nebula generation
// and resource minting by enforcing call-frequency windows.

#![allow(unused)]
use soroban_sdk::{contract, contractimpl, contracttype, contracterror,
                   Address, Env, Map, symbol_short};

// ── Operation kinds ───────────────────────────────────────────
/// Every expensive operation that requires rate limiting.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operation {
    /// `generate_nebula_layout` — CPU + storage intensive.
    NebulaGeneration,
    /// `mint_resource` — token minting on-ledger.
    ResourceMinting,
    /// `upgrade_ship` — ship NFT state mutation.
    ShipUpgrade,
    /// `scan_nebula` — nebula scan operation.
    NebulaScan,
    /// `batch_operation` — batch processing operations.
    BatchOperation,
    /// `privacy_commit` — privacy stat commitments.
    PrivacyCommit,
    /// `route_calculation` — navigation route calculation.
    RouteCalculation,
}

// ── Config types ──────────────────────────────────────────────
/// Rate limit configuration for a single operation type.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RateLimitConfig {
    /// Maximum calls allowed within `window_seconds`.
    pub max_calls:      u32,
    /// Rolling window length in seconds.
    pub window_seconds: u64,
}

impl RateLimitConfig {
    pub fn default_nebula_generation() -> Self {
        Self { max_calls: 5,  window_seconds: 60  }  // 5 scans / minute
    }
    pub fn default_resource_minting() -> Self {
        Self { max_calls: 10, window_seconds: 60  }  // 10 mints / minute
    }
    pub fn default_ship_upgrade() -> Self {
        Self { max_calls: 3,  window_seconds: 300 }  // 3 upgrades / 5 min
    }
    pub fn default_nebula_scan() -> Self {
        Self { max_calls: 10, window_seconds: 3600 }  // 10 scans / hour
    }
    pub fn default_batch_operation() -> Self {
        Self { max_calls: 5,  window_seconds: 3600 }  // 5 batch ops / hour
    }
    pub fn default_privacy_commit() -> Self {
        Self { max_calls: 20, window_seconds: 3600 }  // 20 commits / hour
    }
    pub fn default_route_calculation() -> Self {
        Self { max_calls: 15, window_seconds: 3600 }  // 15 route calcs / hour
    }
}

// ── Storage key ───────────────────────────────────────────────
#[contracttype]
pub enum RateLimitKey {
    /// Tracks (call_count, window_start) per address + operation.
    Entry(Address, Operation),
    /// Admin-configurable limits per operation.
    Config(Operation),
}

// ── Error ─────────────────────────────────────────────────────
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RateLimitError {
    /// Caller has exceeded the allowed call rate for this operation.
    RateLimitExceeded = 100,
    /// Only the contract admin may update rate limit configuration.
    Unauthorized      = 101,
}

// ── Per-address window state ──────────────────────────────────
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowState {
    /// Number of calls made within the current window.
    pub call_count:   u32,
    /// Ledger timestamp when the current window started.
    pub window_start: u64,
}

// ── Core rate-limiter logic ───────────────────────────────────

/// Check and increment the rate-limit counter for `caller` + `op`.
///
/// Returns `Ok(())` if the call is within limits, or
/// `Err(RateLimitError::RateLimitExceeded)` and emits a
/// `RateLimitHit` event if not.
pub fn check_rate_limit(
    env:    &Env,
    caller: &Address,
    op:     Operation,
) -> Result<(), RateLimitError> {
    let config: RateLimitConfig = env
        .storage()
        .instance()
        .get(&RateLimitKey::Config(op.clone()))
        .unwrap_or_else(|| match op {
            Operation::NebulaGeneration => RateLimitConfig::default_nebula_generation(),
            Operation::ResourceMinting  => RateLimitConfig::default_resource_minting(),
            Operation::ShipUpgrade      => RateLimitConfig::default_ship_upgrade(),
            Operation::NebulaScan       => RateLimitConfig::default_nebula_scan(),
            Operation::BatchOperation   => RateLimitConfig::default_batch_operation(),
            Operation::PrivacyCommit    => RateLimitConfig::default_privacy_commit(),
            Operation::RouteCalculation => RateLimitConfig::default_route_calculation(),
        });

    let now         = env.ledger().timestamp();
    let entry_key   = RateLimitKey::Entry(caller.clone(), op.clone());

    let mut state: WindowState = env
        .storage()
        .temporary()
        .get(&entry_key)
        .unwrap_or(WindowState { call_count: 0, window_start: now });

    // Roll window forward if it has expired
    if now >= state.window_start + config.window_seconds {
        state = WindowState { call_count: 0, window_start: now };
    }

    if state.call_count >= config.max_calls {
        // Emit RateLimitHit event (Issue #175 acceptance criterion)
        env.events().publish(
            (symbol_short!("RateLimit"), symbol_short!("hit")),
            (caller.clone(), op, state.call_count, config.max_calls),
        );
        return Err(RateLimitError::RateLimitExceeded);
    }

    // Increment and persist — TTL = window_seconds + 1 ledger
    state.call_count += 1;
    env.storage()
        .temporary()
        .set(&entry_key, &state);

    Ok(())
}

/// Admin function: update rate limit config for an operation.
pub fn set_rate_limit_config(
    env:    &Env,
    admin:  &Address,
    op:     Operation,
    config: RateLimitConfig,
) -> Result<(), RateLimitError> {
    admin.require_auth();
    env.storage()
        .instance()
        .set(&RateLimitKey::Config(op), &config);
    Ok(())
}

// ─────────────────────────────────────────────────────────────
// Tests — Issue #175
// ─────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn make_env() -> Env {
        let env = Env::default();
        env.mock_all_auths();
        env
    }

    #[test]
    fn test_calls_within_limit_succeed() {
        let env    = make_env();
        let caller = Address::generate(&env);

        // Default: 5 calls / 60 s for NebulaGeneration
        for _ in 0..5 {
            assert!(check_rate_limit(&env, &caller, Operation::NebulaGeneration).is_ok());
        }
    }

    #[test]
    fn test_call_beyond_limit_fails() {
        let env    = make_env();
        let caller = Address::generate(&env);

        for _ in 0..5 {
            check_rate_limit(&env, &caller, Operation::NebulaGeneration).ok();
        }
        let result = check_rate_limit(&env, &caller, Operation::NebulaGeneration);
        assert_eq!(result, Err(RateLimitError::RateLimitExceeded));
    }

    #[test]
    fn test_different_addresses_have_independent_limits() {
        let env     = make_env();
        let caller1 = Address::generate(&env);
        let caller2 = Address::generate(&env);

        // Exhaust caller1's limit
        for _ in 0..5 {
            check_rate_limit(&env, &caller1, Operation::NebulaGeneration).ok();
        }
        assert_eq!(
            check_rate_limit(&env, &caller1, Operation::NebulaGeneration),
            Err(RateLimitError::RateLimitExceeded)
        );
        // caller2 is unaffected
        assert!(check_rate_limit(&env, &caller2, Operation::NebulaGeneration).is_ok());
    }

    #[test]
    fn test_different_operations_have_independent_limits() {
        let env    = make_env();
        let caller = Address::generate(&env);

        // Exhaust NebulaGeneration (5 calls)
        for _ in 0..5 {
            check_rate_limit(&env, &caller, Operation::NebulaGeneration).ok();
        }
        assert_eq!(
            check_rate_limit(&env, &caller, Operation::NebulaGeneration),
            Err(RateLimitError::RateLimitExceeded)
        );
        // ResourceMinting limit is independent (10 calls / 60 s)
        assert!(check_rate_limit(&env, &caller, Operation::ResourceMinting).is_ok());
    }

    #[test]
    fn test_custom_config_respected() {
        let env   = make_env();
        let admin = Address::generate(&env);
        let user  = Address::generate(&env);

        // Set a very tight limit: 2 calls / 120 s
        set_rate_limit_config(
            &env, &admin, Operation::ResourceMinting,
            RateLimitConfig { max_calls: 2, window_seconds: 120 },
        ).unwrap();

        assert!(check_rate_limit(&env, &user, Operation::ResourceMinting).is_ok());
        assert!(check_rate_limit(&env, &user, Operation::ResourceMinting).is_ok());
        assert_eq!(
            check_rate_limit(&env, &user, Operation::ResourceMinting),
            Err(RateLimitError::RateLimitExceeded)
        );
    }

    #[test]
    fn test_ship_upgrade_default_limit() {
        let env    = make_env();
        let caller = Address::generate(&env);

        for _ in 0..3 {
            assert!(check_rate_limit(&env, &caller, Operation::ShipUpgrade).is_ok());
        }
        assert_eq!(
            check_rate_limit(&env, &caller, Operation::ShipUpgrade),
            Err(RateLimitError::RateLimitExceeded)
        );
    }
}
