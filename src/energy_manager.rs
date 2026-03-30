use crate::ship_nft::{DataKey as ShipDataKey, ShipNft};
use soroban_sdk::{contracterror, contracttype, symbol_short, Env};

const BASE_RECHARGE_RATE: u32 = 100;
const MAX_ENERGY: u32 = 10000;
const RECHARGE_EFFICIENCY: u32 = 85;

#[derive(Clone)]
#[contracttype]
pub enum EnergyKey {
    EnergyBalance(u64),
    RechargeConfig,
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
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct RechargeResult {
    pub ship_id: u64,
    pub energy_gained: u32,
    pub resources_consumed: i128,
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

    Ok(EnergyBalance {
        ship_id,
        current,
        max: MAX_ENERGY,
    })
}
