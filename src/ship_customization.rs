//! Ship visual customization and skin NFT system

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Bytes, Env, Symbol, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum SkinError {
    SkinNotFound = 1,
    NotOwner = 2,
    AlreadyApplied = 3,
    InvalidRarity = 4,
    SkinLimitReached = 5,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum SkinRarity {
    Common,
    Rare,
    Epic,
    Legendary,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ShipSkin {
    pub skin_id: u64,
    pub owner: Address,
    pub name: Symbol,
    pub rarity: SkinRarity,
    pub color_primary: u32,
    pub color_secondary: u32,
    pub metadata: Bytes,
    pub tradeable: bool,
}

#[contracttype]
#[derive(Clone)]
pub enum SkinKey {
    SkinCounter,
    Skin(u64),
    ShipSkin(u64),
    OwnerSkins(Address),
}

fn next_skin_id(env: &Env) -> u64 {
    let current: u64 = env.storage().instance().get(&SkinKey::SkinCounter).unwrap_or(0);
    env.storage().instance().set(&SkinKey::SkinCounter, &(current + 1));
    current + 1
}

/// Mint a new skin NFT
pub fn mint_skin(
    env: &Env,
    owner: &Address,
    name: Symbol,
    rarity: SkinRarity,
    color_primary: u32,
    color_secondary: u32,
    metadata: Bytes,
) -> Result<ShipSkin, SkinError> {
    owner.require_auth();
    
    let skin_id = next_skin_id(env);
    let skin = ShipSkin {
        skin_id,
        owner: owner.clone(),
        name,
        rarity: rarity.clone(),
        color_primary,
        color_secondary,
        metadata,
        tradeable: true,
    };
    
    env.storage().persistent().set(&SkinKey::Skin(skin_id), &skin);
    
    let mut skins: Vec<u64> = env
        .storage()
        .persistent()
        .get(&SkinKey::OwnerSkins(owner.clone()))
        .unwrap_or_else(|| Vec::new(env));
    skins.push_back(skin_id);
    env.storage().persistent().set(&SkinKey::OwnerSkins(owner.clone()), &skins);
    
    env.events().publish(
        (symbol_short!("skin"), symbol_short!("minted")),
        (skin_id, owner.clone(), rarity),
    );
    
    Ok(skin)
}

/// Apply a skin to a ship
pub fn apply_skin(env: &Env, owner: &Address, ship_id: u64, skin_id: u64) -> Result<(), SkinError> {
    owner.require_auth();
    
    let skin: ShipSkin = env
        .storage()
        .persistent()
        .get(&SkinKey::Skin(skin_id))
        .ok_or(SkinError::SkinNotFound)?;
    
    if skin.owner != *owner {
        return Err(SkinError::NotOwner);
    }
    
    env.storage().persistent().set(&SkinKey::ShipSkin(ship_id), &skin_id);
    
    env.events().publish(
        (symbol_short!("skin"), symbol_short!("applied")),
        (ship_id, skin_id),
    );
    
    Ok(())
}

/// Get the skin applied to a ship
pub fn get_ship_skin(env: &Env, ship_id: u64) -> Option<u64> {
    env.storage().persistent().get(&SkinKey::ShipSkin(ship_id))
}

/// Get all skins owned by an address
pub fn get_owner_skins(env: &Env, owner: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&SkinKey::OwnerSkins(owner.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Transfer skin ownership
pub fn transfer_skin(env: &Env, skin_id: u64, new_owner: &Address) -> Result<ShipSkin, SkinError> {
    let mut skin: ShipSkin = env
        .storage()
        .persistent()
        .get(&SkinKey::Skin(skin_id))
        .ok_or(SkinError::SkinNotFound)?;
    
    skin.owner.require_auth();
    
    if !skin.tradeable {
        return Err(SkinError::AlreadyApplied);
    }
    
    let old_owner = skin.owner.clone();
    skin.owner = new_owner.clone();
    
    env.storage().persistent().set(&SkinKey::Skin(skin_id), &skin);
    
    env.events().publish(
        (symbol_short!("skin"), symbol_short!("xfer")),
        (skin_id, old_owner, new_owner.clone()),
    );
    
    Ok(skin)
}

// ─── Marketplace hooks (Issue #283) ───────────────────────────────────────────
//
// The cosmetic marketplace in `crate::nft_marketplace` needs to read a skin,
// escrow it while listed, and hand it to a buyer. Those operations bypass the
// owner-auth check in `transfer_skin` because consent was established when the
// seller authorized the listing, so they stay crate-private.

/// Fetch a minted skin by ID.
pub fn get_skin(env: &Env, skin_id: u64) -> Option<ShipSkin> {
    env.storage().persistent().get(&SkinKey::Skin(skin_id))
}

/// Toggle a skin's `tradeable` flag.
///
/// The marketplace clears it while a skin is listed, so the same cosmetic
/// cannot be sold twice or transferred out from under an open listing.
pub(crate) fn set_tradeable(
    env: &Env,
    skin_id: u64,
    tradeable: bool,
) -> Result<ShipSkin, SkinError> {
    let mut skin: ShipSkin = env
        .storage()
        .persistent()
        .get(&SkinKey::Skin(skin_id))
        .ok_or(SkinError::SkinNotFound)?;

    skin.tradeable = tradeable;
    env.storage().persistent().set(&SkinKey::Skin(skin_id), &skin);

    Ok(skin)
}

/// Move a skin to `new_owner` without requiring the current owner's auth, and
/// keep both owners' skin lists consistent.
pub(crate) fn transfer_skin_internal(
    env: &Env,
    skin_id: u64,
    new_owner: &Address,
) -> Result<ShipSkin, SkinError> {
    let mut skin: ShipSkin = env
        .storage()
        .persistent()
        .get(&SkinKey::Skin(skin_id))
        .ok_or(SkinError::SkinNotFound)?;

    let old_owner = skin.owner.clone();
    if old_owner == *new_owner {
        return Ok(skin);
    }

    skin.owner = new_owner.clone();
    env.storage().persistent().set(&SkinKey::Skin(skin_id), &skin);

    // Drop the ID from the seller's list …
    let seller_skins: Vec<u64> = env
        .storage()
        .persistent()
        .get(&SkinKey::OwnerSkins(old_owner.clone()))
        .unwrap_or_else(|| Vec::new(env));
    let mut remaining = Vec::new(env);
    for id in seller_skins.iter() {
        if id != skin_id {
            remaining.push_back(id);
        }
    }
    env.storage()
        .persistent()
        .set(&SkinKey::OwnerSkins(old_owner.clone()), &remaining);

    // … and add it to the buyer's.
    let mut buyer_skins: Vec<u64> = env
        .storage()
        .persistent()
        .get(&SkinKey::OwnerSkins(new_owner.clone()))
        .unwrap_or_else(|| Vec::new(env));
    buyer_skins.push_back(skin_id);
    env.storage()
        .persistent()
        .set(&SkinKey::OwnerSkins(new_owner.clone()), &buyer_skins);

    env.events().publish(
        (symbol_short!("skin"), symbol_short!("mktxfer")),
        (skin_id, old_owner, new_owner.clone()),
    );

    Ok(skin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_mint_and_apply_skin() {
        let env = Env::default();
        env.mock_all_auths();
        let owner = Address::generate(&env);
        let metadata = Bytes::from_array(&env, &[0u8; 4]);
        
        let skin = mint_skin(
            &env,
            &owner,
            symbol_short!("flame"),
            SkinRarity::Epic,
            0xFF0000,
            0x00FF00,
            metadata,
        ).unwrap();
        
        assert_eq!(skin.owner, owner);
        assert_eq!(skin.rarity, SkinRarity::Epic);
        
        apply_skin(&env, &owner, 1, skin.skin_id).unwrap();
        let applied = get_ship_skin(&env, 1);
        assert_eq!(applied, Some(skin.skin_id));
    }

    #[test]
    fn test_transfer_skin() {
        let env = Env::default();
        env.mock_all_auths();
        let owner = Address::generate(&env);
        let new_owner = Address::generate(&env);
        let metadata = Bytes::from_array(&env, &[0u8; 4]);
        
        let skin = mint_skin(
            &env,
            &owner,
            symbol_short!("cosmic"),
            SkinRarity::Legendary,
            0x0000FF,
            0xFFFF00,
            metadata,
        ).unwrap();
        
        let transferred = transfer_skin(&env, skin.skin_id, &new_owner).unwrap();
        assert_eq!(transferred.owner, new_owner);
    }

    // ── Marketplace hooks (Issue #283) ────────────────────────────────────
    //
    // Skin state lives in contract storage, so these run inside a contract
    // invocation via `Env::as_contract`.

    use soroban_sdk::{contract, contractimpl};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    fn in_contract<T>(f: impl FnOnce(&Env) -> T) -> T {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register(Stub, ());
        env.as_contract(&contract, || f(&env))
    }

    fn mint_test_skin(env: &Env, owner: &Address) -> ShipSkin {
        mint_skin(
            env,
            owner,
            symbol_short!("void"),
            SkinRarity::Legendary,
            0x000000,
            0x8800FF,
            Bytes::new(env),
        )
        .unwrap()
    }

    #[test]
    fn get_skin_resolves_minted_cosmetics() {
        in_contract(|env| {
            let owner = Address::generate(env);
            let skin = mint_test_skin(env, &owner);

            assert_eq!(get_skin(env, skin.skin_id).unwrap().owner, owner);
            assert!(get_skin(env, 999).is_none());
        });
    }

    #[test]
    fn set_tradeable_locks_and_unlocks_a_skin() {
        in_contract(|env| {
            let owner = Address::generate(env);
            let skin = mint_test_skin(env, &owner);

            assert!(!set_tradeable(env, skin.skin_id, false).unwrap().tradeable);
            // A locked skin cannot be transferred by its owner.
            let other = Address::generate(env);
            assert_eq!(
                transfer_skin(env, skin.skin_id, &other),
                Err(SkinError::AlreadyApplied)
            );

            assert!(set_tradeable(env, skin.skin_id, true).unwrap().tradeable);
            assert_eq!(set_tradeable(env, 999, false), Err(SkinError::SkinNotFound));
        });
    }

    #[test]
    fn internal_transfer_moves_the_skin_between_owner_lists() {
        in_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let skin = mint_test_skin(env, &seller);

            let moved = transfer_skin_internal(env, skin.skin_id, &buyer).unwrap();
            assert_eq!(moved.owner, buyer);
            assert_eq!(get_owner_skins(env, &seller).len(), 0);
            assert_eq!(get_owner_skins(env, &buyer).len(), 1);
            assert_eq!(get_owner_skins(env, &buyer).get(0).unwrap(), skin.skin_id);
        });
    }

    #[test]
    fn internal_transfer_to_the_current_owner_is_a_no_op() {
        in_contract(|env| {
            let owner = Address::generate(env);
            let skin = mint_test_skin(env, &owner);

            transfer_skin_internal(env, skin.skin_id, &owner).unwrap();
            assert_eq!(
                get_owner_skins(env, &owner).len(),
                1,
                "the skin must not be duplicated in the owner's list"
            );
        });
    }

    #[test]
    fn internal_transfer_of_an_unknown_skin_fails() {
        in_contract(|env| {
            let buyer = Address::generate(env);
            assert_eq!(
                transfer_skin_internal(env, 42, &buyer),
                Err(SkinError::SkinNotFound)
            );
        });
    }
}
