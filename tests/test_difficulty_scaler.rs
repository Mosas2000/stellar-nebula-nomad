#![cfg(test)]

use soroban_sdk::testutils::{Ledger, LedgerInfo};
use soroban_sdk::Env;
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

#[test]
fn test_difficulty_level_1() {
    let (_env, client) = setup();
    let result = client.calculate_difficulty(&1u32);
    assert_eq!(result.anomaly_count, 5); // BASE + (1-1)*2 = 5
    assert_eq!(result.difficulty_multiplier, 100);
    // At level 1, common should be high
    assert!(result.rarity_weights.common >= 50);
}

#[test]
fn test_difficulty_level_50() {
    let (_env, client) = setup();
    let result = client.calculate_difficulty(&50u32);
    // 5 + 49*2 = 103
    assert_eq!(result.anomaly_count, 103);
    // Mid-range difficulty multiplier
    assert!(result.difficulty_multiplier > 100);
    assert!(result.difficulty_multiplier < 200);
}

#[test]
fn test_difficulty_level_100() {
    let (_env, client) = setup();
    let result = client.calculate_difficulty(&100u32);
    // 5 + 99*2 = 203
    assert_eq!(result.anomaly_count, 203);
    assert_eq!(result.difficulty_multiplier, 200);
    // At max level, legendary should be higher than at level 1
    assert!(result.rarity_weights.legendary > 1);
    // Common should be lower
    assert!(result.rarity_weights.common < 60);
}

#[test]
fn test_difficulty_invalid_level_zero() {
    let (_env, client) = setup();
    let result = client.try_calculate_difficulty(&0u32);
    assert!(result.is_err());
}

#[test]
fn test_difficulty_invalid_level_over_max() {
    let (_env, client) = setup();
    let result = client.try_calculate_difficulty(&101u32);
    assert!(result.is_err());
}

#[test]
fn test_rarity_weights_sum_to_100() {
    let (_env, client) = setup();
    for level in [1u32, 10, 25, 50, 75, 100] {
        let result = client.calculate_difficulty(&level);
        let w = &result.rarity_weights;
        let sum = w.common + w.uncommon + w.rare + w.epic + w.legendary;
        assert_eq!(sum, 100, "Weights must sum to 100 at level {}", level);
    }
}

#[test]
fn test_difficulty_increases_with_level() {
    let (_env, client) = setup();
    let low = client.calculate_difficulty(&1u32);
    let mid = client.calculate_difficulty(&50u32);
    let high = client.calculate_difficulty(&100u32);

    assert!(low.anomaly_count < mid.anomaly_count);
    assert!(mid.anomaly_count < high.anomaly_count);
    assert!(low.difficulty_multiplier <= mid.difficulty_multiplier);
    assert!(mid.difficulty_multiplier <= high.difficulty_multiplier);
}

#[test]
fn test_apply_scaling_to_layout() {
    let (_env, client) = setup();

    // At level 1, multiplier is 100, so scaled = base * 100 / 100 = base
    let scaled_1 = client.apply_scaling_to_layout(&10u32, &1u32);
    assert_eq!(scaled_1, 10);

    // At level 100, multiplier is 200, so scaled = base * 200 / 100 = 2 * base
    let scaled_100 = client.apply_scaling_to_layout(&10u32, &100u32);
    assert_eq!(scaled_100, 20);
}

#[test]
fn test_apply_scaling_invalid_level() {
    let (_env, client) = setup();
    let result = client.try_apply_scaling_to_layout(&10u32, &0u32);
    assert!(result.is_err());
}

#[test]
fn test_legendary_weight_increases_with_level() {
    let (_env, client) = setup();
    let low = client.calculate_difficulty(&1u32);
    let high = client.calculate_difficulty(&100u32);
    assert!(high.rarity_weights.legendary > low.rarity_weights.legendary);
}
