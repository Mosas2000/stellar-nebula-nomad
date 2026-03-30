use soroban_sdk::{contracterror, contract, contractimpl, contracttype, Address, Env};

// ── Constants ─────────────────────────────────────────────────────────────────

pub const LEDGERS_PER_DAY: u32 = 17_280;
const LEDGERS_PER_YEAR: i128 = (LEDGERS_PER_DAY as i128) * 365;
const BPS_DENOM: i128 = 10_000;

// ── Errors ────────────────────────────────────────────────────────────────────
use crate::ship_nft::{DataKey as ShipDataKey, ShipNft};
use crate::{CellType, NebulaLayout};
use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

pub type AssetId = Symbol;

const ASSET_STELLAR_DUST: Symbol = symbol_short!("dust");
const ASSET_ASTEROID_ORE: Symbol = symbol_short!("ore");
const ASSET_GAS_UNITS: Symbol = symbol_short!("gas");
const ASSET_DARK_MATTER: Symbol = symbol_short!("dark");
const ASSET_EXOTIC_MATTER: Symbol = symbol_short!("exotic");
const ASSET_WORMHOLE_CORE: Symbol = symbol_short!("worm");

#[derive(Clone)]
#[contracttype]
pub enum ResourceKey {
    ResourceCounter,
    HarvestCounter,
    DexOfferCounter,
    ResourceBalance(Address, AssetId),
    DexOffer(u64),
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ResourceError {
    AlreadyInitialized = 1,
    InsufficientResources = 2,
    InvalidAmount = 3,
    InvalidDuration = 4,
    AlreadyStaked = 5,
    TimeLockActive = 6,
    DailyCapExceeded = 7,
    NotInitialized = 8,
}

// ── Data Types ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ResourceType {
    Stardust,
    Plasma,
    Crystals,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct StakeRecord {
    pub amount: i128,
    pub resource_type: ResourceType,
    /// Ledger sequence at which the last yield claim was made.
    pub last_claim_ledger: u32,
    /// Ledger sequence after which unstaking is permitted.
    pub min_lock_ledger: u32,
pub enum HarvestError {
    ShipNotFound = 1,
    EmptyHarvest = 2,
    InvalidPrice = 3,
    AssetNotHarvested = 4,
    PriceOverflow = 5,
    DexFailure = 6,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Config {
    pub daily_harvest_cap: i128,
    pub apy_basis_points: u32,
    pub min_stake_duration: u32,
}

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    Config,
    /// Liquid balance: (player_address, resource_type) → i128
    Balance(Address, ResourceType),
    /// Active stake record for a player.
    Stake(Address),
    /// Harvested amount for a ship within a daily window: (ship_id, window) → i128
    DailyHarvest(u64, u32),
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct ResourceMinter;

#[contractimpl]
impl ResourceMinter {
    /// Initialise the contract.  Can only be called once.
    pub fn init(
        env: Env,
        admin: Address,
        _ship_registry: Address,
        _nebula_explorer: Address,
        apy_bps: u32,
        daily_cap: i128,
        min_stake_duration: u32,
    ) -> Result<(), ResourceError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(ResourceError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(
            &DataKey::Config,
            &Config {
                daily_harvest_cap: daily_cap,
                apy_basis_points: apy_bps,
                min_stake_duration,
            },
        );
        Ok(())
    }

    /// Harvest cosmic resources from a nebula anomaly.
    ///
    /// Returns the amount of Stardust minted.  The amount is:
    /// * `100 + anomaly_index * 10` if within the daily cap, or
    /// * the remaining cap allowance if the raw amount would exceed it.
    ///
    /// Returns `Err(DailyCapExceeded)` if the ship's daily cap is exhausted.
    pub fn harvest_resource(
        env: Env,
        player: Address,
        ship_id: u64,
        anomaly_index: u32,
    ) -> Result<i128, ResourceError> {
        player.require_auth();

        let config: Config = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(ResourceError::NotInitialized)?;

        let raw_amount = 100i128 + (anomaly_index as i128) * 10;
        let current_seq = env.ledger().sequence();
        let window = current_seq / LEDGERS_PER_DAY;

        let harvested: i128 = env
            .storage()
            .temporary()
            .get(&DataKey::DailyHarvest(ship_id, window))
            .unwrap_or(0);

        let remaining = config.daily_harvest_cap - harvested;
        if remaining <= 0 {
            return Err(ResourceError::DailyCapExceeded);
        }

        let amount = raw_amount.min(remaining);

        env.storage()
            .temporary()
            .set(&DataKey::DailyHarvest(ship_id, window), &(harvested + amount));

        let balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(player.clone(), ResourceType::Stardust))
            .unwrap_or(0);
        env.storage().persistent().set(
            &DataKey::Balance(player, ResourceType::Stardust),
            &(balance + amount),
        );

        Ok(amount)
    }

    /// Stake Stardust to earn Plasma yield at the configured APY.
    pub fn stake_for_yield(
        env: Env,
        player: Address,
        resource_type: ResourceType,
        amount: i128,
        duration: u32,
    ) -> Result<(), ResourceError> {
        player.require_auth();

        if amount <= 0 {
            return Err(ResourceError::InvalidAmount);
        }

        let config: Config = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(ResourceError::NotInitialized)?;

        if duration < config.min_stake_duration {
            return Err(ResourceError::InvalidDuration);
        }

        // Check not already staked.
        if env
            .storage()
            .persistent()
            .has(&DataKey::Stake(player.clone()))
        {
            return Err(ResourceError::AlreadyStaked);
        }

        // Deduct from liquid balance.
        let balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(player.clone(), resource_type.clone()))
            .unwrap_or(0);

        if balance < amount {
            return Err(ResourceError::InsufficientResources);
        }

        env.storage().persistent().set(
            &DataKey::Balance(player.clone(), resource_type.clone()),
            &(balance - amount),
        );

        let current_seq = env.ledger().sequence();
        env.storage().persistent().set(
            &DataKey::Stake(player),
            &StakeRecord {
                amount,
                resource_type,
                last_claim_ledger: current_seq,
                min_lock_ledger: current_seq + duration,
            },
        );

        Ok(())
    }

    /// Claim accrued Plasma yield from an active stake.
    pub fn claim_yield(env: Env, player: Address) -> i128 {
        player.require_auth();

        let mut stake: StakeRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Stake(player.clone()))
            .expect("no active stake");

        let config: Config = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .expect("not initialized");

        let current_seq = env.ledger().sequence();
        let yield_amount =
            Self::calculate_yield(&stake, current_seq, config.apy_basis_points);

        if yield_amount > 0 {
            let plasma: i128 = env
                .storage()
                .persistent()
                .get(&DataKey::Balance(player.clone(), ResourceType::Plasma))
                .unwrap_or(0);
            env.storage().persistent().set(
                &DataKey::Balance(player.clone(), ResourceType::Plasma),
                &(plasma + yield_amount),
            );

            stake.last_claim_ledger = current_seq;
            env.storage()
                .persistent()
                .set(&DataKey::Stake(player), &stake);
        }

        yield_amount
    }

    /// Unstake: return principal and auto-claim any residual yield.
    ///
    /// Returns the staked principal amount.  Errors if the time-lock has not
    /// yet expired.
    pub fn unstake(env: Env, player: Address) -> Result<i128, ResourceError> {
        player.require_auth();

        let stake: StakeRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Stake(player.clone()))
            .expect("no active stake");

        let current_seq = env.ledger().sequence();
        if current_seq < stake.min_lock_ledger {
            return Err(ResourceError::TimeLockActive);
        }

        // Auto-claim residual yield.
        let config: Config = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .expect("not initialized");

        let yield_amount = Self::calculate_yield(&stake, current_seq, config.apy_basis_points);
        if yield_amount > 0 {
            let plasma: i128 = env
                .storage()
                .persistent()
                .get(&DataKey::Balance(player.clone(), ResourceType::Plasma))
                .unwrap_or(0);
            env.storage().persistent().set(
                &DataKey::Balance(player.clone(), ResourceType::Plasma),
                &(plasma + yield_amount),
            );
        }

        // Return principal.
        let balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(player.clone(), stake.resource_type.clone()))
            .unwrap_or(0);
        env.storage().persistent().set(
            &DataKey::Balance(player.clone(), stake.resource_type),
            &(balance + stake.amount),
        );

        env.storage()
            .persistent()
            .remove(&DataKey::Stake(player));

        Ok(stake.amount)
    }

    // ── Read-only views ───────────────────────────────────────────────────────

    pub fn get_balance(env: Env, player: Address, resource_type: ResourceType) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(player, resource_type))
            .unwrap_or(0)
    }

    pub fn get_stake(env: Env, player: Address) -> Option<StakeRecord> {
        env.storage().persistent().get(&DataKey::Stake(player))
    }

    pub fn get_pending_yield(env: Env, player: Address) -> i128 {
        let stake: StakeRecord = match env.storage().persistent().get(&DataKey::Stake(player)) {
            Some(s) => s,
            None => return 0,
        };
        let config: Config = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .expect("not initialized");
        Self::calculate_yield(&stake, env.ledger().sequence(), config.apy_basis_points)
    }

    pub fn get_config(env: Env) -> Option<Config> {
        env.storage().instance().get(&DataKey::Config)
    }

    // ── Admin setters ─────────────────────────────────────────────────────────

    pub fn update_daily_cap(env: Env, cap: i128) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();
        let mut config: Config = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .expect("not initialized");
        config.daily_harvest_cap = cap;
        env.storage().instance().set(&DataKey::Config, &config);
    }

    pub fn update_apy(env: Env, apy_bps: u32) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();
        let mut config: Config = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .expect("not initialized");
        config.apy_basis_points = apy_bps;
        env.storage().instance().set(&DataKey::Config, &config);
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /// Pro-rated APY yield calculation.
    ///
    /// ```text
    /// yield = principal × apy_bps / 10_000 × elapsed_ledgers / LEDGERS_PER_YEAR
    /// ```
    ///
    /// Integer division truncates fractional cosmic essence — intentional to
    /// keep the contract deterministic and avoid rounding exploits.
    fn calculate_yield(stake: &StakeRecord, current_ledger: u32, apy_bps: u32) -> i128 {
        let elapsed = current_ledger.saturating_sub(stake.last_claim_ledger) as i128;
        if elapsed == 0 {
            return 0;
/// A single harvested resource entry.
#[derive(Clone)]
#[contracttype]
pub struct HarvestedResource {
    pub asset_id: AssetId,
    pub amount: u32,
}

/// Result of a harvest operation.
#[derive(Clone)]
#[contracttype]
pub struct HarvestResult {
    pub ship_id: u64,
    pub resources: Vec<HarvestedResource>,
    pub total_harvested: u32,
}

/// DEX offer for auto-listing harvested resources.
#[derive(Clone)]
#[contracttype]
pub struct DexOffer {
    pub offer_id: u64,
    pub asset_id: AssetId,
    pub amount: u32,
    pub min_price: i128,
    pub active: bool,
}

/// Map a CellType to its corresponding asset symbol.
fn cell_type_to_asset(cell_type: &CellType) -> Option<AssetId> {
    match cell_type {
        CellType::StellarDust => Some(ASSET_STELLAR_DUST),
        CellType::Asteroid => Some(ASSET_ASTEROID_ORE),
        CellType::GasCloud => Some(ASSET_GAS_UNITS),
        CellType::DarkMatter => Some(ASSET_DARK_MATTER),
        CellType::ExoticMatter => Some(ASSET_EXOTIC_MATTER),
        CellType::Wormhole => Some(ASSET_WORMHOLE_CORE),
        _ => None,
    }
}

#[allow(dead_code)]
fn next_harvest_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&ResourceKey::HarvestCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage()
        .instance()
        .set(&ResourceKey::HarvestCounter, &next);
    next
}

fn next_dex_offer_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&ResourceKey::DexOfferCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage()
        .instance()
        .set(&ResourceKey::DexOfferCounter, &next);
    next
}

/// Gas-optimized single-invocation harvest that scans a layout and
/// collects resources from non-empty cells.
pub fn harvest_resources(
    env: &Env,
    ship_id: u64,
    layout: &NebulaLayout,
) -> Result<HarvestResult, HarvestError> {
    // Verify the ship exists
    let _ship: ShipNft = env
        .storage()
        .persistent()
        .get(&ShipDataKey::Ship(ship_id))
        .ok_or(HarvestError::ShipNotFound)?;

    let mut resources = Vec::new(env);
    let mut total_harvested: u32 = 0;

    // Scan layout cells and harvest resources
    for i in 0..layout.cells.len() {
        if let Some(cell) = layout.cells.get(i) {
            if let Some(asset_id) = cell_type_to_asset(&cell.cell_type) {
                let amount = cell.energy;
                if amount > 0 {
                    resources.push_back(HarvestedResource {
                        asset_id: asset_id.clone(),
                        amount,
                    });
                    total_harvested += amount;

                    // Update balance
                    let key = ResourceKey::ResourceBalance(_ship.owner.clone(), asset_id.clone());
                    let balance: u32 = env.storage().instance().get(&key).unwrap_or(0);
                    env.storage().instance().set(&key, &(balance + amount));
                }
            }
        }
    }

    if total_harvested == 0 {
        return Err(HarvestError::EmptyHarvest);
    }

    env.events().publish(
        (symbol_short!("harvest"), symbol_short!("done")),
        (ship_id, total_harvested),
    );

    Ok(HarvestResult {
        ship_id,
        resources,
        total_harvested,
    })
}

/// Create an AMM-listing hook for a harvested resource.
pub fn auto_list_on_dex(
    env: &Env,
    resource: &AssetId,
    min_price: i128,
) -> Result<DexOffer, HarvestError> {
    if min_price <= 0 {
        return Err(HarvestError::InvalidPrice);
    }

    let offer_id = next_dex_offer_id(env);
    let offer = DexOffer {
        offer_id,
        asset_id: resource.clone(),
        amount: 0,
        min_price,
        active: true,
    };

    env.storage()
        .instance()
        .set(&ResourceKey::DexOffer(offer_id), &offer);

    env.events().publish(
        (symbol_short!("dex"), symbol_short!("listed")),
        (offer_id, resource.clone(), min_price),
    );

    Ok(offer)
}
