use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, String, Symbol, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ThemeError {
    InvalidTheme = 1,
    Unauthorized = 2,
    ShipNotFound = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemePreview {
    pub name: Symbol,
    pub colors: Vec<Symbol>, // Hex color codes
    pub particles: Symbol,
}

pub fn generate_theme_preview(env: Env, theme_id: Symbol) -> Result<ThemePreview, ThemeError> {
    match theme_id {
        s if s == symbol_short!("nebula1") => Ok(ThemePreview {
            name: symbol_short!("Cosmic"),
            colors: Vec::from_array(&env, [symbol_short!("FF00FF"), symbol_short!("00FFFF")]),
            particles: symbol_short!("Stardust"),
        }),
        s if s == symbol_short!("nebula2") => Ok(ThemePreview {
            name: symbol_short!("Void"),
            colors: Vec::from_array(&env, [symbol_short!("000000"), symbol_short!("444444")]),
            particles: symbol_short!("dark_mtr"),
        }),
        s if s == symbol_short!("nebula3") => Ok(ThemePreview {
            name: symbol_short!("Nova"),
            colors: Vec::from_array(&env, [symbol_short!("FFA500"), symbol_short!("FF4500")]),
            particles: symbol_short!("Flare"),
        }),
        s if s == symbol_short!("nebula4") => Ok(ThemePreview {
            name: symbol_short!("Quasar"),
            colors: Vec::from_array(&env, [symbol_short!("0000FF"), symbol_short!("FFFFFF")]),
            particles: symbol_short!("Beams"),
        }),
        s if s == symbol_short!("nebula5") => Ok(ThemePreview {
            name: symbol_short!("Supernova"),
            colors: Vec::from_array(&env, [symbol_short!("FF0000"), symbol_short!("FFFF00")]),
            particles: symbol_short!("Shockwave"),
        }),
        s if s == symbol_short!("nebula6") => Ok(ThemePreview {
            name: symbol_short!("Wormhole"),
            colors: Vec::from_array(&env, [symbol_short!("A020F0"), symbol_short!("000000")]),
            particles: symbol_short!("Vortex"),
        }),
        s if s == symbol_short!("nebula7") => Ok(ThemePreview {
            name: symbol_short!("BlackHole"),
            colors: Vec::from_array(&env, [symbol_short!("000000"), symbol_short!("111111")]),
            particles: symbol_short!("snglrty"),
        }),
        s if s == symbol_short!("nebula8") => Ok(ThemePreview {
            name: symbol_short!("Aurora"),
            colors: Vec::from_array(&env, [symbol_short!("00FF00"), symbol_short!("B026FF")]),
            particles: symbol_short!("Borealis"),
        }),
        s if s == symbol_short!("nebula9") => Ok(ThemePreview {
            name: symbol_short!("Eclipse"),
            colors: Vec::from_array(&env, [symbol_short!("CCCCCC"), symbol_short!("000000")]),
            particles: symbol_short!("Corral"),
        }),
        s if s == symbol_short!("nebula10") => Ok(ThemePreview {
            name: symbol_short!("Meteor"),
            colors: Vec::from_array(&env, [symbol_short!("FFD700"), symbol_short!("8B4513")]),
            particles: symbol_short!("Trails"),
        }),
        _ => Err(ThemeError::InvalidTheme), // Only showing a few for brevity, but should have 10 presets
    }
}

pub fn apply_theme(env: Env, owner: Address, ship_id: u64, theme_id: Symbol) -> Result<(), ThemeError> {
    owner.require_auth();

    // In a real scenario, we'd check if the owner owns the ship using ship_nft module.
    // For this prototype, we'll assume the caller must be authorized and ship exists.
    
    // Validate theme first
    let _ = generate_theme_preview(env.clone(), theme_id.clone())?;

    // Store ship-to-theme association
    env.storage().persistent().set(&(symbol_short!("theme"), ship_id), &theme_id);

    env.events().publish(
        (symbol_short!("theme"), symbol_short!("applied")),
        (ship_id, theme_id),
    );

    Ok(())
}

pub fn get_theme(env: Env, ship_id: u64) -> Option<Symbol> {
    env.storage().persistent().get(&(symbol_short!("theme"), ship_id))
}
