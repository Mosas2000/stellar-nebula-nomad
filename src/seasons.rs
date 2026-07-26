use soroban_sdk::{contracttype, contracterror, symbol_short, Address, Env};

// ─── Duration Constants ───────────────────────────────────────────────────────

/// Full season length: 90 days.
pub const SEASON_DURATION_SECS: u64 = 90 * 24 * 60 * 60;

/// Chapter length: 30 days (3 chapters per season).
pub const CHAPTER_DURATION_SECS: u64 = 30 * 24 * 60 * 60;

/// Number of chapters in a season.
pub const CHAPTERS_PER_SEASON: u32 = 3;

// ─── Reward Constants ─────────────────────────────────────────────────────────

/// Essence reward per scan during a season.
pub const REWARD_PER_SCAN: i128 = 10;
/// Bonus multiplier (in bps) applied to essence collected as a reward.
pub const ESSENCE_REWARD_BPS: i128 = 500; // 5%
/// Additional essence bonus for completing all 3 chapters.
pub const CHAPTER_COMPLETION_BONUS: i128 = 500;

// ─── Season Themes ────────────────────────────────────────────────────────────

/// Rotating season themes. Each theme drives visual identity, seasonal nebula
/// type, battle pass cosmetics, and challenge flavour text for one 90-day season.
///
/// Themes cycle in order and repeat after `NovaBurst`:
///   EmberNebula → VoidTide → StellarApex → CrimsonDrift → AzureVeil → NovaBurst → …
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum SeasonTheme {
    /// Season 1, 7, 13 … — fiery, volcanic nebula clouds.
    EmberNebula,
    /// Season 2, 8, 14 … — dark-matter tides and deep void rifts.
    VoidTide,
    /// Season 3, 9, 15 … — crystal peaks and prismatic formations.
    StellarApex,
    /// Season 4, 10, 16 … — crimson ionic storms and war zones.
    CrimsonDrift,
    /// Season 5, 11, 17 … — azure ice clouds and cold-light nebulae.
    AzureVeil,
    /// Season 6, 12, 18 … — supernova burst zones and stellar nurseries.
    NovaBurst,
}

impl SeasonTheme {
    /// Map a season ID (1-based) to its cycling theme.
    pub fn from_season_id(id: u64) -> SeasonTheme {
        match (id.saturating_sub(1)) % 6 {
            0 => SeasonTheme::EmberNebula,
            1 => SeasonTheme::VoidTide,
            2 => SeasonTheme::StellarApex,
            3 => SeasonTheme::CrimsonDrift,
            4 => SeasonTheme::AzureVeil,
            _ => SeasonTheme::NovaBurst,
        }
    }

    /// Return the exclusive nebula type spawned during this season.
    pub fn seasonal_nebula_type(&self) -> SeasonNebulaType {
        match self {
            SeasonTheme::EmberNebula  => SeasonNebulaType::EmberCloud,
            SeasonTheme::VoidTide     => SeasonNebulaType::VoidRift,
            SeasonTheme::StellarApex  => SeasonNebulaType::CrystalPeak,
            SeasonTheme::CrimsonDrift => SeasonNebulaType::CrimsonStorm,
            SeasonTheme::AzureVeil    => SeasonNebulaType::AzureGlacier,
            SeasonTheme::NovaBurst    => SeasonNebulaType::NovaCrater,
        }
    }
}

// ─── Seasonal Nebula Types ────────────────────────────────────────────────────

/// Exclusive nebula variants that only appear during their corresponding season.
/// These integrate with `nebula_gen`'s anomaly generation: the active seasonal
/// nebula type biases anomaly rolls toward rare outcomes when players explore
/// within the current season window.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum SeasonNebulaType {
    /// EmberNebula season — volcanic ash clouds with heat-fusion resources.
    EmberCloud,
    /// VoidTide season — dark-matter rifts with high-rarity anomaly density.
    VoidRift,
    /// StellarApex season — prismatic crystal formations with crystal resources.
    CrystalPeak,
    /// CrimsonDrift season — ionic storm zones with charged plasma vents.
    CrimsonStorm,
    /// AzureVeil season — frozen nebula clouds with cryo-energy deposits.
    AzureGlacier,
    /// NovaBurst season — supernova remnant craters with ultra-rare matter pockets.
    NovaCrater,
}

// ─── Season Configuration ─────────────────────────────────────────────────────

/// Per-season configuration written at rollover time and immutable thereafter.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct SeasonConfig {
    /// Visual/gameplay theme driving cosmetics and challenge flavour.
    pub theme: SeasonTheme,
    /// Exclusive nebula type active during this season.
    pub seasonal_nebula_type: SeasonNebulaType,
    /// Number of free battle-pass tiers available this season.
    pub free_pass_tiers: u32,
    /// Number of premium battle-pass tiers available this season.
    pub premium_pass_tiers: u32,
    /// Timestamp at which the seasonal leaderboard was last reset.
    pub leaderboard_reset_at: u64,
    /// Whether the seasonal nebula spawn bonus is currently live.
    pub nebula_bonus_active: bool,
}

// ─── Core Season Structs ──────────────────────────────────────────────────────

/// The active (or most-recently-ended) season record.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct Season {
    pub id: u64,
    pub start_time: u64,
    pub end_time: u64,
    pub title: soroban_sdk::String,
    /// Configuration set at season start.
    pub config: SeasonConfig,
    /// Current chapter number (1, 2, or 3). u32 for Soroban Val compatibility.
    pub current_chapter: u32,
}

/// Seasonal participation snapshot for a single player.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ParticipantStats {
    pub profile_id: u64,
    pub season_id: u64,
    pub total_scans: u32,
    pub essence_collected: i128,
    /// Bitmask: bit 0 = chapter 1, bit 1 = chapter 2, bit 2 = chapter 3.
    /// u32 for Soroban Val compatibility.
    pub chapters_active: u32,
    /// Whether the player discovered the season's exclusive nebula type.
    pub found_seasonal_nebula: bool,
}

/// Immutable snapshot of a completed season for historical reference.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct SeasonArchive {
    pub season: Season,
    pub ended_at: u64,
    pub total_participants: u32,
    pub total_essence_collected: i128,
    pub total_rewards_distributed: i128,
    /// How many participants completed all 3 chapters.
    pub full_chapter_completions: u32,
}

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum SeasonKey {
    /// Active season metadata.
    CurrentSeason,
    /// Season count.
    SeasonCount,
    /// Player's seasonal participation: (profile_id, season_id) -> ParticipantStats
    ParticipantStats(u64, u64),
    /// Archived season snapshot: season_id -> SeasonArchive
    ArchivedSeason(u64),
    /// Per-player season reward ready to claim: (profile_id, season_id) -> i128
    SeasonReward(u64, u64),
    /// Seasonal nebula discovery flag: (profile_id, season_id) -> bool
    NebulaDiscovery(u64, u64),
    /// Number of seasons a profile has participated in.
    ProfileSeasonCount(u64),
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SeasonError {
    NoActiveSeason       = 1,
    SeasonAlreadyStarted = 2,
    Unauthorized         = 3,
    SeasonNotExpired     = 4,
    NoRewardToClaim      = 5,
    /// Chapter advance attempted but season has not progressed far enough.
    ChapterNotReady      = 6,
    /// All 3 chapters are already complete.
    AllChaptersDone      = 7,
}

// ─── Initialize ───────────────────────────────────────────────────────────────

/// Initialize the very first season.
///
/// Sets a 90-day season with theme derived from season ID 1 (`EmberNebula`).
pub fn initialize_season(
    env: &Env,
    admin: Address,
    title: soroban_sdk::String,
) -> Result<u64, SeasonError> {
    admin.require_auth();

    if env.storage().instance().has(&SeasonKey::CurrentSeason) {
        return Err(SeasonError::SeasonAlreadyStarted);
    }

    let id = 1u64;
    let start_time = env.ledger().timestamp();
    let end_time = start_time + SEASON_DURATION_SECS;
    let theme = SeasonTheme::from_season_id(id);
    let nebula_type = theme.seasonal_nebula_type();

    let config = SeasonConfig {
        theme,
        seasonal_nebula_type: nebula_type,
        free_pass_tiers: 30,
        premium_pass_tiers: 50,
        leaderboard_reset_at: start_time,
        nebula_bonus_active: true,
    };

    let season = Season {
        id,
        start_time,
        end_time,
        title,
        config,
        current_chapter: 1u32,
    };

    env.storage().instance().set(&SeasonKey::CurrentSeason, &season);
    env.storage().instance().set(&SeasonKey::SeasonCount, &id);

    env.events().publish(
        (symbol_short!("season"), symbol_short!("started")),
        (id, start_time, end_time),
    );

    Ok(id)
}

// ─── Queries ──────────────────────────────────────────────────────────────────

/// Get the current active season.
pub fn get_current_season(env: &Env) -> Result<Season, SeasonError> {
    env.storage()
        .instance()
        .get(&SeasonKey::CurrentSeason)
        .ok_or(SeasonError::NoActiveSeason)
}

/// Return which chapter (1/2/3) is currently active based on elapsed time.
///
/// Chapter 1: days 1–30, Chapter 2: days 31–60, Chapter 3: days 61–90.
/// If the season has ended, returns 3 (final chapter).
pub fn get_current_chapter(env: &Env) -> Result<u32, SeasonError> {
    let season = get_current_season(env)?;
    let now = env.ledger().timestamp();
    let elapsed = now.saturating_sub(season.start_time);
    let chapter = ((elapsed / CHAPTER_DURATION_SECS) as u32 + 1).min(CHAPTERS_PER_SEASON);
    Ok(chapter)
}

/// Return the active season's exclusive nebula type.
pub fn get_seasonal_nebula_type(env: &Env) -> Result<SeasonNebulaType, SeasonError> {
    let season = get_current_season(env)?;
    Ok(season.config.seasonal_nebula_type)
}

/// Return `true` if the seasonal nebula spawn bonus is currently live.
pub fn is_seasonal_nebula_active(env: &Env) -> bool {
    get_current_season(env)
        .map(|s| s.config.nebula_bonus_active)
        .unwrap_or(false)
}

/// Return seconds remaining until the current season ends (0 if expired).
pub fn get_season_time_remaining(env: &Env) -> Result<u64, SeasonError> {
    let season = get_current_season(env)?;
    let now = env.ledger().timestamp();
    Ok(season.end_time.saturating_sub(now))
}

// ─── Chapter Advancement ──────────────────────────────────────────────────────

/// Admin: advance the season's stored `current_chapter` counter.
///
/// Normally the chapter is inferred from elapsed time via `get_current_chapter`.
/// This function updates the persisted field so on-chain queries always reflect
/// the latest chapter without recalculating from timestamps.
///
/// Emits a `"chapter"` / `"advance"` event with (season_id, new_chapter).
pub fn advance_chapter(env: &Env, admin: Address) -> Result<u32, SeasonError> {
    admin.require_auth();

    let mut season = get_current_season(env)?;

    if season.current_chapter >= CHAPTERS_PER_SEASON {
        return Err(SeasonError::AllChaptersDone);
    }

    let now = env.ledger().timestamp();
    let elapsed = now.saturating_sub(season.start_time);
    let computed_chapter =
        ((elapsed / CHAPTER_DURATION_SECS) as u32 + 1).min(CHAPTERS_PER_SEASON);

    if computed_chapter <= season.current_chapter {
        return Err(SeasonError::ChapterNotReady);
    }

    season.current_chapter = computed_chapter;
    env.storage()
        .instance()
        .set(&SeasonKey::CurrentSeason, &season);

    env.events().publish(
        (symbol_short!("chapter"), symbol_short!("advance")),
        (season.id, season.current_chapter),
    );

    Ok(season.current_chapter)
}

// ─── Participation Tracking ───────────────────────────────────────────────────

/// Record seasonal progress for a player after a scan/exploration action.
///
/// Also updates `chapters_active` if the player is active in a new chapter.
pub fn record_participation(
    env: &Env,
    profile_id: u64,
    scans: u32,
    essence: i128,
) -> Result<(), SeasonError> {
    let season = get_current_season(env)?;
    let key = SeasonKey::ParticipantStats(profile_id, season.id);

    let mut stats: ParticipantStats =
        env.storage().persistent().get(&key).unwrap_or(ParticipantStats {
            profile_id,
            season_id: season.id,
            total_scans: 0,
            essence_collected: 0,
            chapters_active: 0,
            found_seasonal_nebula: false,
        });

    stats.total_scans += scans;
    stats.essence_collected += essence;

    // Track which chapter this participation falls in using a u32 bitmask.
    // Bit 0 = chapter 1, bit 1 = chapter 2, bit 2 = chapter 3.
    let now = env.ledger().timestamp();
    let elapsed = now.saturating_sub(season.start_time);
    let chapter_index = ((elapsed / CHAPTER_DURATION_SECS) as u32).min(CHAPTERS_PER_SEASON - 1);
    let chapter_bit: u32 = 1 << chapter_index;
    stats.chapters_active |= chapter_bit;

    env.storage().persistent().set(&key, &stats);

    Ok(())
}

/// Mark that a player has discovered the seasonal exclusive nebula type.
///
/// Grants a small bonus and sets the discovery flag used by `SeasonalExplorer`
/// achievement logic and end-season reward calculation.
pub fn record_seasonal_nebula_discovery(
    env: &Env,
    profile_id: u64,
) -> Result<(), SeasonError> {
    let season = get_current_season(env)?;
    let key = SeasonKey::ParticipantStats(profile_id, season.id);

    let mut stats: ParticipantStats =
        env.storage().persistent().get(&key).unwrap_or(ParticipantStats {
            profile_id,
            season_id: season.id,
            total_scans: 0,
            essence_collected: 0,
            chapters_active: 0,
            found_seasonal_nebula: false,
        });

    stats.found_seasonal_nebula = true;
    env.storage().persistent().set(&key, &stats);

    env.events().publish(
        (symbol_short!("season"), symbol_short!("neb_disc")),
        (profile_id, season.id),
    );

    Ok(())
}

/// Get a player's participation stats for any season (active or archived).
pub fn get_participant_stats(
    env: &Env,
    profile_id: u64,
    season_id: u64,
) -> Option<ParticipantStats> {
    env.storage()
        .persistent()
        .get(&SeasonKey::ParticipantStats(profile_id, season_id))
}

// ─── Season Rollover ──────────────────────────────────────────────────────────

/// Unified season rollover: end the current season, compute and store per-player
/// rewards, archive it, and immediately start the next season.
///
/// This replaces the old `end_season` + `reset_season` split.  Call this once
/// per rollover; the `new_title` argument names the incoming season.
///
/// ### Reward formula
/// ```
/// reward = scans * REWARD_PER_SCAN
///        + essence_collected * ESSENCE_REWARD_BPS / 10_000
///        + (all 3 chapters active ? CHAPTER_COMPLETION_BONUS : 0)
/// ```
///
/// Returns the new season's ID.
pub fn rollover_season(
    env: &Env,
    admin: Address,
    new_title: soroban_sdk::String,
    participant_ids: soroban_sdk::Vec<u64>,
) -> Result<u64, SeasonError> {
    admin.require_auth();

    let season = get_current_season(env)?;
    let now = env.ledger().timestamp();

    if now < season.end_time {
        return Err(SeasonError::SeasonNotExpired);
    }

    let mut total_essence: i128 = 0;
    let mut total_rewards: i128 = 0;
    let mut full_completions: u32 = 0;
    // Bitmask for all 3 chapters active: bits 0,1,2 set = 0b111 = 7u32
    let all_chapters_mask: u32 = (1u32 << CHAPTERS_PER_SEASON) - 1;

    for profile_id in participant_ids.iter() {
        let stats_key = SeasonKey::ParticipantStats(profile_id, season.id);
        if let Some(stats) = env
            .storage()
            .persistent()
            .get::<SeasonKey, ParticipantStats>(&stats_key)
        {
            let chapter_bonus = if (stats.chapters_active & all_chapters_mask) == all_chapters_mask
            {
                full_completions += 1;
                CHAPTER_COMPLETION_BONUS
            } else {
                0
            };

            let reward = (stats.total_scans as i128) * REWARD_PER_SCAN
                + stats.essence_collected * ESSENCE_REWARD_BPS / 10_000
                + chapter_bonus;

            env.storage()
                .persistent()
                .set(&SeasonKey::SeasonReward(profile_id, season.id), &reward);

            // Remove the participation record (reset seasonal progress).
            env.storage().persistent().remove(&stats_key);

            // Increment cross-season participation counter.
            let season_count_key = SeasonKey::ProfileSeasonCount(profile_id);
            let prev_count: u32 = env
                .storage()
                .persistent()
                .get(&season_count_key)
                .unwrap_or(0);
            env.storage()
                .persistent()
                .set(&season_count_key, &(prev_count + 1));

            total_essence += stats.essence_collected;
            total_rewards += reward;
        }
    }

    // Archive the ended season.
    let archive = SeasonArchive {
        season: season.clone(),
        ended_at: now,
        total_participants: participant_ids.len(),
        total_essence_collected: total_essence,
        total_rewards_distributed: total_rewards,
        full_chapter_completions: full_completions,
    };
    env.storage()
        .instance()
        .set(&SeasonKey::ArchivedSeason(season.id), &archive);

    env.events().publish(
        (symbol_short!("season"), symbol_short!("ended")),
        (season.id, now, total_rewards),
    );

    // Start the next season immediately with the theme derived from its new ID.
    let new_id = season.id + 1;
    let new_theme = SeasonTheme::from_season_id(new_id);
    let new_nebula_type = new_theme.seasonal_nebula_type();

    let new_config = SeasonConfig {
        theme: new_theme,
        seasonal_nebula_type: new_nebula_type,
        free_pass_tiers: 30,
        premium_pass_tiers: 50,
        leaderboard_reset_at: now,
        nebula_bonus_active: true,
    };

    let new_season = Season {
        id: new_id,
        start_time: now,
        end_time: now + SEASON_DURATION_SECS,
        title: new_title,
        config: new_config,
        current_chapter: 1u32,
    };

    env.storage()
        .instance()
        .set(&SeasonKey::CurrentSeason, &new_season);
    env.storage()
        .instance()
        .set(&SeasonKey::SeasonCount, &new_id);

    env.events().publish(
        (symbol_short!("season"), symbol_short!("started")),
        (new_id, new_season.start_time, new_season.end_time),
    );

    Ok(new_id)
}

// ─── Reward Claiming ──────────────────────────────────────────────────────────

/// Claim the reward earned by `profile_id` for a completed season.
pub fn claim_season_reward(
    env: &Env,
    season_id: u64,
    profile_id: u64,
) -> Result<i128, SeasonError> {
    let key = SeasonKey::SeasonReward(profile_id, season_id);
    let reward: i128 = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(SeasonError::NoRewardToClaim)?;

    if reward == 0 {
        return Err(SeasonError::NoRewardToClaim);
    }

    env.storage().persistent().remove(&key);

    env.events().publish(
        (symbol_short!("season"), symbol_short!("claimed")),
        (profile_id, season_id, reward),
    );

    Ok(reward)
}

// ─── Archive Queries ──────────────────────────────────────────────────────────

/// Get an archived season by ID.
pub fn get_archived_season(env: &Env, season_id: u64) -> Option<SeasonArchive> {
    env.storage()
        .instance()
        .get(&SeasonKey::ArchivedSeason(season_id))
}

/// Return how many full seasons a player has participated in.
pub fn get_profile_season_count(env: &Env, profile_id: u64) -> u32 {
    env.storage()
        .persistent()
        .get(&SeasonKey::ProfileSeasonCount(profile_id))
        .unwrap_or(0)
}
