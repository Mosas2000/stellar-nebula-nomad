#![no_std]

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Vec};

mod analytics;
mod nebula_explorer;
pub mod resource_minter;
mod ship_registry;

pub use analytics::{AnalyticsError, GlobalStats, LeaderboardEntry};
use soroban_sdk::{contract, contractimpl, Address, Bytes, BytesN, Env, String, Symbol, Vec, symbol_short};

mod blueprint_factory;
mod gifting_system;
mod nebula_explorer;
mod player_profile;
mod referral_system;
mod resource_minter;
mod session_manager;
mod ship_nft;
mod ship_registry;

mod batch_processor;
mod dex_integration;
mod difficulty_scaler;
mod emergency_controls;
mod metadata_resolver;
mod randomness_oracle;
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

mod prize_distributor;
mod portal_registry;
mod constellation_mapper;
mod entanglement_comms;
mod wormhole_traveler;
mod alliance_manager;
mod market_oracle;
mod audio_seed_generator;
mod navigation_planner;

pub use nebula_explorer::{
    calculate_rarity_tier, compute_layout_hash, generate_nebula_layout, CellType, NebulaCell,
    NebulaLayout, Rarity, GRID_SIZE, TOTAL_CELLS,
};
pub use resource_minter::{ResourceError, ResourceType, StakeRecord, Config, LEDGERS_PER_DAY};
pub use resource_minter::{
    auto_list_on_dex, harvest_resources, AssetId, DexOffer, HarvestError, HarvestResult,
    HarvestedResource, Resource, ResourceKey,
};
pub use ship_nft::{ShipError, ShipNft};
pub use blueprint_factory::{Blueprint, BlueprintError, BlueprintRarity};
pub use referral_system::{Referral, ReferralError};
pub use player_profile::{PlayerProfile, ProfileError, ProgressUpdate};
pub use session_manager::{Session, SessionError};
pub use ship_registry::Ship;

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
pub use mobile_views::{get_mobile_dashboard, get_quick_scan_preview, MobileDashboard, MobileViewError, QuickScanPreview};
pub use ship_upgrade::{ShipState, ShipUpgradeError, UpgradeBlueprint};
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
    consume_energy, get_energy_balance, recharge_energy, EnergyBalance, EnergyError, RechargeResult,
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
pub use audit_logger::{AuditEntry, AuditLoggerError, get_audit_count, log_audit_event, query_audit_logs};
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
pub use navigation_planner::{
    initialize_nav_graph, add_nebula_connection, add_nebula_connections_batch,
    calculate_optimal_route, validate_route_safety, get_neighbors, get_connection,
    NavError, NavPath, RouteEdge, NavConfig, MAX_ROUTE_HOPS, MAX_CONNECTIONS_PER_BATCH,
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
    /// Also updates the on-chain analytics counters (total_scans,
    /// total_essence_accrued) and registers the player for the leaderboard.
    pub fn scan_nebula(
        env: Env,
        seed: BytesN<32>,
        player: Address,
    ) -> (NebulaLayout, Rarity) {
    /// Full scan: generates layout, calculates rarity, emits NebulaScanned event.
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
    ) -> Result<Vec<LeaderboardEntry>, AnalyticsError> {
        analytics::snapshot_leaderboard(&env, top_n)
    }
}
        (layout, rarity)
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

    /// Read a ship by ID.
    pub fn get_ship(env: Env, ship_id: u64) -> Result<ShipNft, ShipError> {
        ship_nft::get_ship(&env, ship_id)
    }

    /// Read all ship IDs owned by `owner`.
    pub fn get_ships_by_owner(env: Env, owner: Address) -> Vec<u64> {
        ship_nft::get_ships_by_owner(&env, &owner)
    }

    /// Gas-optimized harvest.
    pub fn harvest_resources(
        env: Env,
        ship_id: u64,
        layout: NebulaLayout,
    ) -> Result<HarvestResult, HarvestError> {
        resource_minter::harvest_resources(&env, ship_id, &layout)
    }

    /// Create an AMM-listing hook for a harvested resource.
    pub fn auto_list_on_dex(
        env: Env,
        resource: AssetId,
        min_price: i128,
    ) -> Result<DexOffer, HarvestError> {
        resource_minter::auto_list_on_dex(&env, &resource, min_price)
    }

    // ─── DEX Integration (Issue #9) ──────────────────────────────────────

    /// Harvest resources and immediately list on DEX.
    pub fn harvest_and_list(
        env: Env,
        player: Address,
        ship_id: u64,
        layout: NebulaLayout,
        resource: Symbol,
        min_price: i128,
    ) -> Result<(HarvestResult, DexOffer), HarvestError> {
        dex_integration::harvest_and_list(&env, &player, ship_id, &layout, &resource, min_price)
    }

    /// Cancel an active DEX listing.
    pub fn cancel_listing(
        env: Env,
        owner: Address,
        offer_id: u64,
    ) -> Result<DexOffer, HarvestError> {
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

    pub fn query_audit_logs(env: Env, filter: Symbol, limit: u32) -> Vec<AuditEntry> {
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
}
