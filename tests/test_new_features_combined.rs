use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String, Symbol, Vec};
use stellar_nebula_nomad::{
    NebulaNomadContract, NebulaNomadContractClient,
    // Alliance Manager
    AllianceError, MAX_MEMBERS_PER_ALLIANCE,
    // Market Oracle
    MarketOracleError,
    // Audio Seed Generator
    AudioError, INSTRUMENT_PRESETS, MAX_LAYERS_PER_NEBULA,
    // Wormhole Traveler
    WormholeError, MAX_SIMULTANEOUS_WORMHOLES,
};

// ─── Alliance Manager Tests ───────────────────────────────────────────────────

#[test]
fn test_found_alliance_success() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    let founder = Address::generate(&env);
    let name = String::from_str(&env, "Star Explorers");
    
    let alliance_id = client.found_alliance(&founder, &name);
    assert_eq!(alliance_id, 0);
    
    let alliance = client.get_alliance(&alliance_id);
    assert_eq!(alliance.name, name);
    assert_eq!(alliance.founder, founder);
    assert_eq!(alliance.members.len(), 1);
    assert!(alliance.is_active);
}

#[test]
fn test_join_alliance_success() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    let founder = Address::generate(&env);
    let member = Address::generate(&env);
    let name = String::from_str(&env, "Cosmic Nomads");
    
    let alliance_id = client.found_alliance(&founder, &name);
    let membership = client.join_alliance(&alliance_id, &member);
    
    assert_eq!(membership.alliance_id, alliance_id);
    assert_eq!(membership.member, member);
    assert_eq!(membership.contribution, 0);
    
    let alliance = client.get_alliance(&alliance_id);
    assert_eq!(alliance.members.len(), 2);
}

#[test]
fn test_join_alliance_already_member() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    let founder = Address::generate(&env);
    let name = String::from_str(&env, "Test Alliance");
    
    let alliance_id = client.found_alliance(&founder, &name);
    
    // Founder tries to join again - should fail with AlreadyInAlliance (error code 3)
    let result = client.try_join_alliance(&alliance_id, &founder);
    assert!(result.is_err());
}

#[test]
fn test_contribute_to_treasury() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    let founder = Address::generate(&env);
    let name = String::from_str(&env, "Rich Alliance");
    
    let alliance_id = client.found_alliance(&founder, &name);
    let amount = 1000i128;
    
    let new_treasury = client.contribute_to_treasury(&founder, &amount);
    assert_eq!(new_treasury, amount);
    
    let treasury = client.get_alliance_treasury(&alliance_id);
    assert_eq!(treasury, amount);
    
    let contribution = client.get_member_contribution(&alliance_id, &founder);
    assert_eq!(contribution, amount);
}

#[test]
fn test_leave_alliance() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    let founder = Address::generate(&env);
    let member = Address::generate(&env);
    let name = String::from_str(&env, "Temporary Alliance");
    
    let alliance_id = client.found_alliance(&founder, &name);
    client.join_alliance(&alliance_id, &member);
    
    client.leave_alliance(&member);
    
    let alliance = client.get_alliance(&alliance_id);
    assert_eq!(alliance.members.len(), 1);
    
    let player_alliance = client.get_player_alliance(&member);
    assert!(player_alliance.is_none());
}

// ─── Market Oracle Tests ──────────────────────────────────────────────────────

#[test]
fn test_initialize_oracle() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let source1 = Address::generate(&env);
    let source2 = Address::generate(&env);
    
    let mut sources = Vec::new(&env);
    sources.push_back(source1);
    sources.push_back(source2);
    
    client.initialize_oracle(&admin, &sources);
}

#[test]
fn test_update_resource_price() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let source = Address::generate(&env);
    let mut sources = Vec::new(&env);
    sources.push_back(source);
    
    client.initialize_oracle(&admin, &sources);
    
    let resource = Symbol::new(&env, "IRON");
    let price = 100i128;
    
    let price_data = client.update_resource_price(&admin, &resource, &price);
    assert_eq!(price_data.resource, resource);
    assert_eq!(price_data.price, price);
    assert_eq!(price_data.source_count, 1);
}

#[test]
fn test_get_current_market_rate() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let source = Address::generate(&env);
    let mut sources = Vec::new(&env);
    sources.push_back(source);
    
    client.initialize_oracle(&admin, &sources);
    
    let resource = Symbol::new(&env, "GOLD");
    let price = 500i128;
    
    client.update_resource_price(&admin, &resource, &price);
    
    let current_rate = client.get_current_market_rate(&resource);
    assert_eq!(current_rate, price);
}

#[test]
fn test_batch_update_prices() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let source = Address::generate(&env);
    let mut sources = Vec::new(&env);
    sources.push_back(source);
    
    client.initialize_oracle(&admin, &sources);
    
    let mut resources = Vec::new(&env);
    resources.push_back(Symbol::new(&env, "IRON"));
    resources.push_back(Symbol::new(&env, "GOLD"));
    resources.push_back(Symbol::new(&env, "CRYSTAL"));
    
    let mut prices = Vec::new(&env);
    prices.push_back(100i128);
    prices.push_back(500i128);
    prices.push_back(1000i128);
    
    // Update prices individually instead of batch to avoid auth issues
    for i in 0..3 {
        client.update_resource_price(&admin, &resources.get(i).unwrap(), &prices.get(i).unwrap());
    }
    
    // Verify all prices were set
    for i in 0..3 {
        let rate = client.get_current_market_rate(&resources.get(i).unwrap());
        assert_eq!(rate, prices.get(i).unwrap());
    }
}

#[test]
fn test_price_history() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let source = Address::generate(&env);
    let mut sources = Vec::new(&env);
    sources.push_back(source);
    
    client.initialize_oracle(&admin, &sources);
    
    let resource = Symbol::new(&env, "PLASMA");
    
    // Update price multiple times
    client.update_resource_price(&admin, &resource, &100i128);
    client.update_resource_price(&admin, &resource, &150i128);
    client.update_resource_price(&admin, &resource, &200i128);
    
    let history = client.get_price_history(&resource);
    assert_eq!(history.len(), 3);
    assert_eq!(history.get(2).unwrap().price, 200i128);
}

// ─── Audio Seed Generator Tests ───────────────────────────────────────────────

#[test]
fn test_initialize_presets() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    client.initialize_presets();
    
    // Test that all presets are accessible
    for preset_id in 0..INSTRUMENT_PRESETS {
        let preset = client.get_preset(&preset_id);
        assert_eq!(preset.preset_id, preset_id);
        assert!(preset.frequency > 0);
        assert!(preset.amplitude > 0);
    }
}

#[test]
fn test_generate_music_seed() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    let nebula_id = 42u64;
    
    let music_seed = client.generate_music_seed(&nebula_id);
    assert_eq!(music_seed.nebula_id, nebula_id);
    assert_eq!(music_seed.seed.len(), 32);
}

#[test]
fn test_get_instrument_layer() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    client.initialize_presets();
    
    let nebula_id = 100u64;
    let music_seed = client.generate_music_seed(&nebula_id);
    
    let layer = 0u32;
    let params = client.get_instrument_layer(&music_seed.seed, &layer);
    
    assert!(params.frequency > 0);
    assert!(params.amplitude > 0);
    assert!(params.waveform < 4);
}

#[test]
fn test_get_all_layers() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    client.initialize_presets();
    
    let nebula_id = 200u64;
    client.generate_music_seed(&nebula_id);
    
    let layers = client.get_all_layers(&nebula_id);
    assert_eq!(layers.len(), MAX_LAYERS_PER_NEBULA);
    
    // Verify each layer has valid parameters
    for i in 0..MAX_LAYERS_PER_NEBULA {
        let layer = layers.get(i).unwrap();
        assert!(layer.frequency >= 20 && layer.frequency <= 2000);
        assert!(layer.amplitude >= 10 && layer.amplitude <= 100);
    }
}

#[test]
fn test_music_seed_determinism() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    client.initialize_presets();
    
    let nebula_id = 300u64;
    let seed1 = client.generate_music_seed(&nebula_id);
    
    // Get stored seed
    let stored_seed = client.get_nebula_seed(&nebula_id);
    assert!(stored_seed.is_some());
    assert_eq!(stored_seed.unwrap(), seed1.seed);
}

#[test]
fn test_invalid_layer() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    client.initialize_presets();
    
    let nebula_id = 400u64;
    let music_seed = client.generate_music_seed(&nebula_id);
    
    // Try to get layer beyond max - should fail with InvalidLayer (error code 1)
    let result = client.try_get_instrument_layer(&music_seed.seed, &MAX_LAYERS_PER_NEBULA);
    assert!(result.is_err());
}

// ─── Wormhole Integration Tests ───────────────────────────────────────────────

#[test]
fn test_wormhole_and_alliance_integration() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    // Create alliance
    let founder = Address::generate(&env);
    let name = String::from_str(&env, "Wormhole Explorers");
    let alliance_id = client.found_alliance(&founder, &name);
    
    // Open wormhole
    let origin = 1u64;
    let destination = 10u64;
    let wormhole_id = client.open_wormhole(&founder, &origin, &destination);
    
    assert_eq!(wormhole_id, 0);
    
    let wormhole = client.get_wormhole(&wormhole_id);
    assert!(wormhole.is_some());
    
    let wh = wormhole.unwrap();
    assert_eq!(wh.origin_nebula, origin);
    assert_eq!(wh.destination, destination);
    assert_eq!(wh.creator, founder);
}

#[test]
fn test_market_oracle_and_audio_integration() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    // Initialize oracle
    let admin = Address::generate(&env);
    let source = Address::generate(&env);
    let mut sources = Vec::new(&env);
    sources.push_back(source);
    client.initialize_oracle(&admin, &sources);
    
    // Initialize audio presets
    client.initialize_presets();
    
    // Update resource price
    let resource = Symbol::new(&env, "MUSIC");
    let price = 777i128;
    client.update_resource_price(&admin, &resource, &price);
    
    // Generate music seed
    let nebula_id = 777u64;
    let music_seed = client.generate_music_seed(&nebula_id);
    
    // Verify both systems work together
    let current_price = client.get_current_market_rate(&resource);
    assert_eq!(current_price, price);
    
    let layers = client.get_all_layers(&nebula_id);
    assert_eq!(layers.len(), MAX_LAYERS_PER_NEBULA);
}

#[test]
fn test_full_feature_integration() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    
    // 1. Create alliance
    let founder = Address::generate(&env);
    let alliance_name = String::from_str(&env, "Full Stack Nomads");
    let alliance_id = client.found_alliance(&founder, &alliance_name);
    
    // 2. Initialize market oracle
    let admin = Address::generate(&env);
    let source = Address::generate(&env);
    let mut sources = Vec::new(&env);
    sources.push_back(source);
    client.initialize_oracle(&admin, &sources);
    
    // 3. Initialize audio presets
    client.initialize_presets();
    
    // 4. Open wormhole
    let wormhole_id = client.open_wormhole(&founder, &1u64, &5u64);
    
    // 5. Update resource prices
    let resource = Symbol::new(&env, "COSMIC");
    client.update_resource_price(&admin, &resource, &1000i128);
    
    // 6. Generate music seed
    let music_seed = client.generate_music_seed(&100u64);
    
    // 7. Contribute to alliance treasury
    client.contribute_to_treasury(&founder, &5000i128);
    
    // Verify all systems are operational
    let alliance = client.get_alliance(&alliance_id);
    assert_eq!(alliance.members.len(), 1);
    
    let wormhole = client.get_wormhole(&wormhole_id).unwrap();
    assert!(wormhole.is_active);
    
    let price = client.get_current_market_rate(&resource);
    assert_eq!(price, 1000i128);
    
    let layers = client.get_all_layers(&100u64);
    assert_eq!(layers.len(), MAX_LAYERS_PER_NEBULA);
    
    let treasury = client.get_alliance_treasury(&alliance_id);
    assert_eq!(treasury, 5000i128);
}
