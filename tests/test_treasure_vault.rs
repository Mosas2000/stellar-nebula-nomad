#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{Address, Env};
use stellar_nebula_nomad::{
    NebulaNomadContract, NebulaNomadContractClient, DEFAULT_MIN_LOCK_DURATION,
};

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
        min_persistent_entry_ttl: 1000,
        max_entry_ttl: 10_000,
    });
}

#[test]
fn test_deposit_treasure_creates_vault() {
    let (_env, client, player) = setup();
    let vault = client.deposit_treasure(&player, &1u64, &500u64);
    assert_eq!(vault.owner, player);
    assert_eq!(vault.ship_id, 1);
    assert_eq!(vault.amount, 500);
    assert!(!vault.claimed);
    assert_eq!(vault.lock_until, 1_000_000 + DEFAULT_MIN_LOCK_DURATION);
}

#[test]
fn test_deposit_zero_amount_fails() {
    let (_env, client, player) = setup();
    let result = client.try_deposit_treasure(&player, &1u64, &0u64);
    assert!(result.is_err());
}

#[test]
fn test_claim_before_lock_expires_fails() {
    let (_env, client, player) = setup();
    let vault = client.deposit_treasure(&player, &1u64, &500u64);
    let result = client.try_claim_treasure(&player, &vault.vault_id);
    assert!(result.is_err());
}

#[test]
fn test_claim_after_lock_expires_succeeds() {
    let (env, client, player) = setup();
    let vault = client.deposit_treasure(&player, &1u64, &1000u64);

    // Advance time past the lock period
    advance_time(&env, DEFAULT_MIN_LOCK_DURATION + 1);

    let payout = client.claim_treasure(&player, &vault.vault_id);
    // 1000 + 10% bonus = 1100
    assert_eq!(payout, 1100);
}

#[test]
fn test_claim_bonus_yield_calculation() {
    let (env, client, player) = setup();
    let vault = client.deposit_treasure(&player, &1u64, &333u64);

    advance_time(&env, DEFAULT_MIN_LOCK_DURATION + 1);

    let payout = client.claim_treasure(&player, &vault.vault_id);
    // 333 + 333 * 1000 / 10000 = 333 + 33 = 366
    assert_eq!(payout, 366);
}

#[test]
fn test_double_claim_fails() {
    let (env, client, player) = setup();
    let vault = client.deposit_treasure(&player, &1u64, &500u64);

    advance_time(&env, DEFAULT_MIN_LOCK_DURATION + 1);

    // First claim succeeds
    client.claim_treasure(&player, &vault.vault_id);

    // Second claim fails
    let result = client.try_claim_treasure(&player, &vault.vault_id);
    assert!(result.is_err());
}

#[test]
fn test_non_owner_cannot_claim() {
    let (env, client, player) = setup();
    let vault = client.deposit_treasure(&player, &1u64, &500u64);

    advance_time(&env, DEFAULT_MIN_LOCK_DURATION + 1);

    let other = Address::generate(&env);
    let result = client.try_claim_treasure(&other, &vault.vault_id);
    assert!(result.is_err());
}

#[test]
fn test_get_vault_returns_data() {
    let (_env, client, player) = setup();
    let vault = client.deposit_treasure(&player, &1u64, &500u64);

    let fetched = client.get_vault(&vault.vault_id);
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    assert_eq!(fetched.vault_id, vault.vault_id);
    assert_eq!(fetched.amount, 500);
}

#[test]
fn test_get_nonexistent_vault_returns_none() {
    let (_env, client, _player) = setup();
    let fetched = client.get_vault(&999u64);
    assert!(fetched.is_none());
}

#[test]
fn test_vault_with_mock_time_progression() {
    let (env, client, player) = setup();
    let vault = client.deposit_treasure(&player, &1u64, &100u64);

    // Still locked at halfway
    advance_time(&env, DEFAULT_MIN_LOCK_DURATION / 2);
    let result = client.try_claim_treasure(&player, &vault.vault_id);
    assert!(result.is_err());

    // Unlock at full duration
    advance_time(&env, DEFAULT_MIN_LOCK_DURATION / 2 + 1);
    let payout = client.claim_treasure(&player, &vault.vault_id);
    assert_eq!(payout, 110); // 100 + 10% bonus
}
