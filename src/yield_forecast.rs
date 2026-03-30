use soroban_sdk::{contracterror, contracttype, symbol_short, Address, BytesN, Env, Symbol, Vec};

// ─── Configuration ─────────────────────────────────────────────────────────

/// Maximum historical data points stored per player.
pub const MAX_HISTORY_POINTS: u32 = 100;

/// Maximum days for forecast.
pub const MAX_FORECAST_DAYS: u32 = 365;

/// Burst limit: Forecast for 100 players per tx.
pub const MAX_FORECAST_BURST: u32 = 100;

// ─── Storage Keys ─────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    /// Admin address.
    Admin,
    /// Historical yield data for a player.
    PlayerHistory(Address),
    /// Cached forecast for a player.
    ForecastCache(Address),
    /// Global forecast model version.
    ModelVersion,
    /// Model parameters.
    ModelParams,
}

// ─── Error Handling ───────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Debug, PartialEq, Eq, Copy)]
#[repr(u32)]
pub enum ForecastError {
    /// Not enough historical data for forecast.
    InsufficientData = 1,
    /// Invalid number of days requested.
    InvalidDays = 2,
    /// Player not found or no data.
    PlayerNotFound = 3,
    /// Forecast days exceed maximum.
    MaxDaysExceeded = 4,
    /// Model not initialized.
    ModelNotInitialized = 5,
    /// Unauthorized caller.
    Unauthorized = 6,
    /// Burst limit exceeded.
    BurstLimitExceeded = 7,
}

// ─── Data Structures ───────────────────────────────────────────────────────

/// A single historical yield data point.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct YieldDataPoint {
    pub timestamp: u64,
    pub yield_amount: i128,
    pub source: Symbol, // e.g., "harvest", "staking", "mission"
}

/// Cached forecast for a player.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct YieldForecast {
    pub player: Address,
    pub forecast_days: u32,
    pub predicted_yield: i128,
    pub confidence_score: u32, // 0-100
    pub calculated_at: u64,
    pub model_version: u32,
}

/// Model parameters for yield prediction.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct ModelParams {
    pub version: u32,
    pub moving_average_window: u32,
    pub trend_weight: u32,      // 0-100
    pub volatility_adjustment: u32, // 0-100
}

impl Default for ModelParams {
    fn default() -> Self {
        Self {
            version: 1,
            moving_average_window: 7,  // 7-day moving average
            trend_weight: 50,          // 50% trend weight
            volatility_adjustment: 20, // 20% volatility dampening
        }
    }
}

// ─── Initialization ───────────────────────────────────────────────────────

/// Initialize the yield forecasting system.
pub fn initialize(env: &Env, admin: &Address) -> Result<(), ForecastError> {
    admin.require_auth();
    
    env.storage().instance().set(&DataKey::Admin, admin);
    env.storage().instance().set(&DataKey::ModelVersion, &1u32);
    env.storage().instance().set(&DataKey::ModelParams, &ModelParams::default());
    
    env.events().publish(
        (symbol_short!("forecast"), symbol_short!("init")),
        admin.clone(),
    );
    
    Ok(())
}

// ─── Core Forecasting Logic ───────────────────────────────────────────────

/// Generate a yield forecast for a player based on historical data.
/// 
/// # Arguments
/// * `env` - The contract environment
/// * `player` - The player address
/// * `days` - Number of days to forecast (1-365)
/// 
/// # Returns
/// `YieldForecast` with predicted yield and confidence score
/// 
/// # Algorithm
/// Uses simple moving average with trend analysis:
/// 1. Calculate moving average of historical yields
/// 2. Detect trend direction and strength
/// 3. Apply volatility adjustment
/// 4. Project forward for requested days
/// 
/// # Burst Limit
/// Supports forecasting for up to 100 players per transaction
pub fn generate_yield_forecast(
    env: &Env,
    player: &Address,
    days: u32,
) -> Result<YieldForecast, ForecastError> {
    // Validate days
    if days == 0 || days > MAX_FORECAST_DAYS {
        return Err(ForecastError::InvalidDays);
    }
    
    // Check burst limit (simplified - in production, track per-tx)
    // For now, we assume caller manages burst externally
    
    // Get model params
    let params: ModelParams = env
        .storage()
        .instance()
        .get(&DataKey::ModelParams)
        .ok_or(ForecastError::ModelNotInitialized)?;
    
    // Get historical data
    let history: Vec<YieldDataPoint> = env
        .storage()
        .persistent()
        .get(&DataKey::PlayerHistory(player.clone()))
        .unwrap_or_else(|| Vec::new(env));
    
    // Need at least moving_average_window data points
    if history.len() < params.moving_average_window as u32 {
        return Err(ForecastError::InsufficientData);
    }
    
    // Calculate moving average
    let recent_data: Vec<YieldDataPoint> = get_recent_data(env, &history, params.moving_average_window);
    let avg_yield = calculate_moving_average(env, &recent_data);
    
    // Calculate trend
    let trend = calculate_trend(env, &recent_data);
    
    // Calculate volatility
    let volatility = calculate_volatility(env, &recent_data);
    
    // Apply adjustments
    let trend_adjustment = (trend * params.trend_weight as i128) / 100;
    let volatility_dampening = 100 - ((volatility * params.volatility_adjustment as i128) / 10000);
    
    // Calculate predicted yield per day
    let base_daily = avg_yield + trend_adjustment;
    let adjusted_daily = (base_daily * volatility_dampening) / 100;
    
    // Total forecast for requested days
    let predicted_yield = adjusted_daily * days as i128;
    
    // Calculate confidence score (0-100)
    // Based on data volume and volatility
    let data_points_score = (history.len().min(100) * 50) / 100; // Up to 50 points
    let volatility_penalty = (volatility.min(100) * 50) / 100; // Up to -50 points
    let confidence_score = data_points_score as u32 + 50 - volatility_penalty as u32;
    let confidence_score = confidence_score.min(100);
    
    let forecast = YieldForecast {
        player: player.clone(),
        forecast_days: days,
        predicted_yield,
        confidence_score,
        calculated_at: env.ledger().timestamp(),
        model_version: params.version,
    };
    
    // Cache the forecast
    env.storage()
        .persistent()
        .set(&DataKey::ForecastCache(player.clone()), &forecast);
    
    // Emit event
    env.events().publish(
        (symbol_short!("forecast"), symbol_short!("gen")),
        (player.clone(), days, predicted_yield, confidence_score),
    );
    
    Ok(forecast)
}

/// Update the forecast model with new data.
/// 
/// # Arguments
/// * `env` - The contract environment
/// * `new_data` - New yield data point to add
/// 
/// # Returns
/// Updated model parameters
pub fn update_forecast_model(
    env: &Env,
    player: &Address,
    data_point: YieldDataPoint,
) -> Result<ModelParams, ForecastError> {
    // Get or create player history
    let mut history: Vec<YieldDataPoint> = env
        .storage()
        .persistent()
        .get(&DataKey::PlayerHistory(player.clone()))
        .unwrap_or_else(|| Vec::new(env));
    
    // Add new data point
    history.push_back(data_point);
    
    // Trim to max history points
    while history.len() > MAX_HISTORY_POINTS {
        history.pop_front();
    }
    
    // Save updated history
    env.storage()
        .persistent()
        .set(&DataKey::PlayerHistory(player.clone()), &history);
    
    // Get and return current model params
    let params: ModelParams = env
        .storage()
        .instance()
        .get(&DataKey::ModelParams)
        .ok_or(ForecastError::ModelNotInitialized)?;
    
    Ok(params)
}

/// Batch generate forecasts for multiple players.
/// 
/// # Burst Limit
/// Maximum 100 players per batch
pub fn batch_generate_forecasts(
    env: &Env,
    players: Vec<(Address, u32)>,
) -> Vec<Result<YieldForecast, ForecastError>> {
    let mut results = Vec::new(env);
    
    let limit = players.len().min(MAX_FORECAST_BURST);
    
    for i in 0..limit {
        let (player, days) = players.get(i).unwrap();
        let result = generate_yield_forecast(env, &player, days);
        results.push_back(result);
    }
    
    results
}

// ─── View Functions ─────────────────────────────────────────────────────────

/// Get cached forecast for a player.
pub fn get_cached_forecast(env: &Env, player: &Address) -> Option<YieldForecast> {
    env.storage()
        .persistent()
        .get(&DataKey::ForecastCache(player.clone()))
}

/// Get historical data for a player.
pub fn get_player_history(env: &Env, player: &Address) -> Vec<YieldDataPoint> {
    env.storage()
        .persistent()
        .get(&DataKey::PlayerHistory(player.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Get number of historical data points for a player.
pub fn get_history_count(env: &Env, player: &Address) -> u32 {
    let history: Vec<YieldDataPoint> = env
        .storage()
        .persistent()
        .get(&DataKey::PlayerHistory(player.clone()))
        .unwrap_or_else(|| Vec::new(env));
    history.len()
}

/// Get current model parameters.
pub fn get_model_params(env: &Env) -> Option<ModelParams> {
    env.storage().instance().get(&DataKey::ModelParams)
}

/// Get current model version.
pub fn get_model_version(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::ModelVersion)
        .unwrap_or(0)
}

// ─── Internal Calculation Functions ───────────────────────────────────────

/// Get the most recent N data points.
fn get_recent_data(
    env: &Env,
    history: &Vec<YieldDataPoint>,
    count: u32,
) -> Vec<YieldDataPoint> {
    let mut result = Vec::new(env);
    let start = if history.len() > count {
        history.len() - count
    } else {
        0
    };
    
    for i in start..history.len() {
        if let Some(point) = history.get(i) {
            result.push_back(point);
        }
    }
    
    result
}

/// Calculate simple moving average.
fn calculate_moving_average(env: &Env, data: &Vec<YieldDataPoint>) -> i128 {
    if data.is_empty() {
        return 0;
    }
    
    let mut total: i128 = 0;
    for i in 0..data.len() {
        if let Some(point) = data.get(i) {
            total += point.yield_amount;
        }
    }
    
    total / data.len() as i128
}

/// Calculate trend (average daily change).
fn calculate_trend(env: &Env, data: &Vec<YieldDataPoint>) -> i128 {
    if data.len() < 2 {
        return 0;
    }
    
    let mut total_change: i128 = 0;
    let mut count: u32 = 0;
    
    for i in 1..data.len() {
        let prev = data.get(i - 1).unwrap();
        let curr = data.get(i).unwrap();
        
        // Calculate time difference in days (approximate)
        let time_diff = if curr.timestamp > prev.timestamp {
            (curr.timestamp - prev.timestamp) / 86400 // seconds in a day
        } else {
            1
        };
        
        if time_diff > 0 {
            let change = (curr.yield_amount - prev.yield_amount) / time_diff as i128;
            total_change += change;
            count += 1;
        }
    }
    
    if count > 0 {
        total_change / count as i128
    } else {
        0
    }
}

/// Calculate volatility (standard deviation approximation).
fn calculate_volatility(env: &Env, data: &Vec<YieldDataPoint>) -> i128 {
    if data.len() < 2 {
        return 0;
    }
    
    let avg = calculate_moving_average(env, data);
    
    // Calculate mean absolute deviation
    let mut total_deviation: i128 = 0;
    for i in 0..data.len() {
        if let Some(point) = data.get(i) {
            let diff = point.yield_amount - avg;
            total_deviation += diff.abs();
        }
    }
    
    let mad = total_deviation / data.len() as i128;
    
    // Convert to percentage of average (avoid divide by zero)
    if avg > 0 {
        (mad * 100) / avg
    } else {
        mad
    }
}

// ─── Admin Functions ───────────────────────────────────────────────────────

/// Update model parameters (admin only).
pub fn update_model_params(
    env: &Env,
    admin: &Address,
    moving_average_window: u32,
    trend_weight: u32,
    volatility_adjustment: u32,
) -> Result<ModelParams, ForecastError> {
    admin.require_auth();
    
    // Verify admin
    let stored_admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(ForecastError::Unauthorized)?;
    
    if admin != &stored_admin {
        return Err(ForecastError::Unauthorized);
    }
    
    // Validate params
    if moving_average_window == 0 || trend_weight > 100 || volatility_adjustment > 100 {
        return Err(ForecastError::InvalidDays);
    }
    
    let current_version: u32 = env
        .storage()
        .instance()
        .get(&DataKey::ModelVersion)
        .unwrap_or(1);
    
    let params = ModelParams {
        version: current_version + 1,
        moving_average_window,
        trend_weight,
        volatility_adjustment,
    };
    
    env.storage().instance().set(&DataKey::ModelParams, &params);
    env.storage().instance().set(&DataKey::ModelVersion, &params.version);
    
    env.events().publish(
        (symbol_short!("forecast"), symbol_short!("model")),
        (params.version, moving_average_window, trend_weight),
    );
    
    Ok(params)
}

/// Clear stale forecast cache (admin only).
pub fn clear_stale_forecasts(
    env: &Env,
    _admin: &Address,
    _older_than_seconds: u64,
) -> Result<u32, ForecastError> {
    _admin.require_auth();
    
    // In a full implementation, this would iterate through cached forecasts
    // and remove those older than the threshold
    // For now, we return 0 as placeholder
    Ok(0)
}
