use soroban_sdk::{contracterror, contracttype, symbol_short, Env};

/// Maximum player level.
pub const MAX_LEVEL: u32 = 100;

/// Base anomaly count at level 1.
const BASE_ANOMALY_COUNT: u32 = 5;

/// Anomaly count increment per level.
const ANOMALY_PER_LEVEL: u32 = 2;

#[derive(Clone)]
#[contracttype]
pub enum DifficultyKey {
    /// Base curve configuration.
    BaseCurve,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum DifficultyError {
    /// Player level is invalid (0 or > 100).
    InvalidLevel = 1,
}

/// Rarity weight distribution for difficulty scaling.
/// Each weight is a percentage (0-100) summing to 100.
#[derive(Clone)]
#[contracttype]
pub struct RarityWeights {
    pub common: u32,
    pub uncommon: u32,
    pub rare: u32,
    pub epic: u32,
    pub legendary: u32,
}

/// Result of difficulty calculation.
#[derive(Clone)]
#[contracttype]
pub struct DifficultyResult {
    pub anomaly_count: u32,
    pub rarity_weights: RarityWeights,
    pub difficulty_multiplier: u32,
}

/// Calculate the difficulty scaling for a given player level.
///
/// Returns the number of anomalies and rarity distribution weights.
/// Higher levels produce more anomalies and shift rarity toward rarer tiers.
///
/// # Errors
/// Returns `DifficultyError::InvalidLevel` if level is 0 or > 100.
pub fn calculate_difficulty(
    env: &Env,
    player_level: u32,
) -> Result<DifficultyResult, DifficultyError> {
    if player_level == 0 || player_level > MAX_LEVEL {
        return Err(DifficultyError::InvalidLevel);
    }

    // Anomaly count scales linearly with level
    let anomaly_count = BASE_ANOMALY_COUNT + (player_level - 1) * ANOMALY_PER_LEVEL;

    // Difficulty multiplier: 100 at level 1, 200 at level 100
    let difficulty_multiplier = 100 + (player_level - 1) * 100 / (MAX_LEVEL - 1);

    // Rarity weights shift toward rarer tiers at higher levels
    // At level 1:  common=60, uncommon=25, rare=10, epic=4, legendary=1
    // At level 100: common=10, uncommon=20, rare=30, epic=25, legendary=15
    let level_frac = (player_level - 1) * 100 / (MAX_LEVEL - 1); // 0..100

    let common = 60u32.saturating_sub(level_frac * 50 / 100);
    let legendary = 1 + level_frac * 14 / 100;
    let epic = 4 + level_frac * 21 / 100;
    let rare = 10 + level_frac * 20 / 100;
    // Uncommon gets the remainder to ensure sum = 100
    let uncommon = 100u32.saturating_sub(common + rare + epic + legendary);

    let rarity_weights = RarityWeights {
        common,
        uncommon,
        rare,
        epic,
        legendary,
    };

    // Emit DifficultyAdjusted event
    env.events().publish(
        (symbol_short!("diff"), symbol_short!("adjust")),
        (player_level, anomaly_count, difficulty_multiplier),
    );

    Ok(DifficultyResult {
        anomaly_count,
        rarity_weights,
        difficulty_multiplier,
    })
}

/// Apply difficulty scaling to a nebula layout.
///
/// Modifies the layout's anomaly count based on the difficulty multiplier
/// for the given player level. Returns the scaled anomaly count.
pub fn apply_scaling_to_layout(
    env: &Env,
    base_anomaly_count: u32,
    player_level: u32,
) -> Result<u32, DifficultyError> {
    let result = calculate_difficulty(env, player_level)?;

    // Scale the base count by the difficulty multiplier
    let scaled = base_anomaly_count * result.difficulty_multiplier / 100;

    Ok(scaled)
}
