#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    symbol_short, vec, Address, Env,
};
use stellar_nebula_nomad::{
    BatchOp, BatchOpType, NebulaNomadContract, NebulaNomadContractClient, MAX_BATCH_SIZE,
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

fn make_op(env: &Env, ship_id: u64, op_type: BatchOpType) -> BatchOp {
    BatchOp {
        ship_id,
        op_type,
        params: 0,
    }
}

#[test]
fn test_queue_and_retrieve_batch() {
    let (env, client, player) = setup();
    let ops = vec![
        &env,
        make_op(&env, 1, BatchOpType::Upgrade),
        make_op(&env, 2, BatchOpType::Repair),
    ];
    let queued = client.queue_batch_operation(&player, &ops);
    assert_eq!(queued, 2);

    let stored = client.get_player_batch(&player);
    assert!(stored.is_some());
    assert_eq!(stored.unwrap().len(), 2);
}

#[test]
fn test_queue_empty_batch_fails() {
    let (env, client, player) = setup();
    let ops: soroban_sdk::Vec<BatchOp> = soroban_sdk::Vec::new(&env);
    let result = client.try_queue_batch_operation(&player, &ops);
    assert!(result.is_err());
}

#[test]
fn test_queue_exceeds_max_fails() {
    let (env, client, player) = setup();
    let mut ops = soroban_sdk::Vec::new(&env);
    for i in 0..=(MAX_BATCH_SIZE as u64) {
        ops.push_back(make_op(&env, i, BatchOpType::Scan));
    }
    let result = client.try_queue_batch_operation(&player, &ops);
    assert!(result.is_err());
}

#[test]
fn test_execute_batch_all_succeed() {
    let (env, client, player) = setup();
    let ops = vec![
        &env,
        make_op(&env, 1, BatchOpType::Upgrade),
        make_op(&env, 2, BatchOpType::Repair),
        make_op(&env, 3, BatchOpType::Harvest),
    ];
    client.queue_batch_operation(&player, &ops);

    let ship_ids = vec![&env, 1u64, 2u64, 3u64];
    let result = client.execute_batch(&player, &ship_ids);
    assert_eq!(result.total_ops, 3);
    assert_eq!(result.succeeded, 3);
    assert_eq!(result.failed, 0);
}

#[test]
fn test_execute_batch_partial_failure() {
    let (env, client, player) = setup();
    let ops = vec![
        &env,
        make_op(&env, 1, BatchOpType::Upgrade),
        make_op(&env, 99, BatchOpType::Repair), // ship 99 not in fleet
    ];
    client.queue_batch_operation(&player, &ops);

    let ship_ids = vec![&env, 1u64, 2u64]; // only ships 1 and 2
    let result = client.execute_batch(&player, &ship_ids);
    assert_eq!(result.total_ops, 2);
    assert_eq!(result.succeeded, 1);
    assert_eq!(result.failed, 1);
}

#[test]
fn test_execute_batch_clears_queue() {
    let (env, client, player) = setup();
    let ops = vec![&env, make_op(&env, 1, BatchOpType::Scan)];
    client.queue_batch_operation(&player, &ops);

    let ship_ids = vec![&env, 1u64];
    client.execute_batch(&player, &ship_ids);

    // Queue should be cleared after execution
    let stored = client.get_player_batch(&player);
    assert!(stored.is_none());
}

#[test]
fn test_execute_empty_queue_fails() {
    let (env, client, player) = setup();
    let ship_ids = vec![&env, 1u64];
    let result = client.try_execute_batch(&player, &ship_ids);
    assert!(result.is_err());
}

#[test]
fn test_clear_batch() {
    let (env, client, player) = setup();
    let ops = vec![&env, make_op(&env, 1, BatchOpType::Upgrade)];
    client.queue_batch_operation(&player, &ops);

    assert!(client.get_player_batch(&player).is_some());
    client.clear_batch(&player);
    assert!(client.get_player_batch(&player).is_none());
}

#[test]
fn test_max_batch_size_is_eight() {
    assert_eq!(MAX_BATCH_SIZE, 8);
}

#[test]
fn test_full_fleet_upgrade_in_one_tx() {
    let (env, client, player) = setup();
    let mut ops = soroban_sdk::Vec::new(&env);
    let mut ship_ids = soroban_sdk::Vec::new(&env);

    for i in 1u64..=8 {
        ops.push_back(make_op(&env, i, BatchOpType::Upgrade));
        ship_ids.push_back(i);
    }

    client.queue_batch_operation(&player, &ops);
    let result = client.execute_batch(&player, &ship_ids);
    assert_eq!(result.total_ops, 8);
    assert_eq!(result.succeeded, 8);
    assert_eq!(result.failed, 0);
}

#[test]
fn test_multiple_players_isolated_queues() {
    let (env, client, player1) = setup();
    let player2 = Address::generate(&env);

    let ops1 = vec![&env, make_op(&env, 1, BatchOpType::Scan)];
    let ops2 = vec![&env, make_op(&env, 2, BatchOpType::Harvest)];

    client.queue_batch_operation(&player1, &ops1);
    client.queue_batch_operation(&player2, &ops2);

    let q1 = client.get_player_batch(&player1).unwrap();
    let q2 = client.get_player_batch(&player2).unwrap();

    assert_eq!(q1.get(0).unwrap().ship_id, 1);
    assert_eq!(q2.get(0).unwrap().ship_id, 2);
}

#[test]
fn test_different_op_types_accepted() {
    let (env, client, player) = setup();
    let ops = vec![
        &env,
        make_op(&env, 1, BatchOpType::Upgrade),
        make_op(&env, 2, BatchOpType::Repair),
        make_op(&env, 3, BatchOpType::Scan),
        make_op(&env, 4, BatchOpType::Harvest),
    ];
    let queued = client.queue_batch_operation(&player, &ops);
    assert_eq!(queued, 4);
}
