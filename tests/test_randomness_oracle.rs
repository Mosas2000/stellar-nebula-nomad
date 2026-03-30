#![cfg(test)]

use soroban_sdk::testutils::{Ledger, LedgerInfo};
use soroban_sdk::{BytesN, Env};
use stellar_nebula_nomad::{NebulaNomadContract, NebulaNomadContractClient};

fn setup() -> (Env, NebulaNomadContractClient<'static>) {
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
    (env, client)
}

fn advance_ledger(env: &Env, seq_delta: u32, ts_delta: u64) {
    let seq = env.ledger().sequence();
    let ts = env.ledger().timestamp();
    env.ledger().set(LedgerInfo {
        protocol_version: 22,
        sequence_number: seq + seq_delta,
        timestamp: ts + ts_delta,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 1000,
        max_entry_ttl: 10_000,
    });
}

#[test]
fn test_request_random_seed_returns_32_bytes() {
    let (env, client) = setup();
    let seed = client.request_random_seed();
    // BytesN<32> is always 32 bytes; just check it's not all zeros
    let zero = BytesN::from_array(&env, &[0u8; 32]);
    assert_ne!(seed, zero);
}

#[test]
fn test_different_blocks_produce_different_seeds() {
    let (env, client) = setup();
    let seed1 = client.request_random_seed();

    advance_ledger(&env, 1, 5);
    let seed2 = client.request_random_seed();

    assert_ne!(seed1, seed2, "Seeds from different blocks should differ");
}

#[test]
fn test_entropy_pool_grows() {
    let (env, client) = setup();

    assert_eq!(client.get_entropy_pool().len(), 0);

    client.request_random_seed();
    assert_eq!(client.get_entropy_pool().len(), 1);

    advance_ledger(&env, 1, 5);
    client.request_random_seed();
    assert_eq!(client.get_entropy_pool().len(), 2);

    advance_ledger(&env, 1, 5);
    client.request_random_seed();
    assert_eq!(client.get_entropy_pool().len(), 3);
}

#[test]
fn test_entropy_pool_caps_at_max() {
    let (env, client) = setup();

    // Generate 12 seeds (pool max is 10)
    for _ in 0..12u32 {
        advance_ledger(&env, 1, 5);
        client.request_random_seed();
    }

    let pool = client.get_entropy_pool();
    assert!(pool.len() <= 10, "Entropy pool should cap at 10");
}

#[test]
fn test_verify_valid_seed_passes_through() {
    let (env, client) = setup();
    let seed = BytesN::from_array(&env, &[42u8; 32]);
    let result = client.verify_and_fallback(&seed);
    assert_eq!(result, seed);
}

#[test]
fn test_verify_zero_seed_with_fallback() {
    let (env, client) = setup();

    // First generate a seed to populate the fallback
    let prev_seed = client.request_random_seed();

    // Now verify a zero seed - should fall back
    let zero = BytesN::from_array(&env, &[0u8; 32]);
    let result = client.verify_and_fallback(&zero);
    assert_eq!(result, prev_seed);
}

#[test]
fn test_verify_zero_seed_no_fallback_fails() {
    let (env, client) = setup();

    // No previous seed generated - fallback should fail
    let zero = BytesN::from_array(&env, &[0u8; 32]);
    let result = client.try_verify_and_fallback(&zero);
    assert!(result.is_err());
}

#[test]
fn test_seed_uniqueness_across_many_blocks() {
    let (env, client) = setup();
    let mut seeds: soroban_sdk::Vec<BytesN<32>> = soroban_sdk::Vec::new(&env);

    for i in 0..20u32 {
        advance_ledger(&env, 1, 5);
        let seed = client.request_random_seed();
        // Check no duplicate among previous seeds
        for j in 0..seeds.len() {
            assert_ne!(
                seeds.get(j).unwrap(),
                seed,
                "Seed at block {} should be unique",
                i
            );
        }
        seeds.push_back(seed);
    }
}

#[test]
fn test_multi_block_mixing_changes_output() {
    let (env, client) = setup();

    // Generate seed at block 100 with empty pool
    let seed_fresh = client.request_random_seed();

    // Reset to same block to test mixing effect (pool now has 1 entry)
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

    let seed_with_pool = client.request_random_seed();

    // Even with same block params, the pool makes the output different
    assert_ne!(
        seed_fresh, seed_with_pool,
        "Entropy pool should change the seed even at the same block"
    );
}
