use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

// Oracle configuration constants
pub const MAX_PRICE_AGE_SECS: u64 = 86_400; // 24 hours
pub const MAX_BATCH_UPDATE: u32 = 20;
pub const MIN_ORACLE_SOURCES: u32 = 1;

#[derive(Clone)]
#[contracttype]
pub enum OracleKey {
    Admin,
    ResourcePrice(Symbol),      // resource -> PriceData
    PriceHistory(Symbol),       // resource -> Vec<PriceData> (last 24h)
    OracleSources,              // -> Vec<Address>
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum OracleError {
    Unauthorized = 1,
    StalePrice = 2,
    InvalidPrice = 3,
    ResourceNotFound = 4,
    TooManyUpdates = 5,
    NoOracleSources = 6,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct PriceData {
    pub resource: Symbol,
    pub price: i128,
    pub timestamp: u64,
    pub source_count: u32,
}

/// Initialize the market oracle with admin and default sources
pub fn initialize_oracle(env: &Env, admin: Address, sources: Vec<Address>) -> Result<(), OracleError> {
    admin.require_auth();
    
    if sources.is_empty() {
        return Err(OracleError::NoOracleSources);
    }
    
    env.storage().instance().set(&OracleKey::Admin, &admin);
    env.storage().persistent().set(&OracleKey::OracleSources, &sources);
    
    env.events().publish(
        (symbol_short!("oracle"), symbol_short!("init")),
        (admin, sources.len()),
    );
    
    Ok(())
}

/// Update resource price with timestamp verification
pub fn update_resource_price(
    env: &Env,
    admin: Address,
    resource: Symbol,
    new_price: i128,
) -> Result<PriceData, OracleError> {
    admin.require_auth();
    
    let stored_admin: Address = env
        .storage()
        .instance()
        .get(&OracleKey::Admin)
        .ok_or(OracleError::Unauthorized)?;
    
    if admin != stored_admin {
        return Err(OracleError::Unauthorized);
    }
    
    if new_price < 0 {
        return Err(OracleError::InvalidPrice);
    }
    
    let current_time = env.ledger().timestamp();
    let price_data = PriceData {
        resource: resource.clone(),
        price: new_price,
        timestamp: current_time,
        source_count: 1,
    };
    
    // Store current price
    env.storage()
        .persistent()
        .set(&OracleKey::ResourcePrice(resource.clone()), &price_data);
    
    // Update 24h history
    let mut history = env
        .storage()
        .persistent()
        .get::<OracleKey, Vec<PriceData>>(&OracleKey::PriceHistory(resource.clone()))
        .unwrap_or(Vec::new(env));
    
    history.push_back(price_data.clone());
    
    // Keep only last 24 entries (hourly updates)
    if history.len() > 24 {
        history.pop_front();
    }
    
    env.storage()
        .persistent()
        .set(&OracleKey::PriceHistory(resource.clone()), &history);
    
    env.events().publish(
        (symbol_short!("oracle"), symbol_short!("price")),
        (resource, new_price, current_time),
    );
    
    Ok(price_data)
}

/// Batch update multiple resource prices
pub fn batch_update_prices(
    env: &Env,
    admin: Address,
    resources: Vec<Symbol>,
    prices: Vec<i128>,
) -> Result<Vec<PriceData>, OracleError> {
    admin.require_auth();
    
    if resources.len() != prices.len() {
        return Err(OracleError::InvalidPrice);
    }
    
    if resources.len() > MAX_BATCH_UPDATE.try_into().unwrap() {
        return Err(OracleError::TooManyUpdates);
    }
    
    let mut results = Vec::new(env);
    
    for i in 0..resources.len() {
        let resource = resources.get(i).unwrap();
        let price = prices.get(i).unwrap();
        
        let price_data = update_resource_price(env, admin.clone(), resource, price)?;
        results.push_back(price_data);
    }
    
    Ok(results)
}

/// Get current market rate for a resource (pure view)
pub fn get_current_market_rate(env: &Env, resource: Symbol) -> Result<i128, OracleError> {
    let price_data = env
        .storage()
        .persistent()
        .get::<OracleKey, PriceData>(&OracleKey::ResourcePrice(resource.clone()))
        .ok_or(OracleError::ResourceNotFound)?;
    
    let current_time = env.ledger().timestamp();
    let age = current_time.saturating_sub(price_data.timestamp);
    
    if age > MAX_PRICE_AGE_SECS {
        return Err(OracleError::StalePrice);
    }
    
    Ok(price_data.price)
}

/// Get price data with metadata
pub fn get_price_data(env: &Env, resource: Symbol) -> Result<PriceData, OracleError> {
    env.storage()
        .persistent()
        .get::<OracleKey, PriceData>(&OracleKey::ResourcePrice(resource))
        .ok_or(OracleError::ResourceNotFound)
}

/// Get 24h price history for a resource
pub fn get_price_history(env: &Env, resource: Symbol) -> Vec<PriceData> {
    env.storage()
        .persistent()
        .get::<OracleKey, Vec<PriceData>>(&OracleKey::PriceHistory(resource))
        .unwrap_or(Vec::new(env))
}

/// Add oracle source (admin only)
pub fn add_oracle_source(env: &Env, admin: Address, new_source: Address) -> Result<(), OracleError> {
    admin.require_auth();
    
    let stored_admin: Address = env
        .storage()
        .instance()
        .get(&OracleKey::Admin)
        .ok_or(OracleError::Unauthorized)?;
    
    if admin != stored_admin {
        return Err(OracleError::Unauthorized);
    }
    
    let mut sources = env
        .storage()
        .persistent()
        .get::<OracleKey, Vec<Address>>(&OracleKey::OracleSources)
        .unwrap_or(Vec::new(env));
    
    sources.push_back(new_source.clone());
    
    env.storage()
        .persistent()
        .set(&OracleKey::OracleSources, &sources);
    
    env.events().publish(
        (symbol_short!("oracle"), symbol_short!("source")),
        (new_source,),
    );
    
    Ok(())
}
