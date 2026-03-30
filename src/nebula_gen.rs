use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, symbol_short,
    Address, BytesN, Env, Symbol, Vec,
};

// ─── Resource & Anomaly Types ─────────────────────────────────────────────────

/// Resource abundance tier derived from anomaly rarity score.
/// Mirrors the classification used throughout the architecture docs:
/// sparse (0-33), moderate (34-66), abundant (67-100).
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum ResourceClass {
    Sparse   = 0,
    Moderate = 1,
    Abundant = 2,
}

/// Cosmic phenomena discoverable in a nebula region.
/// Five types ensure variety across exploration runs.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum AnomalyType {
    DustCloud        = 0,
    IonStorm         = 1,
    CrystalFormation = 2,
    PlasmaVent       = 3,
    DarkMatterPocket = 4,
}

/// A single scannable anomaly within a nebula layout.
#[derive(Clone)]
#[contracttype]
pub struct Anomaly {
    /// X coordinate on a 0–999 nebula grid
    pub x: u32,
    /// Y coordinate on a 0–999 nebula grid
    pub y: u32,
    /// Rarity score 0–100; higher = rarer and more rewarding to harvest
    pub rarity: u32,
    pub resource_class: ResourceClass,
    pub anomaly_type: AnomalyType,
}

/// Complete nebula layout generated for one scan session.
#[derive(Clone)]
#[contracttype]
pub struct NebulaLayout {
    /// All anomalies in this layout (length == size)
    pub anomalies: Vec<Anomaly>,
    /// 32-byte fingerprint of this layout — emitted in events for transparency
    pub layout_hash: BytesN<32>,
    /// Stellar ledger timestamp at generation time
    pub generated_at: u64,
    /// Actual number of anomalies (matches anomalies.len())
    pub size: u32,
}

// ─── Config ───────────────────────────────────────────────────────────────────

/// Configurable nebula generation parameters (updatable by admin).
#[derive(Clone)]
#[contracttype]
pub struct NebulaConfig {
    pub admin: Address,
    /// Default anomalies per generated layout (must be in [min_size, max_size])
    pub default_size: u32,
    /// Absolute minimum anomalies per layout
    pub min_size: u32,
    /// Absolute maximum anomalies per layout
    pub max_size: u32,
}

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Config,
    /// Most-recent layout for a ship, keyed by ship_id.
    /// Overwritten on each new scan; use persistent storage so it survives
    /// across multiple transactions in the same session.
    ActiveLayout(u64),
}

// ─── Errors ───────────────────────────────────────────────────────────────────

/// Custom contract error codes (u32-discriminanted for JSON-compatible surfacing).
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum NebulaError {
    NotInitialized     = 1,
    AlreadyInitialized = 2,
    /// Seed is degenerate (all-zero or otherwise unusable)
    InvalidSeed        = 3,
    /// Requested anomaly index is out of bounds
    InvalidIndex       = 4,
    /// No active layout found for the given ship
    LayoutNotFound     = 5,
    /// Requested nebula size is outside the configured [min_size, max_size] range
    InvalidSize        = 6,
}

// ─── PRNG Engine ─────────────────────────────────────────────────────────────

/// splitmix64 finalizer — the same high-quality mix used in Java's
/// SplittableRandom and many game engines.  Deterministic, avalanche-free,
/// and extremely fast.
#[inline]
fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e3779b97f4a7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

/// Derive a deterministic, independent u64 for a given (seed, anomaly_index, salt).
/// Using different `salt` values for different properties prevents correlations
/// between x/y, rarity, and anomaly type even for the same anomaly.
#[inline]
fn derive(seed: u64, index: u32, salt: u64) -> u64 {
    splitmix64(seed ^ splitmix64((index as u64).wrapping_mul(0x9e3779b97f4a7c15) ^ salt))
}

/// Read 8 consecutive bytes from a `BytesN<32>` starting at `offset` and
/// interpret them as a little-endian u64.
fn read_u64_from_seed(seed: &BytesN<32>, offset: u32) -> u64 {
    let mut val: u64 = 0;
    for i in 0..8u32 {
        let b = seed.get(offset + i).unwrap_or(0) as u64;
        val |= b << (i * 8);
    }
    val
}

/// Fold all 32 bytes of the seed into a single u64 by XOR-ing the four 8-byte
/// chunks.  This ensures every bit of the seed influences the master seed.
fn extract_seed(raw: &BytesN<32>) -> u64 {
    read_u64_from_seed(raw, 0)
        ^ read_u64_from_seed(raw, 8)
        ^ read_u64_from_seed(raw, 16)
        ^ read_u64_from_seed(raw, 24)
}

/// Expand a single u64 into a 32-byte `BytesN` to use as the layout hash.
/// Four independent splitmix64 chains produce 8 bytes each.
fn build_layout_hash(env: &Env, h: u64) -> BytesN<32> {
    let h0 = splitmix64(h);
    let h1 = splitmix64(h ^ 0xdead_cafe_1234_5678);
    let h2 = splitmix64(h.wrapping_add(0x1234_5678_dead_beef));
    let h3 = splitmix64(h.wrapping_mul(0x0101_0101_0101_0101).wrapping_add(1));
    let mut arr = [0u8; 32];
    let b0 = h0.to_le_bytes();
    let b1 = h1.to_le_bytes();
    let b2 = h2.to_le_bytes();
    let b3 = h3.to_le_bytes();
    let mut i = 0usize;
    while i < 8 { arr[i]      = b0[i]; i += 1; }
    let mut i = 0usize;
    while i < 8 { arr[8  + i] = b1[i]; i += 1; }
    let mut i = 0usize;
    while i < 8 { arr[16 + i] = b2[i]; i += 1; }
    let mut i = 0usize;
    while i < 8 { arr[24 + i] = b3[i]; i += 1; }
    BytesN::from_array(env, &arr)
}

fn rarity_to_class(rarity: u32) -> ResourceClass {
    if rarity <= 33 {
        ResourceClass::Sparse
    } else if rarity <= 66 {
        ResourceClass::Moderate
    } else {
        ResourceClass::Abundant
    }
}

fn u64_to_anomaly_type(v: u64) -> AnomalyType {
    match v % 5 {
        0 => AnomalyType::DustCloud,
        1 => AnomalyType::IonStorm,
        2 => AnomalyType::CrystalFormation,
        3 => AnomalyType::PlasmaVent,
        _ => AnomalyType::DarkMatterPocket,
    }
}

// ─── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct NebulaGen;

#[contractimpl]
impl NebulaGen {
    // ── Initialisation ───────────────────────────────────────────────────────

    /// Initialise the nebula generator.  Must be called once by the admin.
    ///
    /// # Parameters
    /// - `default_size` – anomalies per layout (clamped to [min_size, max_size])
    /// - `min_size` / `max_size` – hard bounds for future size updates
    pub fn init(
        env: Env,
        admin: Address,
        default_size: u32,
        min_size: u32,
        max_size: u32,
    ) -> Result<(), NebulaError> {
        if env.storage().instance().has(&DataKey::Config) {
            return Err(NebulaError::AlreadyInitialized);
        }
        admin.require_auth();
        if min_size == 0 || min_size > max_size {
            return Err(NebulaError::InvalidSize);
        }
        let clamped = default_size.max(min_size).min(max_size);
        env.storage().instance().set(
            &DataKey::Config,
            &NebulaConfig { admin, default_size: clamped, min_size, max_size },
        );
        Ok(())
    }

    // ── Generation ───────────────────────────────────────────────────────────

    /// Generate an infinite, verifiable nebula layout for `ship_id` scanning
    /// `region_id`.
    ///
    /// ## Seed construction
    ///
    /// ```text
    /// master_seed =
    ///   splitmix64(player_seed_u64)           // caller-supplied entropy
    ///   ^ splitmix64(ledger_sequence)         // block-level unpredictability
    ///   ^ splitmix64(ledger_timestamp)        // wall-clock unpredictability
    ///   ^ splitmix64(ship_id)                 // per-ship uniqueness
    ///   ^ splitmix64(region_id)               // per-region uniqueness
    /// ```
    ///
    /// Mixing ledger sequence and timestamp into the seed means an attacker who
    /// knows `ship_id` and `region_id` still cannot pre-compute the layout until
    /// the exact ledger is closed — providing fairness without a VRF oracle.
    ///
    /// ## Emits
    /// `NebulaGenerated` event: topics `("neb_gen", caller)`, data `(ship_id, layout_hash, size)`
    pub fn generate_nebula_layout(
        env: Env,
        caller: Address,
        ship_id: u64,
        region_id: u64,
        seed: BytesN<32>,
    ) -> Result<NebulaLayout, NebulaError> {
        caller.require_auth();
        let config = Self::require_config(&env)?;

        // Reject degenerate all-zero seed
        let raw_seed = extract_seed(&seed);
        if raw_seed == 0 {
            return Err(NebulaError::InvalidSeed);
        }

        // Mix caller seed with on-chain ledger state (prevents pre-computation)
        let master = splitmix64(raw_seed)
            ^ splitmix64(env.ledger().sequence() as u64)
            ^ splitmix64(env.ledger().timestamp())
            ^ splitmix64(ship_id)
            ^ splitmix64(region_id);

        let size = config.default_size;
        let mut anomalies: Vec<Anomaly> = Vec::new(&env);

        for i in 0..size {
            let x      = (derive(master, i, 0x0000_1111_2222_3333) % 1000) as u32;
            let y      = (derive(master, i, 0x4444_5555_6666_7777) % 1000) as u32;
            let rarity = (derive(master, i, 0x8888_9999_aaaa_bbbb) % 101)  as u32;
            let atype  =  u64_to_anomaly_type(derive(master, i, 0xcccc_dddd_eeee_ffff));

            anomalies.push_back(Anomaly {
                x,
                y,
                rarity,
                resource_class: rarity_to_class(rarity),
                anomaly_type: atype,
            });
        }

        let layout_hash = build_layout_hash(&env, master);
        let layout = NebulaLayout {
            anomalies,
            layout_hash: layout_hash.clone(),
            generated_at: env.ledger().timestamp(),
            size,
        };

        // Overwrite previous layout for this ship
        env.storage()
            .persistent()
            .set(&DataKey::ActiveLayout(ship_id), &layout);

        // Emit NebulaGenerated
        env.events().publish(
            (symbol_short!("neb_gen"), caller),
            (ship_id, layout_hash, size),
        );

        Ok(layout)
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// Return a single anomaly by index from the active layout of `ship_id`.
    pub fn query_anomaly(
        env: Env,
        ship_id: u64,
        index: u32,
    ) -> Result<Anomaly, NebulaError> {
        let layout = Self::require_layout(&env, ship_id)?;
        layout.anomalies.get(index).ok_or(NebulaError::InvalidIndex)
    }

    /// Return `true` when `anomaly_index` is a valid index in the active layout
    /// for `ship_id`.  Called cross-contract by the Resource Minter.
    pub fn has_anomaly(env: Env, ship_id: u64, anomaly_index: u32) -> bool {
        match env
            .storage()
            .persistent()
            .get::<DataKey, NebulaLayout>(&DataKey::ActiveLayout(ship_id))
        {
            Some(l) => anomaly_index < l.size,
            None => false,
        }
    }

    /// Return the full active layout for `ship_id`, or `None` if none exists.
    pub fn get_layout(env: Env, ship_id: u64) -> Option<NebulaLayout> {
        env.storage()
            .persistent()
            .get(&DataKey::ActiveLayout(ship_id))
    }

    pub fn get_config(env: Env) -> Option<NebulaConfig> {
        env.storage().instance().get(&DataKey::Config)
    }

    // ── Admin ─────────────────────────────────────────────────────────────────

    /// Update the default nebula size.  Must be within [min_size, max_size].
    pub fn update_default_size(env: Env, new_size: u32) -> Result<(), NebulaError> {
        let mut config = Self::require_config(&env)?;
        config.admin.require_auth();
        if new_size < config.min_size || new_size > config.max_size {
            return Err(NebulaError::InvalidSize);
        }
        config.default_size = new_size;
        env.storage().instance().set(&DataKey::Config, &config);
        Ok(())
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn require_config(env: &Env) -> Result<NebulaConfig, NebulaError> {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(NebulaError::NotInitialized)
    }

    fn require_layout(env: &Env, ship_id: u64) -> Result<NebulaLayout, NebulaError> {
        env.storage()
            .persistent()
            .get(&DataKey::ActiveLayout(ship_id))
            .ok_or(NebulaError::LayoutNotFound)
    }
}
