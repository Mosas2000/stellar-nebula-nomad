#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{Address, Env, Symbol, Vec};
use stellar_nebula_nomad::{
    initialize_forecast, generate_yield_forecast, update_forecast_model,
    batch_generate_forecasts, get_cached_forecast, get_player_history, get_history_count,
    get_model_params, get_model_version, update_model_params,
    YieldDataPoint, YieldForecast, ModelParams, ForecastError,
    MAX_HISTORY_POINTS, MAX_FORECAST_DAYS, MAX_FORECAST_BURST,
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
fn test_initialize_forecast() {
    let (env, client, admin) = setup_env();
    
    let result = client.initialize_forecast(&admin);
    assert!(result.is_ok());
    
    // Check model version is set
    assert_eq!(client.get_model_version(), 1);
    
    // Check model params exist
    let params = client.get_model_params();
    assert!(params.is_some());
}

// ─── Data Update Tests ────────────────────────────────────────────────────

#[test]
fn test_update_forecast_model() {
    let (env, client, admin) = setup_env();
    let player = Address::generate(&env);
    client.initialize_forecast(&admin).unwrap();
    
    let data_point = YieldDataPoint {
        timestamp: 1_700_000_000,
        yield_amount: 1000,
        source: Symbol::new(&env, "harvest"),
    };
    
    let result = client.update_forecast_model(&player, &data_point);
    assert!(result.is_ok());
    
    // Check history count
    assert_eq!(client.get_history_count(&player), 1);
    
    // Check history
    let history = client.get_player_history(&player);
    assert_eq!(history.len(), 1);
}

#[test]
fn test_update_forecast_model_multiple_points() {
    let (env, client, admin) = setup_env();
    let player = Address::generate(&env);
    client.initialize_forecast(&admin).unwrap();
    
    // Add 10 data points
    for i in 0..10 {
        let data_point = YieldDataPoint {
            timestamp: 1_700_000_000 + (i as u64 * 86400),
            yield_amount: 1000 + (i as i128 * 100),
            source: Symbol::new(&env, "harvest"),
        };
        client.update_forecast_model(&player, &data_point).unwrap();
    }
    
    assert_eq!(client.get_history_count(&player), 10);
}

// ─── Forecast Generation Tests ────────────────────────────────────────────

#[test]
fn test_generate_yield_forecast_success() {
    let (env, client, admin) = setup_env();
    let player = Address::generate(&env);
    client.initialize_forecast(&admin).unwrap();
    
    // Add 10 data points (need at least 7 for moving average)
    for i in 0..10 {
        let data_point = YieldDataPoint {
            timestamp: 1_700_000_000 + (i as u64 * 86400),
            yield_amount: 1000 + (i as i128 * 50),
            source: Symbol::new(&env, "harvest"),
        };
        client.update_forecast_model(&player, &data_point).unwrap();
    }
    
    // Generate forecast for 30 days
    let forecast = client.generate_yield_forecast(&player, &30);
    assert!(forecast.is_ok());
    
    let forecast = forecast.unwrap();
    assert_eq!(forecast.player, player);
    assert_eq!(forecast.forecast_days, 30);
    assert!(forecast.predicted_yield > 0);
    assert!(forecast.confidence_score > 0 && forecast.confidence_score <= 100);
}

#[test]
fn test_generate_yield_forecast_insufficient_data() {
    let (env, client, admin) = setup_env();
    let player = Address::generate(&env);
    client.initialize_forecast(&admin).unwrap();
    
    // Only add 3 data points (need 7 for moving average)
    for i in 0..3 {
        let data_point = YieldDataPoint {
            timestamp: 1_700_000_000 + (i as u64 * 86400),
            yield_amount: 1000,
            source: Symbol::new(&env, "harvest"),
        };
        client.update_forecast_model(&player, &data_point).unwrap();
    }
    
    let result = client.try_generate_yield_forecast(&player, &30);
    assert!(result.is_err());
    assert!(matches!(result.err().unwrap(), ForecastError::InsufficientData));
}

#[test]
fn test_generate_yield_forecast_invalid_days() {
    let (env, client, admin) = setup_env();
    let player = Address::generate(&env);
    client.initialize_forecast(&admin).unwrap();
    
    // Add sufficient data
    for i in 0..10 {
        let data_point = YieldDataPoint {
            timestamp: 1_700_000_000 + (i as u64 * 86400),
            yield_amount: 1000,
            source: Symbol::new(&env, "harvest"),
        };
        client.update_forecast_model(&player, &data_point).unwrap();
    }
    
    // 0 days should fail
    let result = client.try_generate_yield_forecast(&player, &0);
    assert!(result.is_err());
    
    // Exceeding MAX_FORECAST_DAYS should fail
    let result = client.try_generate_yield_forecast(&player, &(MAX_FORECAST_DAYS + 1));
    assert!(result.is_err());
}

#[test]
fn test_cached_forecast() {
    let (env, client, admin) = setup_env();
    let player = Address::generate(&env);
    client.initialize_forecast(&admin).unwrap();
    
    // No cached forecast initially
    assert!(client.get_cached_forecast(&player).is_none());
    
    // Add data and generate forecast
    for i in 0..10 {
        let data_point = YieldDataPoint {
            timestamp: 1_700_000_000 + (i as u64 * 86400),
            yield_amount: 1000,
            source: Symbol::new(&env, "harvest"),
        };
        client.update_forecast_model(&player, &data_point).unwrap();
    }
    
    let forecast = client.generate_yield_forecast(&player, &30).unwrap();
    
    // Should now be cached
    let cached = client.get_cached_forecast(&player);
    assert!(cached.is_some());
    assert_eq!(cached.unwrap().predicted_yield, forecast.predicted_yield);
}

// ─── Batch Forecast Tests ───────────────────────────────────────────────────

#[test]
fn test_batch_generate_forecasts() {
    let (env, client, admin) = setup_env();
    client.initialize_forecast(&admin).unwrap();
    
    // Create 3 players with data
    let mut players = Vec::new(&env);
    for _ in 0..3 {
        let player = Address::generate(&env);
        
        for i in 0..10 {
            let data_point = YieldDataPoint {
                timestamp: 1_700_000_000 + (i as u64 * 86400),
                yield_amount: 1000,
                source: Symbol::new(&env, "harvest"),
            };
            client.update_forecast_model(&player, &data_point).unwrap();
        }
        
        players.push_back((player, 30u32));
    }
    
    let results = client.batch_generate_forecasts(&players);
    assert_eq!(results.len(), 3);
    
    // All should succeed
    for i in 0..results.len() {
        let result = results.get(i).unwrap();
        assert!(result.is_ok());
    }
}

// ─── Model Parameter Tests ───────────────────────────────────────────────────

#[test]
fn test_get_model_params() {
    let (env, client, admin) = setup_env();
    client.initialize_forecast(&admin).unwrap();
    
    let params = client.get_model_params().unwrap();
    assert_eq!(params.version, 1);
    assert_eq!(params.moving_average_window, 7);
    assert_eq!(params.trend_weight, 50);
    assert_eq!(params.volatility_adjustment, 20);
}

#[test]
fn test_update_model_params() {
    let (env, client, admin) = setup_env();
    client.initialize_forecast(&admin).unwrap();
    
    let new_params = client.update_model_params(&admin, &14, &60, &30);
    assert!(new_params.is_ok());
    
    let params = new_params.unwrap();
    assert_eq!(params.version, 2); // Version increments
    assert_eq!(params.moving_average_window, 14);
    assert_eq!(params.trend_weight, 60);
    assert_eq!(params.volatility_adjustment, 30);
}

#[test]
fn test_update_model_params_unauthorized() {
    let (env, client, admin) = setup_env();
    let not_admin = Address::generate(&env);
    client.initialize_forecast(&admin).unwrap();
    
    let result = client.try_update_model_params(&not_admin, &14, &60, &30);
    assert!(result.is_err());
    assert!(matches!(result.err().unwrap(), ForecastError::Unauthorized));
}

#[test]
fn test_update_model_params_invalid_values() {
    let (env, client, admin) = setup_env();
    client.initialize_forecast(&admin).unwrap();
    
    // trend_weight > 100 should fail
    let result = client.try_update_model_params(&admin, &7, &101, &20);
    assert!(result.is_err());
    
    // volatility_adjustment > 100 should fail
    let result = client.try_update_model_params(&admin, &7, &50, &101);
    assert!(result.is_err());
    
    // moving_average_window = 0 should fail
    let result = client.try_update_model_params(&admin, &0, &50, &20);
    assert!(result.is_err());
}

// ─── History Management Tests ─────────────────────────────────────────────

#[test]
fn test_history_max_points() {
    let (env, client, admin) = setup_env();
    let player = Address::generate(&env);
    client.initialize_forecast(&admin).unwrap();
    
    // Add MAX_HISTORY_POINTS + 10 data points
    for i in 0..(MAX_HISTORY_POINTS + 10) {
        let data_point = YieldDataPoint {
            timestamp: 1_700_000_000 + (i as u64 * 86400),
            yield_amount: 1000,
            source: Symbol::new(&env, "harvest"),
        };
        client.update_forecast_model(&player, &data_point).unwrap();
    }
    
    // Should be capped at MAX_HISTORY_POINTS
    assert_eq!(client.get_history_count(&player), MAX_HISTORY_POINTS);
}

// ─── View Function Tests ───────────────────────────────────────────────────

#[test]
fn test_get_model_version() {
    let (env, client, admin) = setup_env();
    
    // Before initialization
    assert_eq!(client.get_model_version(), 0);
    
    // After initialization
    client.initialize_forecast(&admin).unwrap();
    assert_eq!(client.get_model_version(), 1);
}

#[test]
fn test_get_player_history_empty() {
    let (env, client, _admin) = setup_env();
    let player = Address::generate(&env);
    
    let history = client.get_player_history(&player);
    assert_eq!(history.len(), 0);
}

// ─── Constants Tests ───────────────────────────────────────────────────────

#[test]
fn test_constants() {
    assert_eq!(MAX_HISTORY_POINTS, 100);
    assert_eq!(MAX_FORECAST_DAYS, 365);
    assert_eq!(MAX_FORECAST_BURST, 100);
}

// ─── Integration Tests ─────────────────────────────────────────────────────

#[test]
fn test_full_forecast_lifecycle() {
    let (env, client, admin) = setup_env();
    let player = Address::generate(&env);
    
    // 1. Initialize
    client.initialize_forecast(&admin).unwrap();
    
    // 2. Add historical data (20 days)
    for i in 0..20 {
        let data_point = YieldDataPoint {
            timestamp: 1_700_000_000 + (i as u64 * 86400),
            yield_amount: 1000 + (i as i128 * 25), // Increasing trend
            source: Symbol::new(&env, "harvest"),
        };
        client.update_forecast_model(&player, &data_point).unwrap();
    }
    
    // 3. Generate forecast
    let forecast = client.generate_yield_forecast(&player, &30).unwrap();
    assert!(forecast.predicted_yield > 0);
    assert!(forecast.confidence_score > 0);
    
    // 4. Verify cached
    let cached = client.get_cached_forecast(&player);
    assert!(cached.is_some());
    
    // 5. Update model params
    client.update_model_params(&admin, &14, &75, &40).unwrap();
    assert_eq!(client.get_model_version(), 2);
}

// ─── ForecastError Tests ───────────────────────────────────────────────────

#[test]
fn test_forecast_error_variants() {
    let errors = [
        ForecastError::InsufficientData,
        ForecastError::InvalidDays,
        ForecastError::PlayerNotFound,
        ForecastError::MaxDaysExceeded,
        ForecastError::ModelNotInitialized,
        ForecastError::Unauthorized,
        ForecastError::BurstLimitExceeded,
    ];
    
    assert_eq!(errors[0], ForecastError::InsufficientData);
}
