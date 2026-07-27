//! Predefined skin templates, rarity model and preview rendering.
//!
//! The template catalogue below is the source of truth for what a cosmetic is
//! worth and how often it should appear. Issue #283 adds three things on top of
//! it:
//!
//!   * a **rarity system** — floor prices and drop weights per tier, plus
//!     [`roll_rarity`] for weighted rolls;
//!   * **preview functionality** — [`preview_template`] and [`preview_skin`]
//!     return everything a client needs to render a cosmetic (derived accent,
//!     gradient strip, effect layers, thumbnail seed) without owning it and
//!     without mutating state;
//!   * the pricing floor the cosmetic marketplace in
//!     [`crate::nft_marketplace`] enforces on every listing.

use crate::ship_customization::{ShipSkin, SkinRarity};
use soroban_sdk::{contracttype, symbol_short, Bytes, Env, Symbol, Vec};

// ─── Rarity model ─────────────────────────────────────────────────────────────

/// Basis-point denominator for drop weights.
pub const RARITY_WEIGHT_DENOMINATOR: u32 = 10_000;

/// Number of gradient stops in a [`SkinPreview`].
pub const PREVIEW_GRADIENT_STOPS: u32 = 5;

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SkinTemplate {
    pub name: Symbol,
    pub rarity: SkinRarity,
    pub color_primary: u32,
    pub color_secondary: u32,
    pub price: i128,
}

/// Everything needed to render a cosmetic without owning it.
///
/// Pure function of the template (or minted skin) — safe to call from
/// read-only endpoints and cheap enough to batch for a gallery view.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SkinPreview {
    pub name: Symbol,
    pub rarity: SkinRarity,
    pub color_primary: u32,
    pub color_secondary: u32,
    /// Midpoint of the two base colours, for outlines and UI chrome.
    pub color_accent: u32,
    /// [`PREVIEW_GRADIENT_STOPS`] colours from primary to secondary.
    pub gradient: Vec<u32>,
    /// Visual effect layers the rarity unlocks (0 for Common … 3 for
    /// Legendary). The client maps these onto shader passes.
    pub effect_layers: u32,
    /// Lowest price the cosmetic marketplace will accept for this rarity.
    pub floor_price: i128,
    /// How often the rarity drops, in basis points out of
    /// [`RARITY_WEIGHT_DENOMINATOR`].
    pub drop_weight_bps: u32,
    /// Deterministic 8-byte seed for the client-side thumbnail generator, so
    /// the same cosmetic always renders identically across sessions.
    pub thumbnail_seed: Bytes,
}

/// Catalogue composition, for balance dashboards.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkinRarityStats {
    pub common: u32,
    pub rare: u32,
    pub epic: u32,
    pub legendary: u32,
    pub total: u32,
}

/// Get all available skin templates
pub fn get_skin_templates(env: &Env) -> Vec<SkinTemplate> {
    let mut templates = Vec::new(env);
    
    // Common skins (10 templates)
    templates.push_back(SkinTemplate {
        name: symbol_short!("basic"),
        rarity: SkinRarity::Common,
        color_primary: 0xCCCCCC,
        color_secondary: 0x888888,
        price: 100,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("red"),
        rarity: SkinRarity::Common,
        color_primary: 0xFF0000,
        color_secondary: 0x880000,
        price: 100,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("blue"),
        rarity: SkinRarity::Common,
        color_primary: 0x0000FF,
        color_secondary: 0x000088,
        price: 100,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("green"),
        rarity: SkinRarity::Common,
        color_primary: 0x00FF00,
        color_secondary: 0x008800,
        price: 100,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("yellow"),
        rarity: SkinRarity::Common,
        color_primary: 0xFFFF00,
        color_secondary: 0x888800,
        price: 100,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("purple"),
        rarity: SkinRarity::Common,
        color_primary: 0xFF00FF,
        color_secondary: 0x880088,
        price: 100,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("cyan"),
        rarity: SkinRarity::Common,
        color_primary: 0x00FFFF,
        color_secondary: 0x008888,
        price: 100,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("orange"),
        rarity: SkinRarity::Common,
        color_primary: 0xFF8800,
        color_secondary: 0x884400,
        price: 100,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("pink"),
        rarity: SkinRarity::Common,
        color_primary: 0xFF88FF,
        color_secondary: 0x884488,
        price: 100,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("white"),
        rarity: SkinRarity::Common,
        color_primary: 0xFFFFFF,
        color_secondary: 0xCCCCCC,
        price: 100,
    });
    
    // Rare skins (20 templates)
    templates.push_back(SkinTemplate {
        name: symbol_short!("flame"),
        rarity: SkinRarity::Rare,
        color_primary: 0xFF4400,
        color_secondary: 0xFF8800,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("ice"),
        rarity: SkinRarity::Rare,
        color_primary: 0x88FFFF,
        color_secondary: 0x44CCFF,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("toxic"),
        rarity: SkinRarity::Rare,
        color_primary: 0x88FF00,
        color_secondary: 0x44AA00,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("shadow"),
        rarity: SkinRarity::Rare,
        color_primary: 0x222222,
        color_secondary: 0x000000,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("gold"),
        rarity: SkinRarity::Rare,
        color_primary: 0xFFD700,
        color_secondary: 0xFFAA00,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("silver"),
        rarity: SkinRarity::Rare,
        color_primary: 0xC0C0C0,
        color_secondary: 0x808080,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("bronze"),
        rarity: SkinRarity::Rare,
        color_primary: 0xCD7F32,
        color_secondary: 0x8B4513,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("emerald"),
        rarity: SkinRarity::Rare,
        color_primary: 0x50C878,
        color_secondary: 0x228B22,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("ruby"),
        rarity: SkinRarity::Rare,
        color_primary: 0xE0115F,
        color_secondary: 0x9B111E,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("sapphire"),
        rarity: SkinRarity::Rare,
        color_primary: 0x0F52BA,
        color_secondary: 0x082567,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("amethyst"),
        rarity: SkinRarity::Rare,
        color_primary: 0x9966CC,
        color_secondary: 0x663399,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("topaz"),
        rarity: SkinRarity::Rare,
        color_primary: 0xFFC87C,
        color_secondary: 0xFF9933,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("pearl"),
        rarity: SkinRarity::Rare,
        color_primary: 0xF0EAD6,
        color_secondary: 0xE6D7B8,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("onyx"),
        rarity: SkinRarity::Rare,
        color_primary: 0x353839,
        color_secondary: 0x0F0F0F,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("jade"),
        rarity: SkinRarity::Rare,
        color_primary: 0x00A86B,
        color_secondary: 0x007850,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("coral"),
        rarity: SkinRarity::Rare,
        color_primary: 0xFF7F50,
        color_secondary: 0xFF6347,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("turquois"),
        rarity: SkinRarity::Rare,
        color_primary: 0x40E0D0,
        color_secondary: 0x00CED1,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("crimson"),
        rarity: SkinRarity::Rare,
        color_primary: 0xDC143C,
        color_secondary: 0x8B0000,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("indigo"),
        rarity: SkinRarity::Rare,
        color_primary: 0x4B0082,
        color_secondary: 0x2E0854,
        price: 500,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("violet"),
        rarity: SkinRarity::Rare,
        color_primary: 0x8F00FF,
        color_secondary: 0x5F00A8,
        price: 500,
    });
    
    // Epic skins (15 templates)
    templates.push_back(SkinTemplate {
        name: symbol_short!("plasma"),
        rarity: SkinRarity::Epic,
        color_primary: 0xFF00FF,
        color_secondary: 0x00FFFF,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("nebula"),
        rarity: SkinRarity::Epic,
        color_primary: 0x8844FF,
        color_secondary: 0xFF4488,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("cosmic"),
        rarity: SkinRarity::Epic,
        color_primary: 0x4400FF,
        color_secondary: 0xFF0088,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("aurora"),
        rarity: SkinRarity::Epic,
        color_primary: 0x00FF88,
        color_secondary: 0x88FF00,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("galaxy"),
        rarity: SkinRarity::Epic,
        color_primary: 0x4400AA,
        color_secondary: 0xAA0044,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("quantum"),
        rarity: SkinRarity::Epic,
        color_primary: 0x00AAFF,
        color_secondary: 0xFF00AA,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("photon"),
        rarity: SkinRarity::Epic,
        color_primary: 0xFFFF00,
        color_secondary: 0x00FFFF,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("neutron"),
        rarity: SkinRarity::Epic,
        color_primary: 0x0088FF,
        color_secondary: 0xFF8800,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("pulsar"),
        rarity: SkinRarity::Epic,
        color_primary: 0xFF0044,
        color_secondary: 0x4400FF,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("quasar"),
        rarity: SkinRarity::Epic,
        color_primary: 0x00FF44,
        color_secondary: 0xFF4400,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("supernov"),
        rarity: SkinRarity::Epic,
        color_primary: 0xFFAA00,
        color_secondary: 0x00AAFF,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("blackhol"),
        rarity: SkinRarity::Epic,
        color_primary: 0x000044,
        color_secondary: 0x440000,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("wormhole"),
        rarity: SkinRarity::Epic,
        color_primary: 0x440088,
        color_secondary: 0x884400,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("starborn"),
        rarity: SkinRarity::Epic,
        color_primary: 0xFFFFAA,
        color_secondary: 0xAAFFFF,
        price: 2000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("eclipse"),
        rarity: SkinRarity::Epic,
        color_primary: 0x220044,
        color_secondary: 0x442200,
        price: 2000,
    });
    
    // Legendary skins (5 templates)
    templates.push_back(SkinTemplate {
        name: symbol_short!("void"),
        rarity: SkinRarity::Legendary,
        color_primary: 0x000000,
        color_secondary: 0x8800FF,
        price: 10000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("stellar"),
        rarity: SkinRarity::Legendary,
        color_primary: 0xFFFFFF,
        color_secondary: 0xFFD700,
        price: 10000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("infinity"),
        rarity: SkinRarity::Legendary,
        color_primary: 0x0000FF,
        color_secondary: 0xFF0000,
        price: 10000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("eternity"),
        rarity: SkinRarity::Legendary,
        color_primary: 0xFFFFFF,
        color_secondary: 0x000000,
        price: 10000,
    });
    templates.push_back(SkinTemplate {
        name: symbol_short!("genesis"),
        rarity: SkinRarity::Legendary,
        color_primary: 0xFFD700,
        color_secondary: 0xFFFFFF,
        price: 10000,
    });
    
    templates
}

#[contracttype]
#[derive(Clone)]
pub struct SkinCatalogueEntry {
    pub template: SkinTemplate,
    pub preview: SkinPreview,
    pub mint_count: u32,
}

pub fn get_full_catalogue(env: &Env) -> Vec<SkinCatalogueEntry> {
    let templates = get_skin_templates(env);
    let mut catalogue = Vec::new(env);
    for t in templates.iter() {
        let preview = build_preview(env, t.name.clone(), t.rarity.clone(), t.color_primary, t.color_secondary);
        catalogue.push_back(SkinCatalogueEntry {
            template: t,
            preview,
            mint_count: 0,
        });
    }
    catalogue
}

pub fn search_templates(env: &Env, query: Symbol) -> Vec<SkinTemplate> {
    let all = get_skin_templates(env);
    let mut result = Vec::new(env);
    for t in all.iter() {
        if t.name == query {
            result.push_back(t);
        }
    }
    result
}

pub fn get_templates_by_price_range(env: &Env, min_price: i128, max_price: i128) -> Vec<SkinTemplate> {
    let all = get_skin_templates(env);
    let mut result = Vec::new(env);
    for t in all.iter() {
        if t.price >= min_price && t.price <= max_price {
            result.push_back(t);
        }
    }
    result
}

// ─── Rarity system (Issue #283 / #291) ────────────────────────────────────────

/// Lowest price the cosmetic marketplace accepts for `rarity`.
///
/// Matches the catalogue's mint price, so a cosmetic can never be resold below
/// what it cost to create — that floor is what keeps the secondary market from
/// undercutting the primary one.
pub fn rarity_floor_price(rarity: &SkinRarity) -> i128 {
    match rarity {
        SkinRarity::Common => 100,
        SkinRarity::Rare => 500,
        SkinRarity::Epic => 2_000,
        SkinRarity::Legendary => 10_000,
    }
}

/// Drop weight for `rarity`, in basis points out of
/// [`RARITY_WEIGHT_DENOMINATOR`]. The four weights sum to exactly the
/// denominator, which [`roll_rarity`] relies on.
pub fn rarity_drop_weight_bps(rarity: &SkinRarity) -> u32 {
    match rarity {
        SkinRarity::Common => 6_000,
        SkinRarity::Rare => 3_000,
        SkinRarity::Epic => 900,
        SkinRarity::Legendary => 100,
    }
}

/// Number of visual effect layers `rarity` unlocks.
pub fn rarity_effect_layers(rarity: &SkinRarity) -> u32 {
    match rarity {
        SkinRarity::Common => 0,
        SkinRarity::Rare => 1,
        SkinRarity::Epic => 2,
        SkinRarity::Legendary => 3,
    }
}

/// Pick a rarity from `seed` using the weights in [`rarity_drop_weight_bps`].
///
/// Deterministic: the same seed always yields the same rarity, so a caller can
/// derive a roll from any on-chain entropy source and have it be verifiable.
pub fn roll_rarity(seed: u64) -> SkinRarity {
    let roll = (seed % RARITY_WEIGHT_DENOMINATOR as u64) as u32;

    let common = rarity_drop_weight_bps(&SkinRarity::Common);
    let rare = common + rarity_drop_weight_bps(&SkinRarity::Rare);
    let epic = rare + rarity_drop_weight_bps(&SkinRarity::Epic);

    if roll < common {
        SkinRarity::Common
    } else if roll < rare {
        SkinRarity::Rare
    } else if roll < epic {
        SkinRarity::Epic
    } else {
        SkinRarity::Legendary
    }
}

/// Composition of the template catalogue by rarity.
pub fn get_rarity_stats(env: &Env) -> SkinRarityStats {
    let templates = get_skin_templates(env);
    let mut stats = SkinRarityStats {
        common: 0,
        rare: 0,
        epic: 0,
        legendary: 0,
        total: templates.len(),
    };

    for template in templates.iter() {
        match template.rarity {
            SkinRarity::Common => stats.common += 1,
            SkinRarity::Rare => stats.rare += 1,
            SkinRarity::Epic => stats.epic += 1,
            SkinRarity::Legendary => stats.legendary += 1,
        }
    }

    stats
}

/// Look up a template by its `name` symbol.
pub fn get_template(env: &Env, name: Symbol) -> Option<SkinTemplate> {
    get_skin_templates(env)
        .iter()
        .find(|t| t.name == name)
}

/// All templates of one rarity tier.
pub fn get_templates_by_rarity(env: &Env, rarity: SkinRarity) -> Vec<SkinTemplate> {
    let mut matching = Vec::new(env);
    for template in get_skin_templates(env).iter() {
        if template.rarity == rarity {
            matching.push_back(template);
        }
    }
    matching
}

// ─── Preview rendering (Issue #283) ───────────────────────────────────────────

/// One channel of a linear interpolation between two packed RGB colours.
fn lerp_channel(from: u32, to: u32, shift: u32, step: u32, steps: u32) -> u32 {
    let a = ((from >> shift) & 0xFF) as i64;
    let b = ((to >> shift) & 0xFF) as i64;
    let value = a + ((b - a) * step as i64) / steps as i64;
    ((value as u32) & 0xFF) << shift
}

/// Interpolate `step/steps` of the way from `from` to `to`.
fn lerp_color(from: u32, to: u32, step: u32, steps: u32) -> u32 {
    lerp_channel(from, to, 16, step, steps)
        | lerp_channel(from, to, 8, step, steps)
        | lerp_channel(from, to, 0, step, steps)
}

/// Build the preview payload for an arbitrary colour pair and rarity.
///
/// Shared by [`preview_template`] and [`preview_skin`] so a catalogue entry and
/// a minted NFT of the same cosmetic always preview identically.
pub fn build_preview(
    env: &Env,
    name: Symbol,
    rarity: SkinRarity,
    color_primary: u32,
    color_secondary: u32,
) -> SkinPreview {
    let mut gradient = Vec::new(env);
    let last_stop = PREVIEW_GRADIENT_STOPS - 1;
    for step in 0..PREVIEW_GRADIENT_STOPS {
        gradient.push_back(lerp_color(
            color_primary,
            color_secondary,
            step,
            last_stop,
        ));
    }

    // The seed mixes both colours and the rarity's layer count so visually
    // distinct cosmetics never collide on the same thumbnail.
    let mix = (color_primary as u64) << 32
        | (color_secondary as u64 ^ (rarity_effect_layers(&rarity) as u64) << 24);
    let thumbnail_seed = Bytes::from_array(env, &mix.to_be_bytes());

    SkinPreview {
        name,
        color_primary,
        color_secondary,
        color_accent: lerp_color(color_primary, color_secondary, 1, 2),
        gradient,
        effect_layers: rarity_effect_layers(&rarity),
        floor_price: rarity_floor_price(&rarity),
        drop_weight_bps: rarity_drop_weight_bps(&rarity),
        rarity,
        thumbnail_seed,
    }
}

/// Preview a catalogue template by name, before buying or minting it.
pub fn preview_template(env: &Env, name: Symbol) -> Option<SkinPreview> {
    let template = get_template(env, name)?;
    Some(build_preview(
        env,
        template.name,
        template.rarity,
        template.color_primary,
        template.color_secondary,
    ))
}

/// Preview a minted skin NFT — the same payload as [`preview_template`], but
/// resolved from on-chain skin state so marketplace listings can be rendered.
pub fn preview_skin(env: &Env, skin: &ShipSkin) -> SkinPreview {
    build_preview(
        env,
        skin.name.clone(),
        skin.rarity.clone(),
        skin.color_primary,
        skin.color_secondary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skin_templates_count() {
        let env = Env::default();
        let templates = get_skin_templates(&env);
        assert!(templates.len() >= 50);
    }

    #[test]
    fn test_rarity_pricing() {
        let env = Env::default();
        let templates = get_skin_templates(&env);

        for i in 0..templates.len() {
            let t = templates.get(i).unwrap();
            match t.rarity {
                SkinRarity::Common => assert_eq!(t.price, 100),
                SkinRarity::Rare => assert_eq!(t.price, 500),
                SkinRarity::Epic => assert_eq!(t.price, 2000),
                SkinRarity::Legendary => assert_eq!(t.price, 10000),
            }
        }
    }

    // ── Rarity system (Issue #283) ────────────────────────────────────────

    #[test]
    fn floor_price_matches_the_catalogue_price() {
        let env = Env::default();
        for template in get_skin_templates(&env).iter() {
            assert_eq!(
                rarity_floor_price(&template.rarity),
                template.price,
                "catalogue price and marketplace floor must not drift apart"
            );
        }
    }

    #[test]
    fn floor_prices_increase_with_rarity() {
        assert!(
            rarity_floor_price(&SkinRarity::Common) < rarity_floor_price(&SkinRarity::Rare)
        );
        assert!(rarity_floor_price(&SkinRarity::Rare) < rarity_floor_price(&SkinRarity::Epic));
        assert!(
            rarity_floor_price(&SkinRarity::Epic) < rarity_floor_price(&SkinRarity::Legendary)
        );
    }

    #[test]
    fn drop_weights_sum_to_the_denominator() {
        let total = rarity_drop_weight_bps(&SkinRarity::Common)
            + rarity_drop_weight_bps(&SkinRarity::Rare)
            + rarity_drop_weight_bps(&SkinRarity::Epic)
            + rarity_drop_weight_bps(&SkinRarity::Legendary);
        assert_eq!(total, RARITY_WEIGHT_DENOMINATOR);
    }

    #[test]
    fn drop_weights_decrease_with_rarity() {
        assert!(
            rarity_drop_weight_bps(&SkinRarity::Common)
                > rarity_drop_weight_bps(&SkinRarity::Rare)
        );
        assert!(
            rarity_drop_weight_bps(&SkinRarity::Rare) > rarity_drop_weight_bps(&SkinRarity::Epic)
        );
        assert!(
            rarity_drop_weight_bps(&SkinRarity::Epic)
                > rarity_drop_weight_bps(&SkinRarity::Legendary)
        );
    }

    #[test]
    fn roll_rarity_respects_the_weight_boundaries() {
        assert_eq!(roll_rarity(0), SkinRarity::Common);
        assert_eq!(roll_rarity(5_999), SkinRarity::Common);
        assert_eq!(roll_rarity(6_000), SkinRarity::Rare);
        assert_eq!(roll_rarity(8_999), SkinRarity::Rare);
        assert_eq!(roll_rarity(9_000), SkinRarity::Epic);
        assert_eq!(roll_rarity(9_899), SkinRarity::Epic);
        assert_eq!(roll_rarity(9_900), SkinRarity::Legendary);
        assert_eq!(roll_rarity(9_999), SkinRarity::Legendary);
    }

    #[test]
    fn roll_rarity_is_deterministic_and_wraps() {
        assert_eq!(roll_rarity(12_345), roll_rarity(12_345));
        // Seeds are reduced modulo the denominator.
        assert_eq!(roll_rarity(10_000), roll_rarity(0));
        assert_eq!(roll_rarity(u64::MAX), roll_rarity(u64::MAX % 10_000));
    }

    #[test]
    fn roll_rarity_produces_the_expected_distribution() {
        let mut counts = [0u32; 4];
        for seed in 0..RARITY_WEIGHT_DENOMINATOR as u64 {
            match roll_rarity(seed) {
                SkinRarity::Common => counts[0] += 1,
                SkinRarity::Rare => counts[1] += 1,
                SkinRarity::Epic => counts[2] += 1,
                SkinRarity::Legendary => counts[3] += 1,
            }
        }
        // Sweeping every residue reproduces the weights exactly.
        assert_eq!(counts[0], rarity_drop_weight_bps(&SkinRarity::Common));
        assert_eq!(counts[1], rarity_drop_weight_bps(&SkinRarity::Rare));
        assert_eq!(counts[2], rarity_drop_weight_bps(&SkinRarity::Epic));
        assert_eq!(counts[3], rarity_drop_weight_bps(&SkinRarity::Legendary));
    }

    #[test]
    fn effect_layers_increase_with_rarity() {
        assert_eq!(rarity_effect_layers(&SkinRarity::Common), 0);
        assert_eq!(rarity_effect_layers(&SkinRarity::Rare), 1);
        assert_eq!(rarity_effect_layers(&SkinRarity::Epic), 2);
        assert_eq!(rarity_effect_layers(&SkinRarity::Legendary), 3);
    }

    #[test]
    fn rarity_stats_account_for_every_template() {
        let env = Env::default();
        let stats = get_rarity_stats(&env);
        assert_eq!(
            stats.common + stats.rare + stats.epic + stats.legendary,
            stats.total
        );
        assert!(stats.legendary > 0 && stats.legendary < stats.common);
    }

    // ── Catalogue lookup ──────────────────────────────────────────────────

    #[test]
    fn templates_can_be_looked_up_by_name() {
        let env = Env::default();
        let found = get_template(&env, symbol_short!("void")).unwrap();
        assert_eq!(found.rarity, SkinRarity::Legendary);
        assert!(get_template(&env, symbol_short!("nope")).is_none());
    }

    #[test]
    fn templates_can_be_filtered_by_rarity() {
        let env = Env::default();
        let legendary = get_templates_by_rarity(&env, SkinRarity::Legendary);
        assert_eq!(legendary.len(), get_rarity_stats(&env).legendary);
        for template in legendary.iter() {
            assert_eq!(template.rarity, SkinRarity::Legendary);
        }
    }

    // ── Preview (Issue #283) ──────────────────────────────────────────────

    #[test]
    fn preview_exposes_gradient_accent_and_pricing() {
        let env = Env::default();
        let preview = preview_template(&env, symbol_short!("flame")).unwrap();

        assert_eq!(preview.rarity, SkinRarity::Rare);
        assert_eq!(preview.gradient.len(), PREVIEW_GRADIENT_STOPS);
        assert_eq!(preview.floor_price, rarity_floor_price(&SkinRarity::Rare));
        assert_eq!(
            preview.drop_weight_bps,
            rarity_drop_weight_bps(&SkinRarity::Rare)
        );
        assert_eq!(preview.effect_layers, 1);
        assert_eq!(preview.thumbnail_seed.len(), 8);
    }

    #[test]
    fn preview_gradient_runs_from_primary_to_secondary() {
        let env = Env::default();
        let preview = build_preview(
            &env,
            symbol_short!("test"),
            SkinRarity::Epic,
            0x000000,
            0xFFFFFF,
        );

        assert_eq!(preview.gradient.get(0).unwrap(), 0x000000);
        assert_eq!(
            preview.gradient.get(PREVIEW_GRADIENT_STOPS - 1).unwrap(),
            0xFFFFFF
        );
        // Midpoint of black → white is mid grey on every channel.
        assert_eq!(preview.gradient.get(2).unwrap(), 0x7F7F7F);
        assert_eq!(preview.color_accent, 0x7F7F7F);
    }

    #[test]
    fn preview_handles_a_descending_gradient() {
        let env = Env::default();
        let preview = build_preview(
            &env,
            symbol_short!("test"),
            SkinRarity::Common,
            0xFFFFFF,
            0x000000,
        );

        assert_eq!(preview.gradient.get(0).unwrap(), 0xFFFFFF);
        assert_eq!(preview.gradient.get(2).unwrap(), 0x808080);
        assert_eq!(
            preview.gradient.get(PREVIEW_GRADIENT_STOPS - 1).unwrap(),
            0x000000
        );
    }

    #[test]
    fn preview_is_deterministic_but_distinguishes_cosmetics() {
        let env = Env::default();
        let a = preview_template(&env, symbol_short!("void")).unwrap();
        let b = preview_template(&env, symbol_short!("void")).unwrap();
        let c = preview_template(&env, symbol_short!("stellar")).unwrap();

        assert_eq!(a, b, "previews are pure functions of the template");
        assert_ne!(a.thumbnail_seed, c.thumbnail_seed);
    }

    #[test]
    fn preview_of_an_unknown_template_is_none() {
        let env = Env::default();
        assert!(preview_template(&env, symbol_short!("ghost")).is_none());
    }

    #[test]
    fn minted_skin_previews_match_their_template() {
        let env = Env::default();
        let template = get_template(&env, symbol_short!("plasma")).unwrap();

        let skin = ShipSkin {
            skin_id: 1,
            owner: soroban_sdk::Address::from_str(
                &env,
                "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
            ),
            name: template.name.clone(),
            rarity: template.rarity.clone(),
            color_primary: template.color_primary,
            color_secondary: template.color_secondary,
            metadata: Bytes::new(&env),
            tradeable: true,
        };

        assert_eq!(
            preview_skin(&env, &skin),
            preview_template(&env, symbol_short!("plasma")).unwrap()
        );
    }
}
