use stellar_nebula_nomad::{NebulaNomadContract, NebulaNomadContractClient};
use stellar_nebula_nomad::{
    open_wormhole, traverse_wormhole, get_wormhole, get_active_wormholes,
    get_travel_history, cleanup_expired_wormholes, calculate_travel_cost,
    verify_wormhole_link, WormholeError,
    MAX_SIMULTANEOUS_WORMHOLES, WORMHOLE_LIFETIME_SECS,
};
use soroban_sdk::{symbol_short, Bytes, BytesN, Env, Vec};
use soroban_sdk::testutils::{Ledger, Address as TestAddress};

#[test]
fn test_open_wormhole_success() {
    let env = Env::default();
    env.mock_all_auths();
    
    let creator = <soroban_sdk::Address as TestAddress>::generate(&env);
    let origin_nebula = 100u64;
    let destination = 200u64;
    
    let wormhole_id = open_wormhole(
        &env,
        creator.clone(),
        origin_nebula,
        destination,
    ).unwrap();
    
    assert_eq!(wormhole_id, 0);
    
    let wormhole = get_wormhole(&env, wormhole_id).unwrap();
    assert_eq!(wormhole.wormhole_id, wormhole_id);
    assert_eq!(wormhole.origin_nebula, origin_nebula);
    assert_eq!(wormhole.destination, destination);
    assert_eq!(wormhole.creator, creator);
    assert!(wormhole.is_active);
    assert!(wormhole.travel_cost > 0);
    
    let active_wormholes = get_active_wormholes(&env);
    assert_eq!(active_wormholes.len(), 1);
    assert_eq!(active_wormholes.get(0).unwrap(), wormhole_id);
    
    // Additional assertions
    assert_eq!(wormhole.wormhole_id, wormhole_id);
    assert_eq!(wormhole.origin_nebula, origin_nebula);
    assert_eq!(wormhole.destination, destination);
    assert_eq!(wormhole.creator, creator);
    assert!(wormhole.is_active);
    
    let active_wormholes = get_active_wormholes(&env);
    assert_eq!(active_wormholes.len(), 1);
    assert_eq!(active_wormholes.get(0).unwrap(), wormhole_id);
}

#[test]
fn test_open_wormhole_same_nebula_error() {
    let env = Env::default();
    env.mock_all_auths();
    
    let creator = <soroban_sdk::Address as TestAddress>::generate(&env);
    let nebula_id = 100u64;
    
    let wormhole_id = open_wormhole(
        &env,
        creator,
        nebula_id,
        nebula_id,
    );
    
    assert_eq!(wormhole_id.unwrap_err(), WormholeError::SameNebulaTravel);
}

#[test]
fn test_open_wormhole_max_limit() {
    let env = Env::default();
    env.mock_all_auths();
    
    let creator = <soroban_sdk::Address as TestAddress>::generate(&env);
    
    // Open maximum number of wormholes
    for i in 0..MAX_SIMULTANEOUS_WORMHOLES {
        let wormhole_id = open_wormhole(
            &env,
            creator.clone(),
            i as u64,
            (i + 100) as u64,
        );
        assert!(wormhole_id.is_ok());
    }
    
    // Try to open one more - should fail
    let wormhole_id = open_wormhole(
        &env,
        creator,
        1000,
        1100,
    );
    
    assert_eq!(wormhole_id.unwrap_err(), WormholeError::MaxWormholesReached);
}

#[test]
fn test_traverse_wormhole_success() {
    let env = Env::default();
    env.mock_all_auths();
    
    let creator = <soroban_sdk::Address as TestAddress>::generate(&env);
    let traveler = <soroban_sdk::Address as TestAddress>::generate(&env);
    let origin_nebula = 100u64;
    let destination = 200u64;
    
    // Set up contract client
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    // Mint a ship for the traveler
    let ship = client.mint_ship(
        &traveler,
        &symbol_short!("explorer"),
        &Bytes::from_slice(&env, &[0u8; 32]),
    );
    
    // Open a wormhole
    let wormhole_id = open_wormhole(
        &env,
        creator,
        origin_nebula,
        destination,
    ).unwrap();
    
    // Traverse the wormhole
    let travel_record = traverse_wormhole(
        &env,
        traveler,
        ship.id,
        wormhole_id,
    ).unwrap();
    
    assert_eq!(travel_record.ship_id, ship.id);
    assert_eq!(travel_record.wormhole_id, wormhole_id);
    assert_eq!(travel_record.origin_nebula, origin_nebula);
    assert_eq!(travel_record.destination, destination);
    assert!(travel_record.energy_consumed > 0);
    
    // Check travel history
    let travel_history = get_travel_history(&env, ship.id);
    assert_eq!(travel_history.len(), 1);
    assert_eq!(travel_history.get(0).unwrap().wormhole_id, wormhole_id);
}

#[test]
fn test_traverse_wormhole_insufficient_energy() {
    let env = Env::default();
    env.mock_all_auths();
    
    let creator = <soroban_sdk::Address as TestAddress>::generate(&env);
    let traveler = <soroban_sdk::Address as TestAddress>::generate(&env);
    let origin_nebula = 100u64;
    let destination = 200u64;
    
    // Mint a ship for the traveler
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    let ship = client.mint_ship(
        &traveler,
        &symbol_short!("explorer"),
        &Bytes::from_slice(&env, &[0u8; 32]),
    );
    
    // Open a wormhole with high cost
    let wormhole_id = open_wormhole(
        &env,
        creator,
        origin_nebula,
        10000u64, // Very distant destination = high cost
    ).unwrap();
    
    // Try to traverse without sufficient energy
    let result = traverse_wormhole(
        &env,
        traveler,
        ship.id,
        wormhole_id,
    );
    
    assert_eq!(result.unwrap_err(), WormholeError::InsufficientEnergy);
}

#[test]
fn test_traverse_wormhole_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    
    let creator = <soroban_sdk::Address as TestAddress>::generate(&env);
    let ship_owner = <soroban_sdk::Address as TestAddress>::generate(&env);
    let unauthorized_traveler = <soroban_sdk::Address as TestAddress>::generate(&env);
    let origin_nebula = 100u64;
    let destination = 200u64;
    
    // Mint a ship for the owner
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    let ship = client.mint_ship(
        &ship_owner,
        &symbol_short!("explorer"),
        &Bytes::from_slice(&env, &[0u8; 32]),
    );
    
    // Open a wormhole
    let wormhole_id = open_wormhole(
        &env,
        creator,
        origin_nebula,
        destination,
    ).unwrap();
    
    // Try to traverse with unauthorized traveler
    let result = traverse_wormhole(
        &env,
        unauthorized_traveler,
        ship.id,
        wormhole_id,
    );
    
    assert_eq!(result.unwrap_err(), WormholeError::UnauthorizedTravel);
}

#[test]
fn test_traverse_wormhole_not_found() {
    let env = Env::default();
    env.mock_all_auths();
    
    let traveler = <soroban_sdk::Address as TestAddress>::generate(&env);
    
    // Mint a ship for the traveler
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    let ship = client.mint_ship(
        &traveler,
        &symbol_short!("explorer"),
        &Bytes::from_slice(&env, &[0u8; 32]),
    );
    
    // Try to traverse non-existent wormhole
    let result = traverse_wormhole(
        &env,
        traveler,
        ship.id,
        999,
    );
    
    assert_eq!(result.unwrap_err(), WormholeError::WormholeNotFound);
}

#[test]
fn test_calculate_travel_cost() {
    let env = Env::default();
    
    // Same nebula should cost 0
    assert_eq!(calculate_travel_cost(100, 100), 0);
    
    // Different nebulae should have positive cost
    let cost = calculate_travel_cost(100, 200);
    assert!(cost > 0);
    
    // Distance should affect cost
    let cost_close = calculate_travel_cost(100, 150);
    let cost_far = calculate_travel_cost(100, 500);
    assert!(cost_far > cost_close);
}

#[test]
fn test_verify_wormhole_link() {
    let env = Env::default();
    env.mock_all_auths();
    
    let creator = <soroban_sdk::Address as TestAddress>::generate(&env);
    let origin_nebula = 100u64;
    let destination = 200u64;
    
    // Open a wormhole
    let wormhole_id = open_wormhole(
        &env,
        creator,
        origin_nebula,
        destination,
    ).unwrap();
    
    let wormhole = get_wormhole(&env, wormhole_id).unwrap();
    
    // Verify correct link
    assert!(verify_wormhole_link(&env, wormhole_id, wormhole.verifiable_link.clone()));
    
    // Verify incorrect link
    let fake_link = BytesN::from_array(&env, &[255u8; 32]);
    assert!(!verify_wormhole_link(&env, wormhole_id, fake_link));
    
    // Verify non-existent wormhole
    assert!(!verify_wormhole_link(&env, 999, wormhole.verifiable_link));
}

#[test]
fn test_cleanup_expired_wormholes() {
    let env = Env::default();
    env.mock_all_auths();
    
    let creator = <soroban_sdk::Address as TestAddress>::generate(&env);
    
    // Open some wormholes
    let wormhole1 = open_wormhole(
        &env,
        creator.clone(),
        100,
        200,
    ).unwrap();
    
    let wormhole2 = open_wormhole(
        &env,
        creator.clone(),
        300,
        400,
    ).unwrap();
    
    // Verify both are active
    let active = get_active_wormholes(&env);
    assert_eq!(active.len(), 2);
    
    // Fast-forward time beyond wormhole lifetime
    env.ledger().set_timestamp(env.ledger().timestamp() + WORMHOLE_LIFETIME_SECS + 100);
    
    // Cleanup expired wormholes
    let cleaned_count = cleanup_expired_wormholes(&env);
    assert_eq!(cleaned_count, 2);
    
    // Verify no active wormholes remain
    let active = get_active_wormholes(&env);
    assert_eq!(active.len(), 0);
    
    // Verify wormholes are marked as inactive
    let w1 = get_wormhole(&env, wormhole1).unwrap();
    let w2 = get_wormhole(&env, wormhole2).unwrap();
    assert!(!w1.is_active);
    assert!(!w2.is_active);
}

#[test]
fn test_travel_history_limit() {
    let env = Env::default();
    env.mock_all_auths();
    
    let creator = <soroban_sdk::Address as TestAddress>::generate(&env);
    let traveler = <soroban_sdk::Address as TestAddress>::generate(&env);
    
    // Mint a ship for the traveler
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    let ship = client.mint_ship(
        &traveler,
        &symbol_short!("explorer"),
        &Bytes::from_slice(&env, &[0u8; 32]),
    );
    
    // Create many wormholes and traverse them
    for i in 0..150 {
        let wormhole_id = open_wormhole(
            &env,
            creator.clone(),
            i as u64,
            (i + 100) as u64,
        ).unwrap();
        
        // Recharge energy for each traversal
        env.ledger().set_timestamp(env.ledger().timestamp() + 1);
        
        let _ = traverse_wormhole(
            &env,
            traveler.clone(),
            ship.id,
            wormhole_id,
        ).unwrap();
    }
    
    // Check that travel history is limited to 100 entries
    let travel_history = get_travel_history(&env, ship.id);
    assert_eq!(travel_history.len(), 100);
    
    // Verify the most recent entries are kept
    let last_record = travel_history.get(99).unwrap();
    assert_eq!(last_record.wormhole_id, 149);
}

#[test]
fn test_wormhole_verifiable_link_uniqueness() {
    let env = Env::default();
    env.mock_all_auths();
    
    let creator = <soroban_sdk::Address as TestAddress>::generate(&env);
    let origin_nebula = 100u64;
    let destination = 200u64;
    
    // Open two identical wormholes at different times
    let wormhole1 = open_wormhole(
        &env,
        creator.clone(),
        origin_nebula,
        destination,
    ).unwrap();
    
    // Advance time
    env.ledger().set_timestamp(env.ledger().timestamp() + 10);
    
    let wormhole2 = open_wormhole(
        &env,
        creator,
        origin_nebula,
        destination,
    ).unwrap();
    
    let w1 = get_wormhole(&env, wormhole1).unwrap();
    let w2 = get_wormhole(&env, wormhole2).unwrap();
    
    // Links should be different due to different timestamps
    assert_ne!(w1.verifiable_link, w2.verifiable_link);
}

#[test]
fn test_multi_nebula_journey_simulation() {
    let env = Env::default();
    env.mock_all_auths();
    
    let creator = <soroban_sdk::Address as TestAddress>::generate(&env);
    let traveler = <soroban_sdk::Address as TestAddress>::generate(&env);
    
    // Mint a ship for the traveler
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    let ship = client.mint_ship(
        &traveler,
        &symbol_short!("explorer"),
        &Bytes::from_slice(&env, &[0u8; 32]),
    );
    
    // Create a journey through multiple nebulae
    let nebulae = vec![100u64, 200, 300, 400, 500];
    let mut wormholes = Vec::new(&env);
    
    // Open wormholes for the journey
    for i in 0..nebulae.len() - 1 {
    let wormhole_id = open_wormhole(
        &env,
        creator.clone(),
            *nebulae.get(i).unwrap(),
            *nebulae.get(i + 1).unwrap(),
        ).unwrap();
        wormholes.push_back(wormhole_id);
    }
    
    // Traverse the journey
    for (i, wormhole_id) in wormholes.iter().enumerate() {
        // Recharge energy between jumps
        env.ledger().set_timestamp(env.ledger().timestamp() + i as u64 + 1);
        
        let travel_record = traverse_wormhole(
            &env,
            traveler.clone(),
            ship.id,
            wormhole_id,
        ).unwrap();
        
        assert_eq!(travel_record.origin_nebula, *nebulae.get(i).unwrap());
        assert_eq!(travel_record.destination, *nebulae.get(i + 1).unwrap());
    }
    
    // Verify complete travel history
    let travel_history = get_travel_history(&env, ship.id);
    assert_eq!(travel_history.len(), (nebulae.len() - 1) as u32);
    
    for (i, record) in travel_history.iter().enumerate() {
        assert_eq!(record.origin_nebula, *nebulae.get(i).unwrap());
        assert_eq!(record.destination, *nebulae.get(i + 1).unwrap());
    }
}
