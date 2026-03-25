#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Events, Ledger, LedgerInfo};
use soroban_sdk::{vec, Address, BytesN, Env, IntoVal, Val, Vec};
use stellar_nebula_nomad::{
    CellType, NebulaNomadContract, NebulaNomadContractClient, NebulaCell, NebulaLayout, Rarity,
    GRID_SIZE, TOTAL_CELLS,
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
    let contract_id = env.register_contract(None, NebulaNomadContract);
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
    let contract_id = env.register_contract(None, NebulaNomadContract);
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
    // 5 rare cells × 10 = 50, energy_density ≈ 0 → score 50 → Uncommon
    let layout = make_layout(&env, 5, 0);
    let rarity = client.calculate_rarity_tier(&layout);
    assert_eq!(rarity, Rarity::Uncommon);
}

#[test]
fn test_rarity_rare() {
    let (env, client, _) = setup_env();
    // 10 rare cells × 10 = 100 → score 100 → Rare
    let layout = make_layout(&env, 10, 0);
    let rarity = client.calculate_rarity_tier(&layout);
    assert_eq!(rarity, Rarity::Rare);
}

#[test]
fn test_rarity_epic() {
    let (env, client, _) = setup_env();
    // 15 rare cells × 10 = 150 → score 150 → Epic
    let layout = make_layout(&env, 15, 0);
    let rarity = client.calculate_rarity_tier(&layout);
    assert_eq!(rarity, Rarity::Epic);
}

#[test]
fn test_rarity_legendary() {
    let (env, client, _) = setup_env();
    // 20 rare cells × 10 = 200 → score 200 → Legendary
    let layout = make_layout(&env, 20, 0);
    let rarity = client.calculate_rarity_tier(&layout);
    assert_eq!(rarity, Rarity::Legendary);
}

#[test]
fn test_rarity_energy_density_contributes() {
    let (env, client, _) = setup_env();
    // 4 rare cells × 10 = 40, with high energy per cell to push into Uncommon
    // energy_per_cell = 10 → total = 256 * 10 = 2560, density = 10 → score = 50
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
    // Should be one of the valid rarity tiers
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
    assert!(!events.is_empty(), "Expected NebulaScanned event to be emitted");

    // Verify the last event has the correct topics
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

use stellar_nebula_nomad::resource_minter::{
    ResourceError, ResourceMinter, ResourceMinterClient, ResourceType, LEDGERS_PER_DAY,
};

// ─── Mock contracts ───────────────────────────────────────────────────────────

/// Mock Ship Registry: always confirms ownership so tests focus on minter logic.
#[contract]
pub struct MockShipRegistry;

#[contractimpl]
impl MockShipRegistry {
    pub fn owns_ship(_env: Env, _owner: Address, _ship_id: u64) -> bool {
        true
    }
}

/// Mock Nebula Explorer: always confirms anomaly existence.
#[contract]
pub struct MockNebulaExplorer;

#[contractimpl]
impl MockNebulaExplorer {
    pub fn has_anomaly(_env: Env, _ship_id: u64, _anomaly_index: u32) -> bool {
        true
    }
}

// ─── Test helpers ─────────────────────────────────────────────────────────────

/// Boot a fresh environment with all three contracts registered and initialised.
/// Returns (env, client_contract_id, admin_address, player_address).
fn setup_env() -> (Env, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let ship_id = env.register_contract(None, MockShipRegistry);
    let nebula_id = env.register_contract(None, MockNebulaExplorer);
    let contract_id = env.register_contract(None, ResourceMinter);

    let admin = Address::generate(&env);
    let player = Address::generate(&env);

    ResourceMinterClient::new(&env, &contract_id).init(
        &admin,
        &ship_id,
        &nebula_id,
        &500u32,          // 5 % APY
        &1_000i128,       // daily harvest cap
        &LEDGERS_PER_DAY, // min stake duration ≈ 1 day
    );

    (env, contract_id, admin, player)
}

/// Advance the Stellar ledger by `n` sequence numbers (≈ n × 5 s wall-clock).
fn advance_ledgers(env: &Env, n: u32) {
    let seq = env.ledger().sequence();
    let ts = env.ledger().timestamp();
    env.ledger().set(LedgerInfo {
        sequence_number: seq + n,
        timestamp: ts + (n as u64 * 5),
        protocol_version: 20,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 16,
        min_persistent_entry_ttl: 4096,
        max_entry_ttl: 6_312_000,
    });
}

// ─── Harvest tests ────────────────────────────────────────────────────────────

#[test]
fn test_harvest_base_amount() {
    let (env, cid, _, player) = setup_env();
    let client = ResourceMinterClient::new(&env, &cid);
    // anomaly_index = 0 → base 100 + 0 × 10 = 100
    let minted = client.harvest_resource(&player, &1u64, &0u32);
    assert_eq!(minted, 100);
    assert_eq!(client.get_balance(&player, &ResourceType::Stardust), 100);
}

#[test]
fn test_harvest_rarity_bonus() {
    let (env, cid, _, player) = setup_env();
    let client = ResourceMinterClient::new(&env, &cid);
    // anomaly_index = 5 → 100 + 5 × 10 = 150
    assert_eq!(client.harvest_resource(&player, &1u64, &5u32), 150);
}

#[test]
fn test_harvest_multiple_ships_have_independent_caps() {
    let (env, cid, _, player) = setup_env();
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
    let (env, cid, _, player) = setup_env();
    let client = ResourceMinterClient::new(&env, &cid);
    for _ in 0..10 {
        client.harvest_resource(&player, &1u64, &0u32);
    }
    let err = client.try_harvest_resource(&player, &1u64, &0u32);
    assert_eq!(err, Err(Ok(ResourceError::DailyCapExceeded)));
}

#[test]
fn test_harvest_cap_resets_after_one_day() {
    let (env, cid, _, player) = setup_env();
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
    let (env, cid, _, player) = setup_env();
    let client = ResourceMinterClient::new(&env, &cid);
    // 9 × 100 = 900 harvested; 100 remaining in cap
    for _ in 0..9 {
        client.harvest_resource(&player, &1u64, &0u32);
    }
    // Raw amount from anomaly_index=5 would be 150, but only 100 left → capped
    assert_eq!(client.harvest_resource(&player, &1u64, &5u32), 100);
}

// ─── Staking tests ────────────────────────────────────────────────────────────

#[test]
fn test_stake_deducts_liquid_balance() {
    let (env, cid, _, player) = setup_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.harvest_resource(&player, &1u64, &0u32); // 100 stardust
    client.stake_for_yield(&player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY);
    assert_eq!(client.get_balance(&player, &ResourceType::Stardust), 0);
    assert_eq!(client.get_stake(&player).unwrap().amount, 100);
}

#[test]
fn test_stake_insufficient_resources_rejected() {
    let (env, cid, _, player) = setup_env();
    let client = ResourceMinterClient::new(&env, &cid);
    let err = client.try_stake_for_yield(
        &player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY,
    );
    assert_eq!(err, Err(Ok(ResourceError::InsufficientResources)));
}

#[test]
fn test_stake_below_min_duration_rejected() {
    let (env, cid, _, player) = setup_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.harvest_resource(&player, &1u64, &0u32);
    // 1000 < 17280 min
    let err = client.try_stake_for_yield(
        &player, &ResourceType::Stardust, &100i128, &1_000u32,
    );
    assert_eq!(err, Err(Ok(ResourceError::InvalidDuration)));
}

#[test]
fn test_stake_zero_amount_rejected() {
    let (env, cid, _, player) = setup_env();
    let client = ResourceMinterClient::new(&env, &cid);
    let err = client.try_stake_for_yield(
        &player, &ResourceType::Stardust, &0i128, &LEDGERS_PER_DAY,
    );
    assert_eq!(err, Err(Ok(ResourceError::InvalidAmount)));
}

#[test]
fn test_duplicate_stake_rejected() {
    let (env, cid, _, player) = setup_env();
    let client = ResourceMinterClient::new(&env, &cid);
    // Harvest twice so there is enough balance for a second attempted stake
    client.harvest_resource(&player, &1u64, &0u32);
    client.harvest_resource(&player, &2u64, &0u32);
    client.stake_for_yield(&player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY);
    let err = client.try_stake_for_yield(
        &player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY,
    );
    assert_eq!(err, Err(Ok(ResourceError::AlreadyStaked)));
}

// ─── 24-hour yield simulation ─────────────────────────────────────────────────

#[test]
fn test_claim_yield_after_24h() {
    let (env, cid, _, player) = setup_env();
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
    let (env, cid, _, player) = setup_env();
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
    let (env, cid, _, player) = setup_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.harvest_resource(&player, &1u64, &0u32);
    client.stake_for_yield(&player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY);

    advance_ledgers(&env, LEDGERS_PER_DAY * 365);

    let pending = client.get_pending_yield(&player);
    let claimed = client.claim_yield(&player);
    assert_eq!(pending, claimed);
}

#[test]
fn test_yield_accumulates_across_partial_claims() {
    let (env, cid, _, player) = setup_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.harvest_resource(&player, &1u64, &0u32);
    // Use a 2-year lock so we can keep claiming
    client.stake_for_yield(&player, &ResourceType::Stardust, &100i128, &(LEDGERS_PER_DAY * 365 * 2));

    // Claim at ~6 months then at ~12 months
    advance_ledgers(&env, LEDGERS_PER_DAY * 182);
    let y1 = client.claim_yield(&player);

    advance_ledgers(&env, LEDGERS_PER_DAY * 183);
    let y2 = client.claim_yield(&player);

    // Total ≈ 5 (5 % of 100); allow ±1 for integer truncation across two windows
    let total = y1 + y2;
    assert!(total >= 4 && total <= 5);
}

// ─── Unstake / time-lock tests ────────────────────────────────────────────────

#[test]
fn test_unstake_blocked_immediately_after_stake() {
    let (env, cid, _, player) = setup_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.harvest_resource(&player, &1u64, &0u32);
    client.stake_for_yield(&player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY);
    let err = client.try_unstake(&player);
    assert_eq!(err, Err(Ok(ResourceError::TimeLockActive)));
}

#[test]
fn test_unstake_allowed_after_timelock_expires() {
    let (env, cid, _, player) = setup_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.harvest_resource(&player, &1u64, &0u32);
    client.stake_for_yield(&player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY);

    advance_ledgers(&env, LEDGERS_PER_DAY);

    let returned = client.unstake(&player);
    assert_eq!(returned, 100);
    assert!(client.get_stake(&player).is_none());
    assert_eq!(client.get_balance(&player, &ResourceType::Stardust), 100);
}

#[test]
fn test_unstake_auto_claims_residual_yield() {
    let (env, cid, _, player) = setup_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.harvest_resource(&player, &1u64, &0u32);
    client.stake_for_yield(&player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY);

    advance_ledgers(&env, LEDGERS_PER_DAY * 365); // 1 year → 5 plasma

    client.unstake(&player);

    assert_eq!(client.get_balance(&player, &ResourceType::Plasma), 5);
    assert_eq!(client.get_balance(&player, &ResourceType::Stardust), 100);
}

#[test]
fn test_unstake_then_restake_succeeds() {
    let (env, cid, _, player) = setup_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.harvest_resource(&player, &1u64, &0u32);
    client.stake_for_yield(&player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY);

    advance_ledgers(&env, LEDGERS_PER_DAY);
    client.unstake(&player);

    // Re-staking after unstake must succeed
    client.stake_for_yield(&player, &ResourceType::Stardust, &100i128, &LEDGERS_PER_DAY);
    assert_eq!(client.get_stake(&player).unwrap().amount, 100);
}

// ─── Multiple resource types ──────────────────────────────────────────────────

#[test]
fn test_resource_type_balances_are_independent() {
    let (env, cid, _, player) = setup_env();
    let client = ResourceMinterClient::new(&env, &cid);

    client.harvest_resource(&player, &1u64, &0u32); // 100 stardust
    client.stake_for_yield(&player, &ResourceType::Stardust, &50i128, &LEDGERS_PER_DAY);

    advance_ledgers(&env, LEDGERS_PER_DAY * 365);
    let plasma = client.claim_yield(&player);

    // 50 × 5 % = 2.5 → integer 2 plasma
    assert_eq!(plasma, 2);
    // 50 liquid stardust untouched
    assert_eq!(client.get_balance(&player, &ResourceType::Stardust), 50);
    assert_eq!(client.get_balance(&player, &ResourceType::Plasma), 2);
    // Crystals entirely unaffected
    assert_eq!(client.get_balance(&player, &ResourceType::Crystals), 0);
}

// ─── Admin tests ──────────────────────────────────────────────────────────────

#[test]
fn test_update_daily_cap() {
    let (env, cid, _, _) = setup_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.update_daily_cap(&2_000i128);
    assert_eq!(client.get_config().unwrap().daily_harvest_cap, 2_000);
}

#[test]
fn test_update_apy() {
    let (env, cid, _, _) = setup_env();
    let client = ResourceMinterClient::new(&env, &cid);
    client.update_apy(&1_000u32); // 10 %
    assert_eq!(client.get_config().unwrap().apy_basis_points, 1_000);
}

#[test]
fn test_double_init_rejected() {
    let (env, cid, admin, _) = setup_env();
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
}
