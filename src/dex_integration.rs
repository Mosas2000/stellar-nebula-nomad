use crate::resource_minter::{harvest_resources, DexOffer, HarvestError, HarvestResult};
use crate::NebulaLayout;
use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

const MAX_LISTINGS_PER_SESSION: u32 = 5;

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
/// Combines `harvest_resources` with DEX offer creation in a single call.
/// Limited to `MAX_LISTINGS_PER_SESSION` (5) listings per player per session.
pub fn harvest_and_list(
    env: &Env,
    player: &Address,
    ship_id: u64,
    layout: &NebulaLayout,
    resource: &Symbol,
    min_price: i128,
) -> Result<(HarvestResult, DexOffer), HarvestError> {
    player.require_auth();

    if min_price <= 0 {
        return Err(HarvestError::InvalidPrice);
    }

    // Check session listing limit
    let session_key = DexKey::SessionListings(player.clone());
    let current_listings: u32 = env.storage().instance().get(&session_key).unwrap_or(0);

    if current_listings >= MAX_LISTINGS_PER_SESSION {
        return Err(HarvestError::DexFailure);
    }

    // Perform the harvest
    let harvest_result = harvest_resources(env, ship_id, layout)?;

    // Find the harvested amount for the requested resource
    let mut listed_amount: u32 = 0;
    for i in 0..harvest_result.resources.len() {
        if let Some(hr) = harvest_result.resources.get(i) {
            if hr.asset_id == *resource {
                listed_amount += hr.amount;
            }
        }
    }

    if listed_amount == 0 {
        return Err(HarvestError::AssetNotHarvested);
    }

    // Create the DEX offer
    let offer_id = next_offer_id(env);
    let offer = DexOffer {
        offer_id,
        asset_id: resource.clone(),
        amount: listed_amount,
        min_price,
        active: true,
    };

    env.storage()
        .instance()
        .set(&DexKey::Offer(offer_id), &offer);

    // Increment session listing count
    env.storage()
        .instance()
        .set(&session_key, &(current_listings + 1));

    // Emit OfferListed event
    env.events().publish(
        (symbol_short!("dex"), symbol_short!("listed")),
        (
            offer_id,
            player.clone(),
            resource.clone(),
            listed_amount,
            min_price,
        ),
    );

    Ok((harvest_result, offer))
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

/// Read an offer by ID.
#[allow(dead_code)]
pub fn get_offer(env: &Env, offer_id: u64) -> Option<DexOffer> {
    env.storage().instance().get(&DexKey::Offer(offer_id))
}
