#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    vec, Address, Env,
};
use stellar_nebula_nomad::{NebulaNomadContract, NebulaNomadContractClient, UNPAUSE_DELAY};

fn setup() -> (Env, NebulaNomadContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(LedgerInfo {
        protocol_version: 22,
        sequence_number: 100,
        timestamp: 1_000_000,
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
        min_persistent_entry_ttl: 1000,
        max_entry_ttl: 10_000,
    });
}

#[test]
fn test_initialize_admins() {
    let (env, client, admin) = setup();
    let admins = vec![&env, admin.clone()];
    client.initialize_admins(&admins);
    let stored = client.get_admins();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored.get(0).unwrap(), admin);
}

#[test]
fn test_initialize_admins_twice_fails() {
    let (env, client, admin) = setup();
    let admins = vec![&env, admin.clone()];
    client.initialize_admins(&admins);
    let result = client.try_initialize_admins(&admins);
    assert!(result.is_err());
}

#[test]
fn test_initialize_admins_empty_fails() {
    let (env, client, _admin) = setup();
    let admins: soroban_sdk::Vec<Address> = soroban_sdk::Vec::new(&env);
    let result = client.try_initialize_admins(&admins);
    assert!(result.is_err());
}

#[test]
fn test_is_paused_initially_false() {
    let (env, client, admin) = setup();
    let admins = vec![&env, admin.clone()];
    client.initialize_admins(&admins);
    assert!(!client.is_paused());
}

#[test]
fn test_pause_contract() {
    let (env, client, admin) = setup();
    let admins = vec![&env, admin.clone()];
    client.initialize_admins(&admins);
    client.pause_contract(&admin);
    assert!(client.is_paused());
}

#[test]
fn test_pause_non_admin_fails() {
    let (env, client, admin) = setup();
    let attacker = Address::generate(&env);
    let admins = vec![&env, admin.clone()];
    client.initialize_admins(&admins);
    let result = client.try_pause_contract(&attacker);
    assert!(result.is_err());
}

#[test]
fn test_schedule_and_execute_unpause() {
    let (env, client, admin) = setup();
    let admins = vec![&env, admin.clone()];
    client.initialize_admins(&admins);
    client.pause_contract(&admin);
    assert!(client.is_paused());

    let unpause_at = client.schedule_unpause(&admin);

    // Advance past the delay
    advance_time(&env, UNPAUSE_DELAY + 1);

    client.execute_unpause(&admin);
    assert!(!client.is_paused());
    // unpause_at should be 1_000_000 + UNPAUSE_DELAY
    assert_eq!(unpause_at, 1_000_000 + UNPAUSE_DELAY);
}

#[test]
fn test_execute_unpause_before_delay_fails() {
    let (env, client, admin) = setup();
    let admins = vec![&env, admin.clone()];
    client.initialize_admins(&admins);
    client.pause_contract(&admin);
    client.schedule_unpause(&admin);

    // Do NOT advance time — delay has not elapsed
    let result = client.try_execute_unpause(&admin);
    assert!(result.is_err());
    assert!(client.is_paused());
}

#[test]
fn test_schedule_unpause_when_not_paused_fails() {
    let (env, client, admin) = setup();
    let admins = vec![&env, admin.clone()];
    client.initialize_admins(&admins);
    let result = client.try_schedule_unpause(&admin);
    assert!(result.is_err());
}

#[test]
fn test_emergency_withdraw_emits_event() {
    let (env, client, admin) = setup();
    let admins = vec![&env, admin.clone()];
    client.initialize_admins(&admins);
    // Should not panic — event emission is the observable side-effect
    client.emergency_withdraw(&admin, &soroban_sdk::symbol_short!("ore"));
}

#[test]
fn test_unpause_delay_constant() {
    assert_eq!(UNPAUSE_DELAY, 3_600);
}

#[test]
fn test_multi_admin_initialization() {
    let (env, client, admin1) = setup();
    let admin2 = Address::generate(&env);
    let admins = vec![&env, admin1.clone(), admin2.clone()];
    client.initialize_admins(&admins);
    let stored = client.get_admins();
    assert_eq!(stored.len(), 2);
}
