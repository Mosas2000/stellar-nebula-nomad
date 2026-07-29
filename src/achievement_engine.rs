//! Achievement eligibility evaluation and badge issuance.
//!
use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, String, Symbol, Vec};

use crate::health_monitor;
use crate::player_profile::{get_profile_by_owner, mark_achievement_unlocked};
use crate::ship_nft;

const MAX_BATCH_UNLOCKS: u32 = 5;

#[derive(Clone)]
#[contracttype]
pub enum AchievementKey {
    Template(u64),
    TemplateCount,
    PlayerAchievement(Address, u64),
    PlayerBadges(Address),
    Badge(u64),
    BadgeCounter,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AchievementError {
    AlreadyUnlocked = 1,
    TemplateNotFound = 2,
    ProfileNotFound = 3,
    NotEligible = 4,
    BatchTooLarge = 5,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct AchievementTemplate {
    pub id: u64,
    pub title: String,
    pub description: String,
    pub min_scans: u32,
    pub min_essence: i128,
    pub min_ships: u32,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct AchievementBadge {
    pub badge_id: u64,
    pub achievement_id: u64,
    pub owner: Address,
    pub title: String,
    pub minted_at: u64,
    pub transferable: bool,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct AchievementProgress {
    pub achievement_id: u64,
    pub title: String,
    pub unlocked: bool,
    pub eligible: bool,
    pub progress_pct: u32,
}

fn default_templates(env: &Env) -> Vec<AchievementTemplate> {
    let mut templates = Vec::new(env);

    templates.push_back(AchievementTemplate {
        id: 1,
        title: String::from_str(env, "First Scan"),
        description: String::from_str(env, "Complete the first scan."),
        min_scans: 1,
        min_essence: 0,
        min_ships: 0,
    });
    templates.push_back(AchievementTemplate {
        id: 2,
        title: String::from_str(env, "Surveyor"),
        description: String::from_str(env, "Reach 10 total scans."),
        min_scans: 10,
        min_essence: 0,
        min_ships: 0,
    });
    templates.push_back(AchievementTemplate {
        id: 3,
        title: String::from_str(env, "Pathfinder"),
        description: String::from_str(env, "Reach 25 total scans."),
        min_scans: 25,
        min_essence: 0,
        min_ships: 0,
    });
    templates.push_back(AchievementTemplate {
        id: 4,
        title: String::from_str(env, "Voyager"),
        description: String::from_str(env, "Reach 50 total scans."),
        min_scans: 50,
        min_essence: 0,
        min_ships: 0,
    });
    templates.push_back(AchievementTemplate {
        id: 5,
        title: String::from_str(env, "Navigator"),
        description: String::from_str(env, "Reach 100 total scans."),
        min_scans: 100,
        min_essence: 0,
        min_ships: 0,
    });
    templates.push_back(AchievementTemplate {
        id: 6,
        title: String::from_str(env, "Essence One"),
        description: String::from_str(env, "Earn 100 essence."),
        min_scans: 0,
        min_essence: 100,
        min_ships: 0,
    });
    templates.push_back(AchievementTemplate {
        id: 7,
        title: String::from_str(env, "Essence Two"),
        description: String::from_str(env, "Earn 500 essence."),
        min_scans: 0,
        min_essence: 500,
        min_ships: 0,
    });
    templates.push_back(AchievementTemplate {
        id: 8,
        title: String::from_str(env, "Essence Three"),
        description: String::from_str(env, "Earn 1,000 essence."),
        min_scans: 0,
        min_essence: 1_000,
        min_ships: 0,
    });
    templates.push_back(AchievementTemplate {
        id: 9,
        title: String::from_str(env, "Essence Four"),
        description: String::from_str(env, "Earn 5,000 essence."),
        min_scans: 0,
        min_essence: 5_000,
        min_ships: 0,
    });
    templates.push_back(AchievementTemplate {
        id: 10,
        title: String::from_str(env, "Fleet One"),
        description: String::from_str(env, "Own 1 ship."),
        min_scans: 0,
        min_essence: 0,
        min_ships: 1,
    });
    templates.push_back(AchievementTemplate {
        id: 11,
        title: String::from_str(env, "Fleet Three"),
        description: String::from_str(env, "Own 3 ships."),
        min_scans: 0,
        min_essence: 0,
        min_ships: 3,
    });
    templates.push_back(AchievementTemplate {
        id: 12,
        title: String::from_str(env, "Fleet Five"),
        description: String::from_str(env, "Own 5 ships."),
        min_scans: 0,
        min_essence: 0,
        min_ships: 5,
    });
    templates.push_back(AchievementTemplate {
        id: 13,
        title: String::from_str(env, "Fleet Ten"),
        description: String::from_str(env, "Own 10 ships."),
        min_scans: 0,
        min_essence: 0,
        min_ships: 10,
    });
    templates.push_back(AchievementTemplate {
        id: 14,
        title: String::from_str(env, "Deep Survey"),
        description: String::from_str(env, "Reach 150 total scans."),
        min_scans: 150,
        min_essence: 0,
        min_ships: 0,
    });
    templates.push_back(AchievementTemplate {
        id: 15,
        title: String::from_str(env, "Grand Survey"),
        description: String::from_str(env, "Reach 200 total scans."),
        min_scans: 200,
        min_essence: 0,
        min_ships: 0,
    });
    templates.push_back(AchievementTemplate {
        id: 16,
        title: String::from_str(env, "Cosmic Reach"),
        description: String::from_str(env, "Reach 300 total scans."),
        min_scans: 300,
        min_essence: 0,
        min_ships: 0,
    });
    templates.push_back(AchievementTemplate {
        id: 17,
        title: String::from_str(env, "Cosmic Wealth"),
        description: String::from_str(env, "Earn 10,000 essence."),
        min_scans: 0,
        min_essence: 10_000,
        min_ships: 0,
    });
    templates.push_back(AchievementTemplate {
        id: 18,
        title: String::from_str(env, "Armada"),
        description: String::from_str(env, "Own 20 ships."),
        min_scans: 0,
        min_essence: 0,
        min_ships: 20,
    });
    templates.push_back(AchievementTemplate {
        id: 19,
        title: String::from_str(env, "Trailblazer"),
        description: String::from_str(env, "Reach 400 total scans."),
        min_scans: 400,
        min_essence: 0,
        min_ships: 0,
    });
    templates.push_back(AchievementTemplate {
        id: 20,
        title: String::from_str(env, "Legend"),
        description: String::from_str(env, "Reach 500 scans, 20,000 essence, and 10 ships."),
        min_scans: 500,
        min_essence: 20_000,
        min_ships: 10,
    });

    templates
}

fn ensure_templates(env: &Env) {
    if env.storage().persistent().has(&AchievementKey::TemplateCount) {
        return;
    }

    let templates = default_templates(env);
    let mut i = 0u32;
    while i < templates.len() {
        if let Some(template) = templates.get(i) {
            env.storage()
                .persistent()
                .set(&AchievementKey::Template(template.id), &template);
        }
        i += 1;
    }

    env.storage()
        .persistent()
        .set(&AchievementKey::TemplateCount, &(templates.len() as u64));
}

fn get_template(env: &Env, achievement_id: u64) -> Result<AchievementTemplate, AchievementError> {
    ensure_templates(env);

    env.storage()
        .persistent()
        .get(&AchievementKey::Template(achievement_id))
        .ok_or(AchievementError::TemplateNotFound)
}

fn achievement_progress_pct(
    template: &AchievementTemplate,
    scans: u32,
    essence: i128,
    ships: u32,
) -> u32 {
    let mut pct = 100u32;

    if template.min_scans > 0 {
        pct = pct.min((scans.saturating_mul(100) / template.min_scans).min(100));
    }
    if template.min_essence > 0 {
        let essence_u = if essence < 0 { 0 } else { essence as u128 };
        let required = template.min_essence as u128;
        let current_pct = ((essence_u.saturating_mul(100)) / required).min(100) as u32;
        pct = pct.min(current_pct);
    }
    if template.min_ships > 0 {
        pct = pct.min((ships.saturating_mul(100) / template.min_ships).min(100));
    }

    pct
}

fn meets_template(template: &AchievementTemplate, scans: u32, essence: i128, ships: u32) -> bool {
    achievement_progress_pct(template, scans, essence, ships) >= 100
}

fn badge_counter(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&AchievementKey::BadgeCounter)
        .unwrap_or(0)
}

fn push_badge(env: &Env, player: &Address, badge_id: u64) {
    let key = AchievementKey::PlayerBadges(player.clone());
    let mut badges: Vec<u64> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    badges.push_back(badge_id);
    env.storage().persistent().set(&key, &badges);
}

fn unlocked_flag(env: &Env, player: &Address, achievement_id: u64) -> bool {
    env.storage()
        .persistent()
        .has(&AchievementKey::PlayerAchievement(player.clone(), achievement_id))
}

fn unlock_achievement_inner(
    env: &Env,
    player: &Address,
    achievement_id: u64,
    require_auth: bool,
) -> Result<AchievementBadge, AchievementError> {
    if require_auth {
        player.require_auth();
    }
    ensure_templates(env);

    if unlocked_flag(env, player, achievement_id) {
        return Err(AchievementError::AlreadyUnlocked);
    }

    let template = get_template(env, achievement_id)?;
    let profile = get_profile_by_owner(env, player).map_err(|_| AchievementError::ProfileNotFound)?;
    let ships = ship_nft::get_ships_by_owner(env, player);
    let ship_count = ships.len() as u32;

    if !meets_template(&template, profile.total_scans, profile.essence_earned, ship_count) {
        return Err(AchievementError::NotEligible);
    }

    env.storage().persistent().set(
        &AchievementKey::PlayerAchievement(player.clone(), achievement_id),
        &true,
    );

    let badge_id = badge_counter(env) + 1;
    env.storage()
        .instance()
        .set(&AchievementKey::BadgeCounter, &badge_id);

    let badge = AchievementBadge {
        badge_id,
        achievement_id,
        owner: player.clone(),
        title: template.title.clone(),
        minted_at: env.ledger().timestamp(),
        transferable: true,
    };

    env.storage()
        .persistent()
        .set(&AchievementKey::Badge(badge_id), &badge);
    push_badge(env, player, badge_id);
    mark_achievement_unlocked(env, profile.id, achievement_id)
        .map_err(|_| AchievementError::ProfileNotFound)?;

    health_monitor::record_contract_health(env, symbol_short!("achv"), achievement_id);

    env.events().publish(
        (symbol_short!("achv"), symbol_short!("unlock")),
        (player.clone(), achievement_id, badge_id),
    );

    Ok(badge)
}

pub fn unlock_achievement(
    env: &Env,
    player: Address,
    achievement_id: u64,
) -> Result<AchievementBadge, AchievementError> {
    unlock_achievement_inner(env, &player, achievement_id, true)
}

pub fn batch_unlock_achievements(
    env: &Env,
    player: Address,
    achievement_ids: Vec<u64>,
) -> Result<Vec<AchievementBadge>, AchievementError> {
    player.require_auth();

    if achievement_ids.len() > MAX_BATCH_UNLOCKS {
        return Err(AchievementError::BatchTooLarge);
    }

    let mut badges = Vec::new(env);
    let mut i = 0u32;
    while i < achievement_ids.len() {
        if let Some(achievement_id) = achievement_ids.get(i) {
            badges.push_back(unlock_achievement_inner(env, &player, achievement_id, false)?);
        }
        i += 1;
    }

    Ok(badges)
}

pub fn check_achievement_progress(
    env: &Env,
    player: Address,
) -> Result<Vec<AchievementProgress>, AchievementError> {
    let profile = get_profile_by_owner(env, &player).map_err(|_| AchievementError::ProfileNotFound)?;
    let ships = ship_nft::get_ships_by_owner(env, &player);
    let ship_count = ships.len() as u32;
    ensure_templates(env);

    let template_count: u64 = env
        .storage()
        .persistent()
        .get(&AchievementKey::TemplateCount)
        .unwrap_or(0);

    let mut progress = Vec::new(env);
    let mut i = 1u64;
    while i <= template_count {
        if let Some(template) = env
            .storage()
            .persistent()
            .get::<AchievementKey, AchievementTemplate>(&AchievementKey::Template(i))
        {
            progress.push_back(AchievementProgress {
                achievement_id: i,
                title: template.title.clone(),
                unlocked: unlocked_flag(env, &player, i),
                eligible: meets_template(&template, profile.total_scans, profile.essence_earned, ship_count),
                progress_pct: achievement_progress_pct(&template, profile.total_scans, profile.essence_earned, ship_count),
            });
        }
        i += 1;
    }

    Ok(progress)
}

pub fn get_player_achievement_count(env: &Env, player: Address) -> Result<u32, AchievementError> {
    let profile = get_profile_by_owner(env, &player).map_err(|_| AchievementError::ProfileNotFound)?;
    Ok(profile.achievement_flags.count_ones())
}

pub fn get_player_badges(env: &Env, player: Address) -> Vec<AchievementBadge> {
    let mut badges = Vec::new(env);
    if let Some(ids) = env
        .storage()
        .persistent()
        .get::<AchievementKey, Vec<u64>>(&AchievementKey::PlayerBadges(player))
    {
        let mut i = 0u32;
        while i < ids.len() {
            if let Some(id) = ids.get(i) {
                if let Some(badge) = env
                    .storage()
                    .persistent()
                    .get::<AchievementKey, AchievementBadge>(&AchievementKey::Badge(id))
                {
                    badges.push_back(badge);
                }
            }
            i += 1;
        }
    }
    badges
}
