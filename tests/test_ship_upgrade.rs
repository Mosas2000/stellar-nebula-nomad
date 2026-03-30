#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{symbol_short, Address, Env, Map};
use stellar_nebula_nomad::{
    NebulaNomadContract, NebulaNomadContractClient, ShipUpgradeError, UpgradeBlueprint,
};
use stellar_nebula_nomad::ship_upgrade::{MAX_BATCH_UPGRADES, MAX_MODULES, MAX_MASS};

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
        min_persistent_entry_ttl: 6_312_000,
        max_entry_ttl: 6_312_000,
    });
    let contract_id = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    (env, client, admin)
}

fn make_blueprints(env: &Env) -> Map<soroban_sdk::Symbol, UpgradeBlueprint> {
    let mut blueprints = Map::new(env);
    blueprints.set(
        symbol_short!("scanner"),
        UpgradeBlueprint {
            asset_id:      symbol_short!("dust"),
            resource_cost: 100,
            mass:          10,
            scanner_bonus: 5,
            hull_bonus:    0,
        },
    );
    blueprints.set(
        symbol_short!("hull"),
        UpgradeBlueprint {
            asset_id:      symbol_short!("ore"),
            resource_cost: 80,
            mass:          20,
            scanner_bonus: 0,
            hull_bonus:    15,
        },
    );
    blueprints
}

/// Write a resource balance directly into contract instance storage,
/// mirroring the key layout used by resource_minter::ResourceKey::ResourceBalance.
fn credit_resource(
    env: &Env,
    contract_id: &Address,
    player: &Address,
    asset_id: soroban_sdk::Symbol,
    amount: u32,
) {
    use soroban_sdk::contracttype;

    #[contracttype]
    #[derive(Clone)]
    enum ResourceKey {
        ResourceCounter,
        HarvestCounter,
        DexOfferCounter,
        ResourceBalance(Address, soroban_sdk::Symbol),
        DexOffer(u64),
    }

    env.as_contract(contract_id, || {
        let key = ResourceKey::ResourceBalance(player.clone(), asset_id);
        env.storage().instance().set(&key, &amount);
    });
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[test]
fn test_init_upgrade_config_succeeds() {
    let (env, client, admin) = setup();
    client.init_upgrade_config(&admin, &make_blueprints(&env));
}

#[test]
fn test_init_upgrade_config_rejects_double_init() {
    let (env, client, admin) = setup();
    let bp = make_blueprints(&env);
    client.init_upgrade_config(&admin, &bp);
    let err = client.try_init_upgrade_config(&admin, &bp).unwrap_err();
    assert_eq!(err, Ok(ShipUpgradeError::AlreadyInitialized));
}

#[test]
fn test_apply_upgrade_updates_ship_state() {
    let (env, client, admin) = setup();
    client.init_upgrade_config(&admin, &make_blueprints(&env));

    let player = Address::generate(&env);
    credit_resource(&env, &client.address, &player, symbol_short!("dust"), 200);

    assert!(client.get_ship_state(&1u64).is_none());

    let state = client.apply_upgrade(&player, &1u64, &symbol_short!("scanner"));
    assert_eq!(state.ship_id,       1);
    assert_eq!(state.module_count,  1);
    assert_eq!(state.total_mass,    10);
    assert_eq!(state.scanner_bonus, 5);
    assert_eq!(state.hull_bonus,    0);

    let stored = client.get_ship_state(&1u64).unwrap();
    assert_eq!(stored.module_count, 1);
}

#[test]
fn test_apply_upgrade_burns_resource() {
    let (env, client, admin) = setup();
    client.init_upgrade_config(&admin, &make_blueprints(&env));

    let player = Address::generate(&env);
    // Exactly one scanner's worth of dust.
    credit_resource(&env, &client.address, &player, symbol_short!("dust"), 100);

    client.apply_upgrade(&player, &2u64, &symbol_short!("scanner"));

    // Balance is now 0 — second upgrade must fail.
    let err = client
        .try_apply_upgrade(&player, &2u64, &symbol_short!("scanner"))
        .unwrap_err();
    assert_eq!(err, Ok(ShipUpgradeError::InsufficientResources));
}

#[test]
fn test_apply_upgrade_fails_unknown_component() {
    let (env, client, admin) = setup();
    client.init_upgrade_config(&admin, &make_blueprints(&env));
    let player = Address::generate(&env);
    let err = client
        .try_apply_upgrade(&player, &3u64, &symbol_short!("warp"))
        .unwrap_err();
    assert_eq!(err, Ok(ShipUpgradeError::UnknownComponent));
}

#[test]
fn test_apply_upgrade_fails_without_init() {
    let (env, client, _) = setup();
    let player = Address::generate(&env);
    let err = client
        .try_apply_upgrade(&player, &4u64, &symbol_short!("scanner"))
        .unwrap_err();
    assert_eq!(err, Ok(ShipUpgradeError::NotInitialized));
}

#[test]
fn test_invariant_module_cap_enforced() {
    let (env, client, admin) = setup();
    client.init_upgrade_config(&admin, &make_blueprints(&env));

    let player = Address::generate(&env);
    // Fund enough for MAX_MODULES + 1 installs (each costs 100 dust, mass 10).
    credit_resource(&env, &client.address, &player, symbol_short!("dust"), 100 * (MAX_MODULES + 1));

    for _ in 0..MAX_MODULES {
        client.apply_upgrade(&player, &5u64, &symbol_short!("scanner"));
    }

    // (MAX_MODULES + 1)th install must be rejected.
    let err = client
        .try_apply_upgrade(&player, &5u64, &symbol_short!("scanner"))
        .unwrap_err();
    assert_eq!(err, Ok(ShipUpgradeError::InvariantViolation));
}

#[test]
fn test_invariant_mass_cap_enforced() {
    let (env, client, admin) = setup();
    // A heavy component: 60 mass each — two exceed MAX_MASS (100).
    let mut blueprints = Map::new(&env);
    blueprints.set(
        symbol_short!("heavy"),
        UpgradeBlueprint {
            asset_id:      symbol_short!("ore"),
            resource_cost: 10,
            mass:          60,
            scanner_bonus: 0,
            hull_bonus:    5,
        },
    );
    client.init_upgrade_config(&admin, &blueprints);

    let player = Address::generate(&env);
    credit_resource(&env, &client.address, &player, symbol_short!("ore"), 100);

    client.apply_upgrade(&player, &6u64, &symbol_short!("heavy"));

    let err = client
        .try_apply_upgrade(&player, &6u64, &symbol_short!("heavy"))
        .unwrap_err();
    assert_eq!(err, Ok(ShipUpgradeError::InvariantViolation));
}

#[test]
fn test_batch_upgrade_applies_two_components() {
    let (env, client, admin) = setup();
    client.init_upgrade_config(&admin, &make_blueprints(&env));

    let player = Address::generate(&env);
    credit_resource(&env, &client.address, &player, symbol_short!("dust"), 300);
    credit_resource(&env, &client.address, &player, symbol_short!("ore"),  200);

    let components = soroban_sdk::vec![&env, symbol_short!("scanner"), symbol_short!("hull")];
    let results = client.batch_upgrade(&player, &7u64, &components);

    assert_eq!(results.len(), 2);
    assert_eq!(results.get(0).unwrap().module_count, 1);
    assert_eq!(results.get(1).unwrap().module_count, 2);
}

#[test]
fn test_batch_upgrade_rejects_oversized_batch() {
    let (env, client, admin) = setup();
    client.init_upgrade_config(&admin, &make_blueprints(&env));
    let player = Address::generate(&env);

    // Three components exceed MAX_BATCH_UPGRADES (2).
    let components = soroban_sdk::vec![
        &env,
        symbol_short!("scanner"),
        symbol_short!("hull"),
        symbol_short!("scanner"),
    ];
    let err = client
        .try_batch_upgrade(&player, &8u64, &components)
        .unwrap_err();
    assert_eq!(err, Ok(ShipUpgradeError::BatchTooLarge));
}

#[test]
fn test_stats_accumulate_across_upgrades() {
    let (env, client, admin) = setup();
    client.init_upgrade_config(&admin, &make_blueprints(&env));

    let player = Address::generate(&env);
    credit_resource(&env, &client.address, &player, symbol_short!("dust"), 500);
    credit_resource(&env, &client.address, &player, symbol_short!("ore"),  500);

    client.apply_upgrade(&player, &9u64, &symbol_short!("scanner"));
    client.apply_upgrade(&player, &9u64, &symbol_short!("hull"));

    let state = client.get_ship_state(&9u64).unwrap();
    assert_eq!(state.module_count,  2);
    assert_eq!(state.total_mass,    30); // scanner(10) + hull(20)
    assert_eq!(state.scanner_bonus, 5);
    assert_eq!(state.hull_bonus,   15);
}

#[test]
fn test_max_batch_upgrades_constant_is_two() {
    assert_eq!(MAX_BATCH_UPGRADES, 2);
}

#[test]
fn test_max_modules_and_mass_constants() {
    assert_eq!(MAX_MODULES, 5);
    assert_eq!(MAX_MASS, 100);
}
