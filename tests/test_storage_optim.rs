#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    symbol_short, vec, Address, BytesN, Env, Symbol, Vec,
};
use stellar_nebula_nomad::{
    NebulaNomadContract, NebulaNomadContractClient, OptimResult, OptimizedEntry, ShipNebulaData,
    StorageError, DEFAULT_BUMP_TTL, MAX_BUMP_TTL, MAX_BURST_READS,
};

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
    let admin = Address::generate(&env);
    (env, client, admin)
}

// ─── store_with_bump ──────────────────────────────────────────────────────

#[test]
fn test_store_with_bump_success() {
    let (env, client, admin) = setup();
    let key = symbol_short!("test_key");
    let value = BytesN::from_array(&env, &[42u8; 64]);

    let result = client.store_with_bump(&key, &value);
    assert_eq!(result.key, key);
    assert_eq!(result.ttl_applied, DEFAULT_BUMP_TTL);
    assert_eq!(result.instruction_savings_pct, 30);
}

#[test]
fn test_store_and_retrieve() {
    let (env, client, admin) = setup();
    let key = symbol_short!("mydata");
    let value = BytesN::from_array(&env, &[7u8; 64]);

    client.store_with_bump(&key, &value);

    let entry = client.get_optimized_entry(&key);
    assert_eq!(entry.key, key);
    assert_eq!(entry.data, value);
    assert_eq!(entry.created_at, 1_700_000_000);
    assert_eq!(entry.ttl_ledgers, DEFAULT_BUMP_TTL);
}

#[test]
fn test_get_nonexistent_entry_fails() {
    let (env, client, _admin) = setup();
    let key = symbol_short!("missing");

    let result = client.try_get_optimized_entry(&key);
    assert!(result.is_err() || result.unwrap().is_err());
}

// ─── Batch store ──────────────────────────────────────────────────────────

#[test]
fn test_batch_store_with_bump() {
    let (env, client, _admin) = setup();
    let keys = vec![
        &env,
        symbol_short!("key_a"),
        symbol_short!("key_b"),
        symbol_short!("key_c"),
    ];
    let values = vec![
        &env,
        BytesN::from_array(&env, &[1u8; 64]),
        BytesN::from_array(&env, &[2u8; 64]),
        BytesN::from_array(&env, &[3u8; 64]),
    ];

    let results = client.batch_store_with_bump(&keys, &values);
    assert_eq!(results.len(), 3);

    // Verify each entry is retrievable.
    for i in 0..keys.len() {
        let entry = client.get_optimized_entry(&keys.get(i).unwrap());
        assert_eq!(entry.data, values.get(i).unwrap());
    }
}

// ─── Composite ship-nebula keys ───────────────────────────────────────────

#[test]
fn test_store_ship_nebula_composite() {
    let (_env, client, _admin) = setup();
    let ship_id = 1u64;
    let nebula_id = 42u64;

    client.store_ship_nebula(&ship_id, &nebula_id, &5, &1000);

    let data = client.get_ship_nebula(&ship_id, &nebula_id);
    assert_eq!(data.ship_id, ship_id);
    assert_eq!(data.nebula_id, nebula_id);
    assert_eq!(data.scan_count, 5);
    assert_eq!(data.resource_cache, 1000);
    assert_eq!(data.last_scan_at, 1_700_000_000);
}

#[test]
fn test_get_nonexistent_ship_nebula_fails() {
    let (_env, client, _admin) = setup();

    let result = client.try_get_ship_nebula(&999, &999);
    assert!(result.is_err() || result.unwrap().is_err());
}

// ─── Bump Config ──────────────────────────────────────────────────────────

#[test]
fn test_initialize_bump_config() {
    let (_env, client, admin) = setup();
    client.initialize_bump_config(&admin);

    // After init, store should use default TTL.
    let key = symbol_short!("cfg_test");
    let value = BytesN::from_array(&_env, &[0u8; 64]);
    let result = client.store_with_bump(&key, &value);
    assert_eq!(result.ttl_applied, DEFAULT_BUMP_TTL);
}

#[test]
fn test_update_bump_config() {
    let (env, client, admin) = setup();
    client.initialize_bump_config(&admin);

    let new_default = 100_000u32;
    let new_max = 500_000u32;
    client.update_bump_config(&admin, &new_default, &new_max);

    // Store should now use the updated TTL.
    let key = symbol_short!("updated");
    let value = BytesN::from_array(&env, &[0u8; 64]);
    let result = client.store_with_bump(&key, &value);
    assert_eq!(result.ttl_applied, new_default);
}

#[test]
fn test_update_bump_config_invalid_ttl() {
    let (_env, client, admin) = setup();
    client.initialize_bump_config(&admin);

    // default > max should fail.
    let result = client.try_update_bump_config(&admin, &500_000, &100_000);
    assert!(result.is_err() || result.unwrap().is_err());
}

#[test]
fn test_update_bump_config_zero_ttl_fails() {
    let (_env, client, admin) = setup();
    client.initialize_bump_config(&admin);

    let result = client.try_update_bump_config(&admin, &0, &100_000);
    assert!(result.is_err() || result.unwrap().is_err());
}

// ─── Upgrade Target ───────────────────────────────────────────────────────

#[test]
fn test_set_and_get_upgrade_target() {
    let (env, client, admin) = setup();
    let target = Address::generate(&env);

    client.set_upgrade_target(&admin, &target);

    let stored = client.get_upgrade_target();
    assert_eq!(stored, Some(target));
}

#[test]
fn test_get_upgrade_target_none_by_default() {
    let (_env, client, _admin) = setup();

    let stored = client.get_upgrade_target();
    assert_eq!(stored, None);
}

// ─── Re-Entrancy Guard ───────────────────────────────────────────────────

#[test]
fn test_reentrancy_guard_releases_after_store() {
    let (env, client, _admin) = setup();

    // First store should succeed.
    let key1 = symbol_short!("first");
    let value = BytesN::from_array(&env, &[1u8; 64]);
    client.store_with_bump(&key1, &value);

    // Second store should also succeed (guard released).
    let key2 = symbol_short!("second");
    let result = client.try_store_with_bump(&key2, &value);
    assert!(result.is_ok());
}

// ─── Burst Read Counter ──────────────────────────────────────────────────

#[test]
fn test_reset_burst_counter() {
    let (env, client, _admin) = setup();

    // Store an entry then read it multiple times.
    let key = symbol_short!("burst");
    let value = BytesN::from_array(&env, &[5u8; 64]);
    client.store_with_bump(&key, &value);

    // Should be able to read after reset.
    client.reset_burst_counter();
    let entry = client.get_optimized_entry(&key);
    assert_eq!(entry.key, key);
}
