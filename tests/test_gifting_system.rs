#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{symbol_short, Address, Env};
use stellar_nebula_nomad::{GiftError, NebulaNomadContract, NebulaNomadContractClient, ResourceKey};

fn setup_env() -> (Env, NebulaNomadContractClient<'static>) {
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

fn setup_env_with_high_ttl() -> (Env, NebulaNomadContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(LedgerInfo {
        protocol_version: 22,
        sequence_number: 100,
        timestamp: 1_700_000_000,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 100_000,
        max_entry_ttl: 100_000,
    });
    let contract_id = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    (env, client)
}

fn seed_balance(env: &Env, contract_id: &Address, owner: &Address, resource: &soroban_sdk::Symbol, amount: u32) {
    let key = ResourceKey::ResourceBalance(owner.clone(), resource.clone());
    env.as_contract(contract_id, || {
        env.storage().instance().set(&key, &amount);
    });
}

// ── Basic send / accept flow ──────────────────────────────────────────────

#[test]
fn test_send_and_accept_gift() {
    let (env, client) = setup_env();
    let sender = Address::generate(&env);
    let receiver = Address::generate(&env);
    let resource = symbol_short!("dust");

    seed_balance(&env, &client.address, &sender, &resource, 500);

    let gift_id = client.send_gift(&sender, &receiver, &resource, &100);
    assert!(gift_id > 0);

    let gift = client.get_gift(&gift_id).unwrap();
    assert_eq!(gift.sender, sender);
    assert_eq!(gift.receiver, receiver);
    assert_eq!(gift.amount, 100);
    assert!(!gift.claimed);

    client.accept_gift(&receiver, &gift_id);

    let gift = client.get_gift(&gift_id).unwrap();
    assert!(gift.claimed);
}

#[test]
fn test_send_gift_debits_sender_balance() {
    let (env, client) = setup_env();
    let sender = Address::generate(&env);
    let receiver = Address::generate(&env);
    let resource = symbol_short!("ore");

    seed_balance(&env, &client.address, &sender, &resource, 300);

    client.send_gift(&sender, &receiver, &resource, &200);

    // Send another — only 100 left
    client.send_gift(&sender, &receiver, &resource, &50);

    // 50 left → 100 would fail
    let result = client.try_send_gift(&sender, &receiver, &resource, &100);
    assert_eq!(result, Err(Ok(GiftError::InsufficientBalance)));
}

// ── Error cases ───────────────────────────────────────────────────────────

#[test]
fn test_send_gift_zero_amount() {
    let (env, client) = setup_env();
    let sender = Address::generate(&env);
    let receiver = Address::generate(&env);
    let resource = symbol_short!("dust");

    let result = client.try_send_gift(&sender, &receiver, &resource, &0);
    assert_eq!(result, Err(Ok(GiftError::ZeroAmount)));
}

#[test]
fn test_send_gift_to_self() {
    let (env, client) = setup_env();
    let sender = Address::generate(&env);
    let resource = symbol_short!("dust");

    seed_balance(&env, &client.address, &sender, &resource, 100);

    let result = client.try_send_gift(&sender, &sender, &resource, &50);
    assert_eq!(result, Err(Ok(GiftError::SelfGift)));
}

#[test]
fn test_send_gift_insufficient_balance() {
    let (env, client) = setup_env();
    let sender = Address::generate(&env);
    let receiver = Address::generate(&env);
    let resource = symbol_short!("dust");

    seed_balance(&env, &client.address, &sender, &resource, 10);

    let result = client.try_send_gift(&sender, &receiver, &resource, &50);
    assert_eq!(result, Err(Ok(GiftError::InsufficientBalance)));
}

#[test]
fn test_accept_nonexistent_gift() {
    let (env, client) = setup_env();
    let receiver = Address::generate(&env);

    let result = client.try_accept_gift(&receiver, &999);
    assert_eq!(result, Err(Ok(GiftError::GiftNotFound)));
}

#[test]
fn test_accept_gift_wrong_receiver() {
    let (env, client) = setup_env();
    let sender = Address::generate(&env);
    let receiver = Address::generate(&env);
    let imposter = Address::generate(&env);
    let resource = symbol_short!("dust");

    seed_balance(&env, &client.address, &sender, &resource, 100);

    let gift_id = client.send_gift(&sender, &receiver, &resource, &50);
    let result = client.try_accept_gift(&imposter, &gift_id);
    assert_eq!(result, Err(Ok(GiftError::NotReceiver)));
}

#[test]
fn test_accept_gift_already_claimed() {
    let (env, client) = setup_env();
    let sender = Address::generate(&env);
    let receiver = Address::generate(&env);
    let resource = symbol_short!("dust");

    seed_balance(&env, &client.address, &sender, &resource, 100);

    let gift_id = client.send_gift(&sender, &receiver, &resource, &50);
    client.accept_gift(&receiver, &gift_id);

    let result = client.try_accept_gift(&receiver, &gift_id);
    assert_eq!(result, Err(Ok(GiftError::GiftAlreadyClaimed)));
}

// ── Expiry ────────────────────────────────────────────────────────────────

#[test]
fn test_gift_expires_after_48h() {
    let (env, client) = setup_env_with_high_ttl();
    let sender = Address::generate(&env);
    let receiver = Address::generate(&env);
    let resource = symbol_short!("dust");

    seed_balance(&env, &client.address, &sender, &resource, 100);

    let gift_id = client.send_gift(&sender, &receiver, &resource, &50);

    // Advance past 48h (34_560 ledgers + 1)
    let current = env.ledger().sequence();
    env.ledger().with_mut(|li| {
        li.sequence_number = current + 34_561;
    });

    let result = client.try_accept_gift(&receiver, &gift_id);
    assert_eq!(result, Err(Ok(GiftError::GiftExpired)));
}

#[test]
fn test_gift_accepted_just_before_expiry() {
    let (env, client) = setup_env_with_high_ttl();
    let sender = Address::generate(&env);
    let receiver = Address::generate(&env);
    let resource = symbol_short!("dust");

    seed_balance(&env, &client.address, &sender, &resource, 100);

    let gift_id = client.send_gift(&sender, &receiver, &resource, &50);

    // Advance to exactly the expiry boundary (should still be valid)
    let current = env.ledger().sequence();
    env.ledger().with_mut(|li| {
        li.sequence_number = current + 34_560;
    });

    client.accept_gift(&receiver, &gift_id);
    let gift = client.get_gift(&gift_id).unwrap();
    assert!(gift.claimed);
}

// ── Burst limit ───────────────────────────────────────────────────────────

#[test]
fn test_burst_limit_10_gifts_per_session() {
    let (env, client) = setup_env();
    let sender = Address::generate(&env);
    let resource = symbol_short!("dust");

    seed_balance(&env, &client.address, &sender, &resource, 10_000);

    for i in 0..10u32 {
        let receiver = Address::generate(&env);
        client.send_gift(&sender, &receiver, &resource, &10);
        assert!(i < 10);
    }

    let extra_receiver = Address::generate(&env);
    let result = client.try_send_gift(&sender, &extra_receiver, &resource, &10);
    assert_eq!(result, Err(Ok(GiftError::BurstLimitExceeded)));
}

// ── Multi-player simulation ───────────────────────────────────────────────

#[test]
fn test_multi_player_gift_chain() {
    let (env, client) = setup_env();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let charlie = Address::generate(&env);
    let resource = symbol_short!("exotic");

    seed_balance(&env, &client.address, &alice, &resource, 1000);

    // Alice → Bob (300)
    let g1 = client.send_gift(&alice, &bob, &resource, &300);
    client.accept_gift(&bob, &g1);

    // Bob → Charlie (150) — Bob now has 300 from Alice
    let g2 = client.send_gift(&bob, &charlie, &resource, &150);
    client.accept_gift(&charlie, &g2);

    // Charlie → Alice (50) — Charlie has 150 from Bob
    let g3 = client.send_gift(&charlie, &alice, &resource, &50);
    client.accept_gift(&alice, &g3);

    // Verify final state via gift records
    let gift1 = client.get_gift(&g1).unwrap();
    let gift2 = client.get_gift(&g2).unwrap();
    let gift3 = client.get_gift(&g3).unwrap();
    assert!(gift1.claimed);
    assert!(gift2.claimed);
    assert!(gift3.claimed);
    assert_eq!(gift1.amount, 300);
    assert_eq!(gift2.amount, 150);
    assert_eq!(gift3.amount, 50);
}

#[test]
fn test_multiple_resources_gifted() {
    let (env, client) = setup_env();
    let sender = Address::generate(&env);
    let receiver = Address::generate(&env);

    let dust = symbol_short!("dust");
    let ore = symbol_short!("ore");
    let dark = symbol_short!("dark");

    seed_balance(&env, &client.address, &sender, &dust, 500);
    seed_balance(&env, &client.address, &sender, &ore, 300);
    seed_balance(&env, &client.address, &sender, &dark, 100);

    let g1 = client.send_gift(&sender, &receiver, &dust, &100);
    let g2 = client.send_gift(&sender, &receiver, &ore, &200);
    let g3 = client.send_gift(&sender, &receiver, &dark, &50);

    client.accept_gift(&receiver, &g1);
    client.accept_gift(&receiver, &g2);
    client.accept_gift(&receiver, &g3);

    let gift1 = client.get_gift(&g1).unwrap();
    let gift2 = client.get_gift(&g2).unwrap();
    let gift3 = client.get_gift(&g3).unwrap();
    assert!(gift1.claimed && gift2.claimed && gift3.claimed);
}
