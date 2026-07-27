//! # Anti-Bot Measures (#278)
//!
//! Comprehensive bot detection and prevention for Stellar Nebula Nomad.
//!
//! - **Behavioral Analysis**: Track action patterns and flag anomalous timing.
//! - **CAPTCHA Integration**: Require proof-of-humanity for suspicious actors.
//! - **Pattern Detection**: Identify automated click patterns, impossible
//!   action speeds, and repetitive sequences.
//! - **Rate Limiting**: Per-address action budgets (extends rate_limiter.rs).
//!
//! ## How It Works
//!
//! Every game action calls `bot_detection::record_action` which stores a
//! rolling window of timestamps. `check_suspicious` analyzes the window
//! for bot-like patterns. If flagged, the player must pass a CAPTCHA
//! challenge before their next action is accepted.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

use crate::rate_limiter;

// ─── Storage Keys ─────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum BotKey {
    /// Rolling action timestamps for a player: (player, window_id) → Vec<u64>.
    ActionWindow(Address, u64),
    /// Suspicion score per player (0–100).
    SuspicionScore(Address),
    /// Whether player is CAPTCHA-gated.
    CaptchaRequired(Address),
    /// CAPTCHA challenge: (player, challenge_id) → challenge data.
    CaptchaChallenge(Address, u64),
    /// Global challenge counter.
    ChallengeCounter,
    /// Player trust level (0=untrusted, 1=normal, 2=trusted).
    TrustLevel(Address),
    /// Flagged pattern count per player.
    PatternFlags(Address),
}

// ─── Constants ────────────────────────────────────────────────────────────

/// Minimum time (ms) between actions. Actions faster than this are suspicious.
const MIN_ACTION_INTERVAL_MS: u64 = 200;
/// Maximum actions in a rolling window before suspicion increases.
const WINDOW_SIZE: u32 = 20;
/// Suspicion threshold above which CAPTCHA is required.
const CAPTCHA_THRESHOLD: u32 = 60;
/// How much suspicion decays per window (percentage).
const DECAY_RATE: u32 = 10;
/// Maximum suspicion score.
const MAX_SUSPICION: u32 = 100;

// ─── Data Types ───────────────────────────────────────────────────────────

/// A CAPTCHA challenge issued to a suspicious player.
#[derive(Clone)]
#[contracttype]
pub struct CaptchaChallengeData {
    pub challenge_id: u64,
    pub player: Address,
    /// Hash of the expected answer.
    pub answer_hash: soroban_sdk::BytesN<32>,
    pub issued_at: u64,
    pub expires_at: u64,
    pub solved: bool,
}

// ─── Errors ───────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BotError {
    /// Player is flagged as suspicious and must solve a CAPTCHA.
    CaptchaRequired = 1,
    /// CAPTCHA challenge not found.
    ChallengeNotFound = 2,
    /// CAPTCHA challenge expired.
    ChallengeExpired = 3,
    /// CAPTCHA answer incorrect.
    IncorrectAnswer = 4,
    /// Action too fast — potential automation detected.
    ActionTooFast = 5,
    /// Player is temporarily blocked.
    PlayerBlocked = 6,
}

// ─── Behavioral Analysis ──────────────────────────────────────────────────

/// Record an action timestamp for behavioral analysis.
///
/// Call this at the start of every game action. Returns `Err(BotError)` if
/// the player is CAPTCHA-gated and hasn't solved their challenge.
pub fn record_action(
    env: &Env,
    player: &Address,
    _action: &Symbol,
) -> Result<(), BotError> {
    // Check if player needs CAPTCHA.
    let needs_captcha: bool = env
        .storage()
        .persistent()
        .get(&BotKey::CaptchaRequired(player.clone()))
        .unwrap_or(false);

    if needs_captcha {
        return Err(BotError::CaptchaRequired);
    }

    let now = env.ledger().timestamp();
    let window_id = now / 60_000; // 60-second windows.

    let key = BotKey::ActionWindow(player.clone(), window_id);
    let mut timestamps: Vec<u64> = env
        .storage()
        .temporary()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));

    // Check for impossibly fast actions.
    if timestamps.len() > 0 {
        let last = timestamps.get(timestamps.len() - 1).unwrap();
        if now - last < MIN_ACTION_INTERVAL_MS {
            increase_suspicion(env, player, 20);
            return Err(BotError::ActionTooFast);
        }
    }

    timestamps.push_back(now);

    // Keep only the last WINDOW_SIZE entries.
    if timestamps.len() > WINDOW_SIZE {
        let mut trimmed = Vec::new(env);
        let start = timestamps.len() - WINDOW_SIZE;
        for i in start..timestamps.len() {
            trimmed.push_back(timestamps.get(i).unwrap());
        }
        timestamps = trimmed;
    }

    env.storage().temporary().set(&key, &timestamps);

    // Analyze patterns.
    analyze_patterns(env, player, &timestamps);

    Ok(())
}

/// Analyze action timestamps for bot-like patterns.
fn analyze_patterns(env: &Env, player: &Address, timestamps: &Vec<u64>) {
    if timestamps.len() < 5 {
        return;
    }

    // Check for perfectly regular intervals (bot signature).
    let mut regular_count = 0u32;
    let len = timestamps.len();

    for i in 1..len {
        let curr = timestamps.get(i).unwrap();
        let prev = timestamps.get(i - 1).unwrap();
        let diff = curr - prev;

        // If intervals are within 5ms of each other, that's suspicious.
        if i >= 2 {
            let prev_prev = timestamps.get(i - 2).unwrap();
            let prev_diff = prev - prev_prev;
            if diff.abs_diff(prev_diff) < 5 {
                regular_count += 1;
            }
        }
    }

    // If more than 70% of intervals are regular, flag as suspicious.
    let threshold = (len as u32 - 2) * 7 / 10;
    if regular_count >= threshold && regular_count > 3 {
        increase_suspicion(env, player, 15);
        let flags: u32 = env
            .storage()
            .persistent()
            .get(&BotKey::PatternFlags(player.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&BotKey::PatternFlags(player.clone()), &(flags + 1));

        env.events().publish(
            (symbol_short!("bot"), symbol_short!("pattern")),
            (player.clone(), regular_count),
        );
    }
}

/// Increase a player's suspicion score.
fn increase_suspicion(env: &Env, player: &Address, amount: u32) {
    let current: u32 = env
        .storage()
        .persistent()
        .get(&BotKey::SuspicionScore(player.clone()))
        .unwrap_or(0);

    let new_score = (current + amount).min(MAX_SUSPICION);
    env.storage()
        .persistent()
        .set(&BotKey::SuspicionScore(player.clone()), &new_score);

    if new_score >= CAPTCHA_THRESHOLD {
        env.storage()
            .persistent()
            .set(&BotKey::CaptchaRequired(player.clone()), &true);

        env.events().publish(
            (symbol_short!("bot"), symbol_short!("flagged")),
            (player.clone(), new_score),
        );
    }
}

/// Decay suspicion scores for a player (called periodically).
pub fn decay_suspicion(env: &Env, player: &Address) {
    let current: u32 = env
        .storage()
        .persistent()
        .get(&BotKey::SuspicionScore(player.clone()))
        .unwrap_or(0);

    if current > 0 {
        let decayed = current.saturating_sub(current * DECAY_RATE / 100);
        env.storage()
            .persistent()
            .set(&BotKey::SuspicionScore(player.clone()), &decayed);

        // Remove CAPTCHA requirement if score drops below threshold.
        if decayed < CAPTCHA_THRESHOLD {
            env.storage()
                .persistent()
                .set(&BotKey::CaptchaRequired(player.clone()), &false);
        }
    }
}

// ─── CAPTCHA Integration ──────────────────────────────────────────────────

/// Issue a CAPTCHA challenge to a player.
pub fn issue_captcha(
    env: &Env,
    player: &Address,
    answer_hash: soroban_sdk::BytesN<32>,
    ttl_seconds: u64,
) -> Result<u64, BotError> {
    let challenge_id: u64 = env
        .storage()
        .instance()
        .get(&BotKey::ChallengeCounter)
        .unwrap_or(0)
        + 1;
    env.storage()
        .instance()
        .set(&BotKey::ChallengeCounter, &challenge_id);

    let now = env.ledger().timestamp();
    let challenge = CaptchaChallengeData {
        challenge_id,
        player: player.clone(),
        answer_hash,
        issued_at: now,
        expires_at: now + ttl_seconds,
        solved: false,
    };

    env.storage()
        .persistent()
        .set(&BotKey::CaptchaChallenge(player.clone(), challenge_id), &challenge);

    env.events().publish(
        (symbol_short!("bot"), symbol_short!("captcha")),
        (player.clone(), challenge_id),
    );

    Ok(challenge_id)
}

/// Solve a CAPTCHA challenge.
pub fn solve_captcha(
    env: &Env,
    player: Address,
    challenge_id: u64,
    answer_hash: soroban_sdk::BytesN<32>,
) -> Result<(), BotError> {
    player.require_auth();

    let key = BotKey::CaptchaChallenge(player.clone(), challenge_id);
    let mut challenge: CaptchaChallengeData = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(BotError::ChallengeNotFound)?;

    if env.ledger().timestamp() > challenge.expires_at {
        return Err(BotError::ChallengeExpired);
    }

    // Verify answer hash matches.
    if challenge.answer_hash != answer_hash {
        return Err(BotError::IncorrectAnswer);
    }

    challenge.solved = true;
    env.storage().persistent().set(&key, &challenge);

    // Clear CAPTCHA requirement.
    env.storage()
        .persistent()
        .set(&BotKey::CaptchaRequired(player.clone()), &false);

    // Reduce suspicion.
    let current: u32 = env
        .storage()
        .persistent()
        .get(&BotKey::SuspicionScore(player.clone()))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&BotKey::SuspicionScore(player.clone()), &current.saturating_sub(30));

    // Increase trust level.
    env.storage()
        .persistent()
        .set(&BotKey::TrustLevel(player.clone()), &2u32);

    env.events().publish(
        (symbol_short!("bot"), symbol_short!("solved")),
        (player, challenge_id),
    );

    Ok(())
}

// ─── Query Functions ──────────────────────────────────────────────────────

/// Get a player's current suspicion score.
pub fn get_suspicion_score(env: &Env, player: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&BotKey::SuspicionScore(player.clone()))
        .unwrap_or(0)
}

/// Check if a player is currently CAPTCHA-gated.
pub fn is_captcha_required(env: &Env, player: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&BotKey::CaptchaRequired(player.clone()))
        .unwrap_or(false)
}

/// Get a player's trust level.
pub fn get_trust_level(env: &Env, player: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&BotKey::TrustLevel(player.clone()))
        .unwrap_or(1) // Default: normal trust.
}

/// Admin override: manually set a player's trust level.
pub fn admin_set_trust(
    env: &Env,
    admin: Address,
    player: Address,
    level: u32,
) -> Result<(), BotError> {
    admin.require_auth();

    env.storage()
        .persistent()
        .set(&BotKey::TrustLevel(player.clone()), &level);

    if level >= 2 {
        // Trusted players get cleared.
        env.storage()
            .persistent()
            .set(&BotKey::SuspicionScore(player.clone()), &0u32);
        env.storage()
            .persistent()
            .set(&BotKey::CaptchaRequired(player), &false);
    }

    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};

    fn setup() -> (Env, Address) {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set(LedgerInfo {
            protocol_version: 22,
            sequence_number: 100,
            timestamp: 1_700_000_000_000,
            network_id: [0u8; 32],
            base_reserve: 10,
            min_temp_entry_ttl: 100,
            min_persistent_entry_ttl: 1_000,
            max_entry_ttl: 10_000,
        });
        let player = Address::generate(&env);
        (env, player)
    }

    #[test]
    fn test_record_action_succeeds_normally() {
        let (env, player) = setup();
        assert!(record_action(&env, &player, &symbol_short!("scan")).is_ok());
    }

    #[test]
    fn test_fast_actions_increase_suspicion() {
        let (env, player) = setup();

        // First action is fine.
        record_action(&env, &player, &symbol_short!("scan")).ok();

        // Manually set a very recent timestamp to simulate fast action.
        let now = env.ledger().timestamp();
        let window_id = now / 60_000;
        let mut timestamps = Vec::new(&env);
        timestamps.push_back(now);
        timestamps.push_back(now + 50); // 50ms later — impossibly fast.
        env.storage()
            .temporary()
            .set(&BotKey::ActionWindow(player.clone(), window_id), &timestamps);

        // Next action should be flagged.
        let result = record_action(&env, &player, &symbol_short!("scan"));
        assert_eq!(result, Err(BotError::ActionTooFast));
        assert!(get_suspicion_score(&env, &player) > 0);
    }

    #[test]
    fn test_captcha_gate() {
        let (env, player) = setup();

        // Manually set high suspicion.
        env.storage()
            .persistent()
            .set(&BotKey::SuspicionScore(player.clone()), &70u32);
        env.storage()
            .persistent()
            .set(&BotKey::CaptchaRequired(player.clone()), &true);

        assert_eq!(
            record_action(&env, &player, &symbol_short!("scan")),
            Err(BotError::CaptchaRequired)
        );
    }

    #[test]
    fn test_captcha_solve_clears_gate() {
        let (env, player) = setup();

        let answer_hash: soroban_sdk::BytesN<32> = env.crypto().sha256(&soroban_sdk::Bytes::from_slice(&env, &[42])).into();
        let challenge_id = issue_captcha(&env, &player, answer_hash.clone(), 300).unwrap();

        // Player needs to pass the hash of their answer.
        // For this test, the "answer" is [42], and its hash matches.
        assert!(solve_captcha(&env, player.clone(), challenge_id, answer_hash).is_ok());
        assert!(!is_captcha_required(&env, &player));
    }

    #[test]
    fn test_suspicion_decay() {
        let (env, player) = setup();

        env.storage()
            .persistent()
            .set(&BotKey::SuspicionScore(player.clone()), &50u32);

        decay_suspicion(&env, &player);

        let score = get_suspicion_score(&env, &player);
        assert!(score < 50); // Should have decayed.
    }

    #[test]
    fn test_trust_level_default() {
        let (env, player) = setup();
        assert_eq!(get_trust_level(&env, &player), 1);
    }
}
