use soroban_sdk::{contracterror, contracttype, symbol_short, Env, Symbol, Vec};

const ENVIRONMENTAL_PRESETS: [&str; 8] = [
    "calm",
    "storm",
    "radiate",
    "dense",
    "sparse",
    "charged",
    "temporal",
    "void",
];

#[derive(Clone)]
#[contracttype]
pub enum EnvironmentKey {
    NebulaCondition(u64),
    ConditionRegistry,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum EnvironmentError {
    InvalidCondition = 1,
    InvalidNebula = 2,
    SimulationFailed = 3,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct EnvironmentCondition {
    pub nebula_id: u64,
    pub condition: Symbol,
    pub scan_modifier: i32,
    pub harvest_modifier: i32,
    pub radiation_level: u32,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct ModifierResult {
    pub ship_id: u64,
    pub condition: Symbol,
    pub adjusted_yield: i32,
}

pub fn simulate_conditions(env: &Env, nebula_id: u64) -> Result<EnvironmentCondition, EnvironmentError> {
    if nebula_id == 0 {
        return Err(EnvironmentError::InvalidNebula);
    }

    let ledger_seq = env.ledger().sequence();
    let timestamp = env.ledger().timestamp();
    
    let seed = (ledger_seq as u64)
        .wrapping_mul(timestamp)
        .wrapping_add(nebula_id);
    
    let condition_index = (seed % 8) as usize;
    let condition_name = ENVIRONMENTAL_PRESETS[condition_index];
    
    let condition_symbol = match condition_name {
        "calm" => symbol_short!("calm"),
        "storm" => symbol_short!("storm"),
        "radiate" => symbol_short!("radiate"),
        "dense" => symbol_short!("dense"),
        "sparse" => symbol_short!("sparse"),
        "charged" => symbol_short!("charged"),
        "temporal" => symbol_short!("temporal"),
        "void" => symbol_short!("void"),
        _ => symbol_short!("neutral"),
    };

    let (scan_mod, harvest_mod, radiation) = match condition_name {
        "calm" => (10, 5, 10),
        "storm" => (-15, -10, 80),
        "radiate" => (-5, 15, 95),
        "dense" => (5, 20, 30),
        "sparse" => (-10, -15, 5),
        "charged" => (20, 10, 60),
        "temporal" => (15, -5, 40),
        "void" => (0, 0, 100),
        _ => (0, 0, 50),
    };

    let condition = EnvironmentCondition {
        nebula_id,
        condition: condition_symbol.clone(),
        scan_modifier: scan_mod,
        harvest_modifier: harvest_mod,
        radiation_level: radiation,
    };

    env.storage()
        .persistent()
        .set(&EnvironmentKey::NebulaCondition(nebula_id), &condition);

    env.events().publish(
        (symbol_short!("env"), symbol_short!("simul")),
        (nebula_id, condition_symbol),
    );

    Ok(condition)
}

pub fn apply_environmental_modifier(
    env: &Env,
    ship_id: u64,
    nebula_id: u64,
    base_yield: i32,
) -> Result<ModifierResult, EnvironmentError> {
    let condition = env
        .storage()
        .persistent()
        .get::<EnvironmentKey, EnvironmentCondition>(&EnvironmentKey::NebulaCondition(nebula_id))
        .unwrap_or_else(|| {
            simulate_conditions(env, nebula_id).unwrap_or(EnvironmentCondition {
                nebula_id,
                condition: symbol_short!("neutral"),
                scan_modifier: 0,
                harvest_modifier: 0,
                radiation_level: 50,
            })
        });

    let modifier = condition.harvest_modifier;
    let adjusted = base_yield + modifier;
    let final_yield = if adjusted < 0 { 0 } else { adjusted };

    let result = ModifierResult {
        ship_id,
        condition: condition.condition.clone(),
        adjusted_yield: final_yield,
    };

    Ok(result)
}

pub fn get_nebula_condition(env: &Env, nebula_id: u64) -> Option<EnvironmentCondition> {
    env.storage()
        .persistent()
        .get::<EnvironmentKey, EnvironmentCondition>(&EnvironmentKey::NebulaCondition(nebula_id))
}
