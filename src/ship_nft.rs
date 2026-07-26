use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Bytes, Env, Symbol, Vec};

// ─── Storage Keys ─────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    /// Global auto-incrementing ship ID counter.
    ShipCounter,
    /// Ship data keyed by ship ID: `Ship(ship_id)`.
    Ship(u64),
    /// List of ship IDs owned by an address: `OwnerShips(address)`.
    OwnerShips(Address),
    /// Reentrancy lock for mint/transfer state transitions.
    ReentrancyLock,
}

// ─── Custom Errors ────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum ShipError {
    /// Ship ID unexpectedly already exists.
    ShipAlreadyExists = 1,
    /// Ship with the given ID does not exist.
    ShipNotFound = 2,
    /// Caller is not the owner of the ship.
    NotOwner = 3,
    /// Cannot transfer to the current owner.
    SameOwner = 4,
    /// Batch mint exceeds the maximum of 3 ships per transaction.
    BatchLimitExceeded = 5,
    /// Invalid ship type provided.
    InvalidShipType = 6,
    /// Reentrancy guard is active.
    ReentrancyDetected = 7,
    /// Metadata URI must use a marketplace-compatible URI scheme.
    InvalidMetadataUri = 8,
}

// ─── Ship NFT Data ───────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ShipNft {
    pub id: u64,
    pub owner: Address,
    pub ship_type: Symbol,
    pub hull: u32,
    pub scanner_power: u32,
    pub durability: u32,
    pub max_durability: u32,
    pub metadata: Bytes,
    pub metadata_uri: Bytes,
}

// ─── Ship Type Stats ─────────────────────────────────────────────────────

/// Returns (hull, scanner_power) for a given ship type symbol.
/// Known types: "fighter", "explorer", "hauler".
/// Unknown types return default stats.
fn stats_for_type(ship_type: &Symbol) -> (u32, u32) {
    // Use short symbols for comparison — Soroban Symbol is ≤ 9 ASCII chars.
    if *ship_type == symbol_short!("fighter") {
        (150, 20)
    } else if *ship_type == symbol_short!("explorer") {
        (80, 50)
    } else if *ship_type == symbol_short!("hauler") {
        (200, 10)
    } else {
        // Default stats for custom / future ship types.
        (100, 30)
    }
}

fn has_prefix(bytes: &Bytes, prefix: &[u8]) -> bool {
    if bytes.len() < prefix.len() as u32 {
        return false;
    }

    for (i, expected) in prefix.iter().enumerate() {
        if bytes.get(i as u32).unwrap() != *expected {
            return false;
        }
    }

    true
}

/// Validate NFT metadata URIs accepted by common external marketplaces.
///
/// Supported standards-friendly schemes are:
/// - `ipfs://` for decentralized IPFS JSON metadata.
/// - `https://` for web-hosted JSON metadata.
/// - `ar://` for Arweave-hosted JSON metadata.
fn is_valid_metadata_uri(metadata_uri: &Bytes) -> bool {
    has_prefix(metadata_uri, b"ipfs://")
        || has_prefix(metadata_uri, b"https://")
        || has_prefix(metadata_uri, b"ar://")
}

fn is_valid_ship_type(ship_type: &Symbol) -> bool {
    *ship_type == symbol_short!("fighter")
        || *ship_type == symbol_short!("explorer")
        || *ship_type == symbol_short!("hauler")
}

fn enter_lock(env: &Env) -> Result<(), ShipError> {
    let key = DataKey::ReentrancyLock;
    let locked: bool = env.storage().persistent().get(&key).unwrap_or(false);
    if locked {
        return Err(ShipError::ReentrancyDetected);
    }
    env.storage().persistent().set(&key, &true);
    Ok(())
}

fn exit_lock(env: &Env) {
    env.storage()
        .persistent()
        .set(&DataKey::ReentrancyLock, &false);
}

// ─── Internal Helpers ────────────────────────────────────────────────────

/// Fetch the next ship ID and increment the global counter.
fn next_ship_id(env: &Env) -> u64 {
    let key = DataKey::ShipCounter;
    let current: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    let next = current + 1;
    env.storage().persistent().set(&key, &next);
    next
}

/// Persist a `ShipNft` in contract storage.
fn store_ship(env: &Env, ship: &ShipNft) {
    env.storage()
        .persistent()
        .set(&DataKey::Ship(ship.id), ship);
}

/// Append a ship ID to the owner's ship-list vector.
fn add_ship_to_owner(env: &Env, owner: &Address, ship_id: u64) {
    let key = DataKey::OwnerShips(owner.clone());
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    ids.push_back(ship_id);
    env.storage().persistent().set(&key, &ids);
}

/// Remove a ship ID from the owner's ship-list vector.
fn remove_ship_from_owner(env: &Env, owner: &Address, ship_id: u64) {
    let key = DataKey::OwnerShips(owner.clone());
    let ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));

    let mut new_ids = Vec::new(env);
    for i in 0..ids.len() {
        let id = ids.get(i).unwrap();
        if id != ship_id {
            new_ids.push_back(id);
        }
    }
    env.storage().persistent().set(&key, &new_ids);
}

// ─── Events ──────────────────────────────────────────────────────────────

pub fn emit_ship_minted(env: &Env, ship: &ShipNft) {
    env.events().publish(
        (symbol_short!("ship"), symbol_short!("minted")),
        (
            ship.id,
            ship.owner.clone(),
            ship.ship_type.clone(),
            ship.hull,
            ship.scanner_power,
        ),
    );
}

pub fn emit_ship_transferred(env: &Env, ship_id: u64, from: &Address, to: &Address) {
    env.events().publish(
        (symbol_short!("ship"), symbol_short!("transfer")),
        (ship_id, from.clone(), to.clone()),
    );
}

// ─── Public API (called by NebulaNomadContract in lib.rs) ────────────────

/// Mint a new Ship NFT with initial stats derived from `ship_type`.
///
/// The `owner` must authorize this call. An auto-incremented ID is
/// assigned and a `ShipMinted` event is emitted for frontend indexing.
///
/// `metadata` is optional free-form bytes for future ship visuals / JSON.
pub fn mint_ship(
    env: &Env,
    owner: &Address,
    ship_type: &Symbol,
    metadata: &Bytes,
) -> Result<ShipNft, ShipError> {
    enter_lock(env)?;

    // Authorization: only the owner can mint for themselves.
    owner.require_auth();

    if !is_valid_ship_type(ship_type) {
        exit_lock(env);
        return Err(ShipError::InvalidShipType);
    }

    let id = next_ship_id(env);
    let (hull, scanner_power) = stats_for_type(ship_type);

    if env.storage().persistent().has(&DataKey::Ship(id)) {
        exit_lock(env);
        return Err(ShipError::ShipAlreadyExists);
    }

    let ship = ShipNft {
        id,
        owner: owner.clone(),
        ship_type: ship_type.clone(),
        hull,
        scanner_power,
        durability: 100,
        max_durability: 100,
        metadata: metadata.clone(),
        metadata_uri: Bytes::new(env),
    };

    store_ship(env, &ship);
    add_ship_to_owner(env, owner, id);
    emit_ship_minted(env, &ship);

    exit_lock(env);

    Ok(ship)
}

/// Batch-mint up to 3 ships in a single transaction (onboarding events).
///
/// Returns a `Vec<ShipNft>` of the newly minted ships.
pub fn batch_mint_ships(
    env: &Env,
    owner: &Address,
    ship_types: &Vec<Symbol>,
    metadata: &Bytes,
) -> Result<Vec<ShipNft>, ShipError> {
    enter_lock(env)?;

    if ship_types.len() > 3 {
        exit_lock(env);
        return Err(ShipError::BatchLimitExceeded);
    }

    owner.require_auth();

    let mut ships = Vec::new(env);
    for i in 0..ship_types.len() {
        let st = ship_types.get(i).unwrap();
        if !is_valid_ship_type(&st) {
            exit_lock(env);
            return Err(ShipError::InvalidShipType);
        }

        let id = next_ship_id(env);
        let (hull, scanner_power) = stats_for_type(&st);

        if env.storage().persistent().has(&DataKey::Ship(id)) {
            exit_lock(env);
            return Err(ShipError::ShipAlreadyExists);
        }

        let ship = ShipNft {
            id,
            owner: owner.clone(),
            ship_type: st.clone(),
            hull,
            scanner_power,
            durability: 100,
            max_durability: 100,
            metadata: metadata.clone(),
            metadata_uri: Bytes::new(env),
        };

        store_ship(env, &ship);
        add_ship_to_owner(env, owner, id);
        emit_ship_minted(env, &ship);
        ships.push_back(ship);
    }

    exit_lock(env);
    Ok(ships)
}

/// Transfer ownership of a Ship NFT from one player to another.
///
/// The `from` address must authorize the transfer and must be the current
/// owner. The recipient must differ from the current owner. Ownership indices
/// are updated atomically and a `ShipTransferred` event is emitted.
pub fn transfer_ship(
    env: &Env,
    ship_id: u64,
    from: &Address,
    to: &Address,
) -> Result<ShipNft, ShipError> {
    enter_lock(env)?;

    let key = DataKey::Ship(ship_id);
    let mut ship: ShipNft = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ShipError::ShipNotFound)
        .map_err(|e| {
            exit_lock(env);
            e
        })?;

    from.require_auth();

    if ship.owner != *from {
        exit_lock(env);
        return Err(ShipError::NotOwner);
    }

    if ship.owner == *to {
        exit_lock(env);
        return Err(ShipError::SameOwner);
    }

    remove_ship_from_owner(env, from, ship_id);
    add_ship_to_owner(env, to, ship_id);

    ship.owner = to.clone();
    store_ship(env, &ship);

    emit_ship_transferred(env, ship_id, from, to);

    exit_lock(env);

    Ok(ship)
}

/// Backwards-compatible alias for older callers.
pub fn transfer_ownership(
    env: &Env,
    ship_id: u64,
    new_owner: &Address,
) -> Result<ShipNft, ShipError> {
    let ship = get_ship(env, ship_id)?;
    transfer_ship(env, ship_id, &ship.owner, new_owner)
}

/// Read a ship by ID (view function — no auth required).
pub fn get_ship(env: &Env, ship_id: u64) -> Result<ShipNft, ShipError> {
    env.storage()
        .persistent()
        .get(&DataKey::Ship(ship_id))
        .ok_or(ShipError::ShipNotFound)
}

/// List all ship IDs owned by `owner` (view function).
pub fn get_ships_by_owner(env: &Env, owner: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::OwnerShips(owner.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn repair_ship(env: &Env, owner: &Address, ship_id: u64) -> Result<ShipNft, ShipError> {
    owner.require_auth();
    let key = DataKey::Ship(ship_id);
    let mut ship: ShipNft = env.storage().persistent().get(&key).ok_or(ShipError::ShipNotFound)?;
    if ship.owner != *owner { return Err(ShipError::NotOwner); }

    let old_durability = ship.durability;
    ship.durability = ship.max_durability;
    env.storage().persistent().set(&key, &ship);

    env.events().publish(
        (symbol_short!("ship_rep"), ship_id, owner.clone()),
        old_durability,
    );

    Ok(ship)
}

pub fn damage_ship(env: &Env, ship_id: u64, amount: u32) -> Result<ShipNft, ShipError> {
    let key = DataKey::Ship(ship_id);
    let mut ship: ShipNft = env.storage().persistent().get(&key).ok_or(ShipError::ShipNotFound)?;

    let old_durability = ship.durability;
    let owner = ship.owner.clone();
    ship.durability = ship.durability.saturating_sub(amount);
    env.storage().persistent().set(&key, &ship);

    env.events().publish(
        (symbol_short!("ship_dmg"), ship_id, owner.clone()),
        (old_durability, ship.durability),
    );

    Ok(ship)
}


/// Set the marketplace-compatible metadata URI for a ship NFT.
///
/// The current ship owner must authorize this call. URI values must use a
/// standard NFT metadata scheme (`ipfs://`, `https://`, or `ar://`) so external
/// marketplaces can resolve the ship metadata JSON.
pub fn set_metadata(
    env: &Env,
    owner: &Address,
    ship_id: u64,
    metadata_uri: &Bytes,
) -> Result<ShipNft, ShipError> {
    owner.require_auth();

    if !is_valid_metadata_uri(metadata_uri) {
        return Err(ShipError::InvalidMetadataUri);
    }

    let key = DataKey::Ship(ship_id);
    let mut ship: ShipNft = env.storage().persistent().get(&key).ok_or(ShipError::ShipNotFound)?;

    if ship.owner != *owner {
        return Err(ShipError::NotOwner);
    }

    ship.metadata_uri = metadata_uri.clone();
    env.storage().persistent().set(&key, &ship);

    env.events().publish(
        (symbol_short!("ship"), symbol_short!("meta")),
        (ship_id, metadata_uri.clone(), owner.clone()),
    );

    Ok(ship)
}

/// Read the marketplace-compatible metadata URI for a ship NFT.
pub fn get_metadata(env: &Env, ship_id: u64) -> Result<Bytes, ShipError> {
    let ship = get_ship(env, ship_id)?;
    Ok(ship.metadata_uri)
}
