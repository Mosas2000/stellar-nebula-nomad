#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{Address, Bytes, BytesN, Env, Symbol, Vec};
use stellar_nebula_nomad::{
    compose_with_external_contract, validate_composable_response, batch_compose,
    sanitize_input, emit_batch_summary, record_composition_gas, get_last_composition_gas,
    CompositionBuilder, ComposableResponse, ComposabilityError,
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
    let caller = Address::generate(&env);
    (env, client, caller)
}

// ─── Composition Builder Tests ─────────────────────────────────────────────

#[test]
fn test_composition_builder_basic() {
    let env = Env::default();
    let target = Address::generate(&env);
    let args = Bytes::from_slice(&env, &[1u8, 2u8, 3u8]);
    
    let composition = CompositionBuilder::new(&env)
        .target(target.clone())
        .method(Symbol::new(&env, "test_method"))
        .args(args.clone())
        .build();
    
    assert!(composition.is_some());
    let (t, m, a) = composition.unwrap();
    assert_eq!(t, target);
    assert_eq!(m, Symbol::new(&env, "test_method"));
    assert_eq!(a, args);
}

#[test]
fn test_composition_builder_incomplete() {
    let env = Env::default();
    
    // Missing target
    let composition = CompositionBuilder::new(&env)
        .method(Symbol::new(&env, "test_method"))
        .args(Bytes::new(&env))
        .build();
    assert!(composition.is_none());
    
    // Missing method
    let composition = CompositionBuilder::new(&env)
        .target(Address::generate(&env))
        .args(Bytes::new(&env))
        .build();
    assert!(composition.is_none());
}

// ─── Input Sanitization Tests ──────────────────────────────────────────────

#[test]
fn test_sanitize_input_valid() {
    let env = Env::default();
    let input = Bytes::from_slice(&env, &[1u8, 2u8, 3u8, 4u8, 5u8]);
    
    let result = sanitize_input(&env, &input, 10);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), input);
}

#[test]
fn test_sanitize_input_too_large() {
    let env = Env::default();
    let input = Bytes::from_slice(&env, &[1u8; 100]);
    
    let result = sanitize_input(&env, &input, 50);
    assert!(result.is_err());
    assert!(matches!(result.err().unwrap(), ComposabilityError::InputTooLarge));
}

// ─── Response Validation Tests ─────────────────────────────────────────────

#[test]
fn test_validate_composable_response_valid() {
    let (env, client, _) = setup_env();
    
    // Create a non-zero response
    let mut bytes = [0u8; 64];
    bytes[0] = 1u8;
    let response = BytesN::from_array(&env, &bytes);
    
    let result = client.validate_composable_response(&response);
    assert!(result);
}

#[test]
#[should_panic(expected = "InvalidResponse")]
fn test_validate_composable_response_all_zeros() {
    let (env, client, _) = setup_env();
    
    // Create an all-zero response
    let bytes = [0u8; 64];
    let response = BytesN::from_array(&env, &bytes);
    
    // This should panic because we use .unwrap() in the test client
    client.validate_composable_response(&response);
}

// ─── Batch Composition Tests ───────────────────────────────────────────────

#[test]
fn test_batch_compose_empty() {
    let (env, client, _) = setup_env();
    
    let calls: Vec<(Address, Symbol, Bytes)> = Vec::new(&env);
    let results = client.batch_compose(&calls);
    assert_eq!(results.len(), 0);
}

#[test]
fn test_batch_compose_max_10() {
    let (env, client, _) = setup_env();
    
    // Create 15 calls (max is 10)
    let mut calls = Vec::new(&env);
    for _ in 0..15 {
        let target = Address::generate(&env);
        let method = Symbol::new(&env, "test");
        let args = Bytes::new(&env);
        calls.push_back((target, method, args));
    }
    
    let results = client.batch_compose(&calls);
    // Should be limited to 10
    assert_eq!(results.len(), 10);
}

// ─── Gas Tracking Tests ───────────────────────────────────────────────────

#[test]
fn test_record_and_get_composition_gas() {
    let (env, client, _) = setup_env();
    
    // Initially should be 0
    assert_eq!(client.get_last_composition_gas(), 0);
    
    // Record some gas
    record_composition_gas(&env, 1000);
    
    // Should now be 1000
    assert_eq!(client.get_last_composition_gas(), 1000);
}

// ─── Event Emission Tests ─────────────────────────────────────────────────

#[test]
fn test_emit_batch_summary() {
    let (env, client, _) = setup_env();
    
    // This should not panic
    emit_batch_summary(&env, 5, 4, 10000);
    
    // Verify event was emitted (in a real test, we'd check the events list)
    // For now, we just verify it doesn't panic
}

// ─── Integration Tests ─────────────────────────────────────────────────────

#[test]
fn test_composability_full_workflow() {
    let (env, client, _caller) = setup_env();
    
    // 1. Create a composition
    let target = Address::generate(&env);
    let method = Symbol::new(&env, "harvest");
    let args = Bytes::from_slice(&env, &[1u8; 10]);
    
    // 2. Validate response
    let mut response_bytes = [0u8; 64];
    response_bytes[0] = 1;
    let response = BytesN::from_array(&env, &response_bytes);
    
    let valid = client.validate_composable_response(&response);
    assert!(valid);
    
    // 3. Record composition gas
    record_composition_gas(&env, 5000);
    assert_eq!(client.get_last_composition_gas(), 5000);
    
    // 4. Batch compose (empty calls for testing)
    let calls = Vec::new(&env);
    let results = client.batch_compose(&calls);
    assert_eq!(results.len(), 0);
}

// ─── ComposabilityError Tests ─────────────────────────────────────────────

#[test]
fn test_composability_error_variants() {
    // Verify all error variants exist and can be used
    let errors = [
        ComposabilityError::InvalidTarget,
        ComposabilityError::CallFailed,
        ComposabilityError::InvalidResponse,
        ComposabilityError::InputTooLarge,
        ComposabilityError::ContractNotFound,
        ComposabilityError::Unauthorized,
        ComposabilityError::Timeout,
        ComposabilityError::InvalidMethod,
    ];
    
    // Just verify they can be compared
    assert_eq!(errors[0], ComposabilityError::InvalidTarget);
}

// ─── Cross-Contract Composition Tests ─────────────────────────────────────

#[test]
fn test_compose_with_external_contract_success() {
    let (env, client, _caller) = setup_env();
    
    let target = Address::generate(&env);
    let method = Symbol::new(&env, "test_method");
    let args = Bytes::from_slice(&env, &[1u8, 2u8, 3u8]);
    
    let result = client.compose_with_external_contract(&target, &method, &args);
    
    // Should succeed with simulated response
    assert!(result.success);
    assert_eq!(result.gas_used, 0); // No actual gas consumed in simulation
}
