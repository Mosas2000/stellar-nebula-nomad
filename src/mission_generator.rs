use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, String, Symbol, Vec};

const DAILY_MISSION_LIMIT: u32 = 3;
const MISSION_REWARD_BASE: i128 = 100;

#[derive(Clone)]
#[contracttype]
pub enum MissionKey {
    MissionCounter,
    PlayerMission(Address, u64),
    DailyReset(Address),
    MissionData(u64),
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MissionError {
    MissionAlreadyClaimed = 1,
    InvalidMission = 2,
    DailyLimitReached = 3,
    NotCompleted = 4,
    ProfileNotFound = 5,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct Mission {
    pub mission_id: u64,
    pub player: Address,
    pub mission_type: Symbol,
    pub target_count: u32,
    pub current_progress: u32,
    pub reward: i128,
    pub completed: bool,
    pub claimed: bool,
    pub expires_at: u64,
    pub prerequisite_missions: Vec<u64>,
    pub narrative_tier: Option<Symbol>,
    pub archetype_tag: Option<Symbol>,
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct MissionReward {
    pub mission_id: u64,
    pub player: Address,
    pub reward: i128,
    /// Quest-chain nodes this mission completion advanced (Issue #282).
    /// Missions are the atoms quests are built from, so claiming one also
    /// pushes any active quest with a matching objective forward.
    pub quests_advanced: u32,
}

pub fn generate_daily_mission(env: &Env, player: Address) -> Result<Mission, MissionError> {
    player.require_auth();

    let current_day = env.ledger().timestamp() / 86400;
    let last_reset = env
        .storage()
        .persistent()
        .get::<MissionKey, u64>(&MissionKey::DailyReset(player.clone()))
        .unwrap_or(0);

    let daily_count = if current_day > last_reset {
        env.storage()
            .persistent()
            .set(&MissionKey::DailyReset(player.clone()), &current_day);
        0
    } else {
        let mut count = 0;
        for i in 0..DAILY_MISSION_LIMIT {
            if env
                .storage()
                .persistent()
                .has(&MissionKey::PlayerMission(player.clone(), current_day + (i as u64)))
            {
                count += 1;
            }
        }
        count
    };

    if daily_count >= DAILY_MISSION_LIMIT {
        return Err(MissionError::DailyLimitReached);
    }

    let mission_counter = env
        .storage()
        .persistent()
        .get::<MissionKey, u64>(&MissionKey::MissionCounter)
        .unwrap_or(0) + 1;

    env.storage()
        .persistent()
        .set(&MissionKey::MissionCounter, &mission_counter);

    let ledger_seq = env.ledger().sequence();
    let seed = (ledger_seq as u64)
        .wrapping_mul(mission_counter)
        .wrapping_add(env.ledger().timestamp());

    let mission_types = ["scan", "harvest", "explore", "trade"];
    let mission_type_idx = (seed % 4) as usize;
    let mission_type_str = mission_types[mission_type_idx];

    let mission_type_symbol = match mission_type_str {
        "scan" => symbol_short!("scan"),
        "harvest" => symbol_short!("harvest"),
        "explore" => symbol_short!("explore"),
        "trade" => symbol_short!("trade"),
        _ => symbol_short!("scan"),
    };

    let target = ((seed % 10) + 5) as u32;
    let reward = MISSION_REWARD_BASE.saturating_mul(target as i128);
    let expires_at = env.ledger().timestamp() + 86400;
    
    let mut prerequisites = Vec::new(env);
    if mission_counter > 1 {
        prerequisites.push_back(mission_counter - 1);
    }

    let mission = Mission {
        mission_id: mission_counter,
        player: player.clone(),
        mission_type: mission_type_symbol.clone(),
        target_count: target,
        current_progress: 0,
        reward,
        completed: false,
        claimed: false,
        expires_at,
        prerequisite_missions: prerequisites,
        narrative_tier: None,
        archetype_tag: None,
        title: None,
        description: None,
    };

    env.storage()
        .persistent()
        .set(&MissionKey::MissionData(mission_counter), &mission);
    env.storage()
        .persistent()
        .set(&MissionKey::PlayerMission(player.clone(), mission_counter), &true);

    env.events().publish(
        (symbol_short!("mission"), symbol_short!("generate")),
        (mission_counter, player, mission_type_symbol),
    );

    Ok(mission)
}

pub fn complete_mission(
    env: &Env,
    player: Address,
    mission_id: u64,
) -> Result<MissionReward, MissionError> {
    player.require_auth();

    let mut mission = env
        .storage()
        .persistent()
        .get::<MissionKey, Mission>(&MissionKey::MissionData(mission_id))
        .ok_or(MissionError::InvalidMission)?;

    if mission.player != player {
        return Err(MissionError::InvalidMission);
    }

    if mission.claimed {
        return Err(MissionError::MissionAlreadyClaimed);
    }

    if !mission.completed {
        return Err(MissionError::NotCompleted);
    }

    if env.ledger().timestamp() > mission.expires_at {
        return Err(MissionError::InvalidMission);
    }

    mission.claimed = true;
    env.storage()
        .persistent()
        .set(&MissionKey::MissionData(mission_id), &mission);

    // Issue #282: feed the completion into the quest system so guided content
    // advances from ordinary play. Quests whose objective does not match — or
    // which have already expired — are skipped, so this never fails the claim.
    let quests_advanced = crate::quest_system::on_mission_completed(
        env,
        &player,
        mission.mission_type.clone(),
        mission.target_count,
    );

    let reward = MissionReward {
        mission_id,
        player: player.clone(),
        reward: mission.reward,
        quests_advanced,
    };

    env.events().publish(
        (symbol_short!("mission"), symbol_short!("complete")),
        (mission_id, player, mission.reward, quests_advanced),
    );

    Ok(reward)
}

pub fn update_mission_progress(
    env: &Env,
    mission_id: u64,
    progress: u32,
) -> Result<Mission, MissionError> {
    let mut mission = env
        .storage()
        .persistent()
        .get::<MissionKey, Mission>(&MissionKey::MissionData(mission_id))
        .ok_or(MissionError::InvalidMission)?;

    mission.current_progress = mission.current_progress.saturating_add(progress);

    if mission.current_progress >= mission.target_count {
        mission.completed = true;
    }

    env.storage()
        .persistent()
        .set(&MissionKey::MissionData(mission_id), &mission);

    Ok(mission)
}

pub fn get_player_missions(env: &Env, player: Address) -> Vec<Mission> {
    let mut missions = Vec::new(env);
    
    let mission_counter = env
        .storage()
        .persistent()
        .get::<MissionKey, u64>(&MissionKey::MissionCounter)
        .unwrap_or(0);

    for i in 1..=mission_counter {
        if let Some(mission) = env
            .storage()
            .persistent()
            .get::<MissionKey, Mission>(&MissionKey::MissionData(i))
        {
            if mission.player == player {
                missions.push_back(mission);
            }
        }
    }

    missions
}

pub fn start_mission(env: &Env, player: Address, mission_id: u64) -> Result<(), MissionError> {
    player.require_auth();
    let mission = env.storage().persistent().get::<MissionKey, Mission>(&MissionKey::MissionData(mission_id)).ok_or(MissionError::InvalidMission)?;
    
    for prereq_id in mission.prerequisite_missions.iter() {
        let prereq = env.storage().persistent().get::<MissionKey, Mission>(&MissionKey::MissionData(prereq_id)).ok_or(MissionError::InvalidMission)?;
        if !prereq.completed {
            return Err(MissionError::NotCompleted);
        }
    }
    Ok(())
}

pub fn generate_ai_mission(env: &Env, player: Address) -> Result<Mission, MissionError> {
    player.require_auth();

    let current_day = env.ledger().timestamp() / 86400;
    let last_reset = env
        .storage()
        .persistent()
        .get::<MissionKey, u64>(&MissionKey::DailyReset(player.clone()))
        .unwrap_or(0);

    let daily_count = if current_day > last_reset {
        env.storage()
            .persistent()
            .set(&MissionKey::DailyReset(player.clone()), &current_day);
        0
    } else {
        let mut count = 0;
        for i in 0..DAILY_MISSION_LIMIT {
            if env
                .storage()
                .persistent()
                .has(&MissionKey::PlayerMission(player.clone(), current_day + (i as u64)))
            {
                count += 1;
            }
        }
        count
    };

    if daily_count >= DAILY_MISSION_LIMIT {
        return Err(MissionError::DailyLimitReached);
    }

    let mission_counter = env
        .storage()
        .persistent()
        .get::<MissionKey, u64>(&MissionKey::MissionCounter)
        .unwrap_or(0) + 1;

    env.storage()
        .persistent()
        .set(&MissionKey::MissionCounter, &mission_counter);

    let ledger_seq = env.ledger().sequence();
    let seed = (ledger_seq as u64)
        .wrapping_mul(mission_counter)
        .wrapping_add(env.ledger().timestamp());

    // Generate procedural adaptive mission via AI engine
    let ai_result = crate::ai_mission_engine::generate_ai_mission_internal(env, player.clone(), seed);

    let expires_at = env.ledger().timestamp() + 86400;
    
    let mut prerequisites = Vec::new(env);
    if mission_counter > 1 {
        prerequisites.push_back(mission_counter - 1);
    }

    let mission = Mission {
        mission_id: mission_counter,
        player: player.clone(),
        mission_type: ai_result.mission_type.clone(),
        target_count: ai_result.target_count,
        current_progress: 0,
        reward: ai_result.reward,
        completed: false,
        claimed: false,
        expires_at,
        prerequisite_missions: prerequisites,
        narrative_tier: Some(ai_result.narrative_tier.clone()),
        archetype_tag: Some(ai_result.archetype_tag),
        title: Some(ai_result.title),
        description: Some(ai_result.description),
    };

    env.storage()
        .persistent()
        .set(&MissionKey::MissionData(mission_counter), &mission);
    env.storage()
        .persistent()
        .set(&MissionKey::PlayerMission(player.clone(), mission_counter), &true);

    env.events().publish(
        (symbol_short!("mission"), symbol_short!("ai_gen")),
        (mission_counter, player, ai_result.mission_type, ai_result.narrative_tier),
    );

    Ok(mission)
}
