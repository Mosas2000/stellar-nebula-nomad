//! NFT Marketplace integration — Issues #130 and #283
//!
//! Enables ship NFTs to be listed, purchased, and delisted on-chain.
//! Enforces a configurable royalty paid to the original minter on every sale.
//! Emits events compatible with off-chain marketplace indexers.
//!
//! Issue #283 adds a parallel **cosmetic** market for skin NFTs. It is kept
//! separate from the ship market rather than generalised over both, because the
//! two have different rules: cosmetics carry a creator royalty that follows the
//! item forever, are floor-priced by rarity (see
//! [`crate::skins::rarity_floor_price`]), and are escrowed while listed. None of
//! that applies to ships.
//!
//! Cosmetics are strictly non-functional — a skin changes colours and effect
//! layers only (see [`crate::skins::SkinPreview`]) and never ship stats — so the
//! market is revenue without pay-to-win.
//!
//! Settlement follows the same convention as the ship market above: prices,
//! fees and royalties are computed and recorded on-chain and published as
//! events; the value transfer itself is performed by the payment rail that
//! consumes those events.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env};

use crate::ship_customization::{self, SkinError, SkinRarity};
use crate::skins::{self, SkinPreview};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Royalty basis points paid to the creator on every secondary sale (5 %).
pub const ROYALTY_BPS: i128 = 500;
/// Maximum number of active listings per seller.
pub const MAX_LISTINGS_PER_SELLER: u32 = 20;

/// Basis-point denominator.
pub const BPS_DENOMINATOR: i128 = 10_000;
/// Platform fee taken on every cosmetic sale (2.5 %).
pub const PLATFORM_FEE_BPS: i128 = 250;
/// Creator royalty applied when a cosmetic's creator has not registered a
/// custom rate (5 %).
pub const CREATOR_ROYALTY_BPS_DEFAULT: i128 = 500;
/// Ceiling on a creator-chosen royalty (20 %), so a creator cannot price their
/// own cosmetics out of the secondary market.
pub const MAX_CREATOR_ROYALTY_BPS: i128 = 2_000;
/// Maximum active cosmetic listings per seller.
pub const MAX_COSMETIC_LISTINGS_PER_SELLER: u32 = 50;

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum MarketplaceKey {
    /// Listing keyed by ship_id.
    Listing(u64),
    /// Number of active listings per seller.
    SellerCount(Address),
    /// Total volume traded (sum of sale prices).
    TotalVolume,
    // ── Cosmetic market (Issue #283) ──────────────────────────────────────
    /// Cosmetic listing keyed by skin_id.
    CosmeticListing(u64),
    /// Number of active cosmetic listings per seller.
    CosmeticSellerCount(Address),
    /// Registered creator royalty for a skin_id.
    CreatorRoyalty(u64),
    /// Royalties accrued to a creator across all of their cosmetics.
    CreatorEarnings(Address),
    /// Lifetime royalties withdrawn by a creator.
    CreatorWithdrawn(Address),
    /// Cumulative cosmetic sale volume.
    CosmeticVolume,
    /// Number of cosmetic sales settled.
    CosmeticSales,
    /// Cosmetic listings currently open.
    CosmeticActiveListings,
    /// Cumulative creator royalties credited.
    CosmeticRoyaltiesPaid,
    /// Cumulative platform fees taken.
    CosmeticFeesCollected,
}

// ── Types ──────────────────────────────────────────────────────────────────────

/// An active marketplace listing for a ship NFT.
#[derive(Clone)]
#[contracttype]
pub struct Listing {
    pub ship_id: u64,
    pub seller: Address,
    /// Sale price in stroops.
    pub price: i128,
    pub listed_at: u64,
}

/// A creator's perpetual royalty claim on one cosmetic.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct CreatorRoyalty {
    pub skin_id: u64,
    pub creator: Address,
    /// Royalty rate in basis points, at most [`MAX_CREATOR_ROYALTY_BPS`].
    pub bps: i128,
    pub registered_at: u64,
}

/// An active cosmetic (skin NFT) listing.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct CosmeticListing {
    pub skin_id: u64,
    pub seller: Address,
    /// Sale price in stroops; never below the rarity floor.
    pub price: i128,
    pub rarity: SkinRarity,
    /// Registered creator, when the cosmetic has one.
    pub creator: Option<Address>,
    /// Royalty rate that will be applied on sale.
    pub royalty_bps: i128,
    pub listed_at: u64,
}

/// Aggregate cosmetic-market statistics.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CosmeticMarketStats {
    pub total_volume: i128,
    pub sales_count: u64,
    pub active_listings: u32,
    pub creator_royalties_paid: i128,
    pub platform_fees_collected: i128,
}

/// How a cosmetic's sale price is split.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SaleSplit {
    pub price: i128,
    pub creator_royalty: i128,
    pub platform_fee: i128,
    pub seller_proceeds: i128,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MarketplaceError {
    AlreadyListed = 1,
    NotListed = 2,
    NotSeller = 3,
    InvalidPrice = 4,
    SellerListingCapReached = 5,
    SelfPurchase = 6,
    // ── Cosmetic market (Issue #283) ──────────────────────────────────────
    /// No skin exists with the given ID.
    SkinNotFound = 7,
    /// The seller does not own the skin they are listing.
    NotSkinOwner = 8,
    /// The skin is escrowed by another listing or otherwise locked.
    SkinNotTradeable = 9,
    /// Price is below the floor for the cosmetic's rarity.
    PriceBelowRarityFloor = 10,
    /// Requested royalty exceeds [`MAX_CREATOR_ROYALTY_BPS`].
    RoyaltyTooHigh = 11,
    /// A royalty is already registered for this cosmetic.
    RoyaltyAlreadyRegistered = 12,
    /// The creator has no unwithdrawn royalties.
    NothingToWithdraw = 13,
    /// Fee or royalty accounting overflowed.
    ArithmeticOverflow = 14,
}

impl From<SkinError> for MarketplaceError {
    fn from(e: SkinError) -> Self {
        match e {
            SkinError::NotOwner => MarketplaceError::NotSkinOwner,
            _ => MarketplaceError::SkinNotFound,
        }
    }
}

// ── Functions ─────────────────────────────────────────────────────────────────

/// List a ship NFT for sale at `price` stroops.
///
/// The seller authorizes the call. Emits `ShipListed`.
pub fn list_ship(
    env: &Env,
    seller: &Address,
    ship_id: u64,
    price: i128,
) -> Result<(), MarketplaceError> {
    seller.require_auth();

    if price <= 0 {
        return Err(MarketplaceError::InvalidPrice);
    }
    if env.storage().persistent().has(&MarketplaceKey::Listing(ship_id)) {
        return Err(MarketplaceError::AlreadyListed);
    }

    let count: u32 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::SellerCount(seller.clone()))
        .unwrap_or(0);
    if count >= MAX_LISTINGS_PER_SELLER {
        return Err(MarketplaceError::SellerListingCapReached);
    }

    let listing = Listing {
        ship_id,
        seller: seller.clone(),
        price,
        listed_at: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&MarketplaceKey::Listing(ship_id), &listing);
    env.storage()
        .persistent()
        .set(&MarketplaceKey::SellerCount(seller.clone()), &(count + 1));

    env.events().publish(
        (symbol_short!("market"), symbol_short!("listed")),
        (seller.clone(), ship_id, price),
    );

    Ok(())
}

/// Purchase a listed ship NFT.
///
/// Enforces royalty payment to the original listing seller's platform share.
/// Emits `ShipSold` with buyer, seller, ship_id, price, and royalty.
pub fn buy_ship(env: &Env, buyer: &Address, ship_id: u64) -> Result<(), MarketplaceError> {
    buyer.require_auth();

    let listing: Listing = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::Listing(ship_id))
        .ok_or(MarketplaceError::NotListed)?;

    if &listing.seller == buyer {
        return Err(MarketplaceError::SelfPurchase);
    }

    let royalty = (listing.price * ROYALTY_BPS) / 10_000;
    let seller_proceeds = listing.price - royalty;

    // Accumulate total traded volume
    let volume: i128 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::TotalVolume)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&MarketplaceKey::TotalVolume, &(volume + listing.price));

    // Remove listing and decrement seller count
    env.storage()
        .persistent()
        .remove(&MarketplaceKey::Listing(ship_id));
    let count: u32 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::SellerCount(listing.seller.clone()))
        .unwrap_or(1);
    env.storage()
        .persistent()
        .set(&MarketplaceKey::SellerCount(listing.seller.clone()), &count.saturating_sub(1));

    env.events().publish(
        (symbol_short!("market"), symbol_short!("sold")),
        (buyer.clone(), listing.seller.clone(), ship_id, listing.price, royalty, seller_proceeds),
    );

    Ok(())
}

/// Cancel an active listing (seller only). Emits `ListingCancelled`.
pub fn cancel_listing(env: &Env, seller: &Address, ship_id: u64) -> Result<(), MarketplaceError> {
    seller.require_auth();

    let listing: Listing = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::Listing(ship_id))
        .ok_or(MarketplaceError::NotListed)?;

    if &listing.seller != seller {
        return Err(MarketplaceError::NotSeller);
    }

    env.storage()
        .persistent()
        .remove(&MarketplaceKey::Listing(ship_id));
    let count: u32 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::SellerCount(seller.clone()))
        .unwrap_or(1);
    env.storage()
        .persistent()
        .set(&MarketplaceKey::SellerCount(seller.clone()), &count.saturating_sub(1));

    env.events().publish(
        (symbol_short!("market"), symbol_short!("cancel")),
        (seller.clone(), ship_id),
    );

    Ok(())
}

/// Get the active listing for `ship_id`, if any.
pub fn get_listing(env: &Env, ship_id: u64) -> Option<Listing> {
    env.storage()
        .persistent()
        .get(&MarketplaceKey::Listing(ship_id))
}

/// Return total marketplace trading volume.
pub fn get_total_volume(env: &Env) -> i128 {
    env.storage()
        .persistent()
        .get(&MarketplaceKey::TotalVolume)
        .unwrap_or(0)
}

// ═══ Cosmetic NFT marketplace (Issue #283) ════════════════════════════════════

// ── Creator royalties ─────────────────────────────────────────────────────────

/// Register a perpetual royalty on `skin_id` in favour of `creator`.
///
/// Only the cosmetic's current owner may register — in practice its minter,
/// before the first sale — and only once, so a later holder cannot redirect the
/// royalty stream to themselves. Passing `bps` of 0 opts out of royalties
/// entirely; omit the registration to accept
/// [`CREATOR_ROYALTY_BPS_DEFAULT`] with no named creator.
pub fn register_creator_royalty(
    env: &Env,
    creator: &Address,
    skin_id: u64,
    bps: i128,
) -> Result<CreatorRoyalty, MarketplaceError> {
    creator.require_auth();

    if bps < 0 || bps > MAX_CREATOR_ROYALTY_BPS {
        return Err(MarketplaceError::RoyaltyTooHigh);
    }
    if env
        .storage()
        .persistent()
        .has(&MarketplaceKey::CreatorRoyalty(skin_id))
    {
        return Err(MarketplaceError::RoyaltyAlreadyRegistered);
    }

    let skin = ship_customization::get_skin(env, skin_id).ok_or(MarketplaceError::SkinNotFound)?;
    if skin.owner != *creator {
        return Err(MarketplaceError::NotSkinOwner);
    }

    let royalty = CreatorRoyalty {
        skin_id,
        creator: creator.clone(),
        bps,
        registered_at: env.ledger().timestamp(),
    };
    env.storage()
        .persistent()
        .set(&MarketplaceKey::CreatorRoyalty(skin_id), &royalty);

    env.events().publish(
        (symbol_short!("cosmetic"), symbol_short!("royalty")),
        (creator.clone(), skin_id, bps),
    );

    Ok(royalty)
}

/// The registered royalty for `skin_id`, if any.
pub fn get_creator_royalty(env: &Env, skin_id: u64) -> Option<CreatorRoyalty> {
    env.storage()
        .persistent()
        .get(&MarketplaceKey::CreatorRoyalty(skin_id))
}

/// Royalties credited to `creator` and not yet withdrawn.
pub fn get_creator_earnings(env: &Env, creator: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&MarketplaceKey::CreatorEarnings(creator.clone()))
        .unwrap_or(0)
}

/// Total royalties `creator` has withdrawn over the marketplace's lifetime.
pub fn get_creator_withdrawn(env: &Env, creator: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&MarketplaceKey::CreatorWithdrawn(creator.clone()))
        .unwrap_or(0)
}

/// Withdraw all accrued royalties for `creator`, returning the amount.
///
/// Zeroes the balance before emitting, so a re-entrant call finds nothing left
/// to withdraw.
pub fn withdraw_creator_earnings(
    env: &Env,
    creator: &Address,
) -> Result<i128, MarketplaceError> {
    creator.require_auth();

    let owed = get_creator_earnings(env, creator);
    if owed <= 0 {
        return Err(MarketplaceError::NothingToWithdraw);
    }

    env.storage()
        .persistent()
        .set(&MarketplaceKey::CreatorEarnings(creator.clone()), &0i128);

    let withdrawn = get_creator_withdrawn(env, creator)
        .checked_add(owed)
        .ok_or(MarketplaceError::ArithmeticOverflow)?;
    env.storage().persistent().set(
        &MarketplaceKey::CreatorWithdrawn(creator.clone()),
        &withdrawn,
    );

    env.events().publish(
        (symbol_short!("cosmetic"), symbol_short!("withdraw")),
        (creator.clone(), owed),
    );

    Ok(owed)
}

// ── Price splitting ───────────────────────────────────────────────────────────

/// Split `price` into creator royalty, platform fee and seller proceeds.
///
/// Pure — clients call it to show a seller exactly what they will net before
/// the listing is created.
pub fn compute_sale_split(price: i128, royalty_bps: i128) -> Result<SaleSplit, MarketplaceError> {
    if price <= 0 {
        return Err(MarketplaceError::InvalidPrice);
    }

    let creator_royalty = price
        .checked_mul(royalty_bps)
        .ok_or(MarketplaceError::ArithmeticOverflow)?
        / BPS_DENOMINATOR;
    let platform_fee = price
        .checked_mul(PLATFORM_FEE_BPS)
        .ok_or(MarketplaceError::ArithmeticOverflow)?
        / BPS_DENOMINATOR;

    // royalty_bps + PLATFORM_FEE_BPS <= 2_250 < 10_000, so the seller's share
    // is always positive.
    let seller_proceeds = price - creator_royalty - platform_fee;

    Ok(SaleSplit {
        price,
        creator_royalty,
        platform_fee,
        seller_proceeds,
    })
}

/// Resolve the creator and royalty rate that apply to `skin_id`.
fn resolve_royalty(env: &Env, skin_id: u64) -> (Option<Address>, i128) {
    match get_creator_royalty(env, skin_id) {
        Some(royalty) => (Some(royalty.creator), royalty.bps),
        None => (None, CREATOR_ROYALTY_BPS_DEFAULT),
    }
}

// ── Listing ───────────────────────────────────────────────────────────────────

/// List a cosmetic skin NFT for sale.
///
/// Requires the seller to own the skin and the skin to be unlocked. The price
/// must be at or above the rarity floor. The skin is escrowed — its `tradeable`
/// flag is cleared — for as long as the listing is open.
pub fn list_cosmetic(
    env: &Env,
    seller: &Address,
    skin_id: u64,
    price: i128,
) -> Result<CosmeticListing, MarketplaceError> {
    seller.require_auth();

    if price <= 0 {
        return Err(MarketplaceError::InvalidPrice);
    }
    if env
        .storage()
        .persistent()
        .has(&MarketplaceKey::CosmeticListing(skin_id))
    {
        return Err(MarketplaceError::AlreadyListed);
    }

    let skin = ship_customization::get_skin(env, skin_id).ok_or(MarketplaceError::SkinNotFound)?;
    if skin.owner != *seller {
        return Err(MarketplaceError::NotSkinOwner);
    }
    if !skin.tradeable {
        return Err(MarketplaceError::SkinNotTradeable);
    }
    if price < skins::rarity_floor_price(&skin.rarity) {
        return Err(MarketplaceError::PriceBelowRarityFloor);
    }

    let count: u32 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::CosmeticSellerCount(seller.clone()))
        .unwrap_or(0);
    if count >= MAX_COSMETIC_LISTINGS_PER_SELLER {
        return Err(MarketplaceError::SellerListingCapReached);
    }

    let (creator, royalty_bps) = resolve_royalty(env, skin_id);
    let listing = CosmeticListing {
        skin_id,
        seller: seller.clone(),
        price,
        rarity: skin.rarity.clone(),
        creator,
        royalty_bps,
        listed_at: env.ledger().timestamp(),
    };

    // Escrow the cosmetic so it cannot be transferred while listed.
    ship_customization::set_tradeable(env, skin_id, false)?;

    env.storage()
        .persistent()
        .set(&MarketplaceKey::CosmeticListing(skin_id), &listing);
    env.storage().persistent().set(
        &MarketplaceKey::CosmeticSellerCount(seller.clone()),
        &count.saturating_add(1),
    );
    bump_active_listings(env, 1);

    env.events().publish(
        (symbol_short!("cosmetic"), symbol_short!("listed")),
        (seller.clone(), skin_id, price, skin.rarity),
    );

    Ok(listing)
}

/// Purchase a listed cosmetic.
///
/// Transfers the skin to `buyer`, credits the creator's royalty balance, and
/// records the platform fee and volume. Returns the settled split.
pub fn buy_cosmetic(
    env: &Env,
    buyer: &Address,
    skin_id: u64,
) -> Result<SaleSplit, MarketplaceError> {
    buyer.require_auth();

    let listing: CosmeticListing = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::CosmeticListing(skin_id))
        .ok_or(MarketplaceError::NotListed)?;

    if &listing.seller == buyer {
        return Err(MarketplaceError::SelfPurchase);
    }

    let split = compute_sale_split(listing.price, listing.royalty_bps)?;

    // Hand over the cosmetic and release it from escrow.
    ship_customization::transfer_skin_internal(env, skin_id, buyer)?;
    ship_customization::set_tradeable(env, skin_id, true)?;

    // ── Creator royalty ───────────────────────────────────────────────────
    // Only a registered creator can be credited; without one the royalty share
    // stays with the seller rather than accruing to nobody.
    let creator_royalty = match &listing.creator {
        Some(creator) if split.creator_royalty > 0 => {
            let earnings = get_creator_earnings(env, creator)
                .checked_add(split.creator_royalty)
                .ok_or(MarketplaceError::ArithmeticOverflow)?;
            env.storage()
                .persistent()
                .set(&MarketplaceKey::CreatorEarnings(creator.clone()), &earnings);
            bump_i128(env, MarketplaceKey::CosmeticRoyaltiesPaid, split.creator_royalty)?;
            split.creator_royalty
        }
        _ => 0,
    };
    let settled = SaleSplit {
        price: split.price,
        creator_royalty,
        platform_fee: split.platform_fee,
        seller_proceeds: split.price - creator_royalty - split.platform_fee,
    };

    // ── Bookkeeping ───────────────────────────────────────────────────────
    bump_i128(env, MarketplaceKey::CosmeticVolume, settled.price)?;
    bump_i128(
        env,
        MarketplaceKey::CosmeticFeesCollected,
        settled.platform_fee,
    )?;
    let sales: u64 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::CosmeticSales)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&MarketplaceKey::CosmeticSales, &sales.saturating_add(1));

    close_listing(env, &listing);

    env.events().publish(
        (symbol_short!("cosmetic"), symbol_short!("sold")),
        (
            buyer.clone(),
            listing.seller.clone(),
            skin_id,
            settled.price,
            settled.creator_royalty,
            settled.platform_fee,
            settled.seller_proceeds,
        ),
    );

    Ok(settled)
}

/// Cancel an open cosmetic listing, releasing the skin from escrow.
pub fn cancel_cosmetic_listing(
    env: &Env,
    seller: &Address,
    skin_id: u64,
) -> Result<(), MarketplaceError> {
    seller.require_auth();

    let listing: CosmeticListing = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::CosmeticListing(skin_id))
        .ok_or(MarketplaceError::NotListed)?;

    if &listing.seller != seller {
        return Err(MarketplaceError::NotSeller);
    }

    ship_customization::set_tradeable(env, skin_id, true)?;
    close_listing(env, &listing);

    env.events().publish(
        (symbol_short!("cosmetic"), symbol_short!("cancel")),
        (seller.clone(), skin_id),
    );

    Ok(())
}

/// Remove a listing and decrement the seller's and global counters.
fn close_listing(env: &Env, listing: &CosmeticListing) {
    env.storage()
        .persistent()
        .remove(&MarketplaceKey::CosmeticListing(listing.skin_id));

    let count: u32 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::CosmeticSellerCount(listing.seller.clone()))
        .unwrap_or(1);
    env.storage().persistent().set(
        &MarketplaceKey::CosmeticSellerCount(listing.seller.clone()),
        &count.saturating_sub(1),
    );

    bump_active_listings(env, -1);
}

fn bump_active_listings(env: &Env, delta: i32) {
    let current: u32 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::CosmeticActiveListings)
        .unwrap_or(0);
    let updated = if delta >= 0 {
        current.saturating_add(delta as u32)
    } else {
        current.saturating_sub((-delta) as u32)
    };
    env.storage()
        .persistent()
        .set(&MarketplaceKey::CosmeticActiveListings, &updated);
}

fn bump_i128(env: &Env, key: MarketplaceKey, delta: i128) -> Result<(), MarketplaceError> {
    let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
    let updated = current
        .checked_add(delta)
        .ok_or(MarketplaceError::ArithmeticOverflow)?;
    env.storage().persistent().set(&key, &updated);
    Ok(())
}

// ── Queries ───────────────────────────────────────────────────────────────────

/// The open cosmetic listing for `skin_id`, if any.
pub fn get_cosmetic_listing(env: &Env, skin_id: u64) -> Option<CosmeticListing> {
    env.storage()
        .persistent()
        .get(&MarketplaceKey::CosmeticListing(skin_id))
}

/// Render a listed cosmetic for a storefront card, without owning it.
pub fn preview_cosmetic_listing(env: &Env, skin_id: u64) -> Option<SkinPreview> {
    let skin = ship_customization::get_skin(env, skin_id)?;
    Some(skins::preview_skin(env, &skin))
}

/// Aggregate cosmetic-market statistics.
pub fn get_cosmetic_market_stats(env: &Env) -> CosmeticMarketStats {
    CosmeticMarketStats {
        total_volume: env
            .storage()
            .persistent()
            .get(&MarketplaceKey::CosmeticVolume)
            .unwrap_or(0),
        sales_count: env
            .storage()
            .persistent()
            .get(&MarketplaceKey::CosmeticSales)
            .unwrap_or(0),
        active_listings: env
            .storage()
            .persistent()
            .get(&MarketplaceKey::CosmeticActiveListings)
            .unwrap_or(0),
        creator_royalties_paid: env
            .storage()
            .persistent()
            .get(&MarketplaceKey::CosmeticRoyaltiesPaid)
            .unwrap_or(0),
        platform_fees_collected: env
            .storage()
            .persistent()
            .get(&MarketplaceKey::CosmeticFeesCollected)
            .unwrap_or(0),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests — cosmetic marketplace (Issue #283)
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod cosmetic_tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, testutils::Address as _, Bytes};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    /// Listings, escrow and royalty balances all live in contract storage, so
    /// every test body runs through `Env::as_contract`. Each `run` is its own
    /// invocation frame, which also mirrors reality: listing, buying and
    /// cancelling are separate transactions, and `require_auth` may only be
    /// satisfied once per frame.
    struct Market {
        env: Env,
        contract: Address,
        seller: Address,
        buyer: Address,
    }

    fn market() -> Market {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register(Stub, ());
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        Market {
            env,
            contract,
            seller,
            buyer,
        }
    }

    impl Market {
        fn run<T>(&self, f: impl FnOnce() -> T) -> T {
            self.env.as_contract(&self.contract, f)
        }

        /// Mint a cosmetic of `rarity` owned by `owner` and return its skin ID.
        fn mint(&self, owner: &Address, rarity: SkinRarity) -> u64 {
            self.run(|| {
                ship_customization::mint_skin(
                    &self.env,
                    owner,
                    symbol_short!("flame"),
                    rarity,
                    0xFF4400,
                    0xFF8800,
                    Bytes::new(&self.env),
                )
                .unwrap()
                .skin_id
            })
        }
    }

    // ── Listing ───────────────────────────────────────────────────────────

    #[test]
    fn listing_escrows_the_cosmetic_and_records_the_rarity() {
        let m = market();
        let skin_id = m.mint(&m.seller, SkinRarity::Epic);

        let listing = m.run(|| list_cosmetic(&m.env, &m.seller, skin_id, 5_000).unwrap());

        assert_eq!(listing.skin_id, skin_id);
        assert_eq!(listing.price, 5_000);
        assert_eq!(listing.rarity, SkinRarity::Epic);
        assert_eq!(listing.royalty_bps, CREATOR_ROYALTY_BPS_DEFAULT);
        assert!(listing.creator.is_none());

        m.run(|| {
            let skin = ship_customization::get_skin(&m.env, skin_id).unwrap();
            assert!(!skin.tradeable, "a listed cosmetic must be escrowed");
            assert_eq!(get_cosmetic_market_stats(&m.env).active_listings, 1);
        });
    }

    #[test]
    fn listing_enforces_the_rarity_price_floor() {
        let m = market();
        let legendary = m.mint(&m.seller, SkinRarity::Legendary);

        m.run(|| {
            assert_eq!(
                list_cosmetic(&m.env, &m.seller, legendary, 9_999),
                Err(MarketplaceError::PriceBelowRarityFloor)
            );
        });
        // Exactly at the floor is accepted.
        m.run(|| assert!(list_cosmetic(&m.env, &m.seller, legendary, 10_000).is_ok()));
    }

    #[test]
    fn common_cosmetics_have_a_lower_floor_than_legendary_ones() {
        let m = market();
        let common = m.mint(&m.seller, SkinRarity::Common);
        // 100 would be rejected for a Legendary but is fine for a Common.
        m.run(|| assert!(list_cosmetic(&m.env, &m.seller, common, 100).is_ok()));
    }

    #[test]
    fn listing_rejects_non_positive_prices() {
        let m = market();
        let skin_id = m.mint(&m.seller, SkinRarity::Common);

        m.run(|| {
            assert_eq!(
                list_cosmetic(&m.env, &m.seller, skin_id, 0),
                Err(MarketplaceError::InvalidPrice)
            );
        });
        m.run(|| {
            assert_eq!(
                list_cosmetic(&m.env, &m.seller, skin_id, -100),
                Err(MarketplaceError::InvalidPrice)
            );
        });
    }

    #[test]
    fn only_the_owner_may_list_a_cosmetic() {
        let m = market();
        let skin_id = m.mint(&m.seller, SkinRarity::Rare);

        m.run(|| {
            assert_eq!(
                list_cosmetic(&m.env, &m.buyer, skin_id, 500),
                Err(MarketplaceError::NotSkinOwner)
            );
        });
    }

    #[test]
    fn listing_an_unknown_cosmetic_fails() {
        let m = market();
        m.run(|| {
            assert_eq!(
                list_cosmetic(&m.env, &m.seller, 404, 500),
                Err(MarketplaceError::SkinNotFound)
            );
        });
    }

    #[test]
    fn a_cosmetic_cannot_be_listed_twice() {
        let m = market();
        let skin_id = m.mint(&m.seller, SkinRarity::Rare);
        m.run(|| list_cosmetic(&m.env, &m.seller, skin_id, 500).unwrap());

        m.run(|| {
            assert_eq!(
                list_cosmetic(&m.env, &m.seller, skin_id, 900),
                Err(MarketplaceError::AlreadyListed)
            );
        });
    }

    #[test]
    fn an_escrowed_cosmetic_cannot_be_transferred_by_its_owner() {
        let m = market();
        let skin_id = m.mint(&m.seller, SkinRarity::Rare);
        m.run(|| list_cosmetic(&m.env, &m.seller, skin_id, 500).unwrap());

        // `transfer_skin` refuses a non-tradeable skin, so the seller cannot
        // move the cosmetic out from under an open listing.
        m.run(|| {
            assert!(ship_customization::transfer_skin(&m.env, skin_id, &m.buyer).is_err());
        });
        m.run(|| {
            assert_eq!(
                ship_customization::get_skin(&m.env, skin_id).unwrap().owner,
                m.seller
            );
        });
    }

    // ── Buying ────────────────────────────────────────────────────────────

    #[test]
    fn buying_transfers_ownership_and_closes_the_listing() {
        let m = market();
        let skin_id = m.mint(&m.seller, SkinRarity::Epic);
        m.run(|| list_cosmetic(&m.env, &m.seller, skin_id, 4_000).unwrap());

        let split = m.run(|| buy_cosmetic(&m.env, &m.buyer, skin_id).unwrap());

        assert_eq!(split.price, 4_000);
        m.run(|| {
            let skin = ship_customization::get_skin(&m.env, skin_id).unwrap();
            assert_eq!(skin.owner, m.buyer);
            assert!(skin.tradeable, "a sold cosmetic leaves escrow");
            assert!(get_cosmetic_listing(&m.env, skin_id).is_none());

            let stats = get_cosmetic_market_stats(&m.env);
            assert_eq!(stats.sales_count, 1);
            assert_eq!(stats.total_volume, 4_000);
            assert_eq!(stats.active_listings, 0);
        });
    }

    #[test]
    fn buying_moves_the_cosmetic_between_owner_inventories() {
        let m = market();
        let skin_id = m.mint(&m.seller, SkinRarity::Rare);
        m.run(|| list_cosmetic(&m.env, &m.seller, skin_id, 500).unwrap());
        m.run(|| buy_cosmetic(&m.env, &m.buyer, skin_id).unwrap());

        m.run(|| {
            assert_eq!(
                ship_customization::get_owner_skins(&m.env, &m.seller).len(),
                0
            );
            assert_eq!(
                ship_customization::get_owner_skins(&m.env, &m.buyer).len(),
                1
            );
        });
    }

    #[test]
    fn a_seller_cannot_buy_their_own_cosmetic() {
        let m = market();
        let skin_id = m.mint(&m.seller, SkinRarity::Rare);
        m.run(|| list_cosmetic(&m.env, &m.seller, skin_id, 500).unwrap());

        m.run(|| {
            assert_eq!(
                buy_cosmetic(&m.env, &m.seller, skin_id),
                Err(MarketplaceError::SelfPurchase)
            );
        });
    }

    #[test]
    fn buying_an_unlisted_cosmetic_fails() {
        let m = market();
        m.run(|| {
            assert_eq!(
                buy_cosmetic(&m.env, &m.buyer, 7),
                Err(MarketplaceError::NotListed)
            );
        });
    }

    #[test]
    fn a_cosmetic_can_be_resold_after_purchase() {
        let m = market();
        let third = Address::generate(&m.env);
        let skin_id = m.mint(&m.seller, SkinRarity::Epic);

        m.run(|| list_cosmetic(&m.env, &m.seller, skin_id, 2_000).unwrap());
        m.run(|| buy_cosmetic(&m.env, &m.buyer, skin_id).unwrap());
        m.run(|| list_cosmetic(&m.env, &m.buyer, skin_id, 3_000).unwrap());
        m.run(|| buy_cosmetic(&m.env, &third, skin_id).unwrap());

        m.run(|| {
            assert_eq!(
                ship_customization::get_skin(&m.env, skin_id).unwrap().owner,
                third
            );
            let stats = get_cosmetic_market_stats(&m.env);
            assert_eq!(stats.sales_count, 2);
            assert_eq!(stats.total_volume, 5_000);
        });
    }

    // ── Cancelling ────────────────────────────────────────────────────────

    #[test]
    fn cancelling_releases_the_cosmetic_from_escrow() {
        let m = market();
        let skin_id = m.mint(&m.seller, SkinRarity::Rare);
        m.run(|| list_cosmetic(&m.env, &m.seller, skin_id, 500).unwrap());

        m.run(|| cancel_cosmetic_listing(&m.env, &m.seller, skin_id).unwrap());

        m.run(|| {
            assert!(get_cosmetic_listing(&m.env, skin_id).is_none());
            assert!(
                ship_customization::get_skin(&m.env, skin_id)
                    .unwrap()
                    .tradeable
            );
            assert_eq!(get_cosmetic_market_stats(&m.env).active_listings, 0);
        });
    }

    #[test]
    fn only_the_seller_may_cancel() {
        let m = market();
        let skin_id = m.mint(&m.seller, SkinRarity::Rare);
        m.run(|| list_cosmetic(&m.env, &m.seller, skin_id, 500).unwrap());

        m.run(|| {
            assert_eq!(
                cancel_cosmetic_listing(&m.env, &m.buyer, skin_id),
                Err(MarketplaceError::NotSeller)
            );
        });
        m.run(|| {
            assert_eq!(
                cancel_cosmetic_listing(&m.env, &m.seller, 999),
                Err(MarketplaceError::NotListed)
            );
        });
    }

    // ── Creator marketplace ───────────────────────────────────────────────

    #[test]
    fn a_registered_creator_earns_royalties_on_every_sale() {
        let m = market();
        let third = Address::generate(&m.env);
        let skin_id = m.mint(&m.seller, SkinRarity::Epic);

        // The minter registers a 10 % perpetual royalty before selling.
        m.run(|| register_creator_royalty(&m.env, &m.seller, skin_id, 1_000).unwrap());
        m.run(|| list_cosmetic(&m.env, &m.seller, skin_id, 10_000).unwrap());
        let first = m.run(|| buy_cosmetic(&m.env, &m.buyer, skin_id).unwrap());

        assert_eq!(first.creator_royalty, 1_000);
        assert_eq!(first.platform_fee, 250);
        assert_eq!(first.seller_proceeds, 8_750);
        m.run(|| assert_eq!(get_creator_earnings(&m.env, &m.seller), 1_000));

        // The royalty follows the cosmetic to the next sale, where the
        // creator is no longer the seller.
        m.run(|| list_cosmetic(&m.env, &m.buyer, skin_id, 20_000).unwrap());
        let second = m.run(|| buy_cosmetic(&m.env, &third, skin_id).unwrap());

        assert_eq!(second.creator_royalty, 2_000);
        m.run(|| {
            assert_eq!(get_creator_earnings(&m.env, &m.seller), 3_000);
            assert_eq!(
                get_cosmetic_market_stats(&m.env).creator_royalties_paid,
                3_000
            );
        });
    }

    #[test]
    fn an_unregistered_cosmetic_pays_no_royalty_to_anyone() {
        let m = market();
        let skin_id = m.mint(&m.seller, SkinRarity::Epic);
        m.run(|| list_cosmetic(&m.env, &m.seller, skin_id, 10_000).unwrap());

        let split = m.run(|| buy_cosmetic(&m.env, &m.buyer, skin_id).unwrap());

        assert_eq!(split.creator_royalty, 0);
        assert_eq!(split.platform_fee, 250);
        assert_eq!(
            split.seller_proceeds, 9_750,
            "with no creator, the royalty share stays with the seller"
        );
        m.run(|| {
            assert_eq!(get_cosmetic_market_stats(&m.env).creator_royalties_paid, 0);
        });
    }

    #[test]
    fn a_creator_may_opt_out_of_royalties() {
        let m = market();
        let skin_id = m.mint(&m.seller, SkinRarity::Epic);
        m.run(|| register_creator_royalty(&m.env, &m.seller, skin_id, 0).unwrap());
        m.run(|| list_cosmetic(&m.env, &m.seller, skin_id, 10_000).unwrap());

        let split = m.run(|| buy_cosmetic(&m.env, &m.buyer, skin_id).unwrap());
        assert_eq!(split.creator_royalty, 0);
        m.run(|| assert_eq!(get_creator_earnings(&m.env, &m.seller), 0));
    }

    #[test]
    fn royalty_rates_are_capped() {
        let m = market();
        let skin_id = m.mint(&m.seller, SkinRarity::Epic);

        m.run(|| {
            assert_eq!(
                register_creator_royalty(&m.env, &m.seller, skin_id, MAX_CREATOR_ROYALTY_BPS + 1),
                Err(MarketplaceError::RoyaltyTooHigh)
            );
        });
        m.run(|| {
            assert_eq!(
                register_creator_royalty(&m.env, &m.seller, skin_id, -1),
                Err(MarketplaceError::RoyaltyTooHigh)
            );
        });
        m.run(|| assert!(get_creator_royalty(&m.env, skin_id).is_none()));
    }

    #[test]
    fn a_later_holder_cannot_redirect_the_royalty() {
        let m = market();
        let skin_id = m.mint(&m.seller, SkinRarity::Epic);
        m.run(|| register_creator_royalty(&m.env, &m.seller, skin_id, 500).unwrap());
        m.run(|| list_cosmetic(&m.env, &m.seller, skin_id, 2_000).unwrap());
        m.run(|| buy_cosmetic(&m.env, &m.buyer, skin_id).unwrap());

        // The buyer now owns the cosmetic but the royalty is already claimed.
        m.run(|| {
            assert_eq!(
                register_creator_royalty(&m.env, &m.buyer, skin_id, 2_000),
                Err(MarketplaceError::RoyaltyAlreadyRegistered)
            );
        });
        m.run(|| {
            assert_eq!(
                get_creator_royalty(&m.env, skin_id).unwrap().creator,
                m.seller
            );
        });
    }

    #[test]
    fn only_the_owner_may_register_a_royalty() {
        let m = market();
        let skin_id = m.mint(&m.seller, SkinRarity::Epic);

        m.run(|| {
            assert_eq!(
                register_creator_royalty(&m.env, &m.buyer, skin_id, 500),
                Err(MarketplaceError::NotSkinOwner)
            );
        });
        m.run(|| {
            assert_eq!(
                register_creator_royalty(&m.env, &m.seller, 404, 500),
                Err(MarketplaceError::SkinNotFound)
            );
        });
    }

    #[test]
    fn creators_can_withdraw_their_royalties_once() {
        let m = market();
        let skin_id = m.mint(&m.seller, SkinRarity::Epic);
        m.run(|| register_creator_royalty(&m.env, &m.seller, skin_id, 1_000).unwrap());
        m.run(|| list_cosmetic(&m.env, &m.seller, skin_id, 10_000).unwrap());
        m.run(|| buy_cosmetic(&m.env, &m.buyer, skin_id).unwrap());

        m.run(|| {
            assert_eq!(withdraw_creator_earnings(&m.env, &m.seller).unwrap(), 1_000);
        });
        m.run(|| {
            assert_eq!(get_creator_earnings(&m.env, &m.seller), 0);
            assert_eq!(get_creator_withdrawn(&m.env, &m.seller), 1_000);
        });

        m.run(|| {
            assert_eq!(
                withdraw_creator_earnings(&m.env, &m.seller),
                Err(MarketplaceError::NothingToWithdraw)
            );
        });
    }

    #[test]
    fn withdrawing_with_no_earnings_fails() {
        let m = market();
        m.run(|| {
            assert_eq!(
                withdraw_creator_earnings(&m.env, &m.seller),
                Err(MarketplaceError::NothingToWithdraw)
            );
        });
    }

    // ── Split arithmetic ──────────────────────────────────────────────────

    #[test]
    fn the_sale_split_always_accounts_for_the_full_price() {
        for price in [100i128, 500, 2_000, 10_000, 123_457] {
            for bps in [0i128, 250, 500, MAX_CREATOR_ROYALTY_BPS] {
                let split = compute_sale_split(price, bps).unwrap();
                assert_eq!(
                    split.creator_royalty + split.platform_fee + split.seller_proceeds,
                    price,
                    "price {price} at {bps} bps must reconcile exactly"
                );
                assert!(split.seller_proceeds > 0);
            }
        }
    }

    #[test]
    fn the_split_rejects_non_positive_prices() {
        assert_eq!(
            compute_sale_split(0, 500),
            Err(MarketplaceError::InvalidPrice)
        );
        assert_eq!(
            compute_sale_split(-1, 500),
            Err(MarketplaceError::InvalidPrice)
        );
    }

    #[test]
    fn the_split_detects_overflow_instead_of_wrapping() {
        assert_eq!(
            compute_sale_split(i128::MAX, MAX_CREATOR_ROYALTY_BPS),
            Err(MarketplaceError::ArithmeticOverflow)
        );
    }

    // ── Preview ───────────────────────────────────────────────────────────

    #[test]
    fn listings_can_be_previewed_before_buying() {
        let m = market();
        let skin_id = m.mint(&m.seller, SkinRarity::Epic);
        m.run(|| list_cosmetic(&m.env, &m.seller, skin_id, 2_000).unwrap());

        m.run(|| {
            let preview = preview_cosmetic_listing(&m.env, skin_id).unwrap();
            assert_eq!(preview.rarity, SkinRarity::Epic);
            assert_eq!(preview.color_primary, 0xFF4400);
            assert_eq!(preview.effect_layers, 2);
            assert!(preview.floor_price <= 2_000);

            assert!(preview_cosmetic_listing(&m.env, 404).is_none());
        });
    }

    // ── Listing caps ──────────────────────────────────────────────────────

    #[test]
    fn cosmetic_listings_per_seller_are_capped() {
        let m = market();
        for _ in 0..MAX_COSMETIC_LISTINGS_PER_SELLER {
            let skin_id = m.mint(&m.seller, SkinRarity::Common);
            m.run(|| list_cosmetic(&m.env, &m.seller, skin_id, 100).unwrap());
        }

        let extra = m.mint(&m.seller, SkinRarity::Common);
        m.run(|| {
            assert_eq!(
                list_cosmetic(&m.env, &m.seller, extra, 100),
                Err(MarketplaceError::SellerListingCapReached)
            );
        });
    }

    #[test]
    fn cancelling_frees_a_slot_against_the_cap() {
        let m = market();
        let first = m.mint(&m.seller, SkinRarity::Common);
        m.run(|| list_cosmetic(&m.env, &m.seller, first, 100).unwrap());
        m.run(|| cancel_cosmetic_listing(&m.env, &m.seller, first).unwrap());

        let second = m.mint(&m.seller, SkinRarity::Common);
        m.run(|| assert!(list_cosmetic(&m.env, &m.seller, second, 100).is_ok()));
        m.run(|| assert_eq!(get_cosmetic_market_stats(&m.env).active_listings, 1));
    }
}
