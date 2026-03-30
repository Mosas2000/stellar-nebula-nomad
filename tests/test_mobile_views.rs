#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{symbol_short, Address, Bytes, Env};
use stellar_nebula_nomad::{MobileViewError, NebulaNomadContract, NebulaNomadContractClient};

fn setup_env() -> (Env, NebulaNomadContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(LedgerInfo {
        protocol_version: 22,
        sequence_number: 100,
        timestamp: 1_700_000_000,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 1000,
        max_entry_ttl: 10_000,
    });
    let contract_id = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    let player = Address::generate(&env);
    (env, client, player)
}

// ─── get_mobile_dashboard ─────────────────────────────────────────────────────

/// A player who has never interacted with the contract gets a zero-filled
/// dashboard — no panic, no error.
#[test]
fn dashboard_empty_state_returns_defaults() {
    let (_, client, player) = setup_env();
    let dash = client.get_mobile_dashboard(&player);

    assert_eq!(dash.player, player);
    assert!(!dash.has_profile);
    assert_eq!(dash.total_scans, 0);
    assert_eq!(dash.essence_earned, 0);
    assert_eq!(dash.ship_count, 0);
    assert_eq!(dash.primary_ship_id, 0);
    assert_eq!(dash.primary_hull, 0);
    assert_eq!(dash.primary_scanner_power, 0);
    assert_eq!(dash.dust_balance, 0);
    assert_eq!(dash.ore_balance, 0);
    assert_eq!(dash.gas_balance, 0);
}

/// After creating a profile the dashboard reflects the profile data.
#[test]
fn dashboard_reflects_profile_after_initialize() {
    let (_, client, player) = setup_env();
    client.initialize_profile(&player);

    let dash = client.get_mobile_dashboard(&player);
    assert!(dash.has_profile);
    assert_eq!(dash.total_scans, 0);
    assert_eq!(dash.essence_earned, 0);
}

/// After minting a ship the dashboard reflects ship count and primary stats.
#[test]
fn dashboard_reflects_primary_ship_stats() {
    let (env, client, player) = setup_env();
    let metadata = Bytes::from_array(&env, &[0u8; 4]);

    // explorer: hull=80, scanner_power=50
    let ship = client.mint_ship(&player, &symbol_short!("explorer"), &metadata);

    let dash = client.get_mobile_dashboard(&player);
    assert_eq!(dash.ship_count, 1);
    assert_eq!(dash.primary_ship_id, ship.id);
    assert_eq!(dash.primary_hull, 80);
    assert_eq!(dash.primary_scanner_power, 50);
}

/// When the player owns multiple ships only the first ship is surfaced as the
/// primary — ship_count reflects the total.
#[test]
fn dashboard_ship_count_reflects_all_owned_ships() {
    let (env, client, player) = setup_env();
    let metadata = Bytes::from_array(&env, &[0u8; 4]);

    let first = client.mint_ship(&player, &symbol_short!("fighter"), &metadata);
    client.mint_ship(&player, &symbol_short!("hauler"), &metadata);
    client.mint_ship(&player, &symbol_short!("explorer"), &metadata);

    let dash = client.get_mobile_dashboard(&player);
    assert_eq!(dash.ship_count, 3);
    assert_eq!(dash.primary_ship_id, first.id);
    // fighter stats: hull=150, scanner_power=20
    assert_eq!(dash.primary_hull, 150);
    assert_eq!(dash.primary_scanner_power, 20);
}

/// Profile and ships together produce a complete dashboard.
#[test]
fn dashboard_combined_profile_and_ship() {
    let (env, client, player) = setup_env();
    let metadata = Bytes::from_array(&env, &[1u8; 4]);

    client.initialize_profile(&player);
    client.mint_ship(&player, &symbol_short!("hauler"), &metadata);

    let dash = client.get_mobile_dashboard(&player);
    assert!(dash.has_profile);
    assert_eq!(dash.ship_count, 1);
    // hauler stats: hull=200, scanner_power=10
    assert_eq!(dash.primary_hull, 200);
    assert_eq!(dash.primary_scanner_power, 10);
}

// ─── get_quick_scan_preview ───────────────────────────────────────────────────

/// Querying a non-existent ship returns ShipNotFound.
#[test]
fn quick_scan_preview_ship_not_found() {
    let (_, client, _) = setup_env();
    let result = client.try_get_quick_scan_preview(&999u64);
    assert_eq!(result, Err(Ok(MobileViewError::ShipNotFound)));
}

/// Explorer (scanner_power=50) gets Rare rarity (index 2) and energy bounds
/// that scale linearly with scanner power.
#[test]
fn quick_scan_preview_explorer_rarity_and_energy() {
    let (env, client, player) = setup_env();
    let metadata = Bytes::from_array(&env, &[0u8; 4]);
    let ship = client.mint_ship(&player, &symbol_short!("explorer"), &metadata);

    let preview = client.get_quick_scan_preview(&ship.id);
    assert_eq!(preview.ship_id, ship.id);
    assert_eq!(preview.scanner_power, 50);
    assert_eq!(preview.estimated_energy_min, 50 * 3);  // 150
    assert_eq!(preview.estimated_energy_max, 50 * 8);  // 400
    assert_eq!(preview.predicted_rarity_index, 2);     // Rare
}

/// Fighter (scanner_power=20) gets Uncommon rarity (index 1).
#[test]
fn quick_scan_preview_fighter_uncommon_rarity() {
    let (env, client, player) = setup_env();
    let metadata = Bytes::from_array(&env, &[0u8; 4]);
    let ship = client.mint_ship(&player, &symbol_short!("fighter"), &metadata);

    let preview = client.get_quick_scan_preview(&ship.id);
    assert_eq!(preview.scanner_power, 20);
    assert_eq!(preview.estimated_energy_min, 60);
    assert_eq!(preview.estimated_energy_max, 160);
    assert_eq!(preview.predicted_rarity_index, 1); // Uncommon
}

/// Hauler (scanner_power=10) gets Common rarity (index 0).
#[test]
fn quick_scan_preview_hauler_common_rarity() {
    let (env, client, player) = setup_env();
    let metadata = Bytes::from_array(&env, &[0u8; 4]);
    let ship = client.mint_ship(&player, &symbol_short!("hauler"), &metadata);

    let preview = client.get_quick_scan_preview(&ship.id);
    assert_eq!(preview.scanner_power, 10);
    assert_eq!(preview.estimated_energy_min, 30);
    assert_eq!(preview.estimated_energy_max, 80);
    assert_eq!(preview.predicted_rarity_index, 0); // Common
}

/// Mobile view functions complete within the Soroban instruction budget.
/// This test verifies both functions run end-to-end without hitting the
/// instruction limit, simulating a concurrent mobile call scenario.
#[test]
fn mobile_views_complete_within_budget() {
    let (env, client, player) = setup_env();
    let metadata = Bytes::from_array(&env, &[7u8; 4]);

    client.initialize_profile(&player);
    let ship = client.mint_ship(&player, &symbol_short!("explorer"), &metadata);

    // Both calls must succeed without panicking (budget exceeded would panic).
    let dash = client.get_mobile_dashboard(&player);
    let preview = client.get_quick_scan_preview(&ship.id);

    assert!(dash.has_profile);
    assert_eq!(preview.ship_id, ship.id);
}
