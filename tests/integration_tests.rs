#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Events, Ledger, LedgerInfo};
use soroban_sdk::{vec, Address, Bytes, BytesN, Env, Vec};
use stellar_nebula_nomad::{
    Blueprint, BlueprintError, BlueprintRarity, CellType, NebulaNomadContract,
    NebulaNomadContractClient, NebulaCell, NebulaLayout, ProfileError, ProgressUpdate, Referral,
    ReferralError, Rarity, Session, SessionError, ShipError, GRID_SIZE, TOTAL_CELLS,
    Blueprint, BlueprintError, BlueprintRarity, CellType, Gift, GiftError, NebulaCell,
    NebulaLayout, NebulaNomadContract, NebulaNomadContractClient, ProfileError, ProgressUpdate,
    Rarity, Referral, ReferralError, Session, SessionError, ShipError, GRID_SIZE, TOTAL_CELLS,
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
    let player = Address::generate(&env);
    (env, client, player)
}

// ─── generate_nebula_layout ───────────────────────────────────────────────

#[test]
fn test_generate_layout_dimensions() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[1u8; 32]);
    let layout = client.generate_nebula_layout(&seed, &player);
    assert_eq!(layout.width, GRID_SIZE);
    assert_eq!(layout.height, GRID_SIZE);
    assert_eq!(layout.cells.len(), TOTAL_CELLS);
}

#[test]
fn test_generate_layout_has_energy() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[42u8; 32]);
    let layout = client.generate_nebula_layout(&seed, &player);
    assert!(layout.total_energy > 0);
}

#[test]
fn test_generate_layout_deterministic() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[7u8; 32]);
    let layout1 = client.generate_nebula_layout(&seed, &player);
    let layout2 = client.generate_nebula_layout(&seed, &player);
    assert_eq!(layout1.total_energy, layout2.total_energy);
    assert_eq!(layout1.seed, layout2.seed);
    assert_eq!(layout1.timestamp, layout2.timestamp);
}

#[test]
fn test_different_seeds_produce_different_layouts() {
    let (env, client, player) = setup_env();
    let seed_a = BytesN::from_array(&env, &[1u8; 32]);
    let seed_b = BytesN::from_array(&env, &[2u8; 32]);
    let layout_a = client.generate_nebula_layout(&seed_a, &player);
    let layout_b = client.generate_nebula_layout(&seed_b, &player);
    assert_ne!(layout_a.total_energy, layout_b.total_energy);
}

#[test]
fn test_layout_changes_with_ledger_state() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    let player = Address::generate(&env);
    let seed = BytesN::from_array(&env, &[5u8; 32]);

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
    let layout1 = client.generate_nebula_layout(&seed, &player);

    env.ledger().set(LedgerInfo {
        protocol_version: 22,
        sequence_number: 200,
        timestamp: 2_000_000,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 1000,
        max_entry_ttl: 10_000,
    });
    let layout2 = client.generate_nebula_layout(&seed, &player);

    assert_ne!(layout1.total_energy, layout2.total_energy);
}

#[test]
fn test_layout_cell_coordinates() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[10u8; 32]);
    let layout = client.generate_nebula_layout(&seed, &player);

    for i in 0..layout.cells.len() {
        let cell = layout.cells.get(i).unwrap();
        assert!(cell.x < GRID_SIZE);
        assert!(cell.y < GRID_SIZE);
    }
}

#[test]
fn test_layout_records_timestamp() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[3u8; 32]);
    let layout = client.generate_nebula_layout(&seed, &player);
    assert_eq!(layout.timestamp, 1_700_000_000);
}

#[test]
fn test_zero_seed_works() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[0u8; 32]);
    let layout = client.generate_nebula_layout(&seed, &player);
    assert_eq!(layout.cells.len(), TOTAL_CELLS);
}

// ─── calculate_rarity_tier ────────────────────────────────────────────────

fn make_layout(env: &Env, rare_count: u32, energy_per_cell: u32) -> NebulaLayout {
    let mut cells = Vec::new(env);
    let mut total_energy = 0u32;
    for i in 0..TOTAL_CELLS {
        let (cell_type, energy) = if i < rare_count {
            (CellType::Wormhole, 60 + energy_per_cell)
        } else {
            (CellType::Empty, energy_per_cell)
        };
        total_energy += energy;
        cells.push_back(NebulaCell {
            x: i % GRID_SIZE,
            y: i / GRID_SIZE,
            cell_type,
            energy,
        });
    }
    NebulaLayout {
        width: GRID_SIZE,
        height: GRID_SIZE,
        cells,
        seed: BytesN::from_array(env, &[0u8; 32]),
        timestamp: 0,
        total_energy,
    }
}

#[test]
fn test_rarity_common() {
    let (env, client, _) = setup_env();
    let layout = make_layout(&env, 0, 0);
    let rarity = client.calculate_rarity_tier(&layout);
    assert_eq!(rarity, Rarity::Common);
}

#[test]
fn test_rarity_uncommon() {
    let (env, client, _) = setup_env();
    let layout = make_layout(&env, 5, 0);
    let rarity = client.calculate_rarity_tier(&layout);
    assert_eq!(rarity, Rarity::Uncommon);
}

#[test]
fn test_rarity_rare() {
    let (env, client, _) = setup_env();
    let layout = make_layout(&env, 10, 0);
    let rarity = client.calculate_rarity_tier(&layout);
    assert_eq!(rarity, Rarity::Rare);
}

#[test]
fn test_rarity_epic() {
    let (env, client, _) = setup_env();
    let layout = make_layout(&env, 15, 0);
    let rarity = client.calculate_rarity_tier(&layout);
    assert_eq!(rarity, Rarity::Epic);
}

#[test]
fn test_rarity_legendary() {
    let (env, client, _) = setup_env();
    let layout = make_layout(&env, 20, 0);
    let rarity = client.calculate_rarity_tier(&layout);
    assert_eq!(rarity, Rarity::Legendary);
}

#[test]
fn test_rarity_energy_density_contributes() {
    let (env, client, _) = setup_env();
    let layout = make_layout(&env, 4, 10);
    let rarity = client.calculate_rarity_tier(&layout);
    assert_eq!(rarity, Rarity::Uncommon);
}

#[test]
fn test_rarity_from_generated_layout() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[99u8; 32]);
    let layout = client.generate_nebula_layout(&seed, &player);
    let rarity = client.calculate_rarity_tier(&layout);
    assert!(
        rarity == Rarity::Common
            || rarity == Rarity::Uncommon
            || rarity == Rarity::Rare
            || rarity == Rarity::Epic
            || rarity == Rarity::Legendary
    );
}

// ─── scan_nebula (end-to-end + event emission) ───────────────────────────

#[test]
fn test_scan_nebula_returns_layout_and_rarity() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[50u8; 32]);
    let (layout, rarity) = client.scan_nebula(&seed, &player);
    assert_eq!(layout.width, GRID_SIZE);
    assert_eq!(layout.height, GRID_SIZE);
    assert_eq!(layout.cells.len(), TOTAL_CELLS);
    assert!(
        rarity == Rarity::Common
            || rarity == Rarity::Uncommon
            || rarity == Rarity::Rare
            || rarity == Rarity::Epic
            || rarity == Rarity::Legendary
    );
}

#[test]
fn test_scan_nebula_emits_event() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[77u8; 32]);
    let _result = client.scan_nebula(&seed, &player);

    let events = env.events().all();
    assert!(
        !events.is_empty(),
        "Expected NebulaScanned event to be emitted"
    );

    let last = events.get(events.len() - 1).unwrap();
    let (_contract_addr, topics, _data) = last;
    assert_eq!(topics.len(), 2);
}

#[test]
fn test_scan_nebula_consistency_with_individual_calls() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[33u8; 32]);

    let layout = client.generate_nebula_layout(&seed, &player);
    let rarity = client.calculate_rarity_tier(&layout);

    let (scan_layout, scan_rarity) = client.scan_nebula(&seed, &player);

    assert_eq!(layout.total_energy, scan_layout.total_energy);
    assert_eq!(rarity, scan_rarity);
}

use soroban_sdk::{contract, contractimpl};
use stellar_nebula_nomad::resource_minter::{
    ResourceError, ResourceMinter, ResourceMinterClient, ResourceType, LEDGERS_PER_DAY,
};

// ─── Mock contracts ───────────────────────────────────────────────────────────
// ─── Ship NFT tests ──────────────────────────────────────────────────────

#[test]
fn test_mint_ship_and_transfer_ownership() {
    let (env, client, player) = setup_env();
    let metadata = Bytes::from_slice(&env, &[0u8; 4]);
    let ship = client.mint_ship(&player, &soroban_sdk::symbol_short!("fighter"), &metadata);
    assert_eq!(ship.owner, player);

    let new_owner = Address::generate(&env);
    let transferred = client.transfer_ownership(&ship.id, &new_owner);
    assert_eq!(transferred.owner, new_owner);
}

#[test]
fn test_batch_mint_limit_and_invalid_ship_type() {
    let (env, client, player) = setup_env();
    let metadata = Bytes::from_slice(&env, &[0u8; 4]);
    let types = vec![
        &env,
        soroban_sdk::symbol_short!("fighter"),
        soroban_sdk::symbol_short!("explorer"),
        soroban_sdk::symbol_short!("hauler"),
    ];
    let ships = client.batch_mint_ships(&player, &types, &metadata);
    assert_eq!(ships.len(), 3);
}

// ─── Harvest tests ───────────────────────────────────────────────────────

/// Boot a fresh environment with all three contracts registered and initialised.
/// Returns (env, client_contract_id, admin_address, player_address).
fn setup_minter_env() -> (Env, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(LedgerInfo {
        protocol_version: 22,
        sequence_number: 0,
        timestamp: 0,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 6_312_000,
        max_entry_ttl: 6_312_000,
    });
#[test]
fn test_harvest_resources_single_invocation_and_events() {
    let (env, client, player) = setup_env();
    let metadata = Bytes::from_slice(&env, &[0u8; 4]);

    // Mint a ship first
    let ship = client.mint_ship(&player, &soroban_sdk::symbol_short!("explorer"), &metadata);

    // Generate a layout
    let seed = BytesN::from_array(&env, &[42u8; 32]);
    let layout = client.generate_nebula_layout(&seed, &player);

    // Harvest resources from the layout
    let harvest = client.harvest_resources(&ship.id, &layout);
    assert_eq!(harvest.ship_id, ship.id);
    assert!(harvest.total_harvested > 0);

    // Verify events were emitted
    let events = env.events().all();
    assert!(!events.is_empty());
}

/// Advance the Stellar ledger by `n` sequence numbers (≈ n × 5 s wall-clock).
fn advance_ledgers(env: &Env, n: u32) {
    let seq = env.ledger().sequence();
    let ts = env.ledger().timestamp();
    env.ledger().set(LedgerInfo {
        sequence_number: seq + n,
        timestamp: ts + (n as u64 * 5),
        protocol_version: 22,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 16,
        min_persistent_entry_ttl: 6_312_000,
        max_entry_ttl: 6_312_000,
    });
#[test]
fn test_harvest_fails_for_unknown_ship() {
    let (env, client, player) = setup_env();
    let seed = BytesN::from_array(&env, &[42u8; 32]);
    let layout = client.generate_nebula_layout(&seed, &player);

    // Ship ID 9999 does not exist
    let result = client.try_harvest_resources(&9999u64, &layout);
    assert!(result.is_err());
}

// ─── player profile (issue #15) ───────────────────────────────────────────────

#[test]
fn test_harvest_base_amount() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    // anomaly_index = 0 → base 100 + 0 × 10 = 100
    let minted = client.harvest_resource(&player, &1u64, &0u32);
    assert_eq!(minted, 100);
    assert_eq!(client.get_balance(&player, &ResourceType::Stardust), 100);
}

#[test]
fn test_harvest_rarity_bonus() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    // anomaly_index = 5 → 100 + 5 × 10 = 150
    assert_eq!(client.harvest_resource(&player, &1u64, &5u32), 150);
}

#[test]
fn test_harvest_multiple_ships_have_independent_caps() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    // Drain ship 1's daily cap (10 × 100 = 1000)
    for _ in 0..10 {
        client.harvest_resource(&player, &1u64, &0u32);
    }
    // Ship 2 uses its own independent cap — must succeed
    assert_eq!(client.harvest_resource(&player, &2u64, &0u32), 100);
}

#[test]
fn test_harvest_daily_cap_enforced() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    for _ in 0..10 {
        client.harvest_resource(&player, &1u64, &0u32);
    }
    let err = client.try_harvest_resource(&player, &1u64, &0u32);
    assert_eq!(err, Err(Ok(ResourceError::DailyCapExceeded)));
}

#[test]
fn test_harvest_cap_resets_after_one_day() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    for _ in 0..10 {
        client.harvest_resource(&player, &1u64, &0u32);
    }
    advance_ledgers(&env, LEDGERS_PER_DAY);
    // Should succeed again after window reset
    assert_eq!(client.harvest_resource(&player, &1u64, &0u32), 100);
}

#[test]
fn test_harvest_amount_capped_near_daily_limit() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    // 9 × 100 = 900 harvested; 100 remaining in cap
    for _ in 0..9 {
        client.harvest_resource(&player, &1u64, &0u32);
    }
    // Raw amount from anomaly_index=5 would be 150, but only 100 left → capped
    assert_eq!(client.harvest_resource(&player, &1u64, &5u32), 100);
fn test_initialize_profile_success() {
    let (env, client, player) = setup_env();
    let id = client.initialize_profile(&player);
    assert_eq!(id, 1);
}

#[test]
fn test_initialize_profile_increments_id() {
    let (env, client, _) = setup_env();
    let player_a = Address::generate(&env);
    let player_b = Address::generate(&env);
    let id_a = client.initialize_profile(&player_a);
    let id_b = client.initialize_profile(&player_b);
    assert_eq!(id_a, 1);
    assert_eq!(id_b, 2);
}

#[test]
#[should_panic]
fn test_initialize_profile_duplicate_panics() {
    let (env, client, player) = setup_env();
    client.initialize_profile(&player);
    client.initialize_profile(&player);
}

#[test]
fn test_get_profile_returns_correct_owner() {
    let (env, client, player) = setup_env();
    let id = client.initialize_profile(&player);
    let profile = client.get_profile(&id);
    assert_eq!(profile.owner, player);
    assert_eq!(profile.total_scans, 0);
    assert_eq!(profile.essence_earned, 0);
}

#[test]
#[should_panic]
fn test_get_profile_not_found_panics() {
    let (_env, client, _) = setup_env();
    client.get_profile(&999u64);
}

#[test]
fn test_update_progress_accumulates_stats() {
    let (env, client, player) = setup_env();
    let id = client.initialize_profile(&player);
    client.update_progress(&player, &id, &3u32, &500i128);
    client.update_progress(&player, &id, &2u32, &250i128);
    let profile = client.get_profile(&id);
    assert_eq!(profile.total_scans, 5);
    assert_eq!(profile.essence_earned, 750);
}

#[test]
fn test_stake_deducts_liquid_balance() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.harvest_resource(&player, &1u64, &0u32); // 100 stardust
    client.stake_for_yield(&player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY);
    assert_eq!(client.get_balance(&player, &ResourceType::Stardust), 0);
    assert_eq!(client.get_stake(&player).unwrap().amount, 100);
}

#[test]
fn test_stake_insufficient_resources_rejected() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    let err = client.try_stake_for_yield(
        &player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY,
    );
    assert_eq!(err, Err(Ok(ResourceError::InsufficientResources)));
}

#[test]
fn test_stake_below_min_duration_rejected() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.harvest_resource(&player, &1u64, &0u32);
    // 1000 < 17280 min
    let err = client.try_stake_for_yield(
        &player, &ResourceType::Stardust, &100i128, &1_000u32,
    );
    assert_eq!(err, Err(Ok(ResourceError::InvalidDuration)));
#[should_panic]
fn test_update_progress_wrong_caller_panics() {
    let (env, client, player) = setup_env();
    let intruder = Address::generate(&env);
    let id = client.initialize_profile(&player);
    client.update_progress(&intruder, &id, &1u32, &100i128);
}

#[test]
fn test_batch_update_progress_applies_all() {
    let (env, client, player) = setup_env();
    let id = client.initialize_profile(&player);
    let updates = soroban_sdk::vec![
        &env,
        ProgressUpdate { profile_id: id, scan_count: 1, essence: 100 },
        ProgressUpdate { profile_id: id, scan_count: 2, essence: 200 },
        ProgressUpdate { profile_id: id, scan_count: 1, essence: 50  },
    ];
    client.batch_update_progress(&player, &updates);
    let profile = client.get_profile(&id);
    assert_eq!(profile.total_scans, 4);
    assert_eq!(profile.essence_earned, 350);
}

#[test]
#[should_panic]
fn test_batch_update_exceeds_limit_panics() {
    let (env, client, player) = setup_env();
    let id = client.initialize_profile(&player);
    let updates = soroban_sdk::vec![
        &env,
        ProgressUpdate { profile_id: id, scan_count: 1, essence: 10 },
        ProgressUpdate { profile_id: id, scan_count: 1, essence: 10 },
        ProgressUpdate { profile_id: id, scan_count: 1, essence: 10 },
        ProgressUpdate { profile_id: id, scan_count: 1, essence: 10 },
        ProgressUpdate { profile_id: id, scan_count: 1, essence: 10 },
        ProgressUpdate { profile_id: id, scan_count: 1, essence: 10 },
    ];
    client.batch_update_progress(&player, &updates);
}

#[test]
fn test_profile_emits_nomad_joined_event() {
    let (env, client, player) = setup_env();
    client.initialize_profile(&player);
    let events = env.events().all();
    assert!(!events.is_empty());
}


// ─── session manager (issue #16) ──────────────────────────────────────────────

#[test]
fn test_stake_zero_amount_rejected() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    let err = client.try_stake_for_yield(
        &player, &ResourceType::Stardust, &0i128, &LEDGERS_PER_DAY,
    );
    assert_eq!(err, Err(Ok(ResourceError::InvalidAmount)));
}

#[test]
fn test_duplicate_stake_rejected() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    // Harvest twice so there is enough balance for a second attempted stake
    client.harvest_resource(&player, &1u64, &0u32);
    client.harvest_resource(&player, &2u64, &0u32);
    client.stake_for_yield(&player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY);
    let err = client.try_stake_for_yield(
        &player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY,
    );
    assert_eq!(err, Err(Ok(ResourceError::AlreadyStaked)));
fn test_start_session_success() {
    let (env, client, player) = setup_env();
    let session_id = client.start_session(&player, &42u64);
    assert_eq!(session_id, 1);
}

#[test]
fn test_start_session_records_expiry() {
    let (env, client, player) = setup_env();
    let session_id = client.start_session(&player, &1u64);
    let session = client.get_session(&session_id);
    assert_eq!(session.started_at, 1_700_000_000);
    assert_eq!(session.expires_at, 1_700_000_000 + 86_400);
    assert!(session.active);
}

#[test]
fn test_claim_yield_after_24h() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.harvest_resource(&player, &1u64, &0u32); // 100 stardust
    client.stake_for_yield(&player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY);

    advance_ledgers(&env, LEDGERS_PER_DAY); // +1 day

    let yield_earned = client.claim_yield(&player);
    // 100 × 5% / 365 ≈ 0.0136 → integer truncates to 0; accumulates over weeks
    assert!(yield_earned >= 0);
    assert_eq!(client.get_balance(&player, &ResourceType::Plasma), yield_earned);
}

#[test]
fn test_claim_yield_after_1_year() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.harvest_resource(&player, &1u64, &0u32); // 100 stardust
    client.stake_for_yield(&player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY);

    advance_ledgers(&env, LEDGERS_PER_DAY * 365); // +365 days

    let yield_earned = client.claim_yield(&player);
    // 100 × 500 / 10_000 × (17280×365) / (17280×365) = 5 plasma
    assert_eq!(yield_earned, 5);
    assert_eq!(client.get_balance(&player, &ResourceType::Plasma), 5);
}

#[test]
fn test_pending_yield_matches_claim_amount() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.harvest_resource(&player, &1u64, &0u32);
    client.stake_for_yield(&player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY);

    advance_ledgers(&env, LEDGERS_PER_DAY * 365);
fn test_start_multiple_sessions_up_to_limit() {
    let (env, client, player) = setup_env();
    client.start_session(&player, &1u64);
    client.start_session(&player, &2u64);
    let id3 = client.start_session(&player, &3u64);
    assert_eq!(id3, 3);
}

#[test]
#[should_panic]
fn test_start_session_exceeds_limit_panics() {
    let (env, client, player) = setup_env();
    client.start_session(&player, &1u64);
    client.start_session(&player, &2u64);
    client.start_session(&player, &3u64);
    client.start_session(&player, &4u64); // 4th session — must panic
}

#[test]
fn test_expire_session_by_owner() {
    let (env, client, player) = setup_env();
    let id = client.start_session(&player, &1u64);
    client.expire_session(&player, &id);
    let session = client.get_session(&id);
    assert!(!session.active);
}

#[test]
fn test_expire_session_frees_slot_for_new_session() {
    let (env, client, player) = setup_env();
    client.start_session(&player, &1u64);
    client.start_session(&player, &2u64);
    let id3 = client.start_session(&player, &3u64);
    client.expire_session(&player, &id3);
    // slot freed — fourth session should succeed now
    let id4 = client.start_session(&player, &4u64);
    assert_eq!(id4, 4);
}

#[test]
fn test_yield_accumulates_across_partial_claims() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.harvest_resource(&player, &1u64, &0u32);
    // Use a 2-year lock so we can keep claiming
    client.stake_for_yield(&player, &ResourceType::Stardust, &100i128, &(LEDGERS_PER_DAY * 365 * 2));
#[should_panic]
fn test_expire_already_expired_session_panics() {
    let (env, client, player) = setup_env();
    let id = client.start_session(&player, &1u64);
    client.expire_session(&player, &id);
    client.expire_session(&player, &id); // already inactive — must panic
}

#[test]
fn test_session_emits_started_event() {
    let (env, client, player) = setup_env();
    client.start_session(&player, &1u64);
    let events = env.events().all();
    assert!(!events.is_empty());
}

// ─── blueprint factory (issue #17) ────────────────────────────────────────────

fn make_components(env: &Env, symbols: &[&str]) -> soroban_sdk::Vec<soroban_sdk::Symbol> {
    let mut v = soroban_sdk::Vec::new(env);
    for s in symbols {
        v.push_back(soroban_sdk::Symbol::new(env, s));
    }
    v
}

#[test]
fn test_craft_blueprint_success() {
    let (env, client, player) = setup_env();
    let components = make_components(&env, &["iron", "gas"]);
    let id = client.craft_blueprint(&player, &components);
    assert_eq!(id, 1);
}

#[test]
fn test_unstake_blocked_immediately_after_stake() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.harvest_resource(&player, &1u64, &0u32);
    client.stake_for_yield(&player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY);
    let err = client.try_unstake(&player);
    assert_eq!(err, Err(Ok(ResourceError::TimeLockActive)));
}

#[test]
fn test_unstake_allowed_after_timelock_expires() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.harvest_resource(&player, &1u64, &0u32);
    client.stake_for_yield(&player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY);
fn test_craft_blueprint_rarity_common() {
    let (env, client, player) = setup_env();
    let components = make_components(&env, &["iron", "gas"]);
    let id = client.craft_blueprint(&player, &components);
    let bp = client.get_blueprint(&id);
    assert_eq!(bp.rarity, BlueprintRarity::Common);
    assert!(!bp.applied);
}

#[test]
fn test_craft_blueprint_rarity_uncommon() {
    let (env, client, player) = setup_env();
    let components = make_components(&env, &["iron", "gas", "dust", "void"]);
    let id = client.craft_blueprint(&player, &components);
    let bp = client.get_blueprint(&id);
    assert_eq!(bp.rarity, BlueprintRarity::Uncommon);
}

#[test]
fn test_craft_blueprint_rarity_rare() {
    let (env, client, player) = setup_env();
    let components = make_components(&env, &["a", "b", "c", "d", "e", "f"]);
    let id = client.craft_blueprint(&player, &components);
    let bp = client.get_blueprint(&id);
    assert_eq!(bp.rarity, BlueprintRarity::Rare);
}

#[test]
#[should_panic]
fn test_craft_blueprint_too_few_components_panics() {
    let (env, client, player) = setup_env();
    let components = make_components(&env, &["iron"]); // only 1 — must panic
    client.craft_blueprint(&player, &components);
}

#[test]
fn test_unstake_auto_claims_residual_yield() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.harvest_resource(&player, &1u64, &0u32);
    client.stake_for_yield(&player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY);
fn test_apply_blueprint_to_ship() {
    let (env, client, player) = setup_env();
    let components = make_components(&env, &["iron", "gas"]);
    let bp_id = client.craft_blueprint(&player, &components);
    client.apply_blueprint_to_ship(&player, &bp_id, &10u64);
    let bp = client.get_blueprint(&bp_id);
    assert!(bp.applied);
}

#[test]
#[should_panic]
fn test_apply_blueprint_twice_panics() {
    let (env, client, player) = setup_env();
    let components = make_components(&env, &["iron", "gas"]);
    let bp_id = client.craft_blueprint(&player, &components);
    client.apply_blueprint_to_ship(&player, &bp_id, &10u64);
    client.apply_blueprint_to_ship(&player, &bp_id, &10u64); // already applied — must panic
}

#[test]
#[should_panic]
fn test_apply_blueprint_wrong_owner_panics() {
    let (env, client, player) = setup_env();
    let intruder = Address::generate(&env);
    let components = make_components(&env, &["iron", "gas"]);
    let bp_id = client.craft_blueprint(&player, &components);
    client.apply_blueprint_to_ship(&intruder, &bp_id, &10u64); // not owner — must panic
}

#[test]
fn test_batch_craft_blueprints() {
    let (env, client, player) = setup_env();
    let r1 = make_components(&env, &["iron", "gas"]);
    let r2 = make_components(&env, &["dust", "void"]);
    let mut recipes = soroban_sdk::Vec::new(&env);
    recipes.push_back(r1);
    recipes.push_back(r2);
    let ids = client.batch_craft_blueprints(&player, &recipes);
    assert_eq!(ids.len(), 2);
}

#[test]
fn test_unstake_then_restake_succeeds() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.harvest_resource(&player, &1u64, &0u32);
    client.stake_for_yield(&player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY);
#[should_panic]
fn test_batch_craft_exceeds_limit_panics() {
    let (env, client, player) = setup_env();
    let r = make_components(&env, &["iron", "gas"]);
    let mut recipes = soroban_sdk::Vec::new(&env);
    recipes.push_back(r.clone());
    recipes.push_back(r.clone());
    recipes.push_back(r); // 3 > MAX_BATCH_CRAFT — must panic
    client.batch_craft_blueprints(&player, &recipes);
}

// ─── referral system (issue #19) ──────────────────────────────────────────────

#[test]
fn test_register_referral_success() {
    let (env, client, referrer) = setup_env();
    let new_nomad = Address::generate(&env);
    let id = client.register_referral(&referrer, &new_nomad);
    assert_eq!(id, 1);
}

#[test]
fn test_resource_type_balances_are_independent() {
    let (env, cid, _, player) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
fn test_get_referral_stores_correct_data() {
    let (env, client, referrer) = setup_env();
    let new_nomad = Address::generate(&env);
    client.register_referral(&referrer, &new_nomad);
    let referral = client.get_referral(&new_nomad);
    assert_eq!(referral.referrer, referrer);
    assert_eq!(referral.new_nomad, new_nomad);
    assert!(!referral.claimed);
    assert!(!referral.first_scan_done);
}

#[test]
#[should_panic]
fn test_register_referral_self_panics() {
    let (env, client, player) = setup_env();
    client.register_referral(&player, &player); // self-referral — must panic
}

#[test]
#[should_panic]
fn test_register_referral_duplicate_panics() {
    let (env, client, referrer) = setup_env();
    let new_nomad = Address::generate(&env);
    client.register_referral(&referrer, &new_nomad);
    client.register_referral(&referrer, &new_nomad); // already referred — must panic
}

#[test]
fn test_mark_first_scan_and_claim_reward() {
    let (env, client, referrer) = setup_env();
    let new_nomad = Address::generate(&env);
    client.register_referral(&referrer, &new_nomad);
    client.mark_first_scan(&new_nomad);
    let reward = client.claim_referral_reward(&referrer, &new_nomad);
    assert_eq!(reward, 100);
}

#[test]
#[should_panic]
fn test_claim_reward_before_first_scan_panics() {
    let (env, client, referrer) = setup_env();
    let new_nomad = Address::generate(&env);
    client.register_referral(&referrer, &new_nomad);
    client.claim_referral_reward(&referrer, &new_nomad); // scan not done — must panic
}

#[test]
fn test_update_daily_cap() {
    let (env, cid, _, _) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.update_daily_cap(&2_000i128);
    assert_eq!(client.get_config().unwrap().daily_harvest_cap, 2_000);
}

#[test]
fn test_update_apy() {
    let (env, cid, _, _) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.update_apy(&1_000u32); // 10 %
    assert_eq!(client.get_config().unwrap().apy_basis_points, 1_000);
}

#[test]
fn test_double_init_rejected() {
    let (env, cid, admin, _) = setup_minter_env();
    let client = ResourceMinterClient::new(&env, &cid);
    let dummy = Address::generate(&env);
    let err = client.try_init(
        &admin,
        &dummy,
        &dummy,
        &500u32,
        &1_000i128,
        &LEDGERS_PER_DAY,
    );
    assert_eq!(err, Err(Ok(ResourceError::AlreadyInitialized)));
#[should_panic]
fn test_claim_reward_twice_panics() {
    let (env, client, referrer) = setup_env();
    let new_nomad = Address::generate(&env);
    client.register_referral(&referrer, &new_nomad);
    client.mark_first_scan(&new_nomad);
    client.claim_referral_reward(&referrer, &new_nomad);
    client.claim_referral_reward(&referrer, &new_nomad); // already claimed — must panic
}

#[test]
fn test_referral_claimed_flag_set_after_claim() {
    let (env, client, referrer) = setup_env();
    let new_nomad = Address::generate(&env);
    client.register_referral(&referrer, &new_nomad);
    client.mark_first_scan(&new_nomad);
    client.claim_referral_reward(&referrer, &new_nomad);
    let referral = client.get_referral(&new_nomad);
    assert!(referral.claimed);
    assert!(referral.first_scan_done);
}

#[test]
fn test_referral_emits_registered_event() {
    let (env, client, referrer) = setup_env();
    let new_nomad = Address::generate(&env);
    client.register_referral(&referrer, &new_nomad);
    let events = env.events().all();
    assert!(!events.is_empty());
}
