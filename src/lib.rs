#![no_std]

use soroban_sdk::{contract, contractimpl, Address, Bytes, BytesN, Env, String, Symbol, Vec, symbol_short};

use crate::nebula_explorer::{NebulaLayout, Rarity};

mod access_control;
mod analytics;
mod blueprint_factory;
mod gifting_system;
mod nebula_explorer;
mod player_profile;
mod referral_system;
pub mod resource_minter;
mod session_manager;
mod ship_nft;
mod ship_registry;
mod onboarding_tutorial;
mod leaderboards;
mod content_tools;
mod pvp_combat;

pub mod events;
mod batch_processor;
mod dex_integration;
mod difficulty_scaler;
mod difficulty_curve;
mod health_monitor;
mod achievement_engine;
mod data_exporter;
mod emergency_controls;
mod metadata_resolver;
mod randomness_oracle;
mod rate_limiter;
pub mod nebula_gen;
pub mod ship_upgrade;
#[cfg(any(test, feature = "fuzz"))]
pub mod test_helpers;
mod treasure_vault;

mod yield_farming;
mod governance;
mod theme_customizer;
mod indexer_callbacks;

mod contract_versioning;
mod gas_recovery;
mod bounty_board;
mod recycling_crafter;

mod energy_manager;
mod environment_simulator;
mod mission_generator;
mod escrow_trader;
mod audit_logger;
mod sustainability_metrics;
mod anomaly_classifier;
mod shared_lib;
mod fractional_resources;
mod yield_forecast;

mod gas_sponsor;

mod storage_optim;
mod state_snapshot;
mod metrics_exporter;
mod migration_framework;
mod cache_ttl_manager;

mod prize_distributor;
mod portal_registry;
mod constellation_mapper;
mod entanglement_comms;
mod wormhole_traveler;
mod alliance_manager;
mod market_oracle;
mod audio_seed_generator;
mod privacy_stats;
mod navigation_planner;
mod event_scheduler;

mod rewards;
mod nft_marketplace;
mod trading;
mod seasons;
mod battle_pass;

mod ship_customization;
mod skins;
mod crafting;
pub mod recipes;
pub mod notifications {
    pub mod push_service;
    pub mod alerts;
}

pub mod mobile_views;

#[path = "../integrations/mod.rs"]
pub mod integrations;

mod economics;

// Gas optimization modules
mod gas_optimized_storage;
mod gas_optimized_compute;

// Cross-contract safety: reentrancy guard + composable call helpers.
mod nomad_bonding;
mod reentrancy_guard;
mod composability_examples;
mod reputation;

pub use reentrancy_guard::ReentrancyError;
pub use reputation::{
    ReputationScore, BehaviorRecord, DisputeReport, BehaviorType, ReportStatus,
    ReputationError,
};

pub use analytics::{AnalyticsError, GlobalStats};
pub use access_control::{AccessControlError};
pub use nebula_gen::{
    NebulaError as NebulaGenError, Anomaly, AnomalyType, NebulaLayout as NebulaGenLayout,
    ResourceClass,
};
pub use resource_minter::{AssetId, MinterError, ResourceKey, ResourceRecord, ResourceType};
pub use ship_nft::{ShipError, ShipNft};
pub use blueprint_factory::{Blueprint, BlueprintError, BlueprintRarity};
pub use referral_system::{Referral, ReferralError};
pub use player_profile::{PlayerProfile, ProfileError, ProgressUpdate};
pub use session_manager::{Session, SessionError};
pub use ship_registry::Ship;
pub use onboarding_tutorial::{
    TutorialProgress, PlayerProfile as OnboardingPlayerProfile,
    OnboardingError, TOTAL_STEPS, STEP_REWARDS,
};
pub use leaderboards::{
    LeaderboardEntry, LeaderboardEntry as EnhancedLeaderboardEntry, GuildEntry, RegionalEntry, AchievementEntry,
    LeaderboardRewards, LeaderboardError,
    CATEGORY_ESSENCE, CATEGORY_SCANS, CATEGORY_MISSIONS,
    CATEGORY_NEBULAE_EXPLORED, CATEGORY_SHIPS_MINTED, CATEGORY_TRADES,
    CATEGORY_CRAFTS, CATEGORY_BOUNTIES, CATEGORY_PVP_WINS,
    CATEGORY_PVP_RATING, CATEGORY_GUILD_CONTRIBUTION, CATEGORY_ACHIEVEMENTS,
    PERIOD_DAILY, PERIOD_WEEKLY, PERIOD_MONTHLY, PERIOD_ALL_TIME,
    MAX_LEADERBOARD_ENTRIES,
};
pub use content_tools::{
    CreatedContent, ContentMetadata, MarketplaceListing, VoteResult,
    ContentToolsError, RevenueSplitConfig, PurchaseResult,
    CONTENT_TYPE_NEBULA, CONTENT_TYPE_MISSION, CONTENT_TYPE_EVENT,
    MAX_CONTENT_PER_CREATOR, MAX_MARKETPLACE_LISTINGS,
    DEFAULT_CREATOR_SHARE_BPS, DEFAULT_PLATFORM_SHARE_BPS, MAX_CONTENT_PRICE,
};
pub use pvp_combat::{
    CombatStats, Challenge, CombatState, CombatMove, CombatHistory,
    MatchmakingEntry, RewardsConfig, SpectatorInfo, EloDecayConfig,
    PvPError,
    INITIAL_ELO, K_FACTOR, MAX_HP, MAX_ENERGY, COMBAT_TIMEOUT,
    MAX_SPECTATORS, MAX_QUEUE_SIZE,
    ELO_DECAY_INACTIVITY_SECS, ELO_DECAY_BASE_POINTS, ELO_DECAY_FLOOR,
};

pub use batch_processor::{
    clear_batch, execute_batch, get_player_batch, queue_batch_operation, BatchError, BatchOp,
    BatchOpType, BatchResult, MAX_BATCH_SIZE,
};
pub use dex_integration::{cancel_listing, harvest_and_list};
pub use difficulty_scaler::{
    apply_scaling_to_layout, calculate_difficulty, DifficultyError, DifficultyResult,
    RarityWeights, MAX_LEVEL,
};
pub use emergency_controls::{
    EmergencyError, execute_unpause, get_admins, initialize_admins, is_paused,
    pause_contract, require_not_paused, schedule_unpause, emergency_withdraw, UNPAUSE_DELAY,
};
pub use metadata_resolver::{
    batch_resolve_metadata, get_current_gateway, resolve_metadata, set_gateway, set_metadata_uri,
    MetadataError, TokenMetadata, MAX_METADATA_BATCH,
};
pub use randomness_oracle::{
    get_entropy_pool, request_random_seed, verify_and_fallback, OracleError,
};
pub use ship_upgrade::{ShipState, ShipUpgradeError, UpgradeBlueprint, apply_regen_upgrade};
pub use treasure_vault::{
    claim_treasure, deposit_treasure, get_vault, TreasureVault, VaultError,
    DEFAULT_MIN_LOCK_DURATION,
};
pub use gifting_system::{Gift, GiftError};
pub use contract_versioning::{
    initialize_version, get_version, check_compatibility, set_auto_migrate,
    migrate_data, is_auto_migrate_enabled, get_migration_record,
    CURRENT_VERSION, MIGRATION_BATCH_SIZE, VersioningError, MigrationRecord,
};
pub use gas_recovery::{
    initialize_refund, set_refund_percentage, request_refund,
    verify_refund_eligibility, process_refund_batch, get_refund_request,
    DEFAULT_REFUND_BPS, REFUND_BATCH_SIZE, RefundError, RefundRequest,
};
pub use bounty_board::{
    initialize_bounty_board, set_bounty_expiry, post_bounty, claim_bounty,
    get_bounty, DEFAULT_BOUNTY_EXPIRY, MAX_ACTIVE_BOUNTIES, BountyError, Bounty,
};
pub use recycling_crafter::{
    initialize_recycling, recycle_resource, craft_new_item, get_recipe,
    RECYCLE_CRAFT_BATCH_SIZE, RecyclingError, Recipe, CraftingResult,
};

pub use energy_manager::{
    consume_energy, get_energy_balance, recharge_energy,
    apply_passive_regen, set_regen_rate, get_regen_rate,
    EnergyBalance, EnergyError, RechargeResult, PassiveRegenResult,
};
pub use environment_simulator::{
    apply_environmental_modifier, get_nebula_condition, simulate_conditions, EnvironmentCondition,
    EnvironmentError, ModifierResult,
};
pub use mission_generator::{
    complete_mission, generate_daily_mission, get_player_missions, update_mission_progress,
    Mission, MissionError, MissionReward,
};
pub use escrow_trader::{
    cancel_escrow, complete_escrow, confirm_escrow, get_escrow, initiate_escrow, Escrow,
    EscrowError, EscrowResult, TradeAsset,
};
pub use audit_logger::{AuditEntry, AuditLoggerError, MAX_QUERY_LIMIT, get_audit_count, log_audit_event, query_audit_logs};
pub use sustainability_metrics::{claim_sustainability_reward, get_footprint, record_transaction_footprint, FootprintRecord, SustainabilityError};
pub use anomaly_classifier::{classify_anomaly, classify_batch, get_classification, refine_classification, AnomalyError, ClassificationRecord};
pub use shared_lib::{calculate_yield, validate_address, SharedError};
pub use gas_sponsor::{
    initialize as initialize_sponsorship, sponsor_first_scan, claim_sponsorship_fund,
    has_been_sponsored, get_fund_balance, get_daily_count, get_remaining_daily_slots,
    get_admin, get_config, update_config, mark_profile_verified,
    MAX_DAILY_SPONSORSHIPS, SponsorConfig, SponsorError,
};

pub use fractional_resources::{
    initialize as initialize_fractional, fractionalize_resource, merge_fractions,
    transfer_share, get_share, get_owner_shares, get_total_shares,
    get_original_resource, is_share_owner, update_config as update_fractional_config,
    FractionalShare, OriginalResource, FractionalConfig,
    FractionalError, MAX_FRACTIONS_PER_TX, MIN_SHARE_SIZE,
};
pub use yield_forecast::{
    initialize as initialize_forecast, generate_yield_forecast, update_forecast_model,
    batch_generate_forecasts, get_cached_forecast, get_player_history, get_history_count,
    get_model_params, get_model_version, update_model_params, clear_stale_forecasts,
    YieldDataPoint, YieldForecast, ModelParams, ForecastError,
    MAX_HISTORY_POINTS, MAX_FORECAST_DAYS, MAX_FORECAST_BURST,
};

pub use storage_optim::{
    store_with_bump, get_optimized_entry, batch_store_with_bump, guard_reentrancy,
    release_guard, store_ship_nebula, get_ship_nebula, initialize_bump_config,
    update_bump_config, get_bump_config, set_upgrade_target, get_upgrade_target,
    reset_burst_counter, StorageError, OptimizedEntry, ShipNebulaData, OptimResult,
    BumpConfig, DEFAULT_BUMP_TTL, MAX_BUMP_TTL, MAX_BURST_READS,
};
pub use state_snapshot::{
    take_snapshot, restore_from_snapshot, get_snapshot, get_ship_snapshots,
    auto_snapshot, reset_session_count, StateSnapshot, SnapshotError,
    RestoreResult, MAX_SNAPSHOTS_PER_SESSION, SNAPSHOT_TTL, AUTO_SNAPSHOT_INTERVAL,
};
pub use prize_distributor::{
    initialize_prize_distributor, fund_prize_pool, submit_leaderboard_snapshot,
    distribute_weekly_prizes, get_prize_pool, get_total_distributed, get_last_reset,
    PrizeError, PrizeRecord, WEEK_SECONDS, MAX_PAYOUT_POSITIONS,
};
pub use portal_registry::{
    initialize_portal_registry, register_portal, register_portal_batch, query_portal_status,
    refresh_portal, travel_through_portal, get_portal,
    PortalError, Portal, MAX_PORTALS_PER_TX, MIN_STABLE_PCT, BASE_TRAVEL_COST,
};
pub use constellation_mapper::{
    record_constellation, match_constellation, match_constellations_batch,
    get_constellation, get_constellation_count,
    ConstellationError, Constellation, MatchResult, MIN_STARS, MAX_MATCH_BURST,
};
pub use entanglement_comms::{
    create_entanglement_pair, send_entangled_message, send_entangled_message_batch,
    dissolve_pair, get_entanglement_pair, get_message_count,
    EntanglementError, EntanglementPair, EntangledMessage,
    PAIR_LIFETIME_SECS, MAX_MESSAGE_BURST,
};
pub use wormhole_traveler::{
    open_wormhole, traverse_wormhole, get_wormhole, get_active_wormholes,
    get_travel_history, cleanup_expired_wormholes, calculate_travel_cost,
    verify_wormhole_link, Wormhole, TravelRecord, WormholeError,
    MAX_SIMULTANEOUS_WORMHOLES, WORMHOLE_LIFETIME_SECS,
};
pub use alliance_manager::{
    found_alliance, join_alliance, leave_alliance, contribute_to_treasury,
    get_alliance, get_alliance_treasury, get_member_contribution, get_player_alliance,
    Alliance, MembershipRecord, AllianceError, MAX_MEMBERS_PER_ALLIANCE,
};
pub use market_oracle::{
    initialize_oracle, update_resource_price, batch_update_prices,
    get_current_market_rate, get_price_data, get_price_history, add_oracle_source,
    PriceData, OracleError as MarketOracleError, MAX_BATCH_UPDATE,
};
pub use audio_seed_generator::{
    initialize_presets, generate_music_seed, get_instrument_layer, get_all_layers,
    get_nebula_seed, get_preset, MusicSeed, InstrumentParams, AudioError,
    INSTRUMENT_PRESETS, MAX_LAYERS_PER_NEBULA,
};
pub use privacy_stats::{
    opt_in_privacy, commit_private_stat, verify_private_stat, get_commitment,
    get_commitment_count, batch_commit_stats, is_opted_in, reset_burst_counter as reset_privacy_burst,
    StatCommitment, PrivacyError, MAX_COMMITMENTS_PER_TX,
};
pub use navigation_planner::{
    initialize_nav_graph, add_nebula_connection, add_nebula_connections_batch,
    calculate_optimal_route, validate_route_safety, get_neighbors, get_connection,
    NavError, NavPath, RouteEdge, NavConfig, MAX_ROUTE_HOPS, MAX_CONNECTIONS_PER_BATCH,
};
pub use event_scheduler::{
    initialize_scheduler, schedule_event, trigger_scheduled_event, get_event,
    get_active_events, schedule_weekly_festival, cancel_event, update_participants,
    get_event_count, reset_burst_counter as reset_event_burst,
    ScheduledEvent, EventResult, EventError, MAX_ACTIVE_EVENTS, WEEKLY_FESTIVAL_INTERVAL,
};

pub use ship_customization::{
    mint_skin, apply_skin, get_ship_skin, get_owner_skins, transfer_skin,
    ShipSkin, SkinRarity, SkinError,
};
pub use skins::{get_skin_templates, SkinTemplate};

pub use economics::monitor::{
    initialize_monitor, update_supply_metrics, track_resource_activity,
    get_metrics, get_resource_metrics, calculate_inflation_rate,
    EconomicMetrics, ResourceMetrics,
};
pub use economics::balancer::{
    detect_imbalance, suggest_adjustment, apply_adjustment, generate_report,
    BalanceAdjustment, SupplyDemandRatio,
};

pub use trading::{
    place_limit_order, cancel_limit_order, get_limit_order, get_trader_orders,
    record_trade, get_trading_history, LimitOrder, OrderSide, TradeRecord, TradingError,
    // AMM (Issue #189)
    create_pool, add_liquidity, remove_liquidity, swap_exact_input,
    get_pool, get_lp_balance, get_all_pools, quote_swap,
    LiquidityPool, LiquidityProvider, AmmError,
    MAX_SLIPPAGE_BPS, AMM_MAX_ROUTE_HOPS, SWAP_FEE_BPS,
};

pub use crafting::{craft, add_xp, get_level, get_xp};
pub use recipes::{RecipeError, set_recipe, unlock_rare_recipe};
pub use nomad_bonding::{
    BondError, BondStatus, NomadBond, YieldDelegation,
    create_bond, accept_bond, delegate_yield, claim_yield, dissolve_bond,
    accrue_essence, get_bond, get_yield_delegation, get_essence_balance,
};
pub use notifications::push_service::{Notification, emit_notification};
pub use notifications::alerts::{check_low_resources, notify_rare_discovery, notify_crafting_complete};
pub use mobile_views::{
    MobileDashboard, MobileBatchInfo, MobileViewError, QuickScanPreview,
};

#[contract]
pub struct NebulaNomadContract;

#[contractimpl]
impl NebulaNomadContract {
    /// Generate a 16x16 procedural nebula map using ledger-seeded PRNG.
    pub fn generate_nebula_layout(env: Env, seed: BytesN<32>, player: Address) -> NebulaLayout {
        player.require_auth();
        nebula_explorer::generate_nebula_layout(&env, &seed, &player)
    }

    /// Calculate the rarity tier of a nebula layout.
    pub fn calculate_rarity_tier(env: Env, layout: NebulaLayout) -> Rarity {
        nebula_explorer::calculate_rarity_tier(&env, &layout)
    }

    /// Full scan: generates layout, calculates rarity, and emits a
    /// `NebulaScanned` event containing the layout hash.
    ///
    /// Full scan: generates layout, calculates rarity, emits NebulaScanned event.
    /// Also updates the on-chain analytics counters (total_scans,
    /// total_essence_accrued) and registers the player for the leaderboard.
    pub fn scan_nebula(env: Env, seed: BytesN<32>, player: Address) -> (NebulaLayout, Rarity) {
        player.require_auth();
        let layout = nebula_explorer::generate_nebula_layout(&env, &seed, &player);
        let rarity = nebula_explorer::calculate_rarity_tier(&env, &layout);
        let layout_hash = nebula_explorer::compute_layout_hash(&env, &layout);
        nebula_explorer::emit_nebula_scanned(&env, &player, &layout_hash, &rarity);

        // Record analytics: use total_energy as the essence earned this scan.
        analytics::record_scan(&env, &player, layout.total_energy as u64);

        (layout, rarity)
    }

    /// Return aggregate global statistics (total scans, ships minted, etc.).
    ///
    /// Pure view — no ledger writes, zero gas cost beyond the read.
    pub fn get_global_stats(env: Env) -> GlobalStats {
        analytics::get_global_stats(&env)
    }

    /// Return the top-`top_n` explorers sorted by cumulative cosmic essence.
    ///
    /// Emits a `LeaderboardSnapshot` event so frontends can subscribe via
    /// Stellar event streams.  Returns `Err(InvalidTopN)` when `top_n` is 0
    /// or exceeds 50.
    pub fn snapshot_leaderboard(
        env: Env,
        top_n: u32,
    ) -> Result<Vec<analytics::LeaderboardEntry>, AnalyticsError> {
        analytics::snapshot_leaderboard(&env, top_n)
    }

    // === Enhanced Leaderboards API (Issue #159) ===

    /// Update a player's score in a specific leaderboard category and time period.
    pub fn update_leaderboard_score(
        env: Env,
        player: Address,
        category: Symbol,
        time_period: Symbol,
        score: u64,
    ) -> Result<(), LeaderboardError> {
        leaderboards::update_score(&env, &player, category, time_period, score)
    }

    /// Get leaderboard entries for a category and time period.
    pub fn get_leaderboard(
        env: Env,
        category: Symbol,
        time_period: Symbol,
        limit: u32,
    ) -> Result<Vec<EnhancedLeaderboardEntry>, LeaderboardError> {
        leaderboards::get_leaderboard(&env, category, time_period, limit)
    }

    /// Set a player's guild affiliation.
    pub fn set_player_guild(
        env: Env,
        player: Address,
        guild_name: String,
    ) -> Result<(), LeaderboardError> {
        leaderboards::set_player_guild(&env, &player, guild_name)
    }

    /// Get a player's guild.
    pub fn get_player_guild(env: Env, player: Address) -> Option<String> {
        leaderboards::get_player_guild(&env, &player)
    }

    /// Update guild leaderboard score (admin only).
    pub fn update_guild_score(
        env: Env,
        caller: Address,
        guild_name: String,
        score: u64,
        member_count: u32,
    ) -> Result<(), LeaderboardError> {
        leaderboards::update_guild_score(&env, &caller, guild_name, score, member_count)
    }

    /// Get guild leaderboard.
    pub fn get_guild_leaderboard(
        env: Env,
        limit: u32,
    ) -> Vec<leaderboards::GuildEntry> {
        leaderboards::get_guild_leaderboard(&env, limit)
    }

    /// Update regional leaderboard score.
    pub fn update_regional_score(
        env: Env,
        player: Address,
        region: Symbol,
        score: u64,
    ) -> Result<(), LeaderboardError> {
        leaderboards::update_regional_score(&env, &player, region, score)
    }

    /// Get regional leaderboard.
    pub fn get_regional_leaderboard(
        env: Env,
        region: Symbol,
        limit: u32,
    ) -> Result<Vec<leaderboards::RegionalEntry>, LeaderboardError> {
        leaderboards::get_regional_leaderboard(&env, region, limit)
    }

    /// Update achievement leaderboard.
    pub fn update_achievement_score(
        env: Env,
        player: Address,
        achievement_count: u32,
        points: u64,
    ) -> Result<(), LeaderboardError> {
        leaderboards::update_achievement_score(&env, &player, achievement_count, points)
    }

    /// Get achievement leaderboard.
    pub fn get_achievement_leaderboard(
        env: Env,
        limit: u32,
    ) -> Vec<leaderboards::AchievementEntry> {
        leaderboards::get_achievement_leaderboard(&env, limit)
    }

    /// Distribute rewards to top leaderboard players (admin only).
    pub fn distribute_leaderboard_rewards(
        env: Env,
        caller: Address,
        category: Symbol,
        time_period: Symbol,
        rewards: LeaderboardRewards,
    ) -> Result<(), LeaderboardError> {
        leaderboards::distribute_rewards(&env, &caller, category, time_period, rewards)
    }

    /// Set leaderboard admin (admin only).
    pub fn set_leaderboard_admin(env: Env, admin: Address) {
        leaderboards::set_admin(&env, &admin)
    }

    // === Content Creation Tools API (Issue #158) ===

    /// Create new community content (nebula, mission, or event).
    pub fn create_content(
        env: Env,
        creator: Address,
        name: String,
        description: String,
        content_type: Symbol,
        data: Bytes,
        is_public: bool,
        tags: Vec<Symbol>,
    ) -> Result<u64, ContentToolsError> {
        content_tools::create_content(&env, &creator, name, description, content_type, data, is_public, tags)
    }

    /// Update existing content.
    pub fn update_content(
        env: Env,
        creator: Address,
        content_id: u64,
        name: Option<String>,
        description: Option<String>,
        data: Option<Bytes>,
        is_public: Option<bool>,
        tags: Option<Vec<Symbol>>,
    ) -> Result<(), ContentToolsError> {
        content_tools::update_content(&env, &creator, content_id, name, description, data, is_public, tags)
    }

    /// Get content by ID.
    pub fn get_content(env: Env, content_id: u64) -> Result<CreatedContent, ContentToolsError> {
        content_tools::get_content(&env, content_id)
    }

    /// Get all content IDs created by an address.
    pub fn get_creator_content(env: Env, creator: Address) -> Vec<u64> {
        content_tools::get_creator_content(&env, &creator)
    }

    /// Delete content (creator only).
    pub fn delete_content(
        env: Env,
        creator: Address,
        content_id: u64,
    ) -> Result<(), ContentToolsError> {
        content_tools::delete_content(&env, &creator, content_id)
    }

    /// Approve content (admin only).
    pub fn approve_content(
        env: Env,
        caller: Address,
        content_id: u64,
    ) -> Result<(), ContentToolsError> {
        content_tools::approve_content(&env, &caller, content_id)
    }

    /// Reject content (admin only).
    pub fn reject_content(
        env: Env,
        caller: Address,
        content_id: u64,
    ) -> Result<(), ContentToolsError> {
        content_tools::reject_content(&env, &caller, content_id)
    }

    /// Get content status.
    pub fn get_content_status(env: Env, content_id: u64) -> Symbol {
        content_tools::get_content_status(&env, content_id)
    }

    /// Vote/rate content (1-5 stars).
    pub fn vote_content(
        env: Env,
        voter: Address,
        content_id: u64,
        rating: u32,
    ) -> Result<(), ContentToolsError> {
        content_tools::vote_content(&env, &voter, content_id, rating)
    }

    /// Get vote result for content.
    pub fn get_vote_result(env: Env, content_id: u64) -> VoteResult {
        content_tools::get_vote_result(&env, content_id)
    }

    /// Increment play count for content.
    pub fn increment_play_count(
        env: Env,
        content_id: u64,
    ) -> Result<(), ContentToolsError> {
        content_tools::increment_play_count(&env, content_id)
    }

    /// List content on marketplace.
    pub fn list_on_marketplace(
        env: Env,
        seller: Address,
        content_id: u64,
        price: i128,
    ) -> Result<u64, ContentToolsError> {
        content_tools::list_on_marketplace(&env, &seller, content_id, price)
    }

    /// Unlist content from marketplace.
    pub fn unlist_from_marketplace(
        env: Env,
        seller: Address,
        listing_id: u64,
    ) -> Result<(), ContentToolsError> {
        content_tools::unlist_from_marketplace(&env, &seller, listing_id)
    }

    /// Get marketplace listing.
    pub fn get_marketplace_listing(
        env: Env,
        listing_id: u64,
    ) -> Result<MarketplaceListing, ContentToolsError> {
        content_tools::get_marketplace_listing(&env, listing_id)
    }

    /// Set content tools admin (admin only).
    pub fn set_content_admin(env: Env, admin: Address) {
        content_tools::set_admin(&env, &admin)
    }

    // ─── Content Revenue Sharing (Issue #192) ────────────────────────────────

    /// Set the price of a content item (creator only).
    pub fn set_content_price(
        env: Env,
        creator: Address,
        content_id: u64,
        price: i128,
    ) -> Result<(), ContentToolsError> {
        content_tools::set_content_price(&env, &creator, content_id, price)
    }

    /// Purchase content. Revenue split between creator and platform.
    pub fn purchase_content(
        env: Env,
        buyer: Address,
        content_id: u64,
    ) -> Result<PurchaseResult, ContentToolsError> {
        content_tools::purchase_content(&env, &buyer, content_id)
    }

    /// Get the revenue split configuration.
    pub fn get_revenue_split_config(env: Env) -> RevenueSplitConfig {
        content_tools::get_revenue_split_config(&env)
    }

    /// Set the revenue split configuration (admin only).
    pub fn set_revenue_split_config(
        env: Env,
        caller: Address,
        config: RevenueSplitConfig,
    ) -> Result<(), ContentToolsError> {
        content_tools::set_revenue_split_config(&env, &caller, config)
    }

    /// Get accumulated revenue for a creator.
    pub fn get_creator_revenue(env: Env, creator: Address) -> i128 {
        content_tools::get_creator_revenue(&env, &creator)
    }

    /// Get accumulated platform revenue.
    pub fn get_platform_revenue(env: Env) -> i128 {
        content_tools::get_platform_revenue(&env)
    }

    /// Withdraw creator revenue (creator only).
    pub fn withdraw_creator_revenue(
        env: Env,
        creator: Address,
        amount: i128,
    ) -> Result<i128, ContentToolsError> {
        content_tools::withdraw_creator_revenue(&env, &creator, amount)
    }

    // === PvP Combat API (Issue #152) ===

    /// Get combat stats for a player.
    pub fn get_combat_stats(env: Env, player: Address) -> pvp_combat::CombatStats {
        pvp_combat::get_combat_stats(&env, &player)
    }

    /// Get ELO rating for a player.
    pub fn get_elo_rating(env: Env, player: Address) -> u32 {
        pvp_combat::get_elo_rating(&env, &player)
    }

    /// Create a PvP challenge.
    pub fn create_challenge(
        env: Env,
        challenger: Address,
        opponent: Address,
        stake: i128,
    ) -> Result<u64, PvPError> {
        pvp_combat::create_challenge(&env, &challenger, &opponent, stake)
    }

    /// Accept a PvP challenge.
    pub fn accept_challenge(
        env: Env,
        caller: Address,
        challenge_id: u64,
    ) -> Result<u64, PvPError> {
        pvp_combat::accept_challenge(&env, &caller, challenge_id)
    }

    /// Decline a PvP challenge.
    pub fn decline_challenge(
        env: Env,
        caller: Address,
        challenge_id: u64,
    ) -> Result<(), PvPError> {
        pvp_combat::decline_challenge(&env, &caller, challenge_id)
    }

    /// Get a challenge by ID.
    pub fn get_challenge(env: Env, challenge_id: u64) -> Result<Challenge, PvPError> {
        pvp_combat::get_challenge(&env, challenge_id)
    }

    /// Execute a combat move.
    pub fn execute_combat_move(
        env: Env,
        player: Address,
        combat_id: u64,
        move_type: Symbol,
        power: u32,
    ) -> Result<(), PvPError> {
        pvp_combat::execute_move(&env, &player, combat_id, move_type, power)
    }

    /// Get combat state.
    pub fn get_combat(env: Env, combat_id: u64) -> Result<CombatState, PvPError> {
        pvp_combat::get_combat(&env, combat_id)
    }

    /// Get player's combat history.
    pub fn get_player_combat_history(
        env: Env,
        player: Address,
        limit: u32,
    ) -> Vec<CombatHistory> {
        pvp_combat::get_player_combat_history(&env, &player, limit)
    }

    /// Join matchmaking queue.
    pub fn join_matchmaking(
        env: Env,
        player: Address,
        preferred_stake: i128,
    ) -> Result<(), PvPError> {
        pvp_combat::join_matchmaking(&env, &player, preferred_stake)
    }

    /// Leave matchmaking queue.
    pub fn leave_matchmaking(
        env: Env,
        player: Address,
    ) -> Result<(), PvPError> {
        pvp_combat::leave_matchmaking(&env, &player)
    }

    /// Process matchmaking (admin/system).
    pub fn process_matchmaking(env: Env) -> Result<Option<(Address, Address)>, PvPError> {
        pvp_combat::process_matchmaking(&env)
    }

    /// Get matchmaking queue size.
    pub fn get_matchmaking_queue_size(env: Env) -> u32 {
        pvp_combat::get_matchmaking_queue_size(&env)
    }

    // ─── ELO Decay (Issue #191) ─────────────────────────────────────────────

    /// Apply ELO decay for a player based on inactivity.
    pub fn apply_elo_decay(env: Env, player: Address) -> u32 {
        pvp_combat::apply_elo_decay(&env, &player)
    }

    /// Get the last active timestamp for a player.
    pub fn get_last_active(env: Env, player: Address) -> u64 {
        pvp_combat::get_last_active(&env, &player)
    }

    /// Set ELO decay configuration (admin only).
    pub fn set_elo_decay_config(
        env: Env,
        caller: Address,
        config: EloDecayConfig,
    ) -> Result<(), PvPError> {
        pvp_combat::set_elo_decay_config(&env, &caller, config)
    }

    /// Get ELO decay configuration.
    pub fn get_elo_decay_config(env: Env) -> EloDecayConfig {
        pvp_combat::get_elo_decay_config(&env)
    }

    /// Apply batch ELO decay (admin only).
    pub fn apply_batch_elo_decay(
        env: Env,
        caller: Address,
        players: Vec<Address>,
    ) -> Result<u32, PvPError> {
        pvp_combat::apply_batch_elo_decay(&env, &caller, players)
    }

    /// Add spectator to combat.
    pub fn add_spectator(
        env: Env,
        spectator: Address,
        combat_id: u64,
    ) -> Result<(), PvPError> {
        pvp_combat::add_spectator(&env, &spectator, combat_id)
    }

    /// Remove spectator from combat.
    pub fn remove_spectator(
        env: Env,
        spectator: Address,
        combat_id: u64,
    ) -> Result<(), PvPError> {
        pvp_combat::remove_spectator(&env, &spectator, combat_id)
    }

    /// Get combat spectators.
    pub fn get_spectators(env: Env, combat_id: u64) -> Vec<Address> {
        pvp_combat::get_spectators(&env, combat_id)
    }

    /// Set combat rewards config (admin only).
    pub fn set_combat_rewards_config(
        env: Env,
        caller: Address,
        config: RewardsConfig,
    ) -> Result<(), PvPError> {
        pvp_combat::set_rewards_config(&env, &caller, config)
    }

    /// Get combat rewards config.
    pub fn get_combat_rewards_config(env: Env) -> RewardsConfig {
        pvp_combat::get_rewards_config(&env)
    }

    /// Set PvP combat admin (admin only).
    pub fn set_pvp_admin(env: Env, admin: Address) {
        pvp_combat::set_admin(&env, &admin)
    }

    // === Contract Versioning API ===

    pub fn initialize_version(env: Env) {
        contract_versioning::initialize_version(&env);
    }

    pub fn get_version(env: Env) -> u32 {
        contract_versioning::get_version(&env)
    }

    pub fn check_compatibility(env: Env, version: u32) {
        contract_versioning::check_compatibility(&env, version).unwrap();
    }

    pub fn set_auto_migrate(env: Env, caller: Address, enabled: bool) {
        contract_versioning::set_auto_migrate(&env, &caller, enabled);
    }

    pub fn migrate_data(env: Env, caller: Address, old_version: u32, new_version: u32, batch: Vec<Bytes>) -> MigrationRecord {
        contract_versioning::migrate_data(&env, &caller, old_version, new_version, batch).unwrap()
    }

    // === Role-Based Access Control (RBAC) API ===

    /// Initialize the RBAC system with default roles (admin, nomad, indexer).
    /// Must be called exactly once before any role-gated operations.
    pub fn init_rbac(env: Env, admin: Address) -> Result<(), AccessControlError> {
        access_control::init_roles(&env, admin)
    }

    /// Grant a role to a single address with optional expiry.
    pub fn grant_role(env: Env, caller: Address, role: Symbol, grantee: Address, expiry: Option<u32>) -> Result<(), AccessControlError> {
        access_control::grant_role(&env, caller, role, grantee, expiry)
    }

    /// Grant a role to up to 5 addresses in a single batch operation.
    pub fn grant_role_batch(env: Env, caller: Address, role: Symbol, grantees: Vec<Address>, expiry: Option<u32>) -> Result<(), AccessControlError> {
        access_control::grant_role_batch(&env, caller, role, grantees, expiry)
    }

    /// Revoke a role from an address.
    pub fn revoke_role(env: Env, caller: Address, role: Symbol, revokee: Address) -> Result<(), AccessControlError> {
        access_control::revoke_role(&env, caller, role, revokee)
    }

    /// Grant a permission to a role (allow role to perform action).
    pub fn grant_permission(env: Env, caller: Address, role: Symbol, action: Symbol) -> Result<(), AccessControlError> {
        access_control::grant_permission(&env, caller, role, action)
    }

    /// Revoke a permission from a role.
    pub fn revoke_permission(env: Env, caller: Address, role: Symbol, action: Symbol) -> Result<(), AccessControlError> {
        access_control::revoke_permission(&env, caller, role, action)
    }

    /// Transfer admin privileges to a new address.
    pub fn transfer_admin(env: Env, caller: Address, new_admin: Address) -> Result<(), AccessControlError> {
        access_control::transfer_admin(&env, caller, new_admin)
    }

    /// Check whether an address holds a specific role.
    pub fn has_role(env: Env, role: Symbol, address: Address) -> bool {
        access_control::has_role(&env, &role, &address)
    }

    /// Check whether a role is permitted to perform an action.
    pub fn has_permission(env: Env, role: Symbol, action: Symbol) -> bool {
        access_control::has_permission(&env, &role, &action)
    }

    // === Gas Recovery API ===

    pub fn initialize_refund(env: Env, admin: Address) {
        gas_recovery::initialize_refund(&env, &admin);
    }

    pub fn set_refund_percentage(env: Env, admin: Address, bps: u32) {
        gas_recovery::set_refund_percentage(&env, &admin, bps).unwrap();
    }

    pub fn request_refund(env: Env, caller: Address, tx_hash: BytesN<32>, gas_used: u64) -> RefundRequest {
        gas_recovery::request_refund(&env, &caller, tx_hash, gas_used).unwrap()
    }

    pub fn process_refund_batch(env: Env, admin: Address, tx_hashes: Vec<BytesN<32>>) -> u64 {
        gas_recovery::process_refund_batch(&env, &admin, tx_hashes).unwrap()
    }

    // === Bounty Board API ===

    pub fn initialize_bounty_board(env: Env, admin: Address) {
        bounty_board::initialize_bounty_board(&env, &admin);
    }

    pub fn post_bounty(env: Env, poster: Address, description: String, reward: i128) -> Bounty {
        let result = bounty_board::post_bounty(&env, &poster, description, reward).unwrap();
        let _ = audit_logger::log_audit_event(&env, Some(&poster), symbol_short!("pb"), BytesN::from_array(&env, &[0u8; 128]));
        result
    }

    pub fn claim_bounty(env: Env, claimer: Address, bounty_id: u64, proof: BytesN<32>) -> Bounty {
        let result = bounty_board::claim_bounty(&env, &claimer, bounty_id, proof).unwrap();
        let mut b = [0u8; 128];
        b[0..8].copy_from_slice(&bounty_id.to_be_bytes());
        let _ = audit_logger::log_audit_event(&env, Some(&claimer), symbol_short!("cb"), BytesN::from_array(&env, &b));
        result
    }

    // === Recycling/Crafting API ===

    pub fn initialize_recycling(env: Env) {
        recycling_crafter::initialize_recycling(&env);
    }

    pub fn recycle_resource(env: Env, caller: Address, resource: Symbol, amount: u32) -> Vec<(Symbol, u32)> {
        let result = recycling_crafter::recycle_resource(&env, &caller, resource, amount).unwrap();
        let _ = audit_logger::log_audit_event(&env, Some(&caller), symbol_short!("rr"), BytesN::from_array(&env, &[0u8; 128]));
        result
    }

    pub fn craft_new_item(env: Env, caller: Address, recipe_id: u64, inputs: Vec<Symbol>, quantities: Vec<u32>) -> CraftingResult {
        let result = recycling_crafter::craft_new_item(&env, &caller, recipe_id, inputs, quantities).unwrap();
        let mut b = [0u8; 128];
        b[0..8].copy_from_slice(&recipe_id.to_be_bytes());
        let _ = audit_logger::log_audit_event(&env, Some(&caller), symbol_short!("cn"), BytesN::from_array(&env, &b));
        result
    }

    pub fn get_recipe(env: Env, recipe_id: u64) -> Recipe {
        recycling_crafter::get_recipe(&env, recipe_id).unwrap()
    }

    /// Mint a new ship NFT for `owner` with initial stats derived from
    /// `ship_type` and optional free-form `metadata`.
    pub fn mint_ship(
        env: Env,
        owner: Address,
        ship_type: Symbol,
        metadata: Bytes,
    ) -> Result<ShipNft, ShipError> {
        let result = ship_nft::mint_ship(&env, &owner, &ship_type, &metadata);
        if result.is_ok() {
            let details = BytesN::from_array(&env, &[0u8; 128]);
            let _ = audit_logger::log_audit_event(&env, Some(&owner), symbol_short!("ms"), details);
        }
        result
    }

    /// Batch-mint up to 3 ship NFTs.
    pub fn batch_mint_ships(
        env: Env,
        owner: Address,
        ship_types: Vec<Symbol>,
        metadata: Bytes,
    ) -> Result<Vec<ShipNft>, ShipError> {
        let result = ship_nft::batch_mint_ships(&env, &owner, &ship_types, &metadata);
        if result.is_ok() {
            let details = BytesN::from_array(&env, &[0u8; 128]);
            let _ = audit_logger::log_audit_event(&env, Some(&owner), symbol_short!("bms"), details);
        }
        result
    }

    /// Transfer ship ownership.
    pub fn transfer_ownership(
        env: Env,
        ship_id: u64,
        new_owner: Address,
    ) -> Result<ShipNft, ShipError> {
        let result = ship_nft::transfer_ownership(&env, ship_id, &new_owner);
        if result.is_ok() {
            let mut b = [0u8; 128];
            b[0..8].copy_from_slice(&ship_id.to_be_bytes());
            let details = BytesN::from_array(&env, &b);
            let _ = audit_logger::log_audit_event(&env, Some(&new_owner), symbol_short!("to"), details);
        }
        result
    }

    /// Transfer a ship NFT from `from` to `to`.
    pub fn transfer_ship(
        env: Env,
        ship_id: u64,
        from: Address,
        to: Address,
    ) -> Result<ShipNft, ShipError> {
        let result = ship_nft::transfer_ship(&env, ship_id, &from, &to);
        if result.is_ok() {
            let mut b = [0u8; 128];
            b[0..8].copy_from_slice(&ship_id.to_be_bytes());
            let details = BytesN::from_array(&env, &b);
            let _ = audit_logger::log_audit_event(&env, Some(&from), symbol_short!("ts"), details);
        }
        result
    }

    /// Read a ship by ID.
    pub fn get_ship(env: Env, ship_id: u64) -> Result<ShipNft, ShipError> {
        ship_nft::get_ship(&env, ship_id)
    }

    /// Read all ship IDs owned by `owner`.
    pub fn get_ships_by_owner(env: Env, owner: Address) -> Vec<u64> {
        ship_nft::get_ships_by_owner(&env, &owner)
    }

    /// Set a ship NFT metadata URI for external marketplace compatibility.
    pub fn set_metadata(
        env: Env,
        owner: Address,
        ship_id: u64,
        metadata_uri: Bytes,
    ) -> Result<ShipNft, ShipError> {
        ship_nft::set_metadata(&env, &owner, ship_id, &metadata_uri)
    }

    /// Read a ship NFT metadata URI for marketplace listings.
    pub fn get_metadata(env: Env, ship_id: u64) -> Result<Bytes, ShipError> {
        ship_nft::get_metadata(&env, ship_id)
    }

    /// DEPRECATED: harvest_resources removed after upstream merge
    /// Gas-optimized harvest - functionality moved to resource_minter::mint_resource
    // pub fn harvest_resources(
    //     env: Env,
    //     ship_id: u64,
    //     layout: NebulaLayout,
    // ) -> Result<HarvestResult, HarvestError> {
    //     resource_minter::harvest_resources(&env, ship_id, &layout)
    // }

    /// DEPRECATED: auto_list_on_dex removed after upstream merge
    /// Create an AMM-listing hook for a harvested resource - use trading module instead
    // pub fn auto_list_on_dex(
    //     env: Env,
    //     resource: AssetId,
    //     min_price: i128,
    // ) -> Result<DexOffer, HarvestError> {
    //     resource_minter::auto_list_on_dex(&env, &resource, min_price)
    // }

    // ─── DEX Integration (Issue #9) ──────────────────────────────────────

    /// Harvest resources and immediately list on DEX.
    pub fn harvest_and_list(
        env: Env,
        player: Address,
        ship_id: u64,
        _layout: (),
        resource: Symbol,
        min_price: i128,
    ) -> Result<(dex_integration::HarvestResult, dex_integration::DexOffer), dex_integration::HarvestError> {
        dex_integration::harvest_and_list(&env, &player, ship_id, &_layout, &resource, min_price)
    }

    /// Cancel an active DEX listing.
    pub fn cancel_listing(
        env: Env,
        owner: Address,
        offer_id: u64,
    ) -> Result<dex_integration::DexOffer, dex_integration::HarvestError> {
        dex_integration::cancel_listing(&env, &owner, offer_id)
    }

    // ─── Treasure Vault ───────────────────────────────────────────────────

    /// Deposit resources into a time-locked treasure vault.
    pub fn deposit_treasure(
        env: Env,
        owner: Address,
        ship_id: u64,
        amount: u64,
    ) -> Result<TreasureVault, VaultError> {
        let result = treasure_vault::deposit_treasure(&env, &owner, ship_id, amount);
        if result.is_ok() {
            let mut b = [0u8; 128];
            b[0..8].copy_from_slice(&ship_id.to_be_bytes());
            let _ = audit_logger::log_audit_event(&env, Some(&owner), symbol_short!("dt"), BytesN::from_array(&env, &b));
        }
        result
    }

    /// Claim a treasure vault after the lock period expires.
    pub fn claim_treasure(env: Env, owner: Address, vault_id: u64) -> Result<u64, VaultError> {
        let result = treasure_vault::claim_treasure(&env, &owner, vault_id);
        if result.is_ok() {
            let mut b = [0u8; 128];
            b[0..8].copy_from_slice(&vault_id.to_be_bytes());
            let _ = audit_logger::log_audit_event(&env, Some(&owner), symbol_short!("ct"), BytesN::from_array(&env, &b));
        }
        result
    }

    /// Read a vault by ID.
    pub fn get_vault(env: Env, vault_id: u64) -> Option<TreasureVault> {
        treasure_vault::get_vault(&env, vault_id)
    }

    // ─── Difficulty Scaling ───────────────────────────────────────────────

    /// Calculate difficulty scaling for a player level.
    pub fn calculate_difficulty(
        env: Env,
        player_level: u32,
    ) -> Result<DifficultyResult, DifficultyError> {
        difficulty_scaler::calculate_difficulty(&env, player_level)
    }

    /// Apply difficulty scaling to a layout's anomaly count.
    pub fn apply_scaling_to_layout(
        env: Env,
        base_anomaly_count: u32,
        player_level: u32,
    ) -> Result<u32, DifficultyError> {
        difficulty_scaler::apply_scaling_to_layout(&env, base_anomaly_count, player_level)
    }

    // ─── Randomness Oracle ────────────────────────────────────────────────

    /// Request a ledger-mixed random seed.
    pub fn request_random_seed(env: Env) -> BytesN<32> {
        randomness_oracle::request_random_seed(&env)
    }

    /// Validate a seed or fall back to previous block hash.
    pub fn verify_and_fallback(env: Env, seed: BytesN<32>) -> Result<BytesN<32>, OracleError> {
        randomness_oracle::verify_and_fallback(&env, &seed)
    }

    /// Get the current entropy pool.
    pub fn get_entropy_pool(env: Env) -> Vec<BytesN<32>> {
        randomness_oracle::get_entropy_pool(&env)
    }

    // ─── Player Profile ───────────────────────────────────────────────────────
    // ─── Player Profile ───────────────────────────────────────────────────

    /// Create a new on-chain player profile.
    pub fn initialize_profile(env: Env, owner: Address) -> Result<u64, ProfileError> {
        player_profile::initialize_profile(&env, owner)
    }

    /// Update scan count and essence earned after a harvest.
    pub fn update_progress(
        env: Env,
        caller: Address,
        profile_id: u64,
        scan_count: u32,
        essence: i128,
    ) -> Result<(), ProfileError> {
        player_profile::update_progress(&env, caller, profile_id, scan_count, essence)
    }

    /// Apply up to 5 stat updates in a single transaction.
    pub fn batch_update_progress(
        env: Env,
        caller: Address,
        updates: Vec<ProgressUpdate>,
    ) -> Result<(), ProfileError> {
        player_profile::batch_update_progress(&env, caller, updates)
    }

    /// Retrieve a player profile by ID.
    pub fn get_profile(env: Env, profile_id: u64) -> Result<PlayerProfile, ProfileError> {
        player_profile::get_profile(&env, profile_id)
    }

    // ─── Onboarding Tutorial (Issue #139) ────────────────────────────────

    /// Initialize the onboarding system with an admin.
    pub fn init_onboarding(env: Env, admin: Address) -> Result<(), OnboardingError> {
        onboarding_tutorial::init_onboarding(&env, &admin)
    }

    /// Create a player profile and start the tutorial.
    pub fn create_profile_onboarding(env: Env, player: Address) -> Result<(), OnboardingError> {
        onboarding_tutorial::create_profile(&env, &player)
    }

    /// Start the tutorial for an existing profile.
    pub fn start_tutorial(env: Env, player: Address) -> Result<(), OnboardingError> {
        onboarding_tutorial::start_tutorial(&env, &player)
    }

    /// Complete a tutorial step and earn rewards.
    pub fn complete_tutorial_step(env: Env, player: Address, step_id: u32) -> Result<i128, OnboardingError> {
        onboarding_tutorial::complete_tutorial_step(&env, &player, step_id)
    }

    /// Get tutorial progress for a player.
    pub fn get_tutorial_progress(env: Env, player: Address) -> Option<TutorialProgress> {
        onboarding_tutorial::get_tutorial_progress(&env, &player)
    }

    /// Get starter resources earned from tutorial.
    pub fn get_starter_resources(env: Env, player: Address) -> i128 {
        onboarding_tutorial::get_starter_resources(&env, &player)
    }

    /// Admin function to set a custom tutorial path.
    pub fn set_tutorial_path(env: Env, admin: Address, path: Vec<u32>) -> Result<(), OnboardingError> {
        onboarding_tutorial::set_tutorial_path(&env, &admin, path)
    }

    // ─── Session Manager ──────────────────────────────────────────────────

    /// Start a timed nebula exploration session for a ship.
    pub fn start_session(env: Env, owner: Address, ship_id: u64) -> Result<u64, SessionError> {
        session_manager::start_session(&env, owner, ship_id)
    }

    /// Close a session.
    pub fn expire_session(
        env: Env,
        caller: Address,
        session_id: u64,
    ) -> Result<(), SessionError> {
        session_manager::expire_session(&env, caller, session_id)
    }

    /// Retrieve session data by ID.
    pub fn get_session(env: Env, session_id: u64) -> Result<Session, SessionError> {
        session_manager::get_session(&env, session_id)
    }

    // ─── Blueprint Factory ────────────────────────────────────────────────

    /// Mint a blueprint NFT from harvested resource components.
    pub fn craft_blueprint(
        env: Env,
        owner: Address,
        components: Vec<Symbol>,
    ) -> Result<u64, BlueprintError> {
        blueprint_factory::craft_blueprint(&env, owner, components)
    }

    /// Craft up to 2 blueprints in a single transaction.
    pub fn batch_craft_blueprints(
        env: Env,
        owner: Address,
        recipes: Vec<Vec<Symbol>>,
    ) -> Result<Vec<u64>, BlueprintError> {
        blueprint_factory::batch_craft_blueprints(&env, owner, recipes)
    }

    /// Consume a blueprint and permanently upgrade a ship.
    pub fn apply_blueprint_to_ship(
        env: Env,
        owner: Address,
        blueprint_id: u64,
        ship_id: u64,
    ) -> Result<(), BlueprintError> {
        blueprint_factory::apply_blueprint_to_ship(&env, owner, blueprint_id, ship_id)
    }

    /// Retrieve a blueprint by ID.
    pub fn get_blueprint(env: Env, blueprint_id: u64) -> Result<Blueprint, BlueprintError> {
        blueprint_factory::get_blueprint(&env, blueprint_id)
    }

    // ─── Referral System ──────────────────────────────────────────────────

    /// Record an on-chain referral.
    pub fn register_referral(
        env: Env,
        referrer: Address,
        new_nomad: Address,
    ) -> Result<u64, ReferralError> {
        referral_system::register_referral(&env, referrer, new_nomad)
    }

    /// Mark first scan completed, unlocking referral reward.
    pub fn mark_first_scan(env: Env, nomad: Address) -> Result<(), ReferralError> {
        referral_system::mark_first_scan(&env, nomad)
    }

    /// Claim the essence referral reward.
    pub fn claim_referral_reward(
        env: Env,
        referrer: Address,
        new_nomad: Address,
    ) -> Result<i128, ReferralError> {
        referral_system::claim_referral_reward(&env, referrer, new_nomad)
    }

    /// Retrieve a referral record.
    pub fn get_referral(env: Env, new_nomad: Address) -> Result<Referral, ReferralError> {
        referral_system::get_referral(&env, new_nomad)
    }

    // ─── Ship Upgrade (Issue #7) ─────────────────────────────────────────

    /// Initialise the upgrade config with a blueprint map. Admin-only, once.
    pub fn init_upgrade_config(
        env: Env,
        admin: Address,
        blueprints: soroban_sdk::Map<Symbol, UpgradeBlueprint>,
    ) -> Result<(), ShipUpgradeError> {
        ship_upgrade::init_upgrade_config(&env, &admin, blueprints)
    }

    /// Apply a single component upgrade to a ship, burning the required
    /// resource from the player's harvested balance.
    pub fn apply_upgrade(
        env: Env,
        player: Address,
        ship_id: u64,
        component: Symbol,
    ) -> Result<ShipState, ShipUpgradeError> {
        ship_upgrade::apply_upgrade(&env, &player, ship_id, component)
    }

    /// Apply up to 2 upgrades in a single transaction.
    pub fn batch_upgrade(
        env: Env,
        player: Address,
        ship_id: u64,
        components: Vec<Symbol>,
    ) -> Result<Vec<ShipState>, ShipUpgradeError> {
        ship_upgrade::batch_upgrade(&env, &player, ship_id, components)
    }

    /// Read the current upgrade state of a ship.
    pub fn get_ship_state(env: Env, ship_id: u64) -> Option<ShipState> {
        ship_upgrade::get_ship_state(&env, ship_id)
    }

    /// Apply a regeneration upgrade effect (called after a regen upgrade).
    pub fn apply_regen_upgrade(
        env: Env,
        ship_id: u64,
        bonus: u32,
    ) -> Result<(), ShipUpgradeError> {
        ship_upgrade::apply_regen_upgrade(&env, ship_id, bonus)
    }

    // ─── Cross-Player Resource Gifting (#27) ──────────────────────────────

    /// Send a resource gift to another player.
    pub fn send_gift(
        env: Env,
        sender: Address,
        receiver: Address,
        resource: AssetId,
        amount: i128,
    ) -> Result<u64, GiftError> {
        gifting_system::send_gift(&env, sender, receiver, resource, amount)
    }

    /// Accept a pending gift and claim the resources.
    pub fn accept_gift(env: Env, receiver: Address, gift_id: u64) -> Result<(), GiftError> {
        gifting_system::accept_gift(&env, receiver, gift_id)
    }

    /// Read a gift by ID.
    pub fn get_gift(env: Env, gift_id: u64) -> Option<Gift> {
        gifting_system::get_gift(&env, gift_id)
    }

    // ─── Yield Farming (Issue #36) ───────────────────────────────────────────

    /// Stake resources for boosted yields.
    pub fn deposit_to_pool(
        env: Env,
        owner: Address,
        amount: i128,
        lock_period: u32,
    ) -> Result<u64, yield_farming::FarmError> {
        yield_farming::deposit_to_pool(env, owner, amount, lock_period)
    }

    /// Claim accumulated cosmic rewards.
    pub fn harvest_farm_rewards(
        env: Env,
        owner: Address,
        pool_id: u64,
    ) -> Result<i128, yield_farming::FarmError> {
        yield_farming::harvest_farm_rewards(env, owner, pool_id)
    }

    // ─── Community Governance (Issue #38) ────────────────────────────────────

    /// Submit a proposed config change.
    pub fn create_proposal(
        env: Env,
        creator: Address,
        description: String,
        param_change: BytesN<128>,
    ) -> Result<u64, governance::GovError> {
        governance::create_proposal(env, creator, description, param_change)
    }

    /// Record a vote weighted by essence held.
    pub fn cast_vote(
        env: Env,
        voter: Address,
        proposal_id: u64,
        support: bool,
        weight: i128,
    ) -> Result<(), governance::GovError> {
        governance::cast_vote(env, voter, proposal_id, support, weight)
    }

    // ─── Theme Customizer (Issue #37) ────────────────────────────────────────

    /// Set ship color palette and particle style.
    pub fn apply_theme(
        env: Env,
        owner: Address,
        ship_id: u64,
        theme_id: Symbol,
    ) -> Result<(), theme_customizer::ThemeError> {
        theme_customizer::apply_theme(env, owner, ship_id, theme_id)
    }

    /// Returns theme preview metadata.
    pub fn generate_theme_preview(
        env: Env,
        theme_id: Symbol,
    ) -> Result<theme_customizer::ThemePreview, theme_customizer::ThemeError> {
        theme_customizer::generate_theme_preview(env, theme_id)
    }

    // ─── Indexer Callbacks (Issue #35) ───────────────────────────────────────

    /// Subscribes an external service to events.
    pub fn register_indexer_callback(
        env: Env,
        caller: Address,
        callback_id: Symbol,
    ) -> Result<(), indexer_callbacks::IndexerError> {
        indexer_callbacks::register_indexer_callback(env, caller, callback_id)
    }

    /// Broadcasts rich data for external dashboards.
    pub fn trigger_indexer_event(
        env: Env,
        event_type: Symbol,
        payload: BytesN<256>,
    ) -> Result<(), indexer_callbacks::IndexerError> {
        indexer_callbacks::trigger_indexer_event(env, event_type, payload)
    }

    // ─── Energy Management ────────────────────────────────────────────────

    /// Consume energy for ship operations.
    pub fn consume_energy(
        env: Env,
        ship_id: u64,
        amount: u32,
    ) -> Result<u32, energy_manager::EnergyError> {
        energy_manager::consume_energy(&env, ship_id, amount)
    }

    /// Recharge ship energy using resources.
    pub fn recharge_energy(
        env: Env,
        ship_id: u64,
        resource_amount: i128,
    ) -> Result<energy_manager::RechargeResult, energy_manager::EnergyError> {
        energy_manager::recharge_energy(&env, ship_id, resource_amount)
    }

    /// Get ship energy balance.
    pub fn get_energy_balance(
        env: Env,
        ship_id: u64,
    ) -> Result<energy_manager::EnergyBalance, energy_manager::EnergyError> {
        energy_manager::get_energy_balance(&env, ship_id)
    }

    // ─── Passive Energy Regeneration (Issue #190) ─────────────────────────

    /// Apply passive energy recovery for a ship.
    pub fn apply_passive_regen(
        env: Env,
        ship_id: u64,
    ) -> Result<energy_manager::PassiveRegenResult, energy_manager::EnergyError> {
        energy_manager::apply_passive_regen(&env, ship_id)
    }

    /// Set the passive regeneration rate (admin / upgrade system).
    pub fn set_regen_rate(env: Env, caller: Address, rate: u32) {
        energy_manager::set_regen_rate(&env, &caller, rate)
    }

    /// Get the current passive regeneration rate.
    pub fn get_regen_rate(env: Env) -> u32 {
        energy_manager::get_regen_rate(&env)
    }

    // ─── Environmental Simulation ─────────────────────────────────────────

    /// Simulate environmental conditions for a nebula.
    pub fn simulate_conditions(
        env: Env,
        nebula_id: u64,
    ) -> Result<environment_simulator::EnvironmentCondition, environment_simulator::EnvironmentError> {
        environment_simulator::simulate_conditions(&env, nebula_id)
    }

    /// Apply environmental modifiers to harvest yields.
    pub fn apply_environmental_modifier(
        env: Env,
        ship_id: u64,
        nebula_id: u64,
        base_yield: i32,
    ) -> Result<environment_simulator::ModifierResult, environment_simulator::EnvironmentError> {
        environment_simulator::apply_environmental_modifier(&env, ship_id, nebula_id, base_yield)
    }

    /// Get current nebula environmental condition.
    pub fn get_nebula_condition(
        env: Env,
        nebula_id: u64,
    ) -> Option<environment_simulator::EnvironmentCondition> {
        environment_simulator::get_nebula_condition(&env, nebula_id)
    }

    // ─── Mission System ───────────────────────────────────────────────────

    /// Generate a new daily mission for player.
    pub fn generate_daily_mission(
        env: Env,
        player: Address,
    ) -> Result<mission_generator::Mission, mission_generator::MissionError> {
        mission_generator::generate_daily_mission(&env, player)
    }

    /// Complete a mission and claim rewards.
    pub fn complete_mission(
        env: Env,
        player: Address,
        mission_id: u64,
    ) -> Result<mission_generator::MissionReward, mission_generator::MissionError> {
        mission_generator::complete_mission(&env, player, mission_id)
    }

    /// Update mission progress.
    pub fn update_mission_progress(
        env: Env,
        mission_id: u64,
        progress: u32,
    ) -> Result<mission_generator::Mission, mission_generator::MissionError> {
        mission_generator::update_mission_progress(&env, mission_id, progress)
    }

    /// Get all missions for a player.
    pub fn get_player_missions(env: Env, player: Address) -> Vec<mission_generator::Mission> {
        mission_generator::get_player_missions(&env, player)
    }

    // ─── Escrow Trading ───────────────────────────────────────────────────

    /// Initiate a peer-to-peer escrow trade.
    pub fn initiate_escrow(
        env: Env,
        trader_a: Address,
        trader_b: Address,
        assets_a: Vec<escrow_trader::TradeAsset>,
        assets_b: Vec<escrow_trader::TradeAsset>,
    ) -> Result<escrow_trader::Escrow, escrow_trader::EscrowError> {
        escrow_trader::initiate_escrow(&env, trader_a, trader_b, assets_a, assets_b)
    }

    /// Confirm participation in an escrow trade.
    pub fn confirm_escrow(
        env: Env,
        escrow_id: u64,
        trader: Address,
    ) -> Result<escrow_trader::Escrow, escrow_trader::EscrowError> {
        escrow_trader::confirm_escrow(&env, escrow_id, trader)
    }

    /// Complete an escrow trade atomically.
    pub fn complete_escrow(
        env: Env,
        escrow_id: u64,
    ) -> Result<escrow_trader::EscrowResult, escrow_trader::EscrowError> {
        escrow_trader::complete_escrow(&env, escrow_id)
    }

    /// Cancel an escrow trade.
    pub fn cancel_escrow(
        env: Env,
        escrow_id: u64,
        trader: Address,
    ) -> Result<(), escrow_trader::EscrowError> {
        escrow_trader::cancel_escrow(&env, escrow_id, trader)
    }

    /// Get escrow details by ID.
    pub fn get_escrow(env: Env, escrow_id: u64) -> Option<escrow_trader::Escrow> {
        escrow_trader::get_escrow(&env, escrow_id)
    }

    // ─── Emergency Controls (Issue #29) ──────────────────────────────────

    /// Initialize the multi-sig admin set at deployment. One-time call.
    pub fn initialize_admins(env: Env, admins: Vec<Address>) -> Result<(), EmergencyError> {
        emergency_controls::initialize_admins(&env, admins)
    }

    /// Instantly freeze all mutating contract functions. Admin-only.
    pub fn pause_contract(env: Env, admin: Address) -> Result<(), EmergencyError> {
        emergency_controls::pause_contract(&env, &admin)
    }

    /// Schedule a time-delayed unpause. Admin-only.
    pub fn schedule_unpause(env: Env, admin: Address) -> Result<u64, EmergencyError> {
        emergency_controls::schedule_unpause(&env, &admin)
    }

    /// Execute the unpause after the delay has elapsed. Admin-only.
    pub fn execute_unpause(env: Env, admin: Address) -> Result<(), EmergencyError> {
        emergency_controls::execute_unpause(&env, &admin)
    }

    /// Admin-only emergency recovery of stuck resources.
    pub fn emergency_withdraw(env: Env, admin: Address, resource: Symbol) -> Result<(), EmergencyError> {
        emergency_controls::emergency_withdraw(&env, &admin, resource)
    }

    /// Returns true if the contract is currently paused.
    pub fn is_paused(env: Env) -> bool {
        emergency_controls::is_paused(&env)
    }

    /// Returns the current admin list.
    pub fn get_admins(env: Env) -> Vec<Address> {
        emergency_controls::get_admins(&env)
    }

    // ─── Metadata URI Resolver (Issue #30) ───────────────────────────────

    /// Set the IPFS CID for a token. Immutable after first set.
    pub fn set_metadata_uri(env: Env, caller: Address, token_id: u64, cid: Bytes) -> Result<(), MetadataError> {
        metadata_resolver::set_metadata_uri(&env, &caller, token_id, cid)
    }

    /// Resolve full metadata for a token using the configured gateway.
    pub fn resolve_metadata(env: Env, token_id: u64) -> Result<TokenMetadata, MetadataError> {
        metadata_resolver::resolve_metadata(&env, token_id)
    }

    /// Batch resolve metadata for up to 10 tokens.
    pub fn batch_resolve_metadata(env: Env, token_ids: Vec<u64>) -> Result<Vec<TokenMetadata>, MetadataError> {
        metadata_resolver::batch_resolve_metadata(&env, token_ids)
    }

    /// Update the IPFS gateway prefix. Admin-only.
    pub fn set_gateway(env: Env, admin: Address, gateway: Bytes) {
        metadata_resolver::set_gateway(&env, &admin, gateway)
    }

    /// Return the currently configured IPFS gateway prefix.
    pub fn get_current_gateway(env: Env) -> Bytes {
        metadata_resolver::get_current_gateway(&env)
    }

    // ─── Batch Ship Operations (Issue #31) ───────────────────────────────

    /// Stage up to 8 ship operations into the player's batch queue.
    pub fn queue_batch_operation(env: Env, player: Address, operations: Vec<BatchOp>) -> Result<u32, BatchError> {
        batch_processor::queue_batch_operation(&env, &player, operations)
    }

    /// Execute all queued operations atomically for the provided ship IDs.
    pub fn execute_batch(env: Env, player: Address, ship_ids: Vec<u64>) -> Result<BatchResult, BatchError> {
        batch_processor::execute_batch(&env, &player, ship_ids)
    }

    /// Return the player's currently queued batch.
    pub fn get_player_batch(env: Env, player: Address) -> Option<Vec<BatchOp>> {
        batch_processor::get_player_batch(&env, &player)
    }

    /// Clear the player's pending batch queue.
    pub fn clear_batch(env: Env, player: Address) {
        batch_processor::clear_batch(&env, &player)
    }

// ─── On-chain Audit Logging (Issue #64) ───────────────────────────────

    pub fn log_audit_event(
        env: Env,
        actor: Option<Address>,
        action: Symbol,
        details: BytesN<128>,
    ) -> Result<AuditEntry, AuditLoggerError> {
        audit_logger::log_audit_event(&env, actor.as_ref(), action, details)
    }

    pub fn query_audit_logs(env: Env, filter: Symbol, limit: u32) -> Result<Vec<AuditEntry>, AuditLoggerError> {
        audit_logger::query_audit_logs(&env, filter, limit)
    }

    pub fn get_audit_count(env: Env) -> u64 {
        audit_logger::get_audit_count(&env)
    }

    // ─── Sustainability and Carbon Tracking (Issue #68) ──────────────────

    pub fn record_transaction_footprint(
        env: Env,
        player: Address,
        gas_used: u64,
    ) -> FootprintRecord {
        let record = sustainability_metrics::record_transaction_footprint(&env, &player, gas_used)
            .unwrap();
        let mut details_bytes = [0u8; 128];
        details_bytes[0..8].copy_from_slice(&record.gas_used.to_be_bytes());
        let details = BytesN::from_array(&env, &details_bytes);
        let _ = audit_logger::log_audit_event(&env, Some(&player), symbol_short!("ec"), details);
        record
    }

    pub fn claim_sustainability_reward(
        env: Env,
        player: Address,
    ) -> i128 {
        let reward = sustainability_metrics::claim_sustainability_reward(&env, &player)
            .unwrap();
        let mut details_bytes = [0u8; 128];
        details_bytes[0..8].copy_from_slice(&(reward as i64).to_be_bytes());
        let details = BytesN::from_array(&env, &details_bytes);
        let _ = audit_logger::log_audit_event(&env, Some(&player), symbol_short!("er"), details);
        reward
    }

    pub fn get_footprint(env: Env, player: Address) -> FootprintRecord {
        sustainability_metrics::get_footprint(&env, &player)
    }

    // ─── Cosmic Anomaly Classification Engine (Issue #70) ────────────────

    pub fn classify_anomaly(
        env: Env,
        anomaly_id: u64,
        features: Vec<u32>,
    ) -> ClassificationRecord {
        anomaly_classifier::classify_anomaly(&env, anomaly_id, features)
            .unwrap()
    }

    pub fn refine_classification(
        env: Env,
        anomaly_id: u64,
        new_data: Vec<u32>,
    ) -> ClassificationRecord {
        anomaly_classifier::refine_classification(&env, anomaly_id, new_data)
            .unwrap()
    }

    pub fn classify_batch(
        env: Env,
        items: Vec<(u64, Vec<u32>)>,
    ) -> Vec<ClassificationRecord> {
        anomaly_classifier::classify_batch(&env, items)
    }

    pub fn get_classification(env: Env, anomaly_id: u64) -> Option<ClassificationRecord> {
        anomaly_classifier::get_classification(&env, anomaly_id)
    }

    // ─── Shared Reusability Library (Issue #67) ──────────────────────────

    pub fn validate_address(env: Env, auth: Address) -> Result<(), SharedError> {
        shared_lib::validate_address(&env, auth)
    }

    pub fn calculate_yield(env: Env, base: i128, multiplier: u32) -> Result<i128, SharedError> {
        shared_lib::calculate_yield(base, multiplier)
    }

    // ─── Storage Optimization & Re-Entrancy Guards (Issue #10) ────────────

    /// Initialize the bump storage configuration. Admin-only.
    pub fn initialize_bump_config(env: Env, admin: Address) {
        storage_optim::initialize_bump_config(&env, &admin)
    }

    /// Store data with optimized persistent bump TTL.
    pub fn store_with_bump(
        env: Env,
        key: Symbol,
        value: BytesN<64>,
    ) -> Result<OptimResult, StorageError> {
        storage_optim::store_with_bump(&env, key, value)
    }

    /// Retrieve an optimized storage entry.
    pub fn get_optimized_entry(
        env: Env,
        key: Symbol,
    ) -> Result<OptimizedEntry, StorageError> {
        storage_optim::get_optimized_entry(&env, key)
    }

    /// Batch-store multiple entries with a single re-entrancy guard.
    pub fn batch_store_with_bump(
        env: Env,
        keys: Vec<Symbol>,
        values: Vec<BytesN<64>>,
    ) -> Result<Vec<OptimResult>, StorageError> {
        storage_optim::batch_store_with_bump(&env, keys, values)
    }

    /// Store composite ship-nebula data in a single slot.
    pub fn store_ship_nebula(
        env: Env,
        ship_id: u64,
        nebula_id: u64,
        scan_count: u32,
        resource_cache: u64,
    ) -> Result<(), StorageError> {
        storage_optim::store_ship_nebula(&env, ship_id, nebula_id, scan_count, resource_cache)
    }

    /// Retrieve composite ship-nebula data.
    pub fn get_ship_nebula(
        env: Env,
        ship_id: u64,
        nebula_id: u64,
    ) -> Result<ShipNebulaData, StorageError> {
        storage_optim::get_ship_nebula(&env, ship_id, nebula_id)
    }

    /// Update bump TTL configuration. Admin-only.
    pub fn update_bump_config(
        env: Env,
        admin: Address,
        default_ttl: u32,
        max_ttl: u32,
    ) -> Result<(), StorageError> {
        storage_optim::update_bump_config(&env, &admin, default_ttl, max_ttl)
    }

    /// Set the proxy upgrade target address. Admin-only.
    pub fn set_upgrade_target(
        env: Env,
        admin: Address,
        target: Address,
    ) -> Result<(), StorageError> {
        storage_optim::set_upgrade_target(&env, &admin, target)
    }

    /// Get the current upgrade target if set.
    pub fn get_upgrade_target(env: Env) -> Option<Address> {
        storage_optim::get_upgrade_target(&env)
    }

    /// Reset the burst-read counter for a new invocation.
    pub fn reset_burst_counter(env: Env) {
        storage_optim::reset_burst_counter(&env)
    }

    // ─── On-Chain Game State Snapshots (Issue #58) ───────────────────────

    /// Take a snapshot of the current ship and resource state.
    pub fn take_snapshot(
        env: Env,
        caller: Address,
        ship_id: u64,
    ) -> Result<StateSnapshot, SnapshotError> {
        state_snapshot::take_snapshot(&env, &caller, ship_id)
    }

    /// Restore ship state from a previously taken snapshot.
    pub fn restore_from_snapshot(
        env: Env,
        caller: Address,
        snapshot_id: u64,
    ) -> Result<RestoreResult, SnapshotError> {
        state_snapshot::restore_from_snapshot(&env, &caller, snapshot_id)
    }

    /// Get a snapshot by ID.
    pub fn get_snapshot(
        env: Env,
        snapshot_id: u64,
    ) -> Result<StateSnapshot, SnapshotError> {
        state_snapshot::get_snapshot(&env, snapshot_id)
    }

    /// Get all snapshot IDs for a ship.
    pub fn get_ship_snapshots(env: Env, ship_id: u64) -> Vec<u64> {
        state_snapshot::get_ship_snapshots(&env, ship_id)
    }

    /// Trigger an automatic daily snapshot if the interval has elapsed.
    pub fn auto_snapshot(
        env: Env,
        caller: Address,
        ship_id: u64,
    ) -> Result<StateSnapshot, SnapshotError> {
        state_snapshot::auto_snapshot(&env, &caller, ship_id)
    }

    /// Reset snapshot session counter for a ship.
    pub fn reset_session_count(env: Env, ship_id: u64) {
        state_snapshot::reset_session_count(&env, ship_id)

    }

    // ─── Prize Distributor (Issue #62) ───────────────────────────────────

    /// Initialize the weekly prize distributor. One-time setup.
    pub fn initialize_prize_distributor(env: Env, admin: Address) {
        prize_distributor::initialize_prize_distributor(&env, &admin)
    }

    /// Add funds to the prize pool (sponsor-funded pools supported).
    pub fn fund_prize_pool(env: Env, funder: Address, amount: i128) -> Result<i128, PrizeError> {
        prize_distributor::fund_prize_pool(&env, &funder, amount)
    }

    /// Admin: record the current leaderboard snapshot for payout.
    pub fn submit_leaderboard_snapshot(
        env: Env,
        admin: Address,
        winners: Vec<Address>,
    ) -> Result<u32, PrizeError> {
        prize_distributor::submit_leaderboard_snapshot(&env, &admin, &winners)
    }

    /// Distribute weekly prizes to the top N positions (max 50 per tx).
    pub fn distribute_weekly_prizes(
        env: Env,
        caller: Address,
        top_n: u32,
    ) -> Result<Vec<PrizeRecord>, PrizeError> {
        prize_distributor::distribute_weekly_prizes(&env, &caller, top_n)
    }

    /// Return current prize pool balance.
    pub fn get_prize_pool(env: Env) -> i128 {
        prize_distributor::get_prize_pool(&env)
    }

    /// Return total prizes distributed all-time.
    pub fn get_total_distributed(env: Env) -> i128 {
        prize_distributor::get_total_distributed(&env)
    }

    // ─── Portal Registry (Issue #71) ─────────────────────────────────────

    /// Initialize the inter-nebula portal registry.
    pub fn initialize_portal_registry(env: Env, admin: Address) {
        portal_registry::initialize_portal_registry(&env, &admin)
    }

    /// Register a new portal between two nebulae.
    pub fn register_portal(
        env: Env,
        owner: Address,
        source_nebula: u64,
        target_nebula: u64,
    ) -> Result<u64, PortalError> {
        portal_registry::register_portal(&env, &owner, source_nebula, target_nebula)
    }

    /// Query stability percentage and travel cost for a portal.
    pub fn query_portal_status(env: Env, portal_id: u64) -> Result<(u32, i128), PortalError> {
        portal_registry::query_portal_status(&env, portal_id)
    }

    /// Refresh a portal's stability back to 100%.
    pub fn refresh_portal(env: Env, owner: Address, portal_id: u64) -> Result<(), PortalError> {
        portal_registry::refresh_portal(&env, &owner, portal_id)
    }

    /// Attempt travel through a portal.
    pub fn travel_through_portal(env: Env, portal_id: u64) -> Result<i128, PortalError> {
        portal_registry::travel_through_portal(&env, portal_id)
    }

    // ─── Constellation Mapper (Issue #72) ────────────────────────────────

    /// Record a new star constellation pattern on-chain.
    pub fn record_constellation(
        env: Env,
        recorder: Address,
        stars: Vec<BytesN<32>>,
    ) -> Result<u64, ConstellationError> {
        constellation_mapper::record_constellation(&env, &recorder, &stars)
    }

    /// Find the best matching known constellation for an observed pattern.
    pub fn match_constellation(
        env: Env,
        observed: Vec<BytesN<32>>,
    ) -> Result<MatchResult, ConstellationError> {
        constellation_mapper::match_constellation(&env, &observed)
    }

    /// Return total recorded constellations.
    pub fn get_constellation_count(env: Env) -> u64 {
        constellation_mapper::get_constellation_count(&env)
    }

    // ─── Quantum Entanglement Comms (Issue #73) ───────────────────────────

    /// Establish an entanglement pair between two ships.
    pub fn create_entanglement_pair(
        env: Env,
        owner_a: Address,
        ship_a: u64,
        owner_b: Address,
        ship_b: u64,
    ) -> Result<u64, EntanglementError> {
        entanglement_comms::create_entanglement_pair(&env, &owner_a, ship_a, &owner_b, ship_b)
    }

    /// Send a single encrypted message over an active pair.
    pub fn send_entangled_message(
        env: Env,
        caller: Address,
        pair_id: u64,
        message: BytesN<64>,
    ) -> Result<u64, EntanglementError> {
        entanglement_comms::send_entangled_message(&env, &caller, pair_id, &message)
    }

    /// Send up to 20 messages in one transaction.
    pub fn send_entangled_message_batch(
        env: Env,
        caller: Address,
        pair_id: u64,
        messages: Vec<BytesN<64>>,
    ) -> Result<u64, EntanglementError> {
        entanglement_comms::send_entangled_message_batch(&env, &caller, pair_id, &messages)
    }

    /// Dissolve an entanglement pair.
    pub fn dissolve_pair(
        env: Env,
        caller: Address,
        pair_id: u64,
    ) -> Result<(), EntanglementError> {
        entanglement_comms::dissolve_pair(&env, &caller, pair_id)
    }

    /// Return total messages sent over a pair.
    pub fn get_message_count(env: Env, pair_id: u64) -> u64 {
        entanglement_comms::get_message_count(&env, pair_id)
    }

    // ─── Fractional Resource Ownership API (Issue #89) ────────────────────

    /// Initialize the fractional resource system.
    pub fn initialize_fractional(env: Env, admin: Address) -> Result<(), FractionalError> {
        fractional_resources::initialize(&env, &admin)
    }

    /// Fractionalize a resource into divisible shares.
    pub fn fractionalize_resource(
        env: Env,
        owner: Address,
        resource_type: Symbol,
        total_amount: u32,
        shares: u32,
    ) -> Result<Vec<u64>, FractionalError> {
        fractional_resources::fractionalize_resource(&env, &owner, resource_type, total_amount, shares)
    }

    /// Merge fractional shares back into a whole resource.
    pub fn merge_fractions(
        env: Env,
        owner: Address,
        share_ids: Vec<u64>,
    ) -> Result<u32, FractionalError> {
        fractional_resources::merge_fractions(&env, &owner, share_ids)
    }

    /// Transfer a fractional share to another owner.
    pub fn transfer_share(
        env: Env,
        from: Address,
        to: Address,
        share_id: u64,
    ) -> Result<FractionalShare, FractionalError> {
        fractional_resources::transfer_share(&env, &from, &to, share_id)
    }

    /// Get a fractional share by ID.
    pub fn get_share(env: Env, share_id: u64) -> Option<FractionalShare> {
        fractional_resources::get_share(&env, share_id)
    }

    /// Get all share IDs owned by an address.
    pub fn get_owner_shares(env: Env, owner: Address) -> Vec<u64> {
        fractional_resources::get_owner_shares(&env, &owner)
    }

    /// Get the total number of shares created.
    pub fn get_total_shares(env: Env) -> u64 {
        fractional_resources::get_total_shares(&env)
    }

    /// Get original resource data.
    pub fn get_original_resource(env: Env, resource_type: Symbol) -> Option<OriginalResource> {
        fractional_resources::get_original_resource(&env, resource_type)
    }

    /// Check if an address owns a specific share.
    pub fn is_share_owner(env: Env, owner: Address, share_id: u64) -> bool {
        fractional_resources::is_share_owner(&env, &owner, share_id)
    }

    /// Update fractionalization config (admin only).
    pub fn update_fractional_config(
        env: Env,
        admin: Address,
        min_share_size: u32,
        max_fractions: u32,
    ) -> Result<FractionalConfig, FractionalError> {
        fractional_resources::update_config(&env, &admin, min_share_size, max_fractions)
    }

    // ─── Yield Forecasting API (Issue #90) ─────────────────────────────────

    /// Initialize the yield forecasting system.
    pub fn initialize_forecast(env: Env, admin: Address) -> Result<(), ForecastError> {
        yield_forecast::initialize(&env, &admin)
    }

    /// Generate a yield forecast for a player.
    pub fn generate_yield_forecast(
        env: Env,
        player: Address,
        days: u32,
    ) -> Result<YieldForecast, ForecastError> {
        yield_forecast::generate_yield_forecast(&env, &player, days)
    }

    /// Update the forecast model with new data.
    pub fn update_forecast_model(
        env: Env,
        player: Address,
        data_point: YieldDataPoint,
    ) -> Result<ModelParams, ForecastError> {
        yield_forecast::update_forecast_model(&env, &player, data_point)
    }

    /// Batch generate forecasts for multiple players.
    pub fn batch_generate_forecasts(
        env: Env,
        players: Vec<(Address, u32)>,
    ) -> Vec<Result<YieldForecast, ForecastError>> {
        yield_forecast::batch_generate_forecasts(&env, players)
    }

    /// Get cached forecast for a player.
    pub fn get_cached_forecast(env: Env, player: Address) -> Option<YieldForecast> {
        yield_forecast::get_cached_forecast(&env, &player)
    }

    /// Get historical data for a player.
    pub fn get_player_history(env: Env, player: Address) -> Vec<YieldDataPoint> {
        yield_forecast::get_player_history(&env, &player)
    }

    /// Get number of historical data points for a player.
    pub fn get_history_count(env: Env, player: Address) -> u32 {
        yield_forecast::get_history_count(&env, &player)
    }

    /// Get current model parameters.
    pub fn get_model_params(env: Env) -> Option<ModelParams> {
        yield_forecast::get_model_params(&env)
    }

    /// Get current model version.
    pub fn get_model_version(env: Env) -> u32 {
        yield_forecast::get_model_version(&env)
    }

    /// Update model parameters (admin only).
    pub fn update_model_params(
        env: Env,
        admin: Address,
        moving_average_window: u32,
        trend_weight: u32,
        volatility_adjustment: u32,
    ) -> Result<ModelParams, ForecastError> {
        yield_forecast::update_model_params(&env, &admin, moving_average_window, trend_weight, volatility_adjustment)
    }

    // ─── Inter-Nebula Wormhole Travel System (Issue #77) ─────────────────────

    /// Open a new wormhole between two nebulae with verifiable travel link.
    pub fn open_wormhole(
        env: Env,
        creator: Address,
        origin_nebula: u64,
        destination: u64,
    ) -> Result<u64, WormholeError> {
        let result = wormhole_traveler::open_wormhole(&env, creator.clone(), origin_nebula, destination);
        if result.is_ok() {
            let mut details = [0u8; 128];
            details[0..8].copy_from_slice(&origin_nebula.to_be_bytes());
            details[8..16].copy_from_slice(&destination.to_be_bytes());
            let details_bytes = BytesN::from_array(&env, &details);
            let _ = audit_logger::log_audit_event(&env, Some(&creator), symbol_short!("ow"), details_bytes);
        }
        result
    }

    /// Traverse an existing wormhole with energy cost validation and state sync.
    pub fn traverse_wormhole(
        env: Env,
        traveler: Address,
        ship_id: u64,
        wormhole_id: u64,
    ) -> Result<TravelRecord, WormholeError> {
        let result = wormhole_traveler::traverse_wormhole(&env, traveler.clone(), ship_id, wormhole_id);
        if result.is_ok() {
            let mut details = [0u8; 128];
            details[0..8].copy_from_slice(&ship_id.to_be_bytes());
            details[8..16].copy_from_slice(&wormhole_id.to_be_bytes());
            let details_bytes = BytesN::from_array(&env, &details);
            let _ = audit_logger::log_audit_event(&env, Some(&traveler), symbol_short!("tw"), details_bytes);
        }
        result
    }

    /// Get wormhole details by ID.
    pub fn get_wormhole(env: Env, wormhole_id: u64) -> Option<Wormhole> {
        wormhole_traveler::get_wormhole(&env, wormhole_id)
    }

    /// Get all active wormholes.
    pub fn get_active_wormholes(env: Env) -> Vec<u64> {
        wormhole_traveler::get_active_wormholes(&env)
    }

    /// Get travel history for a ship.
    pub fn get_travel_history(env: Env, ship_id: u64) -> Vec<TravelRecord> {
        wormhole_traveler::get_travel_history(&env, ship_id)
    }

    /// Clean up expired wormholes (maintenance function).
    pub fn cleanup_expired_wormholes(env: Env) -> u32 {
        let cleaned = wormhole_traveler::cleanup_expired_wormholes(&env);
        let mut details = [0u8; 128];
        details[0..4].copy_from_slice(&cleaned.to_be_bytes());
        let details_bytes = BytesN::from_array(&env, &details);
        let _ = audit_logger::log_audit_event(&env, None, symbol_short!("cw"), details_bytes);
        cleaned
    }

    /// Calculate travel cost between two nebulae.
    pub fn calculate_travel_cost(env: Env, origin_nebula: u64, destination: u64) -> u32 {
        wormhole_traveler::calculate_travel_cost(origin_nebula, destination)
    }

    /// Verify wormhole link integrity.
    pub fn verify_wormhole_link(env: Env, wormhole_id: u64, provided_link: BytesN<32>) -> bool {
        wormhole_traveler::verify_wormhole_link(&env, wormhole_id, provided_link)
    }

    // ─── Player Alliance and Faction System (Issue #79) ──────────────────

    /// Found a new alliance with initial treasury.
    pub fn found_alliance(
        env: Env,
        founder: Address,
        name: String,
    ) -> Result<u64, AllianceError> {
        let result = alliance_manager::found_alliance(&env, founder.clone(), name);
        if result.is_ok() {
            let mut details = [0u8; 128];
            if let Ok(alliance_id) = result {
                details[0..8].copy_from_slice(&alliance_id.to_be_bytes());
            }
            let details_bytes = BytesN::from_array(&env, &details);
            let _ = audit_logger::log_audit_event(&env, Some(&founder), symbol_short!("fa"), details_bytes);
        }
        result
    }

    /// Join an existing alliance.
    pub fn join_alliance(
        env: Env,
        alliance_id: u64,
        player: Address,
    ) -> Result<MembershipRecord, AllianceError> {
        let result = alliance_manager::join_alliance(&env, alliance_id, player.clone());
        if result.is_ok() {
            let mut details = [0u8; 128];
            details[0..8].copy_from_slice(&alliance_id.to_be_bytes());
            let details_bytes = BytesN::from_array(&env, &details);
            let _ = audit_logger::log_audit_event(&env, Some(&player), symbol_short!("ja"), details_bytes);
        }
        result
    }

    /// Leave an alliance.
    pub fn leave_alliance(env: Env, player: Address) -> Result<(), AllianceError> {
        alliance_manager::leave_alliance(&env, player)
    }

    /// Contribute resources to alliance treasury.
    pub fn contribute_to_treasury(
        env: Env,
        player: Address,
        amount: i128,
    ) -> Result<i128, AllianceError> {
        alliance_manager::contribute_to_treasury(&env, player, amount)
    }

    /// Get alliance details.
    pub fn get_alliance(env: Env, alliance_id: u64) -> Result<Alliance, AllianceError> {
        alliance_manager::get_alliance(&env, alliance_id)
    }

    /// Get alliance treasury balance.
    pub fn get_alliance_treasury(env: Env, alliance_id: u64) -> i128 {
        alliance_manager::get_alliance_treasury(&env, alliance_id)
    }

    /// Get member's contribution to alliance.
    pub fn get_member_contribution(env: Env, alliance_id: u64, member: Address) -> i128 {
        alliance_manager::get_member_contribution(&env, alliance_id, member)
    }

    /// Get player's current alliance ID.
    pub fn get_player_alliance(env: Env, player: Address) -> Option<u64> {
        alliance_manager::get_player_alliance(&env, player)
    }

    // ─── Dynamic Resource Market Oracle Integration (Issue #78) ──────────

    /// Initialize the market oracle with admin and default sources.
    pub fn initialize_oracle(
        env: Env,
        admin: Address,
        sources: Vec<Address>,
    ) -> Result<(), MarketOracleError> {
        market_oracle::initialize_oracle(&env, admin, sources)
    }

    /// Update resource price with timestamp verification.
    pub fn update_resource_price(
        env: Env,
        admin: Address,
        resource: Symbol,
        new_price: i128,
    ) -> Result<PriceData, MarketOracleError> {
        market_oracle::update_resource_price(&env, admin, resource, new_price)
    }

    /// Batch update multiple resource prices.
    pub fn batch_update_prices(
        env: Env,
        admin: Address,
        resources: Vec<Symbol>,
        prices: Vec<i128>,
    ) -> Result<Vec<PriceData>, MarketOracleError> {
        market_oracle::batch_update_prices(&env, admin, resources, prices)
    }

    /// Get current market rate for a resource (pure view).
    pub fn get_current_market_rate(env: Env, resource: Symbol) -> Result<i128, MarketOracleError> {
        market_oracle::get_current_market_rate(&env, resource)
    }

    /// Get price data with metadata.
    pub fn get_price_data(env: Env, resource: Symbol) -> Result<PriceData, MarketOracleError> {
        market_oracle::get_price_data(&env, resource)
    }

    /// Get 24h price history for a resource.
    pub fn get_price_history(env: Env, resource: Symbol) -> Vec<PriceData> {
        market_oracle::get_price_history(&env, resource)
    }

    /// Add oracle source (admin only).
    pub fn add_oracle_source(
        env: Env,
        admin: Address,
        new_source: Address,
    ) -> Result<(), MarketOracleError> {
        market_oracle::add_oracle_source(&env, admin, new_source)
    }

    // ─── Procedural Music and Sound Seed Generator (Issue #80) ───────────

    /// Initialize default instrument presets.
    pub fn initialize_presets(env: Env) {
        audio_seed_generator::initialize_presets(&env)
    }

    /// Generate deterministic music seed from nebula state.
    pub fn generate_music_seed(env: Env, nebula_id: u64) -> Result<MusicSeed, AudioError> {
        audio_seed_generator::generate_music_seed(&env, nebula_id)
    }

    /// Get instrument layer parameters for frontend rendering.
    pub fn get_instrument_layer(
        env: Env,
        seed: BytesN<32>,
        layer: u32,
    ) -> Result<InstrumentParams, AudioError> {
        audio_seed_generator::get_instrument_layer(&env, seed, layer)
    }

    /// Get all 8 layers for a nebula instantly.
    pub fn get_all_layers(env: Env, nebula_id: u64) -> Result<Vec<InstrumentParams>, AudioError> {
        audio_seed_generator::get_all_layers(&env, nebula_id)
    }

    /// Get stored seed for a nebula.
    pub fn get_nebula_seed(env: Env, nebula_id: u64) -> Option<BytesN<32>> {
        audio_seed_generator::get_nebula_seed(&env, nebula_id)
    }

    /// Get instrument preset by ID.
    pub fn get_preset(env: Env, preset_id: u32) -> Result<InstrumentParams, AudioError> {
        audio_seed_generator::get_preset(&env, preset_id)
    }

    // ─── Privacy-Preserving Player Stats (Issue #XX) ─────────────────────

    /// Opt in to privacy-preserving stat sharing.
    pub fn opt_in_privacy(env: Env, player: Address) -> Result<(), PrivacyError> {
        privacy_stats::opt_in_privacy(&env, player)
    }

    /// Check if a player has opted in to privacy features.
    pub fn is_opted_in_privacy(env: Env, player: Address) -> bool {
        privacy_stats::is_opted_in(&env, &player)
    }

    /// Commit a private stat without revealing the raw value.
    pub fn commit_private_stat(
        env: Env,
        player: Address,
        stat_type: Symbol,
        value: i128,
    ) -> Result<BytesN<32>, PrivacyError> {
        privacy_stats::commit_private_stat(&env, player, stat_type, value)
    }

    /// Verify a private stat commitment using a zero-knowledge proof.
    pub fn verify_private_stat(
        env: Env,
        commitment: BytesN<32>,
        proof: BytesN<64>,
    ) -> Result<bool, PrivacyError> {
        privacy_stats::verify_private_stat(&env, commitment, proof)
    }

    /// Get a commitment for a player and stat type.
    pub fn get_commitment(
        env: Env,
        player: Address,
        stat_type: Symbol,
    ) -> Result<StatCommitment, PrivacyError> {
        privacy_stats::get_commitment(&env, player, stat_type)
    }

    /// Get total number of commitments made across all players.
    pub fn get_commitment_count(env: Env) -> u64 {
        privacy_stats::get_commitment_count(&env)
    }

    /// Batch commit multiple stats in a single transaction (up to 10).
    pub fn batch_commit_stats(
        env: Env,
        player: Address,
        stat_types: Vec<Symbol>,
        values: Vec<i128>,
    ) -> Result<Vec<BytesN<32>>, PrivacyError> {
        privacy_stats::batch_commit_stats(&env, player, stat_types, values)
    }

    /// Reset privacy burst counter for a new transaction.
    pub fn reset_privacy_burst_counter(env: Env) {
        privacy_stats::reset_burst_counter(&env)
    }

    // ─── Nebula Navigation Route Planner (Issue #69) ──────────────────────────

    /// Initialise the nebula navigation graph with an admin address.
    pub fn initialize_nav_graph(env: Env, admin: Address) -> Result<(), NavError> {
        navigation_planner::initialize_nav_graph(&env, &admin)
    }

    /// Register a directed edge (connection) between two nebulae.
    pub fn add_nebula_connection(
        env: Env,
        admin: Address,
        from: u64,
        to: u64,
        fuel_cost: u32,
        hazard_level: u32,
    ) -> Result<(), NavError> {
        navigation_planner::add_nebula_connection(&env, &admin, from, to, fuel_cost, hazard_level)
    }

    /// Add up to MAX_CONNECTIONS_PER_BATCH edges in a single transaction.
    pub fn add_nebula_connections_batch(
        env: Env,
        admin: Address,
        edges: Vec<RouteEdge>,
    ) -> Result<u32, NavError> {
        navigation_planner::add_nebula_connections_batch(&env, &admin, edges)
    }

    /// Dijkstra shortest-fuel-cost route between two nebulae (≤ 12 hops).
    /// Emits RouteCalculated event on success.
    pub fn calculate_optimal_route(
        env: Env,
        start: u64,
        dest: u64,
    ) -> Result<NavPath, NavError> {
        navigation_planner::calculate_optimal_route(&env, start, dest)
    }

    /// Validate a caller-supplied route Vec and return its aggregate risk score.
    pub fn validate_route_safety(env: Env, route: Vec<u64>) -> Result<u32, NavError> {
        navigation_planner::validate_route_safety(&env, route)
    }

    /// Return the adjacency list (outgoing edges) for a nebula.
    pub fn get_neighbors(env: Env, nebula_id: u64) -> Vec<RouteEdge> {
        navigation_planner::get_neighbors(&env, nebula_id)
    }

    /// Return the single directed edge from `from` to `to`, if it exists.
    pub fn get_nav_connection(env: Env, from: u64, to: u64) -> Option<RouteEdge> {
        navigation_planner::get_connection(&env, from, to)
    }

    // ─── Automated Community Event Scheduler ──────────────────────────────

    /// Initialize the event scheduler with an admin address.
    pub fn initialize_scheduler(env: Env, admin: Address) {
        event_scheduler::initialize_scheduler(&env, &admin)
    }

    /// Schedule a new community event.
    pub fn schedule_event(
        env: Env,
        admin: Address,
        event_type: Symbol,
        start_time: u64,
        reward_pool: i128,
    ) -> Result<u64, EventError> {
        event_scheduler::schedule_event(&env, admin, event_type, start_time, reward_pool)
    }

    /// Trigger a scheduled event when its time arrives.
    pub fn trigger_scheduled_event(
        env: Env,
        event_id: u64,
    ) -> Result<EventResult, EventError> {
        event_scheduler::trigger_scheduled_event(&env, event_id)
    }

    /// Get event details by ID.
    pub fn get_event(env: Env, event_id: u64) -> Result<ScheduledEvent, EventError> {
        event_scheduler::get_event(&env, event_id)
    }

    /// Get all active event IDs.
    pub fn get_active_events(env: Env) -> Vec<u64> {
        event_scheduler::get_active_events(&env)
    }

    /// Schedule a weekly nebula festival (template).
    pub fn schedule_weekly_festival(
        env: Env,
        admin: Address,
        reward_pool: i128,
    ) -> Result<u64, EventError> {
        event_scheduler::schedule_weekly_festival(&env, admin, reward_pool)
    }

    /// Cancel a scheduled event (admin only).
    pub fn cancel_event(
        env: Env,
        admin: Address,
        event_id: u64,
    ) -> Result<(), EventError> {
        event_scheduler::cancel_event(&env, admin, event_id)
    }

    /// Update event participant count.
    pub fn update_event_participants(
        env: Env,
        event_id: u64,
        participant_count: u32,
    ) -> Result<(), EventError> {
        event_scheduler::update_participants(&env, event_id, participant_count)
    }

    /// Get total number of events scheduled.
    pub fn get_event_count(env: Env) -> u64 {
        event_scheduler::get_event_count(&env)
    }

    /// Reset event burst counter.
    pub fn reset_event_burst_counter(env: Env) {
        event_scheduler::reset_burst_counter(&env)
    }

    // ─── Ship Customization & Skins ───────────────────────────────────────

    pub fn mint_skin(
        env: Env,
        owner: Address,
        name: Symbol,
        rarity: SkinRarity,
        color_primary: u32,
        color_secondary: u32,
        metadata: Bytes,
    ) -> Result<ShipSkin, SkinError> {
        ship_customization::mint_skin(&env, &owner, name, rarity, color_primary, color_secondary, metadata)
    }

    pub fn apply_skin(env: Env, owner: Address, ship_id: u64, skin_id: u64) -> Result<(), SkinError> {
        ship_customization::apply_skin(&env, &owner, ship_id, skin_id)
    }

    pub fn get_ship_skin(env: Env, ship_id: u64) -> Option<u64> {
        ship_customization::get_ship_skin(&env, ship_id)
    }

    pub fn get_owner_skins(env: Env, owner: Address) -> Vec<u64> {
        ship_customization::get_owner_skins(&env, &owner)
    }

    pub fn transfer_skin(env: Env, skin_id: u64, new_owner: Address) -> Result<ShipSkin, SkinError> {
        ship_customization::transfer_skin(&env, skin_id, &new_owner)
    }

    pub fn get_skin_templates(env: Env) -> Vec<SkinTemplate> {
        skins::get_skin_templates(&env)
    }

    // ─── Economic Monitoring & Balancing ──────────────────────────────────

    pub fn initialize_economic_monitor(env: Env, admin: Address) {
        economics::monitor::initialize_monitor(&env, &admin)
    }

    pub fn update_supply_metrics(
        env: Env,
        admin: Address,
        total_supply: i128,
        circulating_supply: i128,
        staked_supply: i128,
    ) {
        economics::monitor::update_supply_metrics(&env, &admin, total_supply, circulating_supply, staked_supply)
    }

    pub fn track_resource_activity(
        env: Env,
        resource_type: Symbol,
        minted: i128,
        burned: i128,
        avg_price: i128,
    ) {
        economics::monitor::track_resource_activity(&env, resource_type, minted, burned, avg_price)
    }

    pub fn get_economic_metrics(env: Env) -> EconomicMetrics {
        economics::monitor::get_metrics(&env)
    }

    pub fn get_resource_metrics(env: Env, resource_type: Symbol) -> ResourceMetrics {
        economics::monitor::get_resource_metrics(&env, resource_type)
    }

    pub fn calculate_inflation_rate(env: Env, old_supply: i128, new_supply: i128) -> u32 {
        economics::monitor::calculate_inflation_rate(&env, old_supply, new_supply)
    }

    pub fn detect_economic_imbalance(env: Env, resource_type: Symbol, supply: i128, demand: i128) -> SupplyDemandRatio {
        economics::balancer::detect_imbalance(&env, resource_type, supply, demand)
    }

    pub fn suggest_balance_adjustment(env: Env, resource_type: Symbol) -> Option<BalanceAdjustment> {
        economics::balancer::suggest_adjustment(&env, resource_type)
    }

    pub fn apply_balance_adjustment(
        env: Env,
        admin: Address,
        parameter: Symbol,
        new_value: i128,
        reason: Symbol,
    ) {
        economics::balancer::apply_adjustment(&env, &admin, parameter, new_value, reason)
    }

    pub fn generate_economic_report(env: Env) -> (i128, i128, i128) {
        economics::balancer::generate_report(&env)
    }

    // ─── Trading System ───────────────────────────────────────────────────

    pub fn place_limit_order(env: Env, trader: Address, order: LimitOrder) -> Result<u64, TradingError> {
        trading::place_limit_order(&env, &trader, order)
    }

    pub fn cancel_limit_order(env: Env, trader: Address, order_id: u64) -> Result<(), TradingError> {
        trading::cancel_limit_order(&env, &trader, order_id)
    }

    pub fn get_limit_order(env: Env, order_id: u64) -> Option<LimitOrder> {
        trading::get_limit_order(&env, order_id)
    }

    pub fn get_trader_orders(env: Env, trader: Address) -> Vec<LimitOrder> {
        trading::get_trader_orders(&env, &trader)
    }

    pub fn record_trade(env: Env, caller: Address, trade: TradeRecord) -> Result<(), TradingError> {
        trading::record_trade(&env, &caller, trade)
    }

    pub fn get_trading_history(env: Env) -> Vec<TradeRecord> {
        trading::get_trading_history(&env)
    }

    // ─── AMM — Automated Market Maker (Issue #189) ───────────────────────────

    /// Create a new liquidity pool.
    pub fn create_pool(
        env: Env,
        creator: Address,
        resource_a: Symbol,
        resource_b: Symbol,
    ) -> Result<u64, AmmError> {
        trading::create_pool(&env, &creator, resource_a, resource_b)
    }

    /// Add liquidity to a pool and receive LP tokens.
    pub fn add_liquidity(
        env: Env,
        provider: Address,
        pool_id: u64,
        amount_a: i128,
        amount_b: i128,
    ) -> Result<(i128, LiquidityPool), AmmError> {
        trading::add_liquidity(&env, &provider, pool_id, amount_a, amount_b)
    }

    /// Remove liquidity by burning LP tokens.
    pub fn remove_liquidity(
        env: Env,
        provider: Address,
        pool_id: u64,
        lp_amount: i128,
    ) -> Result<(i128, i128), AmmError> {
        trading::remove_liquidity(&env, &provider, pool_id, lp_amount)
    }

    /// Swap exact input for output via a pool route.
    pub fn swap_exact_input(
        env: Env,
        trader: Address,
        resource_in: Symbol,
        amount_in: i128,
        min_amount_out: i128,
        route: Vec<u64>,
    ) -> Result<i128, AmmError> {
        trading::swap_exact_input(&env, &trader, resource_in, amount_in, min_amount_out, route)
    }

    /// Get pool details by ID.
    pub fn get_pool(env: Env, pool_id: u64) -> Option<LiquidityPool> {
        trading::get_pool(&env, pool_id)
    }

    /// Get LP balance for a provider in a pool.
    pub fn get_lp_balance(env: Env, pool_id: u64, provider: Address) -> i128 {
        trading::get_lp_balance(&env, pool_id, &provider)
    }

    /// Get all pool IDs.
    pub fn get_all_pools(env: Env) -> Vec<u64> {
        trading::get_all_pools(&env)
    }

    /// Quote the output amount for a swap without executing.
    pub fn quote_swap(
        env: Env,
        pool_id: u64,
        resource_in: Symbol,
        amount_in: i128,
    ) -> Result<i128, AmmError> {
        trading::quote_swap(&env, pool_id, resource_in, amount_in)
    }

    // ── Mobile interface (Issue #199) ─────────────────────────────────────────

    /// Return a compact dashboard summary for the given player.
    pub fn get_mobile_dashboard(env: Env, player: Address) -> MobileDashboard {
        mobile_views::get_mobile_dashboard(&env, &player)
    }

    /// Return a lightweight scan-result estimate for a specific ship.
    pub fn get_quick_scan_preview(
        env: Env,
        ship_id: u64,
    ) -> Result<QuickScanPreview, MobileViewError> {
        mobile_views::get_quick_scan_preview(&env, ship_id)
    }

    /// Batch dashboard + primary-ship scan preview into a single RPC call.
    pub fn batch_get_mobile_info(env: Env, player: Address) -> MobileBatchInfo {
        mobile_views::batch_get_mobile_info(&env, &player)
    }

    /// Emit a mobile-event subscription marker for off-chain indexers.
    pub fn subscribe_mobile_events(env: Env, player: Address) {
        player.require_auth();
        mobile_views::subscribe_mobile_events(&env, &player);
    }

    // ── Reputation System (Issue #261) ────────────────────────────────────────

    pub fn initialize_reputation_system(env: Env, admin: Address) -> Result<(), ReputationError> {
        reputation::initialize_reputation(&env, &admin)
    }

    pub fn create_player_reputation(env: Env, player: Address) -> Result<(), ReputationError> {
        reputation::create_player_reputation(&env, &player)
    }

    pub fn get_player_reputation_score(env: Env, player: Address) -> Result<u32, ReputationError> {
        reputation::get_reputation_score(&env, &player)
    }

    pub fn get_player_reputation_details(env: Env, player: Address) -> Result<ReputationScore, ReputationError> {
        reputation::get_reputation_details(&env, &player)
    }

    pub fn record_player_behavior(
        env: Env,
        player: Address,
        behavior_type: BehaviorType,
        description: String,
        points: i32,
        reporter: Address,
    ) -> Result<(), ReputationError> {
        reputation::record_behavior(&env, &player, behavior_type, description, points, reporter)
    }

    pub fn submit_dispute_report(
        env: Env,
        reporter: Address,
        accused: Address,
        reason: String,
        evidence: String,
    ) -> Result<u64, ReputationError> {
        reputation::submit_report(&env, &reporter, &accused, reason, evidence)
    }

    pub fn resolve_dispute_report(
        env: Env,
        admin: Address,
        report_id: u64,
        resolved: bool,
    ) -> Result<(), ReputationError> {
        reputation::resolve_report(&env, &admin, report_id, resolved)
    }

    pub fn ban_player_account(env: Env, admin: Address, player: Address) -> Result<(), ReputationError> {
        reputation::ban_player(&env, &admin, &player)
    }

    pub fn check_player_ban_status(env: Env, player: Address) -> bool {
        reputation::is_player_banned(&env, &player)
    }

    pub fn get_player_behavior_history(env: Env, player: Address) -> Vec<BehaviorRecord> {
        reputation::get_player_history(&env, &player)
    }

    pub fn get_player_report_count(env: Env, player: Address) -> u32 {
        reputation::get_player_report_count(&env, &player)
    }

    pub fn get_all_dispute_reports(env: Env) -> Vec<DisputeReport> {
        reputation::get_all_reports(&env)
    }

    pub fn claim_reputation_reward(env: Env, player: Address) -> Result<i128, ReputationError> {
        reputation::claim_reputation_reward(&env, &player)
    }
}
