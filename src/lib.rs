#![no_std]

use soroban_sdk::{contract, contractimpl, Address, Bytes, BytesN, Env, String, Symbol, Vec};

mod blueprint_factory;
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

pub use nebula_explorer::{
    calculate_rarity_tier, compute_layout_hash, generate_nebula_layout, CellType, NebulaCell,
    NebulaLayout, Rarity, GRID_SIZE, TOTAL_CELLS,
};
pub use resource_minter::{
    auto_list_on_dex, harvest_resources, AssetId, DexOffer, HarvestError, HarvestResult,
    HarvestedResource, Resource,
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
pub use treasure_vault::{
    claim_treasure, deposit_treasure, get_vault, TreasureVault, VaultError,
    DEFAULT_MIN_LOCK_DURATION,
};
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

#[contract]
pub struct NebulaNomadContract;

#[contractimpl]
impl NebulaNomadContract {
    /// Generate a 16x16 procedural nebula map using ledger-seeded PRNG.
    ///
    /// Combines the supplied `seed` with on-chain ledger sequence and
    /// timestamp. The player must authorize the call.
    pub fn generate_nebula_layout(env: Env, seed: BytesN<32>, player: Address) -> NebulaLayout {
        player.require_auth();
        nebula_explorer::generate_nebula_layout(&env, &seed, &player)
    }

    /// Calculate the rarity tier of a nebula layout using on-chain
    /// verifiable math (no off-chain RNG).
    pub fn calculate_rarity_tier(env: Env, layout: NebulaLayout) -> Rarity {
        nebula_explorer::calculate_rarity_tier(&env, &layout)
    }

    /// Full scan: generates layout, calculates rarity, and emits a
    /// `NebulaScanned` event containing the layout hash.
    pub fn scan_nebula(env: Env, seed: BytesN<32>, player: Address) -> (NebulaLayout, Rarity) {
        player.require_auth();

        let layout = nebula_explorer::generate_nebula_layout(&env, &seed, &player);
        let rarity = nebula_explorer::calculate_rarity_tier(&env, &layout);
        let layout_hash = nebula_explorer::compute_layout_hash(&env, &layout);

        nebula_explorer::emit_nebula_scanned(&env, &player, &layout_hash, &rarity);

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
        bounty_board::post_bounty(&env, &poster, description, reward).unwrap()
    }

    pub fn claim_bounty(env: Env, claimer: Address, bounty_id: u64, proof: BytesN<32>) -> Bounty {
        bounty_board::claim_bounty(&env, &claimer, bounty_id, proof).unwrap()
    }

    // === Recycling/Crafting API ===

    pub fn initialize_recycling(env: Env) {
        recycling_crafter::initialize_recycling(&env);
    }

    pub fn recycle_resource(env: Env, caller: Address, resource: Symbol, amount: u32) -> Vec<(Symbol, u32)> {
        recycling_crafter::recycle_resource(&env, &caller, resource, amount).unwrap()
    }

    pub fn craft_new_item(env: Env, caller: Address, recipe_id: u64, inputs: Vec<Symbol>, quantities: Vec<u32>) -> CraftingResult {
        recycling_crafter::craft_new_item(&env, &caller, recipe_id, inputs, quantities).unwrap()
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
        ship_nft::mint_ship(&env, &owner, &ship_type, &metadata)
    }

    /// Batch-mint up to 3 ship NFTs in one transaction.
    pub fn batch_mint_ships(
        env: Env,
        owner: Address,
        ship_types: Vec<Symbol>,
        metadata: Bytes,
    ) -> Result<Vec<ShipNft>, ShipError> {
        ship_nft::batch_mint_ships(&env, &owner, &ship_types, &metadata)
    }

    /// Transfer ship ownership to `new_owner`.
    pub fn transfer_ownership(
        env: Env,
        ship_id: u64,
        new_owner: Address,
    ) -> Result<ShipNft, ShipError> {
        ship_nft::transfer_ownership(&env, ship_id, &new_owner)
    }

    /// Read a ship by ID.
    pub fn get_ship(env: Env, ship_id: u64) -> Result<ShipNft, ShipError> {
        ship_nft::get_ship(&env, ship_id)
    }

    /// Read all ship IDs owned by `owner`.
    pub fn get_ships_by_owner(env: Env, owner: Address) -> Vec<u64> {
        ship_nft::get_ships_by_owner(&env, &owner)
    }

    /// Gas-optimized single-invocation harvest that updates balances,
    /// emits harvest telemetry, and creates an auto-list offer hook.
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

    // ─── Treasure Vault (Issue #18) ──────────────────────────────────────

    /// Deposit resources into a time-locked treasure vault.
    pub fn deposit_treasure(
        env: Env,
        owner: Address,
        ship_id: u64,
        amount: u64,
    ) -> Result<TreasureVault, VaultError> {
        treasure_vault::deposit_treasure(&env, &owner, ship_id, amount)
    }

    /// Claim a treasure vault after the lock period expires.
    pub fn claim_treasure(env: Env, owner: Address, vault_id: u64) -> Result<u64, VaultError> {
        treasure_vault::claim_treasure(&env, &owner, vault_id)
    }

    /// Read a vault by ID.
    pub fn get_vault(env: Env, vault_id: u64) -> Option<TreasureVault> {
        treasure_vault::get_vault(&env, vault_id)
    }

    // ─── Difficulty Scaling (Issue #26) ──────────────────────────────────

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

    // ─── Randomness Oracle (Issue #28) ───────────────────────────────────

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

    /// Create a new on-chain player profile. Returns the assigned profile ID.
    pub fn initialize_profile(env: Env, owner: Address) -> Result<u64, ProfileError> {
        player_profile::initialize_profile(&env, owner)
    }

    /// Update scan count and essence earned after a harvest. Owner-only.
    pub fn update_progress(
        env: Env,
        caller: Address,
        profile_id: u64,
        scan_count: u32,
        essence: i128,
    ) -> Result<(), ProfileError> {
        player_profile::update_progress(&env, caller, profile_id, scan_count, essence)
    }

    /// Apply up to 5 stat updates in a single transaction for multi-scan runs.
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

    // ─── Session Manager ──────────────────────────────────────────────────────

    /// Start a timed nebula exploration session for a ship. Max 3 per player.
    pub fn start_session(env: Env, owner: Address, ship_id: u64) -> Result<u64, SessionError> {
        session_manager::start_session(&env, owner, ship_id)
    }

    /// Close a session. Owner can force-close; anyone can clean up expired ones.
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

    // ─── Blueprint Factory ────────────────────────────────────────────────────

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

    // ─── Referral System ──────────────────────────────────────────────────────

    /// Record an on-chain referral from `referrer` for `new_nomad`.
    pub fn register_referral(
        env: Env,
        referrer: Address,
        new_nomad: Address,
    ) -> Result<u64, ReferralError> {
        referral_system::register_referral(&env, referrer, new_nomad)
    }

    /// Mark that `nomad` has completed their first scan, unlocking the reward.
    pub fn mark_first_scan(env: Env, nomad: Address) -> Result<(), ReferralError> {
        referral_system::mark_first_scan(&env, nomad)
    }

    /// Claim the essence referral reward. One-time, capped at 10 claims/day.
    pub fn claim_referral_reward(
        env: Env,
        referrer: Address,
        new_nomad: Address,
    ) -> Result<i128, ReferralError> {
        referral_system::claim_referral_reward(&env, referrer, new_nomad)
    }

    /// Retrieve a referral record by the new nomad's address.
    pub fn get_referral(env: Env, new_nomad: Address) -> Result<Referral, ReferralError> {
        referral_system::get_referral(&env, new_nomad)
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
}
