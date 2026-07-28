//! Reusable progression and difficulty curves.
//!
use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

use crate::health_monitor;
use crate::nebula_explorer::NebulaLayout;
use crate::ship_nft;

pub const MAX_LEVEL: u32 = 200;
const TUNING_WINDOW_SECONDS: u64 = 86_400 * 3;
const SCALE_PPM: i128 = 1_000_000;

#[derive(Clone)]
#[contracttype]
pub enum CurveKey {
    Config,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CurveError {
    InvalidLevel = 1,
    Unauthorized = 2,
    CurveLocked = 3,
    InvalidParameter = 4,
    InvalidValue = 5,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct CurveConfig {
    pub admin: Option<Address>,
    pub base_coefficient: i128,
    pub growth_rate_ppm: i128,
    pub floor: i128,
    pub cap: i128,
    pub tuning_expires_at: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ProgressiveDifficulty {
    pub player_level: u32,
    pub curve_score: i128,
    pub anomaly_count: u32,
    pub difficulty_multiplier: u32,
    pub scan_multiplier: u32,
    pub harvest_multiplier: u32,
    pub rarity_bias: u32,
}

fn default_config(env: &Env) -> CurveConfig {
    CurveConfig {
        admin: None,
        base_coefficient: 100,
        growth_rate_ppm: 1_020_000,
        floor: 100,
        cap: 20_000,
        tuning_expires_at: env.ledger().timestamp() + TUNING_WINDOW_SECONDS,
    }
}

fn load_config(env: &Env) -> CurveConfig {
    env.storage()
        .instance()
        .get(&CurveKey::Config)
        .unwrap_or_else(|| default_config(env))
}

fn persist_config(env: &Env, config: &CurveConfig) {
    env.storage().instance().set(&CurveKey::Config, config);
}

fn curve_score_for_level(config: &CurveConfig, player_level: u32) -> i128 {
    let mut score = config.base_coefficient;
    let mut level = 1;

    while level < player_level {
        score = score.saturating_mul(config.growth_rate_ppm) / SCALE_PPM;
        level += 1;
    }

    score.clamp(config.floor, config.cap)
}

fn level_from_ship_count(env: &Env, player: &Address) -> u32 {
    let ships = ship_nft::get_ships_by_owner(env, player);
    let level = (ships.len() as u32).saturating_add(1);
    level.min(MAX_LEVEL)
}

pub fn calculate_progressive_difficulty(
    env: &Env,
    player_level: u32,
) -> Result<ProgressiveDifficulty, CurveError> {
    if player_level == 0 || player_level > MAX_LEVEL {
        return Err(CurveError::InvalidLevel);
    }

    let config = load_config(env);
    let curve_score = curve_score_for_level(&config, player_level);
    let spread = curve_score.saturating_sub(config.base_coefficient);

    let anomaly_count = 5u32
        .saturating_add(player_level / 4)
        .saturating_add((spread / 800).max(0) as u32);
    let difficulty_multiplier = 100u32 + ((spread / 50).min(200) as u32);
    let scan_multiplier = 100u32 + ((spread / 90).min(140) as u32);
    let harvest_multiplier = 100u32 + ((spread / 120).min(100) as u32);
    let rarity_bias = ((spread / 60).min(100) as u32).min(100);

    health_monitor::record_contract_health(env, symbol_short!("curve"), curve_score as u64);

    Ok(ProgressiveDifficulty {
        player_level,
        curve_score,
        anomaly_count,
        difficulty_multiplier,
        scan_multiplier,
        harvest_multiplier,
        rarity_bias,
    })
}

pub fn adjust_curve_parameter(
    env: &Env,
    admin: &Address,
    param: Symbol,
    value: i128,
) -> Result<CurveConfig, CurveError> {
    admin.require_auth();

    let mut config = load_config(env);

    if config.admin.is_none() {
        config.admin = Some(admin.clone());
        config.tuning_expires_at = env.ledger().timestamp() + TUNING_WINDOW_SECONDS;
    }

    if config.admin.as_ref() != Some(admin) {
        return Err(CurveError::Unauthorized);
    }

    if env.ledger().timestamp() > config.tuning_expires_at {
        return Err(CurveError::CurveLocked);
    }

    if value <= 0 {
        return Err(CurveError::InvalidValue);
    }

    if param == symbol_short!("base") {
        config.base_coefficient = value;
    } else if param == symbol_short!("grow") {
        config.growth_rate_ppm = value;
    } else if param == symbol_short!("floor") {
        config.floor = value;
    } else if param == symbol_short!("cap") {
        config.cap = value;
    } else if param == symbol_short!("window") {
        config.tuning_expires_at = env.ledger().timestamp() + value as u64;
    } else {
        return Err(CurveError::InvalidParameter);
    }

    if config.floor > config.cap {
        return Err(CurveError::InvalidValue);
    }

    persist_config(env, &config);

    env.events().publish(
        (symbol_short!("curve"), symbol_short!("adjust")),
        (param, value, config.tuning_expires_at),
    );

    Ok(config)
}

pub fn get_curve_config(env: &Env) -> CurveConfig {
    load_config(env)
}

pub fn apply_curve_to_layout(
    env: &Env,
    player: &Address,
    layout: &mut NebulaLayout,
) -> Result<ProgressiveDifficulty, CurveError> {
    let level = level_from_ship_count(env, player);
    let difficulty = calculate_progressive_difficulty(env, level)?;

    let mut i = 0u32;
    while i < layout.cells.len() {
        if let Some(mut cell) = layout.cells.get(i) {
            cell.energy = cell.energy.saturating_mul(difficulty.scan_multiplier) / 100;
            layout.cells.set(i, cell);
        }
        i += 1;
    }

    let mut total_energy = 0u32;
    let mut j = 0u32;
    while j < layout.cells.len() {
        if let Some(cell) = layout.cells.get(j) {
            total_energy = total_energy.saturating_add(cell.energy);
        }
        j += 1;
    }
    layout.total_energy = total_energy;

    Ok(difficulty)
}
