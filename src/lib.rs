#![no_std]

use soroban_sdk::{contract, contractimpl, Address, Bytes, BytesN, Env, Symbol, Vec};

mod blueprint_factory;
mod nebula_explorer;
mod player_profile;
mod resource_minter;
mod session_manager;
mod ship_nft;
mod ship_registry;

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
pub use player_profile::{PlayerProfile, ProfileError, ProgressUpdate};
pub use session_manager::{Session, SessionError};
pub use ship_registry::Ship;

#[contract]
pub struct NebulaNomadContract;

#[contractimpl]
impl NebulaNomadContract {
    /// Generate a 16×16 procedural nebula map using ledger-seeded PRNG.
    ///
    /// Combines the supplied `seed` with on-chain ledger sequence and
    /// timestamp. The player must authorize the call.
    pub fn generate_nebula_layout(
        env: Env,
        seed: BytesN<32>,
        player: Address,
    ) -> NebulaLayout {
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
    pub fn scan_nebula(
        env: Env,
        seed: BytesN<32>,
        player: Address,
    ) -> (NebulaLayout, Rarity) {
        player.require_auth();

        let layout = nebula_explorer::generate_nebula_layout(&env, &seed, &player);
        let rarity = nebula_explorer::calculate_rarity_tier(&env, &layout);
        let layout_hash = nebula_explorer::compute_layout_hash(&env, &layout);

        nebula_explorer::emit_nebula_scanned(&env, &player, &layout_hash, &rarity);

        (layout, rarity)
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
}
