#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    symbol_short, Address, Bytes, BytesN, Env,
};
use stellar_nebula_nomad::{
    NebulaNomadContract, NebulaNomadContractClient, SnapshotError, StateSnapshot,
    MAX_SNAPSHOTS_PER_SESSION, AUTO_SNAPSHOT_INTERVAL,
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
    let player = Address::generate(&env);
    (env, client, player)
}

/// Helper: Mint a ship so snapshot tests have valid ship data.
fn mint_test_ship(
    env: &Env,
    client: &NebulaNomadContractClient<'static>,
    owner: &Address,
) -> u64 {
    let ship_type = symbol_short!("explorer");
    let metadata = Bytes::from_slice(env, &[0u8; 8]);
    let ship = client.mint_ship(owner, &ship_type, &metadata);
    ship.id
}

// ─── take_snapshot ────────────────────────────────────────────────────────

#[test]
fn test_take_snapshot_success() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    let snapshot = client.take_snapshot(&player, &ship_id);

    assert_eq!(snapshot.ship_id, ship_id);
    assert_eq!(snapshot.owner, player);
    assert_eq!(snapshot.ship_hull, 80); // explorer hull
    assert_eq!(snapshot.ship_scanner_power, 50); // explorer scanner
    assert_eq!(snapshot.created_at, 1_700_000_000);
    assert!(snapshot.immutable);
}

#[test]
fn test_take_snapshot_increments_id() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    let snap1 = client.take_snapshot(&player, &ship_id);
    let snap2 = client.take_snapshot(&player, &ship_id);

    assert_eq!(snap1.snapshot_id, 1);
    assert_eq!(snap2.snapshot_id, 2);
}

#[test]
fn test_take_snapshot_ship_not_found() {
    let (_env, client, player) = setup();

    let result = client.try_take_snapshot(&player, &9999);
    assert!(result.is_err() || result.unwrap().is_err());
}

#[test]
fn test_take_snapshot_not_owner() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    let other = Address::generate(&env);
    let result = client.try_take_snapshot(&other, &ship_id);
    assert!(result.is_err() || result.unwrap().is_err());
}

// ─── Session limit ────────────────────────────────────────────────────────

#[test]
fn test_session_limit_enforced() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    // Take MAX_SNAPSHOTS_PER_SESSION snapshots successfully.
    for _ in 0..MAX_SNAPSHOTS_PER_SESSION {
        client.take_snapshot(&player, &ship_id);
    }

    // The next one should fail.
    let result = client.try_take_snapshot(&player, &ship_id);
    assert!(result.is_err() || result.unwrap().is_err());
}

#[test]
fn test_session_limit_resets() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    // Exhaust the session limit.
    for _ in 0..MAX_SNAPSHOTS_PER_SESSION {
        client.take_snapshot(&player, &ship_id);
    }

    // Reset session counter.
    client.reset_session_count(&ship_id);

    // Should be able to take snapshots again.
    let snap = client.take_snapshot(&player, &ship_id);
    assert!(snap.snapshot_id > 0);
}

// ─── restore_from_snapshot ────────────────────────────────────────────────

#[test]
fn test_restore_from_snapshot_success() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    let snapshot = client.take_snapshot(&player, &ship_id);

    let result = client.restore_from_snapshot(&player, &snapshot.snapshot_id);
    assert_eq!(result.snapshot_id, snapshot.snapshot_id);
    assert_eq!(result.ship_id, ship_id);
    assert_eq!(result.restored_at, 1_700_000_000);
}

#[test]
fn test_restore_snapshot_not_found() {
    let (_env, client, player) = setup();

    let result = client.try_restore_from_snapshot(&player, &9999);
    assert!(result.is_err() || result.unwrap().is_err());
}

#[test]
fn test_restore_snapshot_not_owner() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);
    let snapshot = client.take_snapshot(&player, &ship_id);

    let other = Address::generate(&env);
    let result = client.try_restore_from_snapshot(&other, &snapshot.snapshot_id);
    assert!(result.is_err() || result.unwrap().is_err());
}

#[test]
fn test_snapshot_restore_preserves_ship_stats() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    // Snapshot current state (explorer: hull=80, scanner=50).
    let snapshot = client.take_snapshot(&player, &ship_id);
    assert_eq!(snapshot.ship_hull, 80);
    assert_eq!(snapshot.ship_scanner_power, 50);

    // Restore from snapshot and verify ship is intact.
    client.restore_from_snapshot(&player, &snapshot.snapshot_id);

    let ship = client.get_ship(&ship_id);
    assert_eq!(ship.hull, 80);
    assert_eq!(ship.scanner_power, 50);
}

// ─── get_snapshot / get_ship_snapshots ────────────────────────────────────

#[test]
fn test_get_snapshot_by_id() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    let snap = client.take_snapshot(&player, &ship_id);
    let retrieved = client.get_snapshot(&snap.snapshot_id);

    assert_eq!(retrieved.snapshot_id, snap.snapshot_id);
    assert_eq!(retrieved.ship_id, ship_id);
    assert_eq!(retrieved.integrity_hash, snap.integrity_hash);
}

#[test]
fn test_get_ship_snapshots_returns_all_ids() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    client.take_snapshot(&player, &ship_id);
    client.take_snapshot(&player, &ship_id);
    client.take_snapshot(&player, &ship_id);

    let ids = client.get_ship_snapshots(&ship_id);
    assert_eq!(ids.len(), 3);
}

#[test]
fn test_get_ship_snapshots_empty_for_new_ship() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    let ids = client.get_ship_snapshots(&ship_id);
    assert_eq!(ids.len(), 0);
}

// ─── auto_snapshot ────────────────────────────────────────────────────────

#[test]
fn test_auto_snapshot_success() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    let snap = client.auto_snapshot(&player, &ship_id);
    assert_eq!(snap.ship_id, ship_id);
}

#[test]
fn test_auto_snapshot_too_soon() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    // First auto-snapshot succeeds.
    client.auto_snapshot(&player, &ship_id);

    // Immediate second attempt should fail (interval not elapsed).
    let result = client.try_auto_snapshot(&player, &ship_id);
    assert!(result.is_err() || result.unwrap().is_err());
}

#[test]
fn test_auto_snapshot_after_interval() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    // First auto-snapshot.
    client.auto_snapshot(&player, &ship_id);

    // Advance time past the interval.
    env.ledger().set(LedgerInfo {
        protocol_version: 22,
        sequence_number: 200,
        timestamp: 1_700_000_000 + AUTO_SNAPSHOT_INTERVAL + 1,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 1000,
        max_entry_ttl: 10_000,
    });

    // Reset session counter since we're in a new "session".
    client.reset_session_count(&ship_id);

    // Second auto-snapshot should now succeed.
    let snap = client.auto_snapshot(&player, &ship_id);
    assert!(snap.snapshot_id > 1);
}

// ─── Integrity hash ──────────────────────────────────────────────────────

#[test]
fn test_snapshot_integrity_hash_consistent() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    let snap1 = client.take_snapshot(&player, &ship_id);
    let snap2 = client.take_snapshot(&player, &ship_id);

    // Same ship state at same timestamp → same integrity hash.
    assert_eq!(snap1.integrity_hash, snap2.integrity_hash);
}

// ─── Multiple ships ───────────────────────────────────────────────────────

#[test]
fn test_snapshots_across_multiple_ships() {
    let (env, client, player) = setup();
    let ship1 = mint_test_ship(&env, &client, &player);
    let ship2 = mint_test_ship(&env, &client, &player);

    let snap1 = client.take_snapshot(&player, &ship1);
    let snap2 = client.take_snapshot(&player, &ship2);

    assert_ne!(snap1.snapshot_id, snap2.snapshot_id);
    assert_eq!(snap1.ship_id, ship1);
    assert_eq!(snap2.ship_id, ship2);

    let ids1 = client.get_ship_snapshots(&ship1);
    let ids2 = client.get_ship_snapshots(&ship2);
    assert_eq!(ids1.len(), 1);
    assert_eq!(ids2.len(), 1);
}
