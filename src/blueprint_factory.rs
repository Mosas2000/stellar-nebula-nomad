use soroban_sdk::{contracttype, contracterror, symbol_short, Address, Env, Symbol, Vec};

/// Maximum blueprints that can be crafted in a single batch transaction.
pub const MAX_BATCH_CRAFT: u32 = 2;
/// Minimum number of components required for a valid blueprint recipe.
pub const MIN_COMPONENTS: u32 = 2;

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum BlueprintKey {
    /// Individual blueprint data keyed by blueprint ID.
    Blueprint(u64),
    /// Global auto-increment counter for blueprint IDs.
    BlueprintCount,
}

// ─── Data Types ───────────────────────────────────────────────────────────────

/// Rarity tier derived from the number of components used.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum BlueprintRarity {
    /// 2–3 components.
    Common,
    /// 4–5 components.
    Uncommon,
    /// 6+ components.
    Rare,
}

/// A tradeable, soul-bound ship upgrade blueprint NFT.
#[derive(Clone)]
#[contracttype]
pub struct Blueprint {
    pub id: u64,
    /// Blueprint is soul-bound to this address until applied.
    pub owner: Address,
    pub components: Vec<Symbol>,
    pub rarity: BlueprintRarity,
    /// True once consumed by apply_blueprint_to_ship.
    pub applied: bool,
    pub created_at: u64,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BlueprintError {
    BlueprintNotFound = 1,
    InvalidComponents = 2,
    AlreadyApplied = 3,
    NotOwner = 4,
    BatchTooLarge = 5,
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn rarity_from_components(count: u32) -> BlueprintRarity {
    match count {
        0..=3 => BlueprintRarity::Common,
        4..=5 => BlueprintRarity::Uncommon,
        _ => BlueprintRarity::Rare,
    }
}

fn mint_blueprint(
    env: &Env,
    owner: &Address,
    components: Vec<Symbol>,
) -> Result<u64, BlueprintError> {
    if components.len() < MIN_COMPONENTS {
        return Err(BlueprintError::InvalidComponents);
    }

    let id: u64 = env
        .storage()
        .instance()
        .get(&BlueprintKey::BlueprintCount)
        .unwrap_or(0u64)
        + 1;
    env.storage()
        .instance()
        .set(&BlueprintKey::BlueprintCount, &id);

    let rarity = rarity_from_components(components.len());
    let blueprint = Blueprint {
        id,
        owner: owner.clone(),
        components,
        rarity,
        applied: false,
        created_at: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&BlueprintKey::Blueprint(id), &blueprint);

    env.events().publish(
        (symbol_short!("blueprnt"), symbol_short!("crafted")),
        (owner.clone(), id),
    );

    Ok(id)
}

// ─── Functions ────────────────────────────────────────────────────────────────

/// Mint a blueprint NFT from harvested resource components.
///
/// Requires at least `MIN_COMPONENTS` symbols. Rarity is derived from
/// component count. The blueprint is soul-bound to `owner` until applied.
/// Emits `BlueprintCrafted`.
pub fn craft_blueprint(
    env: &Env,
    owner: Address,
    components: Vec<Symbol>,
) -> Result<u64, BlueprintError> {
    owner.require_auth();
    mint_blueprint(env, &owner, components)
}

/// Craft up to `MAX_BATCH_CRAFT` blueprints in a single transaction.
///
/// Returns a Vec of the minted blueprint IDs in the same order as the
/// input recipes.
pub fn batch_craft_blueprints(
    env: &Env,
    owner: Address,
    recipes: Vec<Vec<Symbol>>,
) -> Result<Vec<u64>, BlueprintError> {
    owner.require_auth();

    if recipes.len() > MAX_BATCH_CRAFT {
        return Err(BlueprintError::BatchTooLarge);
    }

    let mut ids: Vec<u64> = Vec::new(env);
    for i in 0..recipes.len() {
        let components = recipes.get(i).unwrap();
        let id = mint_blueprint(env, &owner, components)?;
        ids.push_back(id);
    }

    Ok(ids)
}

/// Consume a blueprint and permanently upgrade a ship.
///
/// The blueprint is marked as applied (consumed) and cannot be reused.
/// Emits `BlueprintApplied` — the actual ship stat upgrade is handled
/// by the ship contract that listens for this event.
pub fn apply_blueprint_to_ship(
    env: &Env,
    owner: Address,
    blueprint_id: u64,
    ship_id: u64,
) -> Result<(), BlueprintError> {
    owner.require_auth();

    let mut blueprint: Blueprint = env
        .storage()
        .persistent()
        .get(&BlueprintKey::Blueprint(blueprint_id))
        .ok_or(BlueprintError::BlueprintNotFound)?;

    if blueprint.owner != owner {
        return Err(BlueprintError::NotOwner);
    }

    if blueprint.applied {
        return Err(BlueprintError::AlreadyApplied);
    }

    blueprint.applied = true;
    env.storage()
        .persistent()
        .set(&BlueprintKey::Blueprint(blueprint_id), &blueprint);

    env.events().publish(
        (symbol_short!("blueprnt"), symbol_short!("applied")),
        (owner, blueprint_id, ship_id),
    );

    Ok(())
}

/// Retrieve a blueprint by ID.
pub fn get_blueprint(env: &Env, blueprint_id: u64) -> Result<Blueprint, BlueprintError> {
    env.storage()
        .persistent()
        .get(&BlueprintKey::Blueprint(blueprint_id))
        .ok_or(BlueprintError::BlueprintNotFound)
}
