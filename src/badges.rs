//! Badge definitions, ownership, and award operations.
//!
/// # Badges Module
///
/// Handles Badge NFT minting, display metadata, and transfer logic for the
/// Stellar Nebula Nomad achievement system.
///
/// ## Overview
///
/// Every time a player unlocks an achievement, a Badge NFT is minted on-chain.
/// This module provides:
///
/// - [`BadgeMetadata`] — rich display metadata (image URI, rarity tier, etc.)
/// - [`mint_badge`] — create a new badge and attach full display metadata.
/// - [`get_badge_metadata`] / [`get_badge`] — read badge data.
/// - [`transfer_badge`] — transfer an unlockable / transferable badge.
/// - [`list_badges_by_owner`] — enumerate all badge IDs held by a player.
///
/// ## Badge display in the UI
///
/// The UI should read [`BadgeMetadata`] to render the badge card.  The
/// `image_uri` field should point to an IPFS CID or an on-chain SVG.  The
/// `rarity_tier` field controls the card border colour:
///
/// | tier | colour  |
/// |------|---------|
/// | 0    | grey    |
/// | 1    | green   |
/// | 2    | blue    |
/// | 3    | purple  |
/// | 4    | gold    |
///
/// ## NFT standard
///
/// Badges follow the SEP-0041 token interface pattern on Stellar.  Each badge
/// has a unique `badge_id` (u64 auto-increment) and is tracked in persistent
/// storage.
use soroban_sdk::{contracttype, contracterror, symbol_short, Address, Bytes, Env, String, Vec};

use crate::achievement_engine::{AchievementBadge, AchievementKey};

// ─── Storage Keys ─────────────────────────────────────────────────────────────

/// Storage key namespace for badge-related data.
#[derive(Clone)]
#[contracttype]
pub enum BadgeKey {
    /// Full display metadata for a badge: `BadgeKey::Metadata(badge_id)`.
    Metadata(u64),
    /// List of badge IDs owned by a player: `BadgeKey::OwnerBadges(address)`.
    OwnerBadges(Address),
}

// ─── Data Types ───────────────────────────────────────────────────────────────

/// Rarity tier for badge display.
///
/// Stored as `u32` to keep the on-chain footprint small.
///
/// | Value | Name      | Description                              |
/// |-------|-----------|------------------------------------------|
/// | 0     | Common    | Standard milestone                       |
/// | 1     | Uncommon  | Notable milestone (≥ 5 ships, ≥ 50 scans)|
/// | 2     | Rare      | High milestone (≥ 10 ships, ≥ 100 scans) |
/// | 3     | Epic      | Very high milestone (≥ 200 scans)        |
/// | 4     | Legendary | Combined / maxed achievement             |
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
#[repr(u32)]
pub enum RarityTier {
    Common    = 0,
    Uncommon  = 1,
    Rare      = 2,
    Epic      = 3,
    Legendary = 4,
}

/// Full display metadata attached to a minted badge NFT.
///
/// This data is stored on-chain so that any UI or indexer can render the badge
/// without relying on off-chain infrastructure.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct BadgeMetadata {
    /// The unique badge NFT identifier.
    pub badge_id: u64,
    /// The achievement this badge represents.
    pub achievement_id: u64,
    /// Short display title (copied from the achievement template).
    pub title: String,
    /// Longer description for the badge card.
    pub description: String,
    /// IPFS CID or on-chain SVG URI for the badge image.
    pub image_uri: String,
    /// Rarity tier controlling the card border colour (see module docs).
    pub rarity_tier: u32,
    /// On-chain timestamp (ledger sequence) when the badge was minted.
    pub minted_at: u64,
    /// Owner address at the time of minting.
    pub original_owner: Address,
    /// Whether this badge can be transferred to another player.
    pub transferable: bool,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum BadgeError {
    /// Badge ID not found in storage.
    BadgeNotFound = 1,
    /// Caller is not the current owner of the badge.
    NotOwner = 2,
    /// This badge is soul-bound and cannot be transferred.
    NonTransferable = 3,
    /// Cannot transfer to the current owner.
    SameOwner = 4,
}

// ─── Mint ─────────────────────────────────────────────────────────────────────

/// Mint a badge NFT with full display metadata.
///
/// Reads the base [`AchievementBadge`] record that the engine created, derives
/// a [`RarityTier`] from the achievement ID, and writes [`BadgeMetadata`] to
/// persistent storage.
///
/// Also appends `badge_id` to the owner's badge list.
///
/// # Parameters
///
/// - `badge_id` — the ID returned by the achievement engine when the badge was minted.
/// - `description` — full text shown on the badge card.
/// - `image_uri` — IPFS CID (e.g. `ipfs://Qm…`) or on-chain SVG data URI.
pub fn mint_badge(
    env: &Env,
    badge_id: u64,
    description: String,
    image_uri: String,
) -> Result<BadgeMetadata, BadgeError> {
    // Load the base badge written by the engine.
    let engine_badge: AchievementBadge = env
        .storage()
        .persistent()
        .get(&AchievementKey::Badge(badge_id))
        .ok_or(BadgeError::BadgeNotFound)?;

    let rarity = rarity_for_achievement(engine_badge.achievement_id) as u32;

    let metadata = BadgeMetadata {
        badge_id,
        achievement_id: engine_badge.achievement_id,
        title: engine_badge.title.clone(),
        description,
        image_uri,
        rarity_tier: rarity,
        minted_at: engine_badge.minted_at,
        original_owner: engine_badge.owner.clone(),
        transferable: engine_badge.transferable,
    };

    env.storage()
        .persistent()
        .set(&BadgeKey::Metadata(badge_id), &metadata);

    append_owner_badge(env, &engine_badge.owner, badge_id);

    env.events().publish(
        (symbol_short!("badge_mnt"), engine_badge.owner.clone()),
        (badge_id, engine_badge.achievement_id, rarity),
    );

    Ok(metadata)
}

// ─── Reads ────────────────────────────────────────────────────────────────────

/// Return the full display metadata for a badge.
///
/// # Errors
///
/// [`BadgeError::BadgeNotFound`] if the badge metadata has not been written yet
/// (i.e. [`mint_badge`] has not been called for this `badge_id`).
pub fn get_badge_metadata(env: &Env, badge_id: u64) -> Result<BadgeMetadata, BadgeError> {
    env.storage()
        .persistent()
        .get(&BadgeKey::Metadata(badge_id))
        .ok_or(BadgeError::BadgeNotFound)
}

/// Return the engine-level badge record.
///
/// # Errors
///
/// [`BadgeError::BadgeNotFound`] if the badge does not exist.
pub fn get_badge(env: &Env, badge_id: u64) -> Result<AchievementBadge, BadgeError> {
    env.storage()
        .persistent()
        .get(&AchievementKey::Badge(badge_id))
        .ok_or(BadgeError::BadgeNotFound)
}

/// Return all badge IDs owned by `player` (from the badge module's index).
pub fn list_badges_by_owner(env: &Env, player: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&BadgeKey::OwnerBadges(player.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

// ─── Transfer ─────────────────────────────────────────────────────────────────

/// Transfer a transferable badge from `from` to `to`.
///
/// The caller must be `from` (enforced via `require_auth`).
///
/// # Errors
///
/// - [`BadgeError::BadgeNotFound`] — badge does not exist.
/// - [`BadgeError::NotOwner`] — caller is not the badge owner.
/// - [`BadgeError::NonTransferable`] — badge is soul-bound.
/// - [`BadgeError::SameOwner`] — `from` and `to` are the same address.
pub fn transfer_badge(
    env: &Env,
    from: Address,
    to: Address,
    badge_id: u64,
) -> Result<(), BadgeError> {
    from.require_auth();

    if from == to {
        return Err(BadgeError::SameOwner);
    }

    let engine_badge: AchievementBadge = env
        .storage()
        .persistent()
        .get(&AchievementKey::Badge(badge_id))
        .ok_or(BadgeError::BadgeNotFound)?;

    if engine_badge.owner != from {
        return Err(BadgeError::NotOwner);
    }

    if !engine_badge.transferable {
        return Err(BadgeError::NonTransferable);
    }

    // Update engine badge owner.
    let updated_engine = AchievementBadge {
        owner: to.clone(),
        ..engine_badge
    };
    env.storage()
        .persistent()
        .set(&AchievementKey::Badge(badge_id), &updated_engine);

    // Update metadata original_owner field (keep original_owner as-is, just
    // update the live display).
    if let Some(meta) = env
        .storage()
        .persistent()
        .get::<BadgeKey, BadgeMetadata>(&BadgeKey::Metadata(badge_id))
    {
        // original_owner intentionally preserved for provenance.
        env.storage()
            .persistent()
            .set(&BadgeKey::Metadata(badge_id), &meta);
    }

    // Update owner badge lists.
    remove_owner_badge(env, &from, badge_id);
    append_owner_badge(env, &to, badge_id);

    env.events().publish(
        (symbol_short!("badge_xfr"), from.clone()),
        (badge_id, to.clone()),
    );

    Ok(())
}

// ─── Internal Helpers ─────────────────────────────────────────────────────────

fn append_owner_badge(env: &Env, owner: &Address, badge_id: u64) {
    let key = BadgeKey::OwnerBadges(owner.clone());
    let mut ids: Vec<u64> = env.storage().persistent().get(&key).unwrap_or_else(|| Vec::new(env));
    ids.push_back(badge_id);
    env.storage().persistent().set(&key, &ids);
}

fn remove_owner_badge(env: &Env, owner: &Address, badge_id: u64) {
    let key = BadgeKey::OwnerBadges(owner.clone());
    let ids: Vec<u64> = env.storage().persistent().get(&key).unwrap_or_else(|| Vec::new(env));
    let mut updated: Vec<u64> = Vec::new(env);
    let mut i = 0u32;
    while i < ids.len() {
        if let Some(id) = ids.get(i) {
            if id != badge_id {
                updated.push_back(id);
            }
        }
        i += 1;
    }
    env.storage().persistent().set(&key, &updated);
}

/// Derive a [`RarityTier`] from an achievement ID.
///
/// | Tier      | Achievement IDs                     |
/// |-----------|-------------------------------------|
/// | Legendary | 20 (Legend)                         |
/// | Epic      | 18 (Armada), 19 (Trailblazer)       |
/// | Rare      | 13 (FleetTen), 16 (CosmicReach), 17 |
/// | Uncommon  | 5, 9, 12, 14, 15                    |
/// | Common    | everything else                     |
fn rarity_for_achievement(id: u64) -> RarityTier {
    match id {
        20 => RarityTier::Legendary,
        18 | 19 => RarityTier::Epic,
        13 | 16 | 17 => RarityTier::Rare,
        5 | 9 | 12 | 14 | 15 => RarityTier::Uncommon,
        _ => RarityTier::Common,
    }
}
