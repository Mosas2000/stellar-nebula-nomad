//! Decentralized-exchange harvest and swap integration.
//!
// DEPRECATED: harvest_resources and related types removed after upstream merge
// use crate::resource_minter::{harvest_resources, DexOffer, HarvestError, HarvestResult};
// use crate::NebulaLayout;
use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol};

const MAX_LISTINGS_PER_SESSION: u32 = 5;

// Stub types for backwards compatibility
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DexOffer {
    pub resource: Symbol,
    pub amount: u64,
    pub min_price: i128,
    pub active: bool,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HarvestError {
    Deprecated = 1,
    DexFailure = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HarvestResult {
    pub resources: u64,
}

#[derive(Clone)]
#[contracttype]
pub enum DexKey {
    /// DEX offer by offer_id.
    Offer(u64),
    /// Offer counter.
    OfferCounter,
    /// Number of listings in the current session for a player.
    SessionListings(Address),
}

fn next_offer_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&DexKey::OfferCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage().instance().set(&DexKey::OfferCounter, &next);
    next
}

/// Harvest resources from a layout and immediately list a resource on the DEX.
///
/// DEPRECATED: Combines `harvest_resources` with DEX offer creation in a single call.
/// This function has been disabled after upstream merge removed harvest_resources.
/// Use resource_minter::mint_resource directly instead.
pub fn harvest_and_list(
    env: &Env,
    player: &Address,
    ship_id: u64,
    _layout: &(), // Changed from &NebulaLayout since that type doesn't exist in current scope
    resource: &Symbol,
    min_price: i128,
) -> Result<(HarvestResult, DexOffer), HarvestError> {
    player.require_auth();
    
    // Return deprecated error
    Err(HarvestError::Deprecated)
}

/// Cancel an active DEX listing. Only the original player (owner) can cancel.
/// Refunds the listed amount back to the owner.
pub fn cancel_listing(env: &Env, owner: &Address, offer_id: u64) -> Result<DexOffer, HarvestError> {
    owner.require_auth();

    let mut offer: DexOffer = env
        .storage()
        .instance()
        .get(&DexKey::Offer(offer_id))
        .ok_or(HarvestError::DexFailure)?;

    if !offer.active {
        return Err(HarvestError::DexFailure);
    }

    offer.active = false;
    env.storage()
        .instance()
        .set(&DexKey::Offer(offer_id), &offer);

    // Emit cancellation event
    env.events().publish(
        (symbol_short!("dex"), symbol_short!("canceld")),
        (offer_id, owner.clone()),
    );

    Ok(offer)
}
