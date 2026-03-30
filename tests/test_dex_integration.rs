#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{symbol_short, Address, Bytes, BytesN, Env};
use stellar_nebula_nomad::{NebulaLayout, NebulaNomadContract, NebulaNomadContractClient};

fn setup() -> (Env, NebulaNomadContractClient<'static>, Address) {
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

/// Mint a ship and generate a layout for testing.
fn mint_and_layout(
    env: &Env,
    client: &NebulaNomadContractClient,
    player: &Address,
) -> (u64, NebulaLayout) {
    let metadata = Bytes::from_slice(env, &[0u8; 4]);
    let ship = client.mint_ship(player, &symbol_short!("explorer"), &metadata);
    let seed = BytesN::from_array(env, &[42u8; 32]);
    let layout = client.generate_nebula_layout(&seed, player);
    (ship.id, layout)
}

#[test]
fn test_harvest_and_list_creates_offer() {
    let (env, client, player) = setup();
    let (ship_id, layout) = mint_and_layout(&env, &client, &player);

    let resource = symbol_short!("dust");
    let (harvest, offer) = client.harvest_and_list(&player, &ship_id, &layout, &resource, &100i128);
    assert!(harvest.total_harvested > 0);
    assert!(offer.active);
    assert_eq!(offer.asset_id, resource);
    assert_eq!(offer.min_price, 100);
}

#[test]
fn test_cancel_listing_deactivates_offer() {
    let (env, client, player) = setup();
    let (ship_id, layout) = mint_and_layout(&env, &client, &player);

    let resource = symbol_short!("dust");
    let (_, offer) = client.harvest_and_list(&player, &ship_id, &layout, &resource, &50i128);

    let cancelled = client.cancel_listing(&player, &offer.offer_id);
    assert!(!cancelled.active);
}

#[test]
fn test_cancel_already_cancelled_fails() {
    let (env, client, player) = setup();
    let (ship_id, layout) = mint_and_layout(&env, &client, &player);

    let resource = symbol_short!("dust");
    let (_, offer) = client.harvest_and_list(&player, &ship_id, &layout, &resource, &50i128);

    // First cancel succeeds
    client.cancel_listing(&player, &offer.offer_id);
    // Second cancel should fail
    let result = client.try_cancel_listing(&player, &offer.offer_id);
    assert!(result.is_err());
}

#[test]
fn test_harvest_and_list_invalid_price_fails() {
    let (env, client, player) = setup();
    let (ship_id, layout) = mint_and_layout(&env, &client, &player);

    let resource = symbol_short!("dust");
    let result = client.try_harvest_and_list(&player, &ship_id, &layout, &resource, &0i128);
    assert!(result.is_err());
}

#[test]
fn test_burst_limit_enforced() {
    let (env, client, player) = setup();
    let metadata = Bytes::from_slice(&env, &[0u8; 4]);
    let resource = symbol_short!("dust");

    // Create 5 ships, each with its own layout
    for i in 0..5u8 {
        let ship = client.mint_ship(&player, &symbol_short!("explorer"), &metadata);
        let mut seed_bytes = [0u8; 32];
        seed_bytes[0] = i + 1;
        let seed = BytesN::from_array(&env, &seed_bytes);
        let layout = client.generate_nebula_layout(&seed, &player);
        // Should succeed (try_ returns Result)
        let result = client.try_harvest_and_list(&player, &ship.id, &layout, &resource, &10i128);
        assert!(result.is_ok(), "Listing {} should succeed", i);
    }

    // 6th listing should fail due to session limit
    let ship6 = client.mint_ship(&player, &symbol_short!("explorer"), &metadata);
    let seed6 = BytesN::from_array(&env, &[99u8; 32]);
    let layout6 = client.generate_nebula_layout(&seed6, &player);
    let result = client.try_harvest_and_list(&player, &ship6.id, &layout6, &resource, &10i128);
    assert!(result.is_err());
}
