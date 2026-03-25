use soroban_sdk::{contracttype, symbol_short, Address, Bytes, BytesN, Env, Vec};

pub const GRID_SIZE: u32 = 16;
pub const TOTAL_CELLS: u32 = GRID_SIZE * GRID_SIZE;

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum CellType {
    Empty,
    Star,
    Asteroid,
    GasCloud,
    DarkMatter,
    ExoticMatter,
    StellarDust,
    Wormhole,
}

#[derive(Clone)]
#[contracttype]
pub struct NebulaCell {
    pub x: u32,
    pub y: u32,
    pub cell_type: CellType,
    pub energy: u32,
}

#[derive(Clone)]
#[contracttype]
pub struct NebulaLayout {
    pub width: u32,
    pub height: u32,
    pub cells: Vec<NebulaCell>,
    pub seed: BytesN<32>,
    pub timestamp: u64,
    pub total_energy: u32,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

/// Deterministic XorShift64 PRNG for on-chain verifiable randomness.
/// Uses only arithmetic operations—no off-chain RNG.
struct Xorshift64(u64);

impl Xorshift64 {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 1 } else { seed })
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// Combine the caller-supplied seed with on-chain ledger entropy.
/// Returns a SHA-256 hash mixing seed + ledger_sequence + ledger_timestamp.
fn compute_combined_hash(env: &Env, seed: &BytesN<32>) -> BytesN<32> {
    let mut input = Bytes::new(env);
    let seed_bytes: Bytes = seed.clone().into();
    input.append(&seed_bytes);
    input.append(&Bytes::from_slice(env, &env.ledger().sequence().to_be_bytes()));
    input.append(&Bytes::from_slice(env, &env.ledger().timestamp().to_be_bytes()));
    env.crypto().sha256(&input).into()
}

/// Extract a u64 PRNG seed from the first 8 bytes of a BytesN<32>.
fn seed_from_hash(_env: &Env, hash: &BytesN<32>) -> u64 {
    let bytes: Bytes = hash.clone().into();
    let mut val: u64 = 0;
    for i in 0..8u32 {
        val = (val << 8) | (bytes.get(i).unwrap_or(0) as u64);
    }
    if val == 0 { 1 } else { val }
}

/// Map a random u64 value to a CellType with weighted distribution.
fn cell_type_from_val(val: u64) -> CellType {
    match val % 100 {
        0..=29 => CellType::Empty,        // 30%
        30..=44 => CellType::Star,         // 15%
        45..=59 => CellType::Asteroid,     // 15%
        60..=74 => CellType::GasCloud,     // 15%
        75..=84 => CellType::StellarDust,  // 10%
        85..=92 => CellType::DarkMatter,   //  8%
        93..=97 => CellType::ExoticMatter, //  5%
        _ => CellType::Wormhole,           //  2%
    }
}

/// Compute energy for a cell based on its type and a random modifier.
fn energy_for_cell(cell_type: &CellType, val: u64) -> u32 {
    let base: u32 = match cell_type {
        CellType::Empty => 0,
        CellType::Star => 10,
        CellType::Asteroid => 5,
        CellType::GasCloud => 8,
        CellType::StellarDust => 15,
        CellType::DarkMatter => 25,
        CellType::ExoticMatter => 40,
        CellType::Wormhole => 60,
    };
    base + (val % 10) as u32
}

/// Generate a 16×16 procedural nebula map using ledger-seeded PRNG.
///
/// Combines the caller-supplied `seed` with the current ledger sequence and
/// timestamp to produce deterministic, on-chain verifiable output.
/// The `player` address is authenticated via `require_auth` and recorded
/// in the emitted event for attribution.
pub fn generate_nebula_layout(env: &Env, seed: &BytesN<32>, _player: &Address) -> NebulaLayout {
    let combined = compute_combined_hash(env, seed);
    let prng_seed = seed_from_hash(env, &combined);
    let mut rng = Xorshift64::new(prng_seed);
    let timestamp = env.ledger().timestamp();

    let mut cells = Vec::new(env);
    let mut total_energy: u32 = 0;

    for y in 0..GRID_SIZE {
        for x in 0..GRID_SIZE {
            let type_val = rng.next();
            let energy_val = rng.next();
            let cell_type = cell_type_from_val(type_val);
            let energy = energy_for_cell(&cell_type, energy_val);
            total_energy += energy;
            cells.push_back(NebulaCell {
                x,
                y,
                cell_type,
                energy,
            });
        }
    }

    NebulaLayout {
        width: GRID_SIZE,
        height: GRID_SIZE,
        cells,
        seed: combined,
        timestamp,
        total_energy,
    }
}

/// Calculate rarity tier from a NebulaLayout using on-chain verifiable math.
///
/// Scores the layout based on the count of rare cells (DarkMatter,
/// ExoticMatter, Wormhole) and average energy density. Returns one of five
/// `Rarity` tiers from Common to Legendary.
pub fn calculate_rarity_tier(_env: &Env, layout: &NebulaLayout) -> Rarity {
    let mut rare_cells: u32 = 0;

    for i in 0..layout.cells.len() {
        if let Some(cell) = layout.cells.get(i) {
            match cell.cell_type {
                CellType::DarkMatter | CellType::ExoticMatter | CellType::Wormhole => {
                    rare_cells += 1;
                }
                _ => {}
            }
        }
    }

    let energy_density = layout.total_energy / TOTAL_CELLS;
    let rarity_score = rare_cells * 10 + energy_density;

    match rarity_score {
        0..=49 => Rarity::Common,
        50..=99 => Rarity::Uncommon,
        100..=149 => Rarity::Rare,
        150..=199 => Rarity::Epic,
        _ => Rarity::Legendary,
    }
}

/// Compute a deterministic SHA-256 hash of a NebulaLayout for event emission.
pub fn compute_layout_hash(env: &Env, layout: &NebulaLayout) -> BytesN<32> {
    let mut input = Bytes::new(env);
    let seed_bytes: Bytes = layout.seed.clone().into();
    input.append(&seed_bytes);
    input.append(&Bytes::from_slice(env, &layout.width.to_be_bytes()));
    input.append(&Bytes::from_slice(env, &layout.height.to_be_bytes()));
    input.append(&Bytes::from_slice(env, &layout.total_energy.to_be_bytes()));
    input.append(&Bytes::from_slice(env, &layout.timestamp.to_be_bytes()));
    input.append(&Bytes::from_slice(env, &layout.cells.len().to_be_bytes()));
    env.crypto().sha256(&input).into()
}

/// Emit the NebulaScanned event with player address, layout hash, and rarity.
pub fn emit_nebula_scanned(
    env: &Env,
    player: &Address,
    layout_hash: &BytesN<32>,
    rarity: &Rarity,
) {
    env.events().publish(
        (symbol_short!("nebula"), symbol_short!("scanned")),
        (player.clone(), layout_hash.clone(), rarity.clone()),
    );
}

