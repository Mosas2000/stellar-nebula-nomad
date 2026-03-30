//! Test helpers and scenario builders for Nebula Nomad smart contracts.
//!
//! Available when the `fuzz` feature is enabled or during `cargo test`:
//! ```
//! cargo test --features fuzz
//! ```

extern crate std;

use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{symbol_short, Address, Bytes, Env};

use crate::{NebulaNomadContract, NebulaNomadContractClient};

// ─── Scenario Context ─────────────────────────────────────────────────────────

/// State produced by [`generate_test_scenario`].
///
/// Contains the player address and optional IDs created as part of the
/// scenario setup, so fuzz tests can reference them without needing to
/// re-query storage.
#[derive(Clone)]
pub struct ScenarioContext {
    /// The primary player address for this scenario.
    pub player: Address,
    /// Ship ID if the scenario minted a ship, else `None`.
    pub ship_id: Option<u64>,
    /// Profile ID if the scenario created a profile, else `None`.
    pub profile_id: Option<u64>,
    /// Vault ID if the scenario created a vault, else `None`.
    pub vault_id: Option<u64>,
    /// Session ID if the scenario started a session, else `None`.
    pub session_id: Option<u64>,
}

// ─── Environment Setup ────────────────────────────────────────────────────────

/// Create a fresh [`Env`] with deterministic ledger settings and return it
/// together with a registered contract client and a generated player address.
///
/// This is the canonical setup used by all fuzz helpers and can be called
/// directly by hand-written property tests.
pub fn make_env() -> (Env, NebulaNomadContractClient<'static>, Address) {
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

/// Advance the ledger timestamp by `seconds` (keeps sequence monotonically
/// increasing). Use this in vault / session expiry tests.
pub fn advance_time(env: &Env, seconds: u64) {
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

/// Build a reproducible on-chain test scenario identified by `scenario_type`.
///
/// Returns `(Env, client, ScenarioContext)`. The environment is freshly
/// initialized for every call, giving full isolation between fuzz iterations.
///
/// # Supported scenario types
///
/// | `scenario_type` | What gets created |
/// |-----------------|-------------------|
/// | `"empty"`       | Fresh contract — no state |
/// | `"with_ship"`   | Player + one explorer ship |
/// | `"with_profile"`| Player + profile |
/// | `"full"`        | Player + profile + explorer ship + treasure vault |
///
/// Unknown values fall back to `"empty"`.
pub fn generate_test_scenario(
    scenario_type: &str,
) -> (Env, NebulaNomadContractClient<'static>, ScenarioContext) {
    let (env, client, player) = make_env();
    let metadata = Bytes::from_array(&env, &[0u8; 4]);

    let mut ctx = ScenarioContext {
        player: player.clone(),
        ship_id: None,
        profile_id: None,
        vault_id: None,
        session_id: None,
    };

    match scenario_type {
        "with_ship" => {
            let ship = client.mint_ship(&player, &symbol_short!("explorer"), &metadata);
            ctx.ship_id = Some(ship.id);
        }

        "with_profile" => {
            let pid = client.initialize_profile(&player);
            ctx.profile_id = Some(pid);
        }

        "full" => {
            let pid = client.initialize_profile(&player);
            ctx.profile_id = Some(pid);

            let ship = client.mint_ship(&player, &symbol_short!("explorer"), &metadata);
            ctx.ship_id = Some(ship.id);

            let vault = client.deposit_treasure(&player, &ship.id, &100u64);
            ctx.vault_id = Some(vault.vault_id);
        }

        // "empty" or any unknown value → no state
        _ => {}
    }

    (env, client, ctx)
}

// ─── Edge-case input generators ──────────────────────────────────────────────

/// Returns ship-type strings that cover all valid and several invalid cases.
///
/// Valid: `"fighter"`, `"explorer"`, `"hauler"`
/// Invalid: `"unknown"`, `"ghost"`, `""` (empty symbol via a placeholder)
pub fn ship_type_corpus() -> std::vec::Vec<&'static str> {
    std::vec!["fighter", "explorer", "hauler", "unknown", "ghost", "turret"]
}

/// Returns a set of player-level values that exercise boundary conditions in
/// `calculate_difficulty`:
///
/// - `1` (minimum valid)
/// - `50` (midpoint)
/// - `100` (maximum valid — `MAX_LEVEL`)
/// - `0`, `101` (both invalid)
pub fn difficulty_level_corpus() -> std::vec::Vec<u32> {
    std::vec![0, 1, 2, 49, 50, 51, 99, 100, 101, u32::MAX]
}

/// Returns deposit amounts for treasure vault fuzzing, including zero and
/// overflow-adjacent values.
pub fn vault_amount_corpus() -> std::vec::Vec<u64> {
    std::vec![0, 1, 100, 999, 1_000, u64::MAX / 2, u64::MAX]
}

/// Returns a set of ship IDs that cover existing (1) and non-existing cases.
pub fn ship_id_corpus() -> std::vec::Vec<u64> {
    std::vec![0, 1, 2, 999, u64::MAX]
}
