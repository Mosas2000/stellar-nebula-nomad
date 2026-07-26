use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

use crate::alliance_manager::{get_player_alliance, add_alliance_xp, credit_alliance_treasury};

// ─── Data Types ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct GuildQuest {
    pub quest_id: u64,
    pub alliance_id: u64,
    pub quest_type: Symbol, // scan, harvest, boss_kill
    pub target_count: u64,
    pub current_progress: u64,
    pub reward_essence: i128,
    pub reward_xp: u64,
    pub expires_at: u64,
    pub completed: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum GuildQuestKey {
    Quest(u64, u64),        // (alliance_id, quest_id) -> GuildQuest
    QuestCount(u64),        // alliance_id -> u64
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum GuildQuestError {
    QuestNotFound = 1,
    QuestExpired = 2,
    QuestAlreadyCompleted = 3,
    NotAllianceMember = 4,
    Unauthorized = 5,
}

// ─── Functions ────────────────────────────────────────────────────────────────

/// Create a new cooperative guild quest.
pub fn create_guild_quest(
    env: &Env,
    admin: Address,
    alliance_id: u64,
    quest_type: Symbol,
    target_count: u64,
    reward_essence: i128,
    reward_xp: u64,
    duration_secs: u64,
) -> Result<u64, GuildQuestError> {
    admin.require_auth(); // Simple auth for admin / creator

    let count_key = GuildQuestKey::QuestCount(alliance_id);
    let quest_id = env.storage().persistent().get(&count_key).unwrap_or(0u64) + 1;
    env.storage().persistent().set(&count_key, &quest_id);

    let expires_at = env.ledger().timestamp() + duration_secs;

    let quest = GuildQuest {
        quest_id,
        alliance_id,
        quest_type: quest_type.clone(),
        target_count,
        current_progress: 0,
        reward_essence,
        reward_xp,
        expires_at,
        completed: false,
    };

    let quest_key = GuildQuestKey::Quest(alliance_id, quest_id);
    env.storage().persistent().set(&quest_key, &quest);

    env.events().publish(
        (symbol_short!("g_quest"), symbol_short!("created")),
        (alliance_id, quest_id, quest_type, target_count),
    );

    Ok(quest_id)
}

/// Increment progress on active quests for a player's alliance.
/// Called automatically when players scan, harvest, or complete boss encounters.
pub fn contribute_quest_progress(
    env: &Env,
    player: Address,
    quest_type: Symbol,
    amount: u64,
) -> Result<(), GuildQuestError> {
    let alliance_id = get_player_alliance(env, player.clone())
        .ok_or(GuildQuestError::NotAllianceMember)?;

    let count_key = GuildQuestKey::QuestCount(alliance_id);
    let total_quests = env.storage().persistent().get(&count_key).unwrap_or(0u64);

    let now = env.ledger().timestamp();

    for qid in 1..=total_quests {
        let quest_key = GuildQuestKey::Quest(alliance_id, qid);
        if let Some(mut quest) = env.storage().persistent().get::<_, GuildQuest>(&quest_key) {
            if !quest.completed && now <= quest.expires_at && quest.quest_type == quest_type {
                quest.current_progress = quest.current_progress.saturating_add(amount);

                if quest.current_progress >= quest.target_count {
                    quest.completed = true;

                    // Award rewards to shared treasury and bump guild level XP
                    let _ = credit_alliance_treasury(env, alliance_id, quest.reward_essence);
                    let _ = add_alliance_xp(env, alliance_id, quest.reward_xp);

                    env.events().publish(
                        (symbol_short!("g_quest"), symbol_short!("done")),
                        (alliance_id, quest.quest_id, quest.reward_essence, quest.reward_xp),
                    );
                }

                env.storage().persistent().set(&quest_key, &quest);
            }
        }
    }

    Ok(())
}

/// Get all active (non-expired, uncompleted) cooperative quests for an alliance.
pub fn get_active_guild_quests(env: &Env, alliance_id: u64) -> Vec<GuildQuest> {
    let mut active = Vec::new(env);
    let count_key = GuildQuestKey::QuestCount(alliance_id);
    let total_quests = env.storage().persistent().get(&count_key).unwrap_or(0u64);
    let now = env.ledger().timestamp();

    for qid in 1..=total_quests {
        let quest_key = GuildQuestKey::Quest(alliance_id, qid);
        if let Some(quest) = env.storage().persistent().get::<_, GuildQuest>(&quest_key) {
            if !quest.completed && now <= quest.expires_at {
                active.push_back(quest);
            }
        }
    }

    active
}
