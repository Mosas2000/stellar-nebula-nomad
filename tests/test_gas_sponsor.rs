#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{Address, Env};
use stellar_nebula_nomad::{
    initialize_sponsorship, sponsor_first_scan, claim_sponsorship_fund,
    has_been_sponsored, get_sponsor_fund_balance, get_daily_sponsor_count,
    get_remaining_sponsor_slots, get_sponsor_admin, get_sponsor_config,
    update_sponsor_config, mark_profile_verified, SponsorConfig, SponsorError,
    NebulaNomadContract, NebulaNomadContractClient,
};

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
    let admin = Address::generate(&env);
    (env, client, admin)
}

// ─── Initialization Tests ─────────────────────────────────────────────────

#[test]
fn test_initialize_sponsorship() {
    let (env, client, admin) = setup_env();
    
    let result = client.initialize_sponsorship(&admin, &10_000_000);
    assert!(result.is_ok());
    
    // Check fund balance
    let balance = client.get_sponsor_fund_balance();
    assert_eq!(balance, 10_000_000);
    
    // Check admin
    let stored_admin = client.get_sponsor_admin();
    assert_eq!(stored_admin, Some(admin));
    
    // Check config exists
    let config = client.get_sponsor_config();
    assert!(config.is_some());
}

#[test]
fn test_initialize_sponsorship_invalid_amount() {
    let (env, client, admin) = setup_env();
    
    let result = client.try_initialize_sponsorship(&admin, &0);
    assert!(result.is_err());
}

// ─── Sponsorship Grant Tests ──────────────────────────────────────────────

#[test]
fn test_sponsor_first_scan_success() {
    let (env, client, admin) = setup_env();
    let player = Address::generate(&env);
    
    // Initialize sponsorship
    client.initialize_sponsorship(&admin, &10_000_000);
    
    // Mark player profile as verified
    mark_profile_verified(&env, &player);
    
    // Sponsor the player
    let amount = client.sponsor_first_scan(&player);
    assert!(amount.is_ok());
    assert_eq!(amount.unwrap(), 100_000); // Default sponsor_amount
    
    // Check player is now sponsored
    assert!(client.has_been_sponsored(&player));
    
    // Check fund balance decreased
    let balance = client.get_sponsor_fund_balance();
    assert_eq!(balance, 9_900_000); // 10M - 100K
}

#[test]
fn test_sponsor_first_scan_already_sponsored() {
    let (env, client, admin) = setup_env();
    let player = Address::generate(&env);
    
    client.initialize_sponsorship(&admin, &10_000_000);
    mark_profile_verified(&env, &player);
    
    // First sponsorship succeeds
    client.sponsor_first_scan(&player).unwrap();
    
    // Second sponsorship fails
    let result = client.try_sponsor_first_scan(&player);
    assert!(result.is_err());
    
    let err = result.err().unwrap();
    assert!(matches!(err, SponsorError::AlreadySponsored));
}

#[test]
fn test_sponsor_daily_cap() {
    let (env, client, admin) = setup_env();
    
    client.initialize_sponsorship(&admin, &100_000_000);
    
    // Sponsor up to daily cap (100 players)
    for i in 0..100 {
        let player = Address::generate(&env);
        mark_profile_verified(&env, &player);
        let result = client.try_sponsor_first_scan(&player);
        assert!(result.is_ok(), "Failed at player {}", i);
    }
    
    // 101st player should fail
    let player101 = Address::generate(&env);
    mark_profile_verified(&env, &player101);
    let result = client.try_sponsor_first_scan(&player101);
    assert!(result.is_err());
    assert!(matches!(result.err().unwrap(), SponsorError::DailyCapReached));
}

#[test]
fn test_sponsor_insufficient_funds() {
    let (env, client, admin) = setup_env();
    let player = Address::generate(&env);
    
    // Initialize with minimal fund (less than sponsor_amount)
    client.initialize_sponsorship(&admin, &50_000);
    mark_profile_verified(&env, &player);
    
    // Should fail due to insufficient funds (default sponsor_amount is 100_000)
    let result = client.try_sponsor_first_scan(&player);
    assert!(result.is_err());
    assert!(matches!(result.err().unwrap(), SponsorError::InsufficientFunds));
}

// ─── Fund Replenishment Tests ───────────────────────────────────────────────

#[test]
fn test_claim_sponsorship_fund() {
    let (env, client, admin) = setup_env();
    
    client.initialize_sponsorship(&admin, &10_000_000);
    
    // Replenish fund
    let new_balance = client.claim_sponsorship_fund(&admin, &5_000_000);
    assert!(new_balance.is_ok());
    assert_eq!(new_balance.unwrap(), 15_000_000);
    
    // Check balance
    assert_eq!(client.get_sponsor_fund_balance(), 15_000_000);
}

#[test]
fn test_claim_sponsorship_fund_unauthorized() {
    let (env, client, admin) = setup_env();
    let fake_admin = Address::generate(&env);
    
    client.initialize_sponsorship(&admin, &10_000_000);
    
    // Non-admin tries to replenish
    let result = client.try_claim_sponsorship_fund(&fake_admin, &5_000_000);
    assert!(result.is_err());
    assert!(matches!(result.err().unwrap(), SponsorError::Unauthorized));
}

#[test]
fn test_claim_sponsorship_fund_invalid_amount() {
    let (env, client, admin) = setup_env();
    
    client.initialize_sponsorship(&admin, &10_000_000);
    
    let result = client.try_claim_sponsorship_fund(&admin, &0);
    assert!(result.is_err());
    assert!(matches!(result.err().unwrap(), SponsorError::InvalidAmount));
}

// ─── View Function Tests ──────────────────────────────────────────────────

#[test]
fn test_get_remaining_daily_slots() {
    let (env, client, admin) = setup_env();
    
    client.initialize_sponsorship(&admin, &100_000_000);
    
    // Initially 100 slots
    assert_eq!(client.get_remaining_sponsor_slots(), 100);
    
    // Use 5 slots
    for _ in 0..5 {
        let player = Address::generate(&env);
        mark_profile_verified(&env, &player);
        client.sponsor_first_scan(&player).unwrap();
    }
    
    // Now 95 slots
    assert_eq!(client.get_remaining_sponsor_slots(), 95);
    assert_eq!(client.get_daily_sponsor_count(), 5);
}

// ─── Configuration Tests ───────────────────────────────────────────────────

#[test]
fn test_update_sponsor_config() {
    let (env, client, admin) = setup_env();
    
    client.initialize_sponsorship(&admin, &10_000_000);
    
    // Update config
    let new_config = client.update_sponsor_config(
        &admin,
        &20_000_000,  // min_threshold
        &200_000,    // sponsor_amount
        &50,         // daily_cap
    );
    
    assert!(new_config.is_ok());
    let config = new_config.unwrap();
    assert_eq!(config.min_threshold, 20_000_000);
    assert_eq!(config.sponsor_amount, 200_000);
    assert_eq!(config.daily_cap, 50);
}

#[test]
fn test_update_sponsor_config_unauthorized() {
    let (env, client, admin) = setup_env();
    let fake_admin = Address::generate(&env);
    
    client.initialize_sponsorship(&admin, &10_000_000);
    
    let result = client.try_update_sponsor_config(
        &fake_admin,
        &20_000_000,
        &200_000,
        &50,
    );
    
    assert!(result.is_err());
    assert!(matches!(result.err().unwrap(), SponsorError::Unauthorized));
}

// ─── Daily Counter Reset Tests ─────────────────────────────────────────────

#[test]
fn test_daily_counter_reset_after_24h() {
    let (env, client, admin) = setup_env();
    
    client.initialize_sponsorship(&admin, &100_000_000);
    
    // Use 5 slots
    for _ in 0..5 {
        let player = Address::generate(&env);
        mark_profile_verified(&env, &player);
        client.sponsor_first_scan(&player).unwrap();
    }
    
    assert_eq!(client.get_daily_sponsor_count(), 5);
    
    // Advance time by 25 hours (90000 seconds)
    env.ledger().set(LedgerInfo {
        protocol_version: 22,
        sequence_number: 100,
        timestamp: 1_700_000_000 + 90000,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 1000,
        max_entry_ttl: 10_000,
    });
    
    // Counter should reset to 0
    assert_eq!(client.get_daily_sponsor_count(), 0);
    assert_eq!(client.get_remaining_sponsor_slots(), 100);
}

// ─── Integration Tests ────────────────────────────────────────────────────

#[test]
fn test_full_sponsorship_lifecycle() {
    let (env, client, admin) = setup_env();
    let player = Address::generate(&env);
    
    // 1. Initialize sponsorship
    client.initialize_sponsorship(&admin, &10_000_000);
    
    // 2. Mark profile verified
    mark_profile_verified(&env, &player);
    
    // 3. Player gets sponsored
    let sponsored_amount = client.sponsor_first_scan(&player).unwrap();
    assert_eq!(sponsored_amount, 100_000);
    
    // 4. Verify player is sponsored
    assert!(client.has_been_sponsored(&player));
    
    // 5. Admin replenishes fund
    let new_balance = client.claim_sponsorship_fund(&admin, &10_000_000).unwrap();
    assert_eq!(new_balance, 19_900_000);
    
    // 6. Check daily stats
    assert_eq!(client.get_daily_sponsor_count(), 1);
    assert_eq!(client.get_remaining_sponsor_slots(), 99);
}
