use soroban_sdk::{contracttype, contracterror, symbol_short, Address, Env, String, Symbol, Vec};
use crate::seasons::get_current_season;

// ─── XP Constants ─────────────────────────────────────────────────────────────

/// XP earned per nebula scan.
pub const XP_PER_SCAN: u32 = 10;
/// XP earned per unit of essence collected.
pub const XP_PER_ESSENCE: u32 = 1;
/// Bonus XP burst for completing a time-limited challenge.
pub const XP_PER_CHALLENGE_COMPLETE: u32 = 100;
/// Bonus XP awarded when a player discovers the season's exclusive nebula type.
pub const XP_PER_SEASONAL_NEBULA: u32 = 50;

// ─── Tier Constants ───────────────────────────────────────────────────────────

/// Maximum free battle pass tiers per season.
pub const MAX_FREE_TIERS: u32 = 30;
/// Maximum premium battle pass tiers per season.
pub const MAX_PREMIUM_TIERS: u32 = 50;

// ─── Pass Tier ────────────────────────────────────────────────────────────────

/// Indicates which reward track a `BattlePassReward` belongs to.
///
/// Free-track rewards (essence, XP items, utility boosts) are accessible to all
/// players.  Premium-track rewards are cosmetic (skins, title badges, palette
/// swaps) and never confer a stat advantage — no pay-to-win.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum PassTier {
    /// Available to all players.
    Free,
    /// Requires premium pass ownership for this season.
    Premium,
}

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum BattlePassKey {
    /// Player's battle pass state: (profile_id, season_id) -> BattlePassState
    State(u64, u64),
    /// Reward template: (season_id, tier, pass_tier_u8) -> BattlePassReward
    /// pass_tier_u8: 0 = Free, 1 = Premium
    Reward(u64, u32, u32),
    /// Whether a player holds the premium pass for a season: (profile_id, season_id) -> bool
    PremiumUnlock(u64, u64),
    /// Seasonal cosmetic definition: cosmetic_id -> SeasonalCosmetic
    SeasonalCosmetic(u32),
    /// Tracks XP granted for a challenge to prevent double-award: (profile_id, challenge_id) -> bool
    ChallengeXpGranted(u64, u64),
}

// ─── Data Structs ─────────────────────────────────────────────────────────────

/// A single player's battle pass progress for one season.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct BattlePassState {
    pub profile_id: u64,
    pub season_id: u64,
    /// Total accumulated XP this season.
    pub xp: u64,
    /// Bitmask of free tiers claimed (bit N = tier N claimed). Supports up to 30 free tiers.
    pub free_rewards_claimed: u32,
    /// Two u32 bitmasks covering 50 premium tiers (bits 0–31 and 32–49).
    pub premium_rewards_claimed_lo: u32, // tiers 1–32
    pub premium_rewards_claimed_hi: u32, // tiers 33–50 (bits 0–17)
    /// Whether this player holds the premium pass this season.
    pub has_premium: bool,
}

/// A reward available at a specific battle pass tier.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct BattlePassReward {
    /// Tier number (1-based).
    pub tier: u32,
    /// Total XP required to unlock this tier.
    pub xp_required: u64,
    /// Essence / token amount granted (0 for cosmetic-only rewards).
    pub reward_amount: i128,
    /// Which pass track this reward belongs to.
    pub pass_tier: PassTier,
    /// Optional cosmetic skin ID linking to the `skins` module (None = no cosmetic).
    pub cosmetic_skin_id: u32,
    /// Optional title badge symbol (empty symbol = no badge).
    pub title_badge: Symbol,
}

/// A season-exclusive cosmetic reward that lives in a player's collection.
///
/// Cosmetics are forever in the recipient's wallet once earned, but the earn
/// opportunity closes when the season ends — delivering FOMO without stat gating.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct SeasonalCosmetic {
    /// Unique cosmetic identifier (sequential across all seasons).
    pub cosmetic_id: u32,
    /// Display name symbol.
    pub name: Symbol,
    /// Long-form display name.
    pub display_name: String,
    /// Primary color (0xRRGGBB).
    pub skin_color_primary: u32,
    /// Secondary color (0xRRGGBB).
    pub skin_color_secondary: u32,
    /// Season this cosmetic is associated with.
    pub season_id: u64,
    /// `true` means it cannot be earned in future seasons.
    pub is_exclusive: bool,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum BattlePassError {
    NotEnoughXP        = 1,
    AlreadyClaimed     = 2,
    InvalidTier        = 3,
    NoActiveSeason     = 4,
    PremiumRequired    = 5,
    TierOutOfRange     = 6,
    /// XP for this challenge was already granted to this player.
    ChallengeXpAlreadyGranted = 7,
}

// ─── XP Accumulation ──────────────────────────────────────────────────────────

/// Add XP to a player's battle pass for the current season based on scan and
/// essence activity.
pub fn add_xp(
    env: &Env,
    profile_id: u64,
    scans: u32,
    essence: i128,
) -> Result<u64, BattlePassError> {
    let season = get_current_season(env).map_err(|_| BattlePassError::NoActiveSeason)?;
    let key = BattlePassKey::State(profile_id, season.id);

    let mut state: BattlePassState =
        env.storage().persistent().get(&key).unwrap_or(BattlePassState {
            profile_id,
            season_id: season.id,
            xp: 0,
            free_rewards_claimed: 0,
            premium_rewards_claimed_lo: 0,
            premium_rewards_claimed_hi: 0,
            has_premium: false,
        });

    let gained_xp =
        (scans as u64 * XP_PER_SCAN as u64) + (essence.max(0) as u64 * XP_PER_ESSENCE as u64);
    state.xp += gained_xp;

    env.storage().persistent().set(&key, &state);

    env.events().publish(
        (symbol_short!("bp"), symbol_short!("xp_gain")),
        (profile_id, state.xp, gained_xp),
    );

    Ok(state.xp)
}

/// Grant XP for completing a time-limited challenge.  Idempotent — calling
/// twice for the same (profile_id, challenge_id) pair is a no-op after the
/// first call, returning `ChallengeXpAlreadyGranted`.
pub fn add_xp_for_challenge(
    env: &Env,
    profile_id: u64,
    challenge_id: u64,
) -> Result<u64, BattlePassError> {
    let season = get_current_season(env).map_err(|_| BattlePassError::NoActiveSeason)?;

    let dedup_key = BattlePassKey::ChallengeXpGranted(profile_id, challenge_id);
    let already: bool = env.storage().persistent().get(&dedup_key).unwrap_or(false);
    if already {
        return Err(BattlePassError::ChallengeXpAlreadyGranted);
    }

    env.storage().persistent().set(&dedup_key, &true);

    let key = BattlePassKey::State(profile_id, season.id);
    let mut state: BattlePassState =
        env.storage().persistent().get(&key).unwrap_or(BattlePassState {
            profile_id,
            season_id: season.id,
            xp: 0,
            free_rewards_claimed: 0,
            premium_rewards_claimed_lo: 0,
            premium_rewards_claimed_hi: 0,
            has_premium: false,
        });

    let gained = XP_PER_CHALLENGE_COMPLETE as u64;
    state.xp += gained;
    env.storage().persistent().set(&key, &state);

    env.events().publish(
        (symbol_short!("bp"), symbol_short!("chal_xp")),
        (profile_id, challenge_id, state.xp),
    );

    Ok(state.xp)
}

/// Grant bonus XP for discovering the season's exclusive nebula type.
pub fn add_xp_for_seasonal_nebula(
    env: &Env,
    profile_id: u64,
) -> Result<u64, BattlePassError> {
    let season = get_current_season(env).map_err(|_| BattlePassError::NoActiveSeason)?;
    let key = BattlePassKey::State(profile_id, season.id);

    let mut state: BattlePassState =
        env.storage().persistent().get(&key).unwrap_or(BattlePassState {
            profile_id,
            season_id: season.id,
            xp: 0,
            free_rewards_claimed: 0,
            premium_rewards_claimed_lo: 0,
            premium_rewards_claimed_hi: 0,
            has_premium: false,
        });

    let gained = XP_PER_SEASONAL_NEBULA as u64;
    state.xp += gained;
    env.storage().persistent().set(&key, &state);

    env.events().publish(
        (symbol_short!("bp"), symbol_short!("neb_xp")),
        (profile_id, season.id, state.xp),
    );

    Ok(state.xp)
}

// ─── Premium Pass ─────────────────────────────────────────────────────────────

/// Unlock the premium battle pass for a player for the current season.
///
/// This marks the player as a premium holder and retroactively makes all
/// already-completed premium tiers claimable.  Actual reward claiming still
/// goes through `claim_reward_v2`.
pub fn unlock_premium_pass(
    env: &Env,
    player: Address,
    profile_id: u64,
) -> Result<(), BattlePassError> {
    player.require_auth();

    let season = get_current_season(env).map_err(|_| BattlePassError::NoActiveSeason)?;
    let premium_key = BattlePassKey::PremiumUnlock(profile_id, season.id);
    env.storage().persistent().set(&premium_key, &true);

    // Also update the cached flag on the state struct for fast reads.
    let state_key = BattlePassKey::State(profile_id, season.id);
    let mut state: BattlePassState =
        env.storage().persistent().get(&state_key).unwrap_or(BattlePassState {
            profile_id,
            season_id: season.id,
            xp: 0,
            free_rewards_claimed: 0,
            premium_rewards_claimed_lo: 0,
            premium_rewards_claimed_hi: 0,
            has_premium: false,
        });
    state.has_premium = true;
    env.storage().persistent().set(&state_key, &state);

    env.events().publish(
        (symbol_short!("bp"), symbol_short!("premium")),
        (profile_id, season.id),
    );

    Ok(())
}

/// Return whether a player holds the premium pass for a specific season.
pub fn is_premium_holder(env: &Env, profile_id: u64, season_id: u64) -> bool {
    env.storage()
        .persistent()
        .get(&BattlePassKey::PremiumUnlock(profile_id, season_id))
        .unwrap_or(false)
}

// ─── Reward Claiming ──────────────────────────────────────────────────────────

/// Claim a battle pass reward at `tier` from `pass_tier` track.
///
/// Premium rewards require the player to hold the premium pass (`has_premium`).
/// Calling with `PassTier::Premium` for a non-premium player returns
/// `BattlePassError::PremiumRequired`.
///
/// Returns the `reward_amount` credited to the player.
pub fn claim_reward_v2(
    env: &Env,
    player: Address,
    profile_id: u64,
    tier: u32,
    pass_tier: PassTier,
) -> Result<i128, BattlePassError> {
    player.require_auth();

    let season = get_current_season(env).map_err(|_| BattlePassError::NoActiveSeason)?;
    let state_key = BattlePassKey::State(profile_id, season.id);

    let mut state: BattlePassState = env
        .storage()
        .persistent()
        .get(&state_key)
        .ok_or(BattlePassError::NotEnoughXP)?;

    // Validate tier range.
    let max_tier = match pass_tier {
        PassTier::Free    => MAX_FREE_TIERS,
        PassTier::Premium => MAX_PREMIUM_TIERS,
    };
    if tier == 0 || tier > max_tier {
        return Err(BattlePassError::TierOutOfRange);
    }

    // Check premium ownership.
    if pass_tier == PassTier::Premium && !state.has_premium {
        return Err(BattlePassError::PremiumRequired);
    }

    // Check claimed bitmask.
    let pass_tier_u32 = match pass_tier {
        PassTier::Free    => 0u32,
        PassTier::Premium => 1u32,
    };

    let already_claimed = match pass_tier {
        PassTier::Free => (state.free_rewards_claimed & (1 << (tier - 1))) != 0,
        PassTier::Premium => {
            if tier <= 32 {
                (state.premium_rewards_claimed_lo & (1 << (tier - 1))) != 0
            } else {
                (state.premium_rewards_claimed_hi & (1 << (tier - 33))) != 0
            }
        }
    };

    if already_claimed {
        return Err(BattlePassError::AlreadyClaimed);
    }

    // Look up the reward template.
    let reward: BattlePassReward = env
        .storage()
        .instance()
        .get(&BattlePassKey::Reward(season.id, tier, pass_tier_u32))
        .ok_or(BattlePassError::InvalidTier)?;

    // Verify XP threshold.
    if state.xp < reward.xp_required {
        return Err(BattlePassError::NotEnoughXP);
    }

    // Mark as claimed.
    match pass_tier {
        PassTier::Free => state.free_rewards_claimed |= 1 << (tier - 1),
        PassTier::Premium => {
            if tier <= 32 {
                state.premium_rewards_claimed_lo |= 1 << (tier - 1);
            } else {
                state.premium_rewards_claimed_hi |= 1 << (tier - 33);
            }
        }
    }

    env.storage().persistent().set(&state_key, &state);

    env.events().publish(
        (symbol_short!("bp"), symbol_short!("claimed")),
        (profile_id, tier, reward.reward_amount, pass_tier_u32),
    );

    Ok(reward.reward_amount)
}

// ─── Season Reward Initialization ─────────────────────────────────────────────

/// Initialize the reward table for a given season.
///
/// This replaces the old hardcoded `init_battle_pass_rewards`.  Pass a `Vec`
/// of `BattlePassReward` structs — one per (tier, pass_tier) pair you want
/// defined.  Can be called multiple times to add tiers incrementally.
///
/// Free-tier XP thresholds start at 100 XP for tier 1 and increase by 150 XP
/// each tier (100, 250, 400 … up to tier 30).
/// Premium-tier thresholds start at 200 XP and increase by 100 XP each tier.
pub fn init_season_rewards(
    env: &Env,
    admin: Address,
    season_id: u64,
    rewards: Vec<BattlePassReward>,
) -> Result<(), BattlePassError> {
    admin.require_auth();

    for i in 0..rewards.len() {
        if let Some(reward) = rewards.get(i) {
            let pass_tier_u32 = match reward.pass_tier {
                PassTier::Free    => 0u32,
                PassTier::Premium => 1u32,
            };
            env.storage()
                .instance()
                .set(&BattlePassKey::Reward(season_id, reward.tier, pass_tier_u32), &reward);
        }
    }

    env.events().publish(
        (symbol_short!("bp"), symbol_short!("init")),
        (season_id, rewards.len()),
    );

    Ok(())
}

/// Build and store the default free + premium reward tables for a new season.
///
/// Free tiers (1–30): pure essence rewards, XP thresholds starting at 100 and
/// increasing by 150 per tier.  No cosmetics.
///
/// Premium tiers (1–50): escalating essence plus cosmetic skin IDs cycling
/// through seasonal palettes (skin IDs are illustrative; wire to `skins.rs`
/// as the cosmetic catalog grows).
pub fn init_default_season_rewards(
    env: &Env,
    admin: Address,
    season_id: u64,
) -> Result<(), BattlePassError> {
    admin.require_auth();

    // ── Free tiers ─────────────────────────────────────────────────────────────
    for tier in 1u32..=MAX_FREE_TIERS {
        let xp_required = 100u64 + (tier as u64 - 1) * 150;
        let reward_amount = 50i128 + (tier as i128 - 1) * 25; // 50, 75, 100 …
        let reward = BattlePassReward {
            tier,
            xp_required,
            reward_amount,
            pass_tier: PassTier::Free,
            cosmetic_skin_id: 0, // no cosmetic
            title_badge: symbol_short!(""),
        };
        env.storage()
            .instance()
            .set(&BattlePassKey::Reward(season_id, tier, 0u32), &reward);
    }

    // ── Premium tiers ─────────────────────────────────────────────────────────
    for tier in 1u32..=MAX_PREMIUM_TIERS {
        let xp_required = 200u64 + (tier as u64 - 1) * 100;
        let reward_amount = 75i128 + (tier as i128 - 1) * 30; // 75, 105, 135 …
        // Assign a cosmetic skin at milestone tiers (every 5 tiers).
        let cosmetic_skin_id = if tier % 5 == 0 { tier } else { 0 };
        let title_badge = if tier == 50 {
            symbol_short!("grandnom") // "Grand Nomad" title at tier 50
        } else if tier == 25 {
            symbol_short!("nomad")    // "Nomad" title at tier 25
        } else {
            symbol_short!("")
        };
        let reward = BattlePassReward {
            tier,
            xp_required,
            reward_amount,
            pass_tier: PassTier::Premium,
            cosmetic_skin_id,
            title_badge,
        };
        env.storage()
            .instance()
            .set(&BattlePassKey::Reward(season_id, tier, 1u32), &reward);
    }

    env.events().publish(
        (symbol_short!("bp"), symbol_short!("def_init")),
        season_id,
    );

    Ok(())
}

// ─── Cosmetic Registry ────────────────────────────────────────────────────────

/// Register a seasonal cosmetic definition.
pub fn register_seasonal_cosmetic(
    env: &Env,
    admin: Address,
    cosmetic: SeasonalCosmetic,
) {
    admin.require_auth();
    let key = BattlePassKey::SeasonalCosmetic(cosmetic.cosmetic_id);
    env.storage().instance().set(&key, &cosmetic);
}

/// Retrieve a seasonal cosmetic by ID.
pub fn get_seasonal_cosmetic(env: &Env, cosmetic_id: u32) -> Option<SeasonalCosmetic> {
    env.storage()
        .instance()
        .get(&BattlePassKey::SeasonalCosmetic(cosmetic_id))
}

// ─── Read Queries ─────────────────────────────────────────────────────────────

/// Get a player's current battle pass state for the active season.
pub fn get_battle_pass_state(
    env: &Env,
    profile_id: u64,
) -> Result<BattlePassState, BattlePassError> {
    let season = get_current_season(env).map_err(|_| BattlePassError::NoActiveSeason)?;
    let key = BattlePassKey::State(profile_id, season.id);
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(BattlePassError::NotEnoughXP)
}

/// Get a player's battle pass state for a specific season (useful for past-season queries).
pub fn get_pass_progress(
    env: &Env,
    profile_id: u64,
    season_id: u64,
) -> Option<BattlePassState> {
    env.storage()
        .persistent()
        .get(&BattlePassKey::State(profile_id, season_id))
}

/// Legacy shim: claim a free-track reward using the original single-tier API.
/// Delegates to `claim_reward_v2` with `PassTier::Free`.
pub fn claim_reward(
    env: &Env,
    player: Address,
    profile_id: u64,
    tier: u32,
) -> Result<i128, BattlePassError> {
    claim_reward_v2(env, player, profile_id, tier, PassTier::Free)
}
