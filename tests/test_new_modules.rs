#![cfg(test)]

use soroban_sdk::{testutils::{Address as _, Ledger}, Address, BytesN, Env, Vec};
use stellar_nebula_nomad::{NebulaNomadContract, NebulaNomadContractClient};

// ─── Prize Distributor Tests (Issue #62) ─────────────────────────────────────

#[test]
fn test_prize_fund_and_pool_balance() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize_prize_distributor(&admin);

    assert_eq!(client.get_prize_pool(), 0i128);

    let sponsor = Address::generate(&env);
    let new_balance = client.fund_prize_pool(&sponsor, &1_000i128);
    assert_eq!(new_balance, 1_000i128);
    assert_eq!(client.get_prize_pool(), 1_000i128);
}

#[test]
fn test_prize_fund_invalid_amount_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize_prize_distributor(&admin);

    let result = client.try_fund_prize_pool(&admin, &0i128);
    assert!(result.is_err());
}

#[test]
fn test_prize_weekly_cycle() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize_prize_distributor(&admin);
    client.fund_prize_pool(&admin, &1_000i128);

    // Build winner list (3 addresses, rank 1-3)
    let w1 = Address::generate(&env);
    let w2 = Address::generate(&env);
    let w3 = Address::generate(&env);
    let mut winners: Vec<Address> = Vec::new(&env);
    winners.push_back(w1);
    winners.push_back(w2);
    winners.push_back(w3);

    let snapshot_size = client.submit_leaderboard_snapshot(&admin, &winners);
    assert_eq!(snapshot_size, 3u32);

    let records = client.distribute_weekly_prizes(&admin, &3u32);
    assert_eq!(records.len(), 3u32);

    // Pool should be (nearly) empty after full distribution.
    let remaining = client.get_prize_pool();
    assert!(remaining < 10i128, "pool={remaining}");

    // Total distributed matches funded amount minus dust.
    let total = client.get_total_distributed();
    assert!(total >= 990i128, "total={total}");
}

#[test]
fn test_prize_insufficient_pool_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize_prize_distributor(&admin);
    // Pool is 0 — distribution should error.

    let w1 = Address::generate(&env);
    let mut winners: Vec<Address> = Vec::new(&env);
    winners.push_back(w1);
    client.submit_leaderboard_snapshot(&admin, &winners);

    let result = client.try_distribute_weekly_prizes(&admin, &1u32);
    assert!(result.is_err());
}

#[test]
fn test_prize_no_snapshot_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize_prize_distributor(&admin);
    client.fund_prize_pool(&admin, &1_000i128);

    // No snapshot submitted → should error.
    let result = client.try_distribute_weekly_prizes(&admin, &1u32);
    assert!(result.is_err());
}

// ─── Portal Registry Tests (Issue #71) ───────────────────────────────────────

#[test]
fn test_portal_register_and_query() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize_portal_registry(&admin);

    let owner = Address::generate(&env);
    let portal_id = client.register_portal(&owner, &1u64, &2u64);
    assert_eq!(portal_id, 0u64);

    let (stability, cost) = client.query_portal_status(&portal_id);
    assert_eq!(stability, 100u32);
    // cost at 100 % stability = BASE_TRAVEL_COST * 100 / 100 = 100
    assert_eq!(cost, 100i128);
}

#[test]
fn test_portal_decay_reduces_stability() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize_portal_registry(&admin);
    let owner = Address::generate(&env);
    let portal_id = client.register_portal(&owner, &10u64, &20u64);

    // Advance 5 days: 5 * 86400 seconds.
    env.ledger().set_timestamp(5 * 86_400);

    let (stability, _cost) = client.query_portal_status(&portal_id);
    // Expected: 100 - 5*2 = 90
    assert_eq!(stability, 90u32);
}

#[test]
fn test_portal_unstable_travel_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize_portal_registry(&admin);
    let owner = Address::generate(&env);
    let portal_id = client.register_portal(&owner, &5u64, &6u64);

    // Advance 50 days → stability = 0 (below MIN_STABLE_PCT=10).
    env.ledger().set_timestamp(50 * 86_400);

    let result = client.try_travel_through_portal(&portal_id);
    assert!(result.is_err());
}

#[test]
fn test_portal_refresh_restores_stability() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize_portal_registry(&admin);
    let owner = Address::generate(&env);
    let portal_id = client.register_portal(&owner, &7u64, &8u64);

    // Decay for 20 days → stability = 60.
    env.ledger().set_timestamp(20 * 86_400);
    let (stability_before, _) = client.query_portal_status(&portal_id);
    assert_eq!(stability_before, 60u32);

    // Refresh → back to 100.
    client.refresh_portal(&owner, &portal_id);
    let (stability_after, _) = client.query_portal_status(&portal_id);
    assert_eq!(stability_after, 100u32);
}

#[test]
fn test_portal_same_nebula_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize_portal_registry(&admin);
    let owner = Address::generate(&env);

    let result = client.try_register_portal(&owner, &42u64, &42u64);
    assert!(result.is_err());
}

// ─── Constellation Mapper Tests (Issue #72) ───────────────────────────────────

fn make_star(env: &Env, val: u8) -> BytesN<32> {
    BytesN::from_array(env, &[val; 32])
}

#[test]
fn test_constellation_record_and_count() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let mut stars: Vec<BytesN<32>> = Vec::new(&env);
    stars.push_back(make_star(&env, 1));
    stars.push_back(make_star(&env, 2));
    stars.push_back(make_star(&env, 3));

    let id = client.record_constellation(&user, &stars);
    assert_eq!(id, 0u64);
    assert_eq!(client.get_constellation_count(), 1u64);
}

#[test]
fn test_constellation_too_few_stars_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let mut stars: Vec<BytesN<32>> = Vec::new(&env);
    stars.push_back(make_star(&env, 1));
    stars.push_back(make_star(&env, 2));
    // Only 2 stars, MIN is 3.

    let result = client.try_record_constellation(&user, &stars);
    assert!(result.is_err());
}

#[test]
fn test_constellation_match_finds_best() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);

    // Record constellation A: stars 1,2,3
    let mut a: Vec<BytesN<32>> = Vec::new(&env);
    a.push_back(make_star(&env, 1));
    a.push_back(make_star(&env, 2));
    a.push_back(make_star(&env, 3));
    let id_a = client.record_constellation(&user, &a);

    // Record constellation B: stars 4,5,6
    let mut b: Vec<BytesN<32>> = Vec::new(&env);
    b.push_back(make_star(&env, 4));
    b.push_back(make_star(&env, 5));
    b.push_back(make_star(&env, 6));
    client.record_constellation(&user, &b);

    // Observe stars 1,2,3,7 — should match A (3 overlapping).
    let mut observed: Vec<BytesN<32>> = Vec::new(&env);
    observed.push_back(make_star(&env, 1));
    observed.push_back(make_star(&env, 2));
    observed.push_back(make_star(&env, 3));
    observed.push_back(make_star(&env, 7));

    let result = client.match_constellation(&observed);
    assert_eq!(result.constellation_id, id_a);
    assert_eq!(result.matched_stars, 3u32);
}

#[test]
fn test_constellation_no_match_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    // No constellations recorded — match should error.
    let mut observed: Vec<BytesN<32>> = Vec::new(&env);
    observed.push_back(make_star(&env, 10));
    observed.push_back(make_star(&env, 11));
    observed.push_back(make_star(&env, 12));

    let result = client.try_match_constellation(&observed);
    assert!(result.is_err());
}

// ─── Entanglement Comms Tests (Issue #73) ────────────────────────────────────

fn zero_message(env: &Env) -> BytesN<64> {
    BytesN::from_array(env, &[0u8; 64])
}

#[test]
fn test_entanglement_pair_creation() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let owner_a = Address::generate(&env);
    let owner_b = Address::generate(&env);
    let pair_id = client.create_entanglement_pair(&owner_a, &1u64, &owner_b, &2u64);
    assert_eq!(pair_id, 0u64);
}

#[test]
fn test_entanglement_same_ship_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let owner_a = Address::generate(&env);
    let owner_b = Address::generate(&env);

    let result = client.try_create_entanglement_pair(&owner_a, &5u64, &owner_b, &5u64);
    assert!(result.is_err());
}

#[test]
fn test_entanglement_send_message() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let owner_a = Address::generate(&env);
    let owner_b = Address::generate(&env);
    let pair_id = client.create_entanglement_pair(&owner_a, &1u64, &owner_b, &2u64);

    let seq = client.send_entangled_message(&owner_a, &pair_id, &zero_message(&env));
    assert_eq!(seq, 1u64);

    let seq2 = client.send_entangled_message(&owner_b, &pair_id, &zero_message(&env));
    assert_eq!(seq2, 2u64);

    assert_eq!(client.get_message_count(&pair_id), 2u64);
}

#[test]
fn test_entanglement_expired_pair_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let owner_a = Address::generate(&env);
    let owner_b = Address::generate(&env);
    let pair_id = client.create_entanglement_pair(&owner_a, &1u64, &owner_b, &2u64);

    // Advance past 30-day lifetime.
    env.ledger().set_timestamp(31 * 86_400);

    let result = client.try_send_entangled_message(&owner_a, &pair_id, &zero_message(&env));
    assert!(result.is_err());
}

#[test]
fn test_entanglement_dissolve_stops_messages() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let owner_a = Address::generate(&env);
    let owner_b = Address::generate(&env);
    let pair_id = client.create_entanglement_pair(&owner_a, &1u64, &owner_b, &2u64);

    client.dissolve_pair(&owner_a, &pair_id);

    let result = client.try_send_entangled_message(&owner_b, &pair_id, &zero_message(&env));
    assert!(result.is_err());
}

#[test]
fn test_entanglement_unauthorized_send_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);

    let owner_a = Address::generate(&env);
    let owner_b = Address::generate(&env);
    let outsider = Address::generate(&env);
    let pair_id = client.create_entanglement_pair(&owner_a, &1u64, &owner_b, &2u64);

    let result = client.try_send_entangled_message(&outsider, &pair_id, &zero_message(&env));
    assert!(result.is_err());
}
