use crate::player_profile::ProfileKey;
use crate::resource_minter::ResourceKey;
use crate::ship_nft::DataKey as ShipDataKey;
use soroban_sdk::{contracttype, contracterror, symbol_short, Address, Env, Vec};

// ─── Data Types ───────────────────────────────────────────────────────────────

/// Compact player summary optimised for mobile frontends.
///
/// All fields have safe defaults — the struct is always returned, even when
/// the player has no on-chain profile or ships. Callers should check
/// `has_profile` before displaying profile-specific data.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct MobileDashboard {
    /// Player address this dashboard belongs to.
    pub player: Address,
    /// Whether a player profile has been created on-chain.
    pub has_profile: bool,
    /// Lifetime nebula scan count (0 if no profile).
    pub total_scans: u32,
    /// Lifetime essence earned (0 if no profile).
    pub essence_earned: i128,
    /// Total number of ships owned by this player.
    pub ship_count: u32,
    /// ID of the player's first ship (0 if no ships).
    pub primary_ship_id: u64,
    /// Hull stat of the primary ship (0 if no ships).
    pub primary_hull: u32,
    /// Scanner power of the primary ship (0 if no ships).
    pub primary_scanner_power: u32,
    /// Current stellar-dust balance.
    pub dust_balance: u32,
    /// Current asteroid-ore balance.
    pub ore_balance: u32,
    /// Current gas-units balance.
    pub gas_balance: u32,
}

/// Lightweight scan-result estimate for a ship, computed without generating
/// a full 256-cell nebula layout.
///
/// Useful for mobile UIs that want to show expected yield before the player
/// commits to a full scan.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct QuickScanPreview {
    /// Ship the estimate applies to.
    pub ship_id: u64,
    /// Scanner power read from on-chain ship data.
    pub scanner_power: u32,
    /// Estimated minimum harvestable energy.
    pub estimated_energy_min: u32,
    /// Estimated maximum harvestable energy.
    pub estimated_energy_max: u32,
    /// Predicted rarity index: 0 = Common … 4 = Legendary.
    pub predicted_rarity_index: u32,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum MobileViewError {
    /// No ship with the given ID exists on-chain.
    ShipNotFound = 1,
}

// ─── Internal helpers ────────────────────────────────────────────────────────

/// Map a scanner-power value to a rarity index (0 = Common … 4 = Legendary).
///
/// Thresholds are calibrated against the three built-in ship types:
///
/// | Scanner power | Ship type(s)        | Predicted rarity |
/// |---------------|---------------------|-----------------|
/// | < 20          | hauler (10)         | 0 — Common       |
/// | 20..34        | fighter (20), default (30) | 1 — Uncommon |
/// | 35..54        | explorer (50)       | 2 — Rare         |
/// | 55..74        | upgraded explorer   | 3 — Epic         |
/// | ≥ 75          | max-upgraded ships  | 4 — Legendary    |
fn rarity_index_for_scanner(scanner_power: u32) -> u32 {
    if scanner_power < 20 {
        0
    } else if scanner_power < 35 {
        1
    } else if scanner_power < 55 {
        2
    } else if scanner_power < 75 {
        3
    } else {
        4
    }
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Return a compact dashboard summary for `player`.
///
/// This is a **pure read-only** function — it touches no more than eight
/// storage keys and performs no writes or token transfers.  Instruction
/// count is sub-100 k regardless of the number of ships or resources the
/// player owns (primary ship stats + three resource balances only).
///
/// Missing data is represented by zero-values rather than errors, enabling
/// graceful empty-state rendering on mobile UIs.
pub fn get_mobile_dashboard(env: &Env, player: &Address) -> MobileDashboard {
    // ── Profile ──────────────────────────────────────────────────────────────
    let profile_id_opt: Option<u64> = env
        .storage()
        .persistent()
        .get(&ProfileKey::OwnerProfile(player.clone()));

    let (has_profile, total_scans, essence_earned) = match profile_id_opt {
        Some(pid) => {
            use crate::player_profile::PlayerProfile;
            let profile: Option<PlayerProfile> = env
                .storage()
                .persistent()
                .get(&ProfileKey::Profile(pid));
            match profile {
                Some(p) => (true, p.total_scans, p.essence_earned),
                None => (false, 0u32, 0i128),
            }
        }
        None => (false, 0u32, 0i128),
    };

    // ── Ships ─────────────────────────────────────────────────────────────────
    let ship_ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&ShipDataKey::OwnerShips(player.clone()))
        .unwrap_or_else(|| Vec::new(env));

    let ship_count = ship_ids.len();

    let (primary_ship_id, primary_hull, primary_scanner_power) = if ship_count > 0 {
        let first_id = ship_ids.get(0).unwrap();
        use crate::ship_nft::ShipNft;
        let ship: Option<ShipNft> = env
            .storage()
            .persistent()
            .get(&ShipDataKey::Ship(first_id));
        match ship {
            Some(s) => (s.id, s.hull, s.scanner_power),
            None => (0u64, 0u32, 0u32),
        }
    } else {
        (0u64, 0u32, 0u32)
    };

    // ── Resource balances ─────────────────────────────────────────────────────
    let dust_balance: u32 = env
        .storage()
        .instance()
        .get(&ResourceKey::ResourceBalance(
            player.clone(),
            symbol_short!("dust"),
        ))
        .unwrap_or(0);
    let ore_balance: u32 = env
        .storage()
        .instance()
        .get(&ResourceKey::ResourceBalance(
            player.clone(),
            symbol_short!("ore"),
        ))
        .unwrap_or(0);
    let gas_balance: u32 = env
        .storage()
        .instance()
        .get(&ResourceKey::ResourceBalance(
            player.clone(),
            symbol_short!("gas"),
        ))
        .unwrap_or(0);

    MobileDashboard {
        player: player.clone(),
        has_profile,
        total_scans,
        essence_earned,
        ship_count,
        primary_ship_id,
        primary_hull,
        primary_scanner_power,
        dust_balance,
        ore_balance,
        gas_balance,
    }
}

/// Return a lightweight scan-result preview for `ship_id`.
///
/// This is a **pure read-only** function — it reads a single storage key
/// (the ship record) and performs only integer arithmetic.  Instruction
/// count is well under 100 k for any valid input.
///
/// Returns `MobileViewError::ShipNotFound` if `ship_id` does not exist.
pub fn get_quick_scan_preview(
    env: &Env,
    ship_id: u64,
) -> Result<QuickScanPreview, MobileViewError> {
    use crate::ship_nft::ShipNft;
    let ship: ShipNft = env
        .storage()
        .persistent()
        .get(&ShipDataKey::Ship(ship_id))
        .ok_or(MobileViewError::ShipNotFound)?;

    let scanner_power = ship.scanner_power;

    // Energy estimates: linear in scanner power.
    // min = scanner_power * 3, max = scanner_power * 8.
    let estimated_energy_min = scanner_power.saturating_mul(3);
    let estimated_energy_max = scanner_power.saturating_mul(8);

    let predicted_rarity_index = rarity_index_for_scanner(scanner_power);

    Ok(QuickScanPreview {
        ship_id,
        scanner_power,
        estimated_energy_min,
        estimated_energy_max,
        predicted_rarity_index,
    })
}
