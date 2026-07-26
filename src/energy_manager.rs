use crate::ship_nft::{DataKey as ShipDataKey, ShipNft};
use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env};

const BASE_RECHARGE_RATE: u32 = 100;
const MAX_ENERGY: u32 = 10000;
const RECHARGE_EFFICIENCY: u32 = 85;

/// Base passive regeneration per ledger tick (0.1% of max energy per 10 ledgers).
const BASE_PASSIVE_REGEN: u32 = 10;
/// Ledgers between passive regeneration ticks.
const PASSIVE_REGEN_INTERVAL: u64 = 10;
/// Storage key for last regen timestamp.
const LAST_REGEN_KEY: &str = "last_regen";

#[derive(Clone)]
#[contracttype]
pub enum EnergyKey {
    EnergyBalance(u64),
    RechargeConfig,
    /// Last ledger timestamp when passive regeneration was applied.
    LastRegen(u64),
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum EnergyError {
    InsufficientEnergy = 1,
    ShipNotFound = 2,
    InvalidAmount = 3,
    EnergyOverflow = 4,
    NegativeBalance = 5,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct EnergyBalance {
    pub ship_id: u64,
    pub current: u32,
    pub max: u32,
    /// Regeneration rate in energy units per tick (0 = no regen).
    pub regeneration_rate: u32,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct RechargeResult {
    pub ship_id: u64,
    pub energy_gained: u32,
    pub resources_consumed: i128,
}

/// Result of a passive energy recovery tick.
#[derive(Clone, Debug)]
#[contracttype]
pub struct PassiveRegenResult {
    pub ship_id: u64,
    pub energy_recovered: u32,
    pub new_balance: u32,
}

pub fn consume_energy(env: &Env, ship_id: u64, amount: u32) -> Result<u32, EnergyError> {
    if amount == 0 {
        return Err(EnergyError::InvalidAmount);
    }

    let _ship = env
        .storage()
        .persistent()
        .get::<ShipDataKey, ShipNft>(&ShipDataKey::Ship(ship_id))
        .ok_or(EnergyError::ShipNotFound)?;

    let current = env
        .storage()
        .persistent()
        .get::<EnergyKey, u32>(&EnergyKey::EnergyBalance(ship_id))
        .unwrap_or(MAX_ENERGY);

    if current < amount {
        return Err(EnergyError::InsufficientEnergy);
    }

    let new_balance = current.checked_sub(amount).ok_or(EnergyError::NegativeBalance)?;

    env.storage()
        .persistent()
        .set(&EnergyKey::EnergyBalance(ship_id), &new_balance);

    env.events().publish(
        (symbol_short!("energy"), symbol_short!("consume")),
        (ship_id, amount, new_balance),
    );

    Ok(new_balance)
}

pub fn recharge_energy(
    env: &Env,
    ship_id: u64,
    resource_amount: i128,
) -> Result<RechargeResult, EnergyError> {
    if resource_amount <= 0 {
        return Err(EnergyError::InvalidAmount);
    }

    let _ship = env
        .storage()
        .persistent()
        .get::<ShipDataKey, ShipNft>(&ShipDataKey::Ship(ship_id))
        .ok_or(EnergyError::ShipNotFound)?;

    let current = env
        .storage()
        .persistent()
        .get::<EnergyKey, u32>(&EnergyKey::EnergyBalance(ship_id))
        .unwrap_or(MAX_ENERGY);

    let energy_to_add = ((resource_amount as u32)
        .saturating_mul(RECHARGE_EFFICIENCY)
        .saturating_div(100))
    .min(BASE_RECHARGE_RATE);

    let new_balance = current.saturating_add(energy_to_add).min(MAX_ENERGY);

    env.storage()
        .persistent()
        .set(&EnergyKey::EnergyBalance(ship_id), &new_balance);

    let result = RechargeResult {
        ship_id,
        energy_gained: new_balance.saturating_sub(current),
        resources_consumed: resource_amount,
    };

    env.events().publish(
        (symbol_short!("energy"), symbol_short!("recharge")),
        (ship_id, result.energy_gained, new_balance),
    );

    Ok(result)
}

pub fn get_energy_balance(env: &Env, ship_id: u64) -> Result<EnergyBalance, EnergyError> {
    let _ship = env
        .storage()
        .persistent()
        .get::<ShipDataKey, ShipNft>(&ShipDataKey::Ship(ship_id))
        .ok_or(EnergyError::ShipNotFound)?;

    let current = env
        .storage()
        .persistent()
        .get::<EnergyKey, u32>(&EnergyKey::EnergyBalance(ship_id))
        .unwrap_or(MAX_ENERGY);

    let regen_rate = env
        .storage()
        .persistent()
        .get::<EnergyKey, u32>(&EnergyKey::RechargeConfig)
        .unwrap_or(BASE_PASSIVE_REGEN);

    let regen_key = EnergyKey::LastRegen(ship_id);
    let last_regen: u64 = env.storage().persistent().get(&regen_key).unwrap_or(0);

    Ok(EnergyBalance {
        ship_id,
        current,
        max: MAX_ENERGY,
        regeneration_rate: regen_rate,
    })
}

/// Apply passive energy recovery for a ship based on elapsed ledgers.
/// Call this before any energy-consuming operation to ensure regen is up to date.
pub fn apply_passive_regen(env: &Env, ship_id: u64) -> Result<PassiveRegenResult, EnergyError> {
    let _ship = env
        .storage()
        .persistent()
        .get::<ShipDataKey, ShipNft>(&ShipDataKey::Ship(ship_id))
        .ok_or(EnergyError::ShipNotFound)?;

    let now = env.ledger().timestamp();
    let regen_key = EnergyKey::LastRegen(ship_id);
    let last_regen: u64 = env.storage().persistent().get(&regen_key).unwrap_or(now);

    if now < last_regen + PASSIVE_REGEN_INTERVAL {
        return Ok(PassiveRegenResult {
            ship_id,
            energy_recovered: 0,
            new_balance: env
                .storage()
                .persistent()
                .get::<EnergyKey, u32>(&EnergyKey::EnergyBalance(ship_id))
                .unwrap_or(MAX_ENERGY),
        });
    }

    let elapsed_ticks = (now - last_regen) / PASSIVE_REGEN_INTERVAL;
    if elapsed_ticks == 0 {
        return Ok(PassiveRegenResult {
            ship_id,
            energy_recovered: 0,
            new_balance: env
                .storage()
                .persistent()
                .get::<EnergyKey, u32>(&EnergyKey::EnergyBalance(ship_id))
                .unwrap_or(MAX_ENERGY),
        });
    }

    let current = env
        .storage()
        .persistent()
        .get::<EnergyKey, u32>(&EnergyKey::EnergyBalance(ship_id))
        .unwrap_or(MAX_ENERGY);

    let regen_rate = env
        .storage()
        .persistent()
        .get::<EnergyKey, u32>(&EnergyKey::RechargeConfig)
        .unwrap_or(BASE_PASSIVE_REGEN);

    let total_regen = (regen_rate as u64)
        .saturating_mul(elapsed_ticks as u64)
        .min((MAX_ENERGY as u64).saturating_sub(current as u64)) as u32;

    if total_regen == 0 {
        return Ok(PassiveRegenResult {
            ship_id,
            energy_recovered: 0,
            new_balance: current,
        });
    }

    let new_balance = current.saturating_add(total_regen).min(MAX_ENERGY);
    env.storage()
        .persistent()
        .set(&EnergyKey::EnergyBalance(ship_id), &new_balance);
    env.storage().persistent().set(&regen_key, &now);

    env.events().publish(
        (symbol_short!("energy"), symbol_short!("passive")),
        (ship_id, total_regen, new_balance),
    );

    Ok(PassiveRegenResult {
        ship_id,
        energy_recovered: total_regen,
        new_balance,
    })
}

/// Set the regeneration rate for ships (admin / upgrade system).
pub fn set_regen_rate(env: &Env, caller: &Address, rate: u32) {
    caller.require_auth();
    env.storage()
        .persistent()
        .set(&EnergyKey::RechargeConfig, &rate);
}

/// Get the current passive regeneration rate.
pub fn get_regen_rate(env: &Env) -> u32 {
    env.storage()
        .persistent()
        .get(&EnergyKey::RechargeConfig)
        .unwrap_or(BASE_PASSIVE_REGEN)
}
