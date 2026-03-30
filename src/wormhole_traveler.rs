use crate::energy_manager::{consume_energy, get_energy_balance};
use crate::ship_nft::{DataKey as ShipDataKey, ShipNft};
use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, BytesN, Vec};

// Travel cost multipliers for balanced progression
pub const BASE_TRAVEL_COST: u32 = 50;
pub const DISTANCE_MULTIPLIER: u32 = 2;
pub const MAX_SIMULTANEOUS_WORMHOLES: u32 = 5;
pub const WORMHOLE_LIFETIME_SECS: u64 = 3600; // 1 hour

#[derive(Clone)]
#[contracttype]
pub enum WormholeKey {
    Wormhole(u64),           // wormhole_id -> Wormhole
    ActiveWormholes,        // -> Vec<u64> (active wormhole IDs)
    WormholeCount,          // -> u64 (next wormhole ID)
    TravelHistory(u64),     // ship_id -> Vec<TravelRecord>
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum WormholeError {
    InvalidDestination = 1,
    InsufficientEnergy = 2,
    ShipNotFound = 3,
    WormholeNotFound = 4,
    WormholeExpired = 5,
    MaxWormholesReached = 6,
    UnauthorizedTravel = 7,
    SameNebulaTravel = 8,
    WormholeClosed = 9,
    EnergyManagerError = 10,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct Wormhole {
    pub wormhole_id: u64,
    pub origin_nebula: u64,
    pub destination: u64,
    pub creator: Address,
    pub created_at: u64,
    pub expires_at: u64,
    pub is_active: bool,
    pub travel_cost: u32,
    pub verifiable_link: BytesN<32>,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct TravelRecord {
    pub ship_id: u64,
    pub wormhole_id: u64,
    pub origin_nebula: u64,
    pub destination: u64,
    pub traveled_at: u64,
    pub energy_consumed: u32,
}

/// Open a new wormhole between two nebulae with verifiable travel link
pub fn open_wormhole(
    env: &Env,
    creator: Address,
    origin_nebula: u64,
    destination: u64,
) -> Result<u64, WormholeError> {
    creator.require_auth();
    
    // Validate destinations
    if origin_nebula == destination {
        return Err(WormholeError::SameNebulaTravel);
    }
    
    // Check maximum simultaneous wormholes
    let active_wormholes = env
        .storage()
        .persistent()
        .get::<WormholeKey, Vec<u64>>(&WormholeKey::ActiveWormholes)
        .unwrap_or(Vec::new(env));
    
    if active_wormholes.len() >= MAX_SIMULTANEOUS_WORMHOLES.try_into().unwrap() {
        return Err(WormholeError::MaxWormholesReached);
    }
    
    // Generate wormhole ID
    let wormhole_id = env
        .storage()
        .persistent()
        .get::<WormholeKey, u64>(&WormholeKey::WormholeCount)
        .unwrap_or(0);
    
    // Calculate travel cost based on distance
    let distance = if destination > origin_nebula {
        destination - origin_nebula
    } else {
        origin_nebula - destination
    };
    let travel_cost = BASE_TRAVEL_COST + ((distance as u32) * DISTANCE_MULTIPLIER);
    
    // Generate verifiable travel link using ledger seed
    let mut link_data = [0u8; 32];
    let ledger_seq = env.ledger().sequence();
    let timestamp = env.ledger().timestamp();
    
    // Create deterministic hash from origin, destination, and ledger data
    // Use simple XOR-based hash for verifiability
    for (i, byte) in origin_nebula.to_be_bytes().iter().enumerate() {
        link_data[i % 32] ^= byte;
    }
    for (i, byte) in destination.to_be_bytes().iter().enumerate() {
        link_data[(i + 8) % 32] ^= byte;
    }
    for (i, byte) in ledger_seq.to_be_bytes().iter().enumerate() {
        link_data[(i + 16) % 32] ^= byte;
    }
    for (i, byte) in timestamp.to_be_bytes().iter().enumerate() {
        link_data[(i + 24) % 32] ^= byte;
    }
    
    let verifiable_link = BytesN::from_array(env, &link_data);
    
    let current_time = env.ledger().timestamp();
    let wormhole = Wormhole {
        wormhole_id,
        origin_nebula,
        destination,
        creator: creator.clone(),
        created_at: current_time,
        expires_at: current_time + WORMHOLE_LIFETIME_SECS,
        is_active: true,
        travel_cost,
        verifiable_link: verifiable_link.clone(),
    };
    
    // Store wormhole
    env.storage()
        .persistent()
        .set(&WormholeKey::Wormhole(wormhole_id), &wormhole);
    
    // Update wormhole count
    env.storage()
        .persistent()
        .set(&WormholeKey::WormholeCount, &(wormhole_id + 1));
    
    // Add to active list
    let mut updated_active = active_wormholes;
    updated_active.push_back(wormhole_id);
    env.storage()
        .persistent()
        .set(&WormholeKey::ActiveWormholes, &updated_active);
    
    // Emit event
    env.events().publish(
        (symbol_short!("wormhole"), symbol_short!("opened")),
        (wormhole_id, origin_nebula, destination, creator, verifiable_link),
    );
    
    Ok(wormhole_id)
}

/// Traverse an existing wormhole with energy cost validation and state sync
pub fn traverse_wormhole(
    env: &Env,
    traveler: Address,
    ship_id: u64,
    wormhole_id: u64,
) -> Result<TravelRecord, WormholeError> {
    traveler.require_auth();
    
    // Get wormhole
    let wormhole = env
        .storage()
        .persistent()
        .get::<WormholeKey, Wormhole>(&WormholeKey::Wormhole(wormhole_id))
        .ok_or(WormholeError::WormholeNotFound)?;
    
    // Validate wormhole is active and not expired
    if !wormhole.is_active {
        return Err(WormholeError::WormholeClosed);
    }
    
    let current_time = env.ledger().timestamp();
    if current_time > wormhole.expires_at {
        return Err(WormholeError::WormholeExpired);
    }
    
    // Verify ship ownership
    let ship = env
        .storage()
        .persistent()
        .get::<ShipDataKey, ShipNft>(&ShipDataKey::Ship(ship_id))
        .ok_or(WormholeError::ShipNotFound)?;
    
    if ship.owner != traveler {
        return Err(WormholeError::UnauthorizedTravel);
    }
    
    // Check energy balance and consume energy
    let energy_balance = get_energy_balance(env, ship_id)
        .map_err(|_| WormholeError::EnergyManagerError)?;
    
    if energy_balance.current < wormhole.travel_cost {
        return Err(WormholeError::InsufficientEnergy);
    }
    
    consume_energy(env, ship_id, wormhole.travel_cost)
        .map_err(|_| WormholeError::EnergyManagerError)?;
    
    // Create travel record
    let travel_record = TravelRecord {
        ship_id,
        wormhole_id,
        origin_nebula: wormhole.origin_nebula,
        destination: wormhole.destination,
        traveled_at: current_time,
        energy_consumed: wormhole.travel_cost,
    };
    
    // Update travel history
    let mut travel_history = env
        .storage()
        .persistent()
        .get::<WormholeKey, Vec<TravelRecord>>(&WormholeKey::TravelHistory(ship_id))
        .unwrap_or(Vec::new(env));
    
    travel_history.push_back(travel_record.clone());
    
    // Keep only last 100 travel records per ship
    if travel_history.len() > 100 {
        travel_history.pop_front();
    }
    
    env.storage()
        .persistent()
        .set(&WormholeKey::TravelHistory(ship_id), &travel_history);
    
    // Emit travel completion event
    env.events().publish(
        (symbol_short!("wormhole"), symbol_short!("traversed")),
        (ship_id, wormhole_id, wormhole.origin_nebula, wormhole.destination, traveler),
    );
    
    Ok(travel_record)
}

/// Get wormhole details by ID
pub fn get_wormhole(env: &Env, wormhole_id: u64) -> Option<Wormhole> {
    env.storage()
        .persistent()
        .get::<WormholeKey, Wormhole>(&WormholeKey::Wormhole(wormhole_id))
}

/// Get all active wormholes
pub fn get_active_wormholes(env: &Env) -> Vec<u64> {
    env.storage()
        .persistent()
        .get::<WormholeKey, Vec<u64>>(&WormholeKey::ActiveWormholes)
        .unwrap_or(Vec::new(env))
}

/// Get travel history for a ship
pub fn get_travel_history(env: &Env, ship_id: u64) -> Vec<TravelRecord> {
    env.storage()
        .persistent()
        .get::<WormholeKey, Vec<TravelRecord>>(&WormholeKey::TravelHistory(ship_id))
        .unwrap_or(Vec::new(env))
}

/// Close expired wormholes (maintenance function)
pub fn cleanup_expired_wormholes(env: &Env) -> u32 {
    let mut cleaned_count = 0;
    let current_time = env.ledger().timestamp();
    
    let active_wormholes = env
        .storage()
        .persistent()
        .get::<WormholeKey, Vec<u64>>(&WormholeKey::ActiveWormholes)
        .unwrap_or(Vec::new(env));
    
    let mut updated_active = Vec::new(env);
    
    for wormhole_id in active_wormholes.iter() {
        if let Some(wormhole) = get_wormhole(env, wormhole_id) {
            if current_time <= wormhole.expires_at && wormhole.is_active {
                updated_active.push_back(wormhole_id);
            } else {
                // Mark as inactive
                let mut updated_wormhole = wormhole;
                updated_wormhole.is_active = false;
                env.storage()
                    .persistent()
                    .set(&WormholeKey::Wormhole(wormhole_id), &updated_wormhole);
                cleaned_count += 1;
            }
        }
    }
    
    env.storage()
        .persistent()
        .set(&WormholeKey::ActiveWormholes, &updated_active);
    
    cleaned_count
}

/// Calculate travel cost between two nebulae
pub fn calculate_travel_cost(origin_nebula: u64, destination: u64) -> u32 {
    if origin_nebula == destination {
        return 0;
    }
    
    let distance = if destination > origin_nebula {
        destination - origin_nebula
    } else {
        origin_nebula - destination
    };
    
    BASE_TRAVEL_COST + ((distance as u32) * DISTANCE_MULTIPLIER)
}

/// Verify wormhole link integrity
pub fn verify_wormhole_link(env: &Env, wormhole_id: u64, provided_link: BytesN<32>) -> bool {
    if let Some(wormhole) = get_wormhole(env, wormhole_id) {
        wormhole.verifiable_link == provided_link
    } else {
        false
    }
}
