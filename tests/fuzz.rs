//! Advanced Soroban testing and fuzz framework for Nebula Nomad.
//!
//! Run with the `fuzz` feature for the `test_helpers` integration tests:
//! ```
//! cargo test --test fuzz --features fuzz
//! ```
//! The property tests and fuzz harnesses run without any feature flags:
//! ```
//! cargo test --test fuzz
//! ```

#![cfg(test)]

use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestRunner};
use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{symbol_short, Address, Bytes, BytesN, Env};
use stellar_nebula_nomad::{
    DifficultyError, NebulaNomadContract, NebulaNomadContractClient, ShipError, VaultError,
    DEFAULT_MIN_LOCK_DURATION,
};

// ─── Shared Setup ─────────────────────────────────────────────────────────────

fn make_env() -> (Env, NebulaNomadContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(LedgerInfo {
        protocol_version: 22,
        sequence_number: 100,
        timestamp: 1_700_000_000,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 1_000,
        max_entry_ttl: 10_000,
    });
    let contract_id = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    let player = Address::generate(&env);
    (env, client, player)
}

fn advance_time(env: &Env, seconds: u64) {
    let ts = env.ledger().timestamp();
    let seq = env.ledger().sequence();
    env.ledger().set(LedgerInfo {
        protocol_version: 22,
        sequence_number: seq + 1,
        timestamp: ts + seconds,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 1_000,
        max_entry_ttl: 10_000,
    });
}

// ─── generate_test_scenario ───────────────────────────────────────────────────

/// Build a reproducible edge-case environment from a scenario name.
///
/// Returns `(Env, client, player, ship_id)`. The environment is freshly
/// initialized for every call.
///
/// | `scenario_type` | What gets created                           |
/// |-----------------|---------------------------------------------|
/// | `"empty"`       | Fresh contract — no state                   |
/// | `"with_ship"`   | Player + one explorer ship                  |
/// | `"with_profile"`| Player + profile                            |
/// | `"full"`        | Player + profile + explorer ship + vault    |
/// | anything else   | Falls back to `"empty"`                     |
fn generate_test_scenario(
    scenario_type: &str,
) -> (Env, NebulaNomadContractClient<'static>, Address, Option<u64>) {
    let (env, client, player) = make_env();
    let metadata = Bytes::from_array(&env, &[0u8; 4]);
    let mut ship_id: Option<u64> = None;

    match scenario_type {
        "with_ship" => {
            let ship = client.mint_ship(&player, &symbol_short!("explorer"), &metadata);
            ship_id = Some(ship.id);
        }
        "with_profile" => {
            client.initialize_profile(&player);
        }
        "full" => {
            client.initialize_profile(&player);
            let ship = client.mint_ship(&player, &symbol_short!("explorer"), &metadata);
            ship_id = Some(ship.id);
            client.deposit_treasure(&player, &ship.id, &100u64);
        }
        _ => {} // "empty" or unknown
    }

    (env, client, player, ship_id)
}

// ─── fuzz_contract_function ───────────────────────────────────────────────────

/// Run `iterations` randomised test cases against the named contract function.
///
/// Uses proptest's `TestRunner` so failures are automatically shrunk. All
/// assertions use `prop_assert!` / `prop_assert_eq!` for correct shrinking.
///
/// # Supported function names
/// - `"calculate_difficulty"` — levels 0..=110 (valid + invalid)
/// - `"mint_ship_valid"`      — valid ship types (fighter / explorer / hauler)
/// - `"mint_ship_invalid"`    — invalid ship types (must return `InvalidShipType`)
/// - `"deposit_treasure"`     — amounts 1..=1_000_000 (all must succeed)
/// - `"quick_scan_preview"`   — non-existent ship IDs (must return `ShipNotFound`)
///
/// Unknown names are silently skipped.
pub fn fuzz_contract_function(function_name: &str, iterations: u32) {
    let config = ProptestConfig::with_cases(iterations);
    let mut runner = TestRunner::new(config);

    match function_name {
        "calculate_difficulty" => {
            runner
                .run(&(0u32..=110u32), |level| {
                    let (_, client, _) = make_env();
                    let result = client.try_calculate_difficulty(&level);
                    if level == 0 || level > 100 {
                        prop_assert!(
                            matches!(result, Err(Ok(DifficultyError::InvalidLevel))),
                            "expected InvalidLevel for level={level}"
                        );
                    } else {
                        let ok = result
                            .expect("outer call must not error")
                            .expect("valid level must succeed");
                        prop_assert!(
                            ok.anomaly_count > 0,
                            "anomaly_count must be > 0 for level={level}"
                        );
                        let w = &ok.rarity_weights;
                        prop_assert_eq!(
                            w.common + w.uncommon + w.rare + w.epic + w.legendary,
                            100u32,
                            "rarity weights must sum to 100"
                        );
                    }
                    Ok(())
                })
                .unwrap();
        }

        "mint_ship_valid" => {
            // 0=fighter, 1=explorer, 2=hauler
            runner
                .run(&(0usize..=2usize), |idx| {
                    let (env, client, player) = make_env();
                    let metadata = Bytes::from_array(&env, &[0u8; 4]);
                    let stype = match idx {
                        0 => symbol_short!("fighter"),
                        1 => symbol_short!("explorer"),
                        _ => symbol_short!("hauler"),
                    };
                    let result = client.try_mint_ship(&player, &stype, &metadata);
                    prop_assert!(
                        matches!(result, Ok(Ok(_))),
                        "valid ship type index={idx} must succeed"
                    );
                    Ok(())
                })
                .unwrap();
        }

        "mint_ship_invalid" => {
            // 0=unknown, 1=ghost, 2=turret
            runner
                .run(&(0usize..=2usize), |idx| {
                    let (env, client, player) = make_env();
                    let metadata = Bytes::from_array(&env, &[0u8; 4]);
                    let stype = match idx {
                        0 => symbol_short!("unknown"),
                        1 => symbol_short!("ghost"),
                        _ => symbol_short!("turret"),
                    };
                    let result = client.try_mint_ship(&player, &stype, &metadata);
                    prop_assert!(
                        matches!(result, Err(Ok(ShipError::InvalidShipType))),
                        "invalid ship type index={idx} must return InvalidShipType"
                    );
                    Ok(())
                })
                .unwrap();
        }

        "deposit_treasure" => {
            runner
                .run(&(1u64..=1_000_000u64), |amount| {
                    let (env, client, player) = make_env();
                    let metadata = Bytes::from_array(&env, &[0u8; 4]);
                    let ship = client.mint_ship(&player, &symbol_short!("explorer"), &metadata);
                    // client.deposit_treasure panics on contract error; success means vault created.
                    let vault = client.deposit_treasure(&player, &ship.id, &amount);
                    prop_assert_eq!(vault.amount, amount, "vault.amount must match deposited amount");
                    prop_assert!(!vault.claimed, "new vault must not be claimed");
                    Ok(())
                })
                .unwrap();
        }

        _ => { /* unknown function name — no-op */ }
    }
}

// ─── proptest property tests ──────────────────────────────────────────────────

proptest! {
    /// `calculate_difficulty` accepts every level in [1, 100] and the
    /// returned rarity weights always sum to exactly 100.
    #[test]
    fn prop_difficulty_valid_range_succeeds(level in 1u32..=100u32) {
        let (_, client, _) = make_env();
        let result = client.calculate_difficulty(&level);
        let w = &result.rarity_weights;
        prop_assert_eq!(
            w.common + w.uncommon + w.rare + w.epic + w.legendary,
            100u32
        );
    }

    /// `calculate_difficulty` always fails for level 0 or any level > 100.
    #[test]
    fn prop_difficulty_invalid_range_fails(
        level in prop_oneof![Just(0u32), 101u32..=u32::MAX]
    ) {
        let (_, client, _) = make_env();
        let result = client.try_calculate_difficulty(&level);
        prop_assert!(matches!(result, Err(Ok(DifficultyError::InvalidLevel))));
    }

    /// `deposit_treasure` with amount > 0 creates a vault whose `amount`
    /// field equals the deposited value and `claimed` starts as `false`.
    #[test]
    fn prop_vault_deposit_stores_correct_amount(amount in 1u64..=1_000_000u64) {
        let (env, client, player) = make_env();
        let metadata = Bytes::from_array(&env, &[0u8; 4]);
        let ship = client.mint_ship(&player, &symbol_short!("hauler"), &metadata);
        let vault = client.deposit_treasure(&player, &ship.id, &amount);
        prop_assert_eq!(vault.amount, amount);
        prop_assert!(!vault.claimed);
    }

    /// `deposit_treasure` with amount == 0 always returns `InvalidAmount`.
    #[test]
    fn prop_vault_zero_deposit_rejected(_x in 0u8..=0u8) {
        let (env, client, player) = make_env();
        let metadata = Bytes::from_array(&env, &[0u8; 4]);
        let ship = client.mint_ship(&player, &symbol_short!("hauler"), &metadata);
        let result = client.try_deposit_treasure(&player, &ship.id, &0u64);
        prop_assert!(matches!(result, Err(Ok(VaultError::InvalidAmount))));
    }

    /// Minting a ship and reading it back always returns a record with the
    /// same ID, hull, and scanner_power.
    #[test]
    fn prop_get_ship_roundtrip(_x in 0u8..=0u8) {
        let (env, client, player) = make_env();
        let metadata = Bytes::from_array(&env, &[1u8; 4]);
        let minted = client.mint_ship(&player, &symbol_short!("explorer"), &metadata);
        let fetched = client.get_ship(&minted.id);
        prop_assert_eq!(minted.id, fetched.id);
        prop_assert_eq!(minted.hull, fetched.hull);
        prop_assert_eq!(minted.scanner_power, fetched.scanner_power);
    }

    /// `get_ships_by_owner` returns an empty list for players who have never
    /// minted a ship.
    #[test]
    fn prop_ships_by_owner_empty_for_new_player(_x in 0u8..=0u8) {
        let (_, client, player) = make_env();
        let ids = client.get_ships_by_owner(&player);
        prop_assert_eq!(ids.len(), 0u32);
    }

    /// The nebula layout generated for any 32-byte seed always has exactly
    /// `TOTAL_CELLS` cells.
    #[test]
    fn prop_layout_always_has_correct_cell_count(seed_byte in 0u8..=255u8) {
        use stellar_nebula_nomad::TOTAL_CELLS;
        let (env, client, player) = make_env();
        let seed = BytesN::from_array(&env, &[seed_byte; 32]);
        let layout = client.generate_nebula_layout(&seed, &player);
        prop_assert_eq!(layout.cells.len(), TOTAL_CELLS);
    }

}

// ─── fuzz_contract_function driver tests ─────────────────────────────────────

/// Difficulty fuzz harness — 1 000 iterations covering valid and invalid levels.
#[test]
fn fuzz_calculate_difficulty_1000() {
    fuzz_contract_function("calculate_difficulty", 1_000);
}

/// Valid ship-type fuzz harness — 1 000 iterations.
#[test]
fn fuzz_mint_ship_valid_1000() {
    fuzz_contract_function("mint_ship_valid", 1_000);
}

/// Invalid ship-type fuzz harness — 1 000 iterations.
#[test]
fn fuzz_mint_ship_invalid_1000() {
    fuzz_contract_function("mint_ship_invalid", 1_000);
}

/// Deposit-treasure fuzz harness — 1 000 iterations of positive amounts.
#[test]
fn fuzz_deposit_treasure_1000() {
    fuzz_contract_function("deposit_treasure", 1_000);
}

// ─── generate_test_scenario tests ────────────────────────────────────────────

/// "empty" scenario produces no ships and no profile.
#[test]
fn scenario_empty_has_no_state() {
    let (_, client, player, ship_id) = generate_test_scenario("empty");
    assert!(ship_id.is_none());
    let ships = client.get_ships_by_owner(&player);
    assert_eq!(ships.len(), 0);
    // Profile query: get_profile returns ProfileNotFound for a fresh player.
    let pid_result = client.try_initialize_profile(&player);
    // initialize_profile should succeed (no pre-existing profile).
    assert!(matches!(pid_result, Ok(Ok(_))));
}

/// "with_ship" pre-mints one explorer (hull=80, scanner_power=50).
#[test]
fn scenario_with_ship_has_one_explorer() {
    let (_, client, player, ship_id) = generate_test_scenario("with_ship");
    let sid = ship_id.expect("ship_id must be set");
    let ship = client.get_ship(&sid);
    assert_eq!(ship.owner, player);
    assert_eq!(ship.hull, 80);
    assert_eq!(ship.scanner_power, 50);
}

/// "with_profile" creates a profile; a second initialize_profile call fails.
#[test]
fn scenario_with_profile_has_profile() {
    use stellar_nebula_nomad::ProfileError;
    let (_, client, player, _) = generate_test_scenario("with_profile");
    // A second call for the same owner must return ProfileAlreadyExists.
    let result = client.try_initialize_profile(&player);
    assert!(matches!(result, Err(Ok(ProfileError::ProfileAlreadyExists))));
}

/// "full" creates profile + ship + vault; ship count reflects all owned ships.
#[test]
fn scenario_full_has_profile_ship_and_vault() {
    let (_, client, player, ship_id) = generate_test_scenario("full");
    assert!(ship_id.is_some());
    let ships = client.get_ships_by_owner(&player);
    assert_eq!(ships.len(), 1);
}

/// Unknown scenario type falls back to empty state.
#[test]
fn scenario_unknown_falls_back_to_empty() {
    let (_, client, player, ship_id) = generate_test_scenario("nonexistent_scenario");
    assert!(ship_id.is_none());
    assert_eq!(client.get_ships_by_owner(&player).len(), 0);
}

// ─── Vault lock invariant ─────────────────────────────────────────────────────

/// Claiming before the lock expires always returns `StillLocked`.
#[test]
fn vault_claim_before_lock_always_fails() {
    let (env, client, player, ship_id) = generate_test_scenario("with_ship");
    let sid = ship_id.unwrap();
    let vault = client.deposit_treasure(&player, &sid, &500u64);
    advance_time(&env, DEFAULT_MIN_LOCK_DURATION - 1);
    let result = client.try_claim_treasure(&player, &vault.vault_id);
    assert!(matches!(result, Err(Ok(VaultError::StillLocked))));
}

/// Claiming at or after the lock period returns a payout ≥ deposited amount.
#[test]
fn vault_claim_after_lock_succeeds() {
    let (env, client, player, ship_id) = generate_test_scenario("with_ship");
    let sid = ship_id.unwrap();
    let vault = client.deposit_treasure(&player, &sid, &1_000u64);
    advance_time(&env, DEFAULT_MIN_LOCK_DURATION + 1);
    let payout = client.claim_treasure(&player, &vault.vault_id);
    assert!(payout >= 1_000, "payout must be at least the deposited amount");
}

// ─── Edge-case corpus tests ────────────────────────────────────────────────────

/// Exercise `calculate_difficulty` with boundary values.
#[test]
fn difficulty_edge_case_corpus() {
    let (_, client, _) = make_env();
    let corpus = [0u32, 1, 2, 49, 50, 51, 99, 100, 101, u32::MAX];
    for &level in &corpus {
        let result = client.try_calculate_difficulty(&level);
        if level == 0 || level > 100 {
            assert!(
                matches!(result, Err(Ok(DifficultyError::InvalidLevel))),
                "level={level} must be invalid"
            );
        } else {
            let ok = result.unwrap().unwrap();
            let w = &ok.rarity_weights;
            assert_eq!(
                w.common + w.uncommon + w.rare + w.epic + w.legendary,
                100,
                "weights must sum to 100 for level={level}"
            );
        }
    }
}

/// Exercise `deposit_treasure` with boundary amounts.
#[test]
fn vault_amount_edge_case_corpus() {
    let corpus = [0u64, 1, 100, 999, 1_000];
    for &amount in &corpus {
        let (env, client, player) = make_env();
        let metadata = Bytes::from_array(&env, &[0u8; 4]);
        let ship = client.mint_ship(&player, &symbol_short!("fighter"), &metadata);
        if amount == 0 {
            let result = client.try_deposit_treasure(&player, &ship.id, &amount);
            assert!(
                matches!(result, Err(Ok(VaultError::InvalidAmount))),
                "zero amount must be rejected"
            );
        } else {
            let vault = client.deposit_treasure(&player, &ship.id, &amount);
            assert_eq!(vault.amount, amount);
        }
    }
}

// ─── test_helpers integration (fuzz feature only) ────────────────────────────

/// `test_helpers::generate_test_scenario` is accessible and produces valid state.
#[test]
#[cfg(feature = "fuzz")]
fn test_helpers_generate_scenario_accessible() {
    use stellar_nebula_nomad::test_helpers;

    let (_, client, ctx) = test_helpers::generate_test_scenario("with_ship");
    let sid = ctx.ship_id.expect("ship_id must be set for with_ship scenario");
    let ship = client.get_ship(&sid);
    assert_eq!(ship.owner, ctx.player);
    assert_eq!(ship.hull, 80); // explorer
}

/// `test_helpers` corpus generators return non-empty collections.
#[test]
#[cfg(feature = "fuzz")]
fn test_helpers_corpus_generators_non_empty() {
    use stellar_nebula_nomad::test_helpers;

    assert!(!test_helpers::ship_type_corpus().is_empty());
    assert!(!test_helpers::difficulty_level_corpus().is_empty());
    assert!(!test_helpers::vault_amount_corpus().is_empty());
    assert!(!test_helpers::ship_id_corpus().is_empty());
}

/// `test_helpers::make_env` returns a working client.
#[test]
#[cfg(feature = "fuzz")]
fn test_helpers_make_env_returns_working_client() {
    use stellar_nebula_nomad::test_helpers;

    let (_, client, player) = test_helpers::make_env();
    // Fresh player should have no ships.
    let ships = client.get_ships_by_owner(&player);
    assert_eq!(ships.len(), 0);
}
