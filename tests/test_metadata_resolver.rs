#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    vec, Address, Bytes, Env,
};
use stellar_nebula_nomad::{
    MetadataError, NebulaNomadContract, NebulaNomadContractClient, MAX_METADATA_BATCH,
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
    let caller = Address::generate(&env);
    (env, client, caller)
}

fn cid(env: &Env, s: &[u8]) -> Bytes {
    Bytes::from_slice(env, s)
}

#[test]
fn test_set_and_resolve_metadata() {
    let (env, client, caller) = setup();
    let ipfs_cid = cid(&env, b"QmXoypizjW3WknFiJnKLwHCnL72vedxjQkDDP1mXWo6uco");
    client.set_metadata_uri(&caller, &1u64, &ipfs_cid);

    let meta = client.resolve_metadata(&1u64);
    assert_eq!(meta.token_id, 1);
    assert_eq!(meta.cid, ipfs_cid);
}

#[test]
fn test_resolve_nonexistent_token_fails() {
    let (_env, client, _caller) = setup();
    let result = client.try_resolve_metadata(&999u64);
    assert!(result.is_err());
}

#[test]
fn test_set_empty_cid_fails() {
    let (env, client, caller) = setup();
    let empty = Bytes::new(&env);
    let result = client.try_set_metadata_uri(&caller, &1u64, &empty);
    assert!(result.is_err());
}

#[test]
fn test_metadata_immutable_after_first_set() {
    let (env, client, caller) = setup();
    let cid1 = cid(&env, b"QmOriginal");
    let cid2 = cid(&env, b"QmUpdated");

    client.set_metadata_uri(&caller, &1u64, &cid1);
    let result = client.try_set_metadata_uri(&caller, &1u64, &cid2);
    assert!(result.is_err());

    // Original CID is preserved
    let meta = client.resolve_metadata(&1u64);
    assert_eq!(meta.cid, cid1);
}

#[test]
fn test_default_gateway_is_ipfs_io() {
    let (env, client, _caller) = setup();
    let gateway = client.get_current_gateway();
    let expected = Bytes::from_slice(&env, b"https://ipfs.io/ipfs/");
    assert_eq!(gateway, expected);
}

#[test]
fn test_set_custom_gateway() {
    let (env, client, admin) = setup();
    let new_gateway = Bytes::from_slice(&env, b"https://cloudflare-ipfs.com/ipfs/");
    client.set_gateway(&admin, &new_gateway);
    assert_eq!(client.get_current_gateway(), new_gateway);
}

#[test]
fn test_resolve_uses_configured_gateway() {
    let (env, client, caller) = setup();
    let new_gateway = Bytes::from_slice(&env, b"https://gateway.pinata.cloud/ipfs/");
    client.set_gateway(&caller, &new_gateway);

    let ipfs_cid = cid(&env, b"QmPinataTest");
    client.set_metadata_uri(&caller, &5u64, &ipfs_cid);

    let meta = client.resolve_metadata(&5u64);
    assert_eq!(meta.gateway, new_gateway);
}

#[test]
fn test_batch_resolve_metadata() {
    let (env, client, caller) = setup();

    for i in 1u64..=3 {
        let c = cid(&env, &[b'Q', b'm', b'A' + (i as u8)]);
        client.set_metadata_uri(&caller, &i, &c);
    }

    let ids = vec![&env, 1u64, 2u64, 3u64];
    let results = client.batch_resolve_metadata(&ids);
    assert_eq!(results.len(), 3);
    assert_eq!(results.get(0).unwrap().token_id, 1);
    assert_eq!(results.get(1).unwrap().token_id, 2);
    assert_eq!(results.get(2).unwrap().token_id, 3);
}

#[test]
fn test_batch_resolve_missing_token_fails() {
    let (env, client, caller) = setup();
    let ipfs_cid = cid(&env, b"QmExists");
    client.set_metadata_uri(&caller, &1u64, &ipfs_cid);

    // Token 2 does not exist
    let ids = vec![&env, 1u64, 2u64];
    let result = client.try_batch_resolve_metadata(&ids);
    assert!(result.is_err());
}

#[test]
fn test_batch_resolve_exceeds_max_fails() {
    let (env, client, _caller) = setup();
    let mut ids = soroban_sdk::Vec::new(&env);
    for i in 0..(MAX_METADATA_BATCH + 1) {
        ids.push_back(i as u64);
    }
    let result = client.try_batch_resolve_metadata(&ids);
    assert!(result.is_err());
}

#[test]
fn test_max_batch_size_constant() {
    assert_eq!(MAX_METADATA_BATCH, 10);
}

#[test]
fn test_different_tokens_independent() {
    let (env, client, caller) = setup();
    let cid_a = cid(&env, b"QmShipCID");
    let cid_b = cid(&env, b"QmResourceCID");

    client.set_metadata_uri(&caller, &1u64, &cid_a);
    client.set_metadata_uri(&caller, &2u64, &cid_b);

    assert_eq!(client.resolve_metadata(&1u64).cid, cid_a);
    assert_eq!(client.resolve_metadata(&2u64).cid, cid_b);
}
