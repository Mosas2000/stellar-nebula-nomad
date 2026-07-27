// ============================================================
// nebula_gen.rs — Merged: Input Validation + Lifecycle Management
//
// Resolved from:
//   • all-issues/combined-fixed  (fix/nebula-input-validation, Issue #170)
//   • main  (configurable TTL, admin cleanup, storage rent)
//
// Resolution decisions:
//   • Contract struct name: NebulaGen  (main's rename wins as trunk)
//   • Error enum: unified NebulaError; added InvalidShipId, InvalidRegionId,
//     AnomalyOutOfBounds from combined-fixed into main's set
//   • Input validation (ship_id, region_id, seed) kept in generate_validated_nebula_layout
//   • has_anomaly returns Result<bool, NebulaError> — preserves InvalidShipId
//     signalling while also surfacing expiry (LayoutNotFound)
//   • TTL / extend_ttl / admin sweep logic kept from main
//   • Both test suites merged and deduplicated
// ============================================================

#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, log, symbol_short,
    Address, BytesN, Env, Vec,
};

// ── Constants ────────────────────────────────────────────────

/// Maximum valid region ID.  Keeps storage bounded (Issue #170).
pub const MAX_REGION_ID: u64 = 1_000_000;
/// Minimum valid ship ID (must be > 0).
pub const MIN_SHIP_ID: u64 = 1;
/// Default number of anomalies generated per layout.
pub const DEFAULT_ANOMALY_COUNT: u32 = 16;
/// Default time-to-live for an active layout: 24 hours (in ledger seconds).
pub const DEFAULT_LAYOUT_TTL: u64 = 86_400;

// ── Error enum ───────────────────────────────────────────────

/// Unified contract error codes.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum NebulaError {
    NotInitialized      = 1,
    AlreadyInitialized  = 2,
    /// Seed is degenerate (all-zero bytes).
    InvalidSeed         = 3,
    /// Requested anomaly index is out of bounds for this layout.
    InvalidIndex        = 4,
    /// No active (non-expired) layout found for the given ship.
    LayoutNotFound      = 5,
    /// Requested nebula size is outside the configured [min_size, max_size] range.
    InvalidSize         = 6,
    /// Provided layout TTL is zero / invalid.
    InvalidTtl          = 7,
    /// ship_id must be greater than zero.
    InvalidShipId       = 8,
    /// region_id must be between 1 and MAX_REGION_ID (inclusive).
    InvalidRegionId     = 9,
    /// Anomaly index is out of bounds for this layout.
    AnomalyOutOfBounds  = 10,
}

// ── Data types ───────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourceClass {
    Sparse,
    Moderate,
    Abundant,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AnomalyType {
    DustCloud,
    IonStorm,
    CrystalFormation,
    PlasmaVent,
    DarkMatterPocket,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Anomaly {
    pub x:              u64,
    pub y:              u64,
    pub rarity:         u64,
    pub anomaly_type:   AnomalyType,
    pub resource_class: ResourceClass,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct NebulaLayout {
    pub ship_id:      u64,
    pub region_id:    u64,
    pub layout_hash:  BytesN<32>,
    pub anomalies:    Vec<Anomaly>,
    pub size:         u32,
    /// Ledger timestamp at which the layout was generated.
    pub generated_at: u64,
}

// ── Config ───────────────────────────────────────────────────

/// Configurable nebula generation parameters (updatable by admin).
#[contracttype]
#[derive(Clone)]
pub struct NebulaConfig {
    pub admin:        Address,
    /// Default anomalies per generated layout (clamped to [min_size, max_size]).
    pub default_size: u32,
    /// Absolute minimum anomalies per layout.
    pub min_size:     u32,
    /// Absolute maximum anomalies per layout.
    pub max_size:     u32,
    /// Lifetime of an active layout in seconds.
    pub layout_ttl:   u64,
}

// ── Storage Keys ─────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Config,
    /// Most-recent layout for a ship, keyed by ship_id.
    ActiveLayout(u64),
}

// ── PRNG Engine ──────────────────────────────────────────────

/// SplitMix64 finalizer — bijective, high-quality, deterministic.
#[inline]
fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e3779b97f4a7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

/// Derive a deterministic, independent u64 for (seed, index, salt).
/// Different salt values for x/y/rarity/type prevent inter-property correlations.
#[inline]
fn derive(seed: u64, index: u32, salt: u64) -> u64 {
    splitmix64(seed ^ splitmix64((index as u64).wrapping_mul(0x9e3779b97f4a7c15) ^ salt))
}

// Salt constants for derive()
const SALT_X: u64 = 0x9e3779b97f4a7c15;
const SALT_Y: u64 = 0x6c62272e07bb0142;
const SALT_R: u64 = 0xbf58476d1ce4e5b9;
const SALT_T: u64 = 0x94d049bb133111eb;

/// Read 8 consecutive bytes from a `BytesN<32>` starting at `offset`
/// and interpret them as a little-endian u64.
fn read_u64_from_seed(seed: &BytesN<32>, offset: u32) -> u64 {
    let mut val: u64 = 0;
    for i in 0..8u32 {
        let b = seed.get(offset + i).unwrap_or(0) as u64;
        val |= b << (i * 8);
    }
    val
}

/// Fold all 32 bytes of the seed into a single u64 by XOR-ing the four
/// 8-byte chunks so that every bit of the seed influences generation.
fn extract_seed(raw: &BytesN<32>) -> u64 {
    read_u64_from_seed(raw, 0)
        ^ read_u64_from_seed(raw, 8)
        ^ read_u64_from_seed(raw, 16)
        ^ read_u64_from_seed(raw, 24)
}

/// Expand a u64 into a 32-byte layout hash using four independent splitmix64 chains.
fn build_layout_hash(env: &Env, h: u64) -> BytesN<32> {
    let h0 = splitmix64(h);
    let h1 = splitmix64(h ^ 0xdeadcafe12345678);
    let h2 = splitmix64(h.wrapping_add(0x12345678deadbeef));
    let h3 = splitmix64(h.wrapping_mul(0x0101010101010101).wrapping_add(1));
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

fn u64_to_anomaly_type(v: u64) -> AnomalyType {
    match v % 5 {
        0 => AnomalyType::DustCloud,
        1 => AnomalyType::IonStorm,
        2 => AnomalyType::CrystalFormation,
        3 => AnomalyType::PlasmaVent,
        _ => AnomalyType::DarkMatterPocket,
    }
}

fn rarity_to_class(rarity: u64) -> ResourceClass {
    if rarity <= 33 {
        ResourceClass::Sparse
    } else if rarity <= 66 {
        ResourceClass::Moderate
    } else {
        ResourceClass::Abundant
    }
}

/// Returns `true` when a layout generated at `generated_at` has outlived
/// `ttl` seconds. Saturating addition prevents overflow false-positives.
fn is_expired(now: u64, generated_at: u64, ttl: u64) -> bool {
    now > generated_at.saturating_add(ttl)
}

// ── Contract ─────────────────────────────────────────────────

#[contract]
pub struct NebulaGen;

#[contractimpl]
impl NebulaGen {
    // ── Initialisation ────────────────────────────────────────

    /// Initialise the nebula generator. Must be called once by the admin.
    ///
    /// # Parameters
    /// - `default_size` – anomalies per layout (clamped to [min_size, max_size])
    /// - `min_size` / `max_size` – hard bounds
    /// - `layout_ttl` – layout lifetime in seconds; pass `0` to use [`DEFAULT_LAYOUT_TTL`]
    pub fn init(
        env:          Env,
        admin:        Address,
        default_size: u32,
        min_size:     u32,
        max_size:     u32,
        layout_ttl:   u64,
    ) -> Result<(), NebulaError> {
        if env.storage().instance().has(&DataKey::Config) {
            return Err(NebulaError::AlreadyInitialized);
        }
        admin.require_auth();
        if min_size == 0 || min_size > max_size {
            return Err(NebulaError::InvalidSize);
        }
        let ttl     = if layout_ttl == 0 { DEFAULT_LAYOUT_TTL } else { layout_ttl };
        let clamped = default_size.max(min_size).min(max_size);
        env.storage().instance().set(
            &DataKey::Config,
            &NebulaConfig { admin, default_size: clamped, min_size, max_size, layout_ttl: ttl },
        );
        Ok(())
    }

    // ── Generation ────────────────────────────────────────────

    /// Generate a deterministic nebula layout for a given ship / region.
    ///
    /// # Validation (Issue #170)
    /// - `ship_id`   must be > 0
    /// - `region_id` must be in [1, MAX_REGION_ID]
    /// - `seed`      must not be all-zero bytes
    pub fn generate_validated_nebula_layout(
        env:       Env,
        caller:    Address,
        ship_id:   u64,
        region_id: u64,
        seed:      BytesN<32>,
    ) -> Result<NebulaLayout, NebulaError> {
        let config = Self::require_config(&env)?;

        // ── Require caller authentication ─────────────────────
        caller.require_auth();

        // ── Input validation (Issue #170) ─────────────────────
        if ship_id < MIN_SHIP_ID {
            log!(&env, "NebulaGen: invalid ship_id={}", ship_id);
            return Err(NebulaError::InvalidShipId);
        }
        if region_id < 1 || region_id > MAX_REGION_ID {
            log!(&env, "NebulaGen: invalid region_id={}", region_id);
            return Err(NebulaError::InvalidRegionId);
        }
        // Seed must not be all-zero
        let seed_u64 = extract_seed(&seed);
        if seed_u64 == 0 {
            return Err(NebulaError::InvalidSeed);
        }

        // ── Build entropy master ──────────────────────────────
        let ledger_seq = env.ledger().sequence() as u64;
        let timestamp  = env.ledger().timestamp();

        let master: u64 = splitmix64(seed_u64)
            ^ splitmix64(ledger_seq)
            ^ splitmix64(timestamp)
            ^ splitmix64(ship_id)
            ^ splitmix64(region_id);

        // ── Generate anomalies ────────────────────────────────
        let size = config.default_size;
        let mut anomalies = Vec::new(&env);
        for i in 0..size {
            let x      = derive(master, i, SALT_X) % 1000;
            let y      = derive(master, i, SALT_Y) % 1000;
            let rarity = derive(master, i, SALT_R) % 101;
            let t      = derive(master, i, SALT_T);

            anomalies.push_back(Anomaly {
                x,
                y,
                rarity,
                anomaly_type:   u64_to_anomaly_type(t),
                resource_class: rarity_to_class(rarity),
            });
        }

        // ── Build layout hash ─────────────────────────────────
        let layout_hash = build_layout_hash(&env, master);

        let layout = NebulaLayout {
            ship_id,
            region_id,
            layout_hash: layout_hash.clone(),
            anomalies,
            size,
            generated_at: timestamp,
        };

        // ── Persist layout ────────────────────────────────────
        env.storage()
            .persistent()
            .set(&DataKey::ActiveLayout(ship_id), &layout);

        // Tie storage rent to the configured logical TTL (~5 s per ledger).
        let ttl_ledgers = (config.layout_ttl / 5) as u32;
        env.storage().persistent().extend_ttl(
            &DataKey::ActiveLayout(ship_id),
            ttl_ledgers,
            ttl_ledgers,
        );

        // ── Emit event ────────────────────────────────────────
        env.events().publish(
            (symbol_short!("NebulaGen"), symbol_short!("generated")),
            (ship_id, layout_hash, size),
        );

        log!(&env, "NebulaGen: generated layout ship_id={} region_id={} size={}",
             ship_id, region_id, size);

        Ok(layout)
    }

    // ── Queries ───────────────────────────────────────────────

    /// Return a single anomaly by index from the active layout of `ship_id`.
    /// Expired layouts are cleaned up and treated as absent.
    pub fn query_anomaly(
        env:     Env,
        ship_id: u64,
        index:   u32,
    ) -> Result<Anomaly, NebulaError> {
        if ship_id < MIN_SHIP_ID {
            return Err(NebulaError::InvalidShipId);
        }
        let layout = Self::get_live_layout(&env, ship_id)
            .ok_or(NebulaError::LayoutNotFound)?;
        layout.anomalies.get(index).ok_or(NebulaError::InvalidIndex)
    }

    /// Return the full active layout for `ship_id`, or `None` if absent or expired.
    pub fn get_layout(env: Env, ship_id: u64) -> Option<NebulaLayout> {
        Self::get_live_layout(&env, ship_id)
    }

    /// Check whether `anomaly_index` is valid for `ship_id`'s active layout.
    /// Returns `Err(InvalidShipId)` for ship_id == 0, `Err(LayoutNotFound)` when
    /// no live layout exists, `Err(AnomalyOutOfBounds)` for an out-of-range index,
    /// and `Ok(true)` when the anomaly is present.
    pub fn has_anomaly(
        env:           Env,
        ship_id:       u64,
        anomaly_index: u32,
    ) -> Result<bool, NebulaError> {
        if ship_id < MIN_SHIP_ID {
            return Err(NebulaError::InvalidShipId);
        }
        let layout = Self::get_live_layout(&env, ship_id)
            .ok_or(NebulaError::LayoutNotFound)?;
        if anomaly_index >= layout.size {
            return Err(NebulaError::AnomalyOutOfBounds);
        }
        Ok(true)
    }

    // ── Admin operations ──────────────────────────────────────

    /// Update the active-layout TTL (seconds). Must be non-zero. Admin only.
    pub fn update_layout_ttl(env: Env, new_ttl: u64) -> Result<(), NebulaError> {
        let mut config = Self::require_config(&env)?;
        config.admin.require_auth();
        if new_ttl == 0 {
            return Err(NebulaError::InvalidTtl);
        }
        config.layout_ttl = new_ttl;
        env.storage().instance().set(&DataKey::Config, &config);
        Ok(())
    }

    /// Remove the active layout for `ship_id` if it has expired. Admin only.
    /// Returns `true` when an expired layout was removed.
    pub fn clean_expired_layout(env: Env, ship_id: u64) -> Result<bool, NebulaError> {
        let config = Self::require_config(&env)?;
        config.admin.require_auth();
        Ok(Self::remove_if_expired(&env, &config, ship_id))
    }

    /// Sweep a batch of ship layouts, removing any that have expired. Admin only.
    /// Returns the number of layouts removed.
    pub fn clean_expired_layouts(env: Env, ship_ids: Vec<u64>) -> Result<u32, NebulaError> {
        let config = Self::require_config(&env)?;
        config.admin.require_auth();
        let mut removed = 0u32;
        for i in 0..ship_ids.len() {
            let ship_id = ship_ids.get(i).unwrap();
            if Self::remove_if_expired(&env, &config, ship_id) {
                removed += 1;
            }
        }
        Ok(removed)
    }

    // ── Internal helpers ──────────────────────────────────────

    fn require_config(env: &Env) -> Result<NebulaConfig, NebulaError> {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(NebulaError::NotInitialized)
    }

    /// Fetch the active layout for `ship_id`, lazily removing and returning
    /// `None` if it has expired.
    fn get_live_layout(env: &Env, ship_id: u64) -> Option<NebulaLayout> {
        let layout: NebulaLayout = env
            .storage()
            .persistent()
            .get(&DataKey::ActiveLayout(ship_id))?;

        let ttl = match env.storage().instance().get::<DataKey, NebulaConfig>(&DataKey::Config) {
            Some(c) => c.layout_ttl,
            None    => DEFAULT_LAYOUT_TTL,
        };

        if is_expired(env.ledger().timestamp(), layout.generated_at, ttl) {
            env.storage()
                .persistent()
                .remove(&DataKey::ActiveLayout(ship_id));
            env.events().publish(
                (symbol_short!("neb_gen"), symbol_short!("expired")),
                ship_id,
            );
            return None;
        }
        Some(layout)
    }

    /// Remove the active layout for `ship_id` if expired under `config`.
    fn remove_if_expired(env: &Env, config: &NebulaConfig, ship_id: u64) -> bool {
        let layout: Option<NebulaLayout> = env
            .storage()
            .persistent()
            .get(&DataKey::ActiveLayout(ship_id));
        match layout {
            Some(l) if is_expired(env.ledger().timestamp(), l.generated_at, config.layout_ttl) => {
                env.storage()
                    .persistent()
                    .remove(&DataKey::ActiveLayout(ship_id));
                env.events().publish(
                    (symbol_short!("neb_gen"), symbol_short!("cleaned")),
                    ship_id,
                );
                true
            }
            _ => false,
        }
    }
}

// ── Tests ────────────────────────────────────────────────────
//
// Merged from both branches:
//   • Input validation edge cases (Issue #170) — from all-issues/combined-fixed
//   • TTL expiry / admin lifecycle — from main

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger, LedgerInfo},
        Address, BytesN, Env, Vec,
    };

    const SHORT_TTL: u64 = 100; // seconds

    fn ledger_info(seq: u32, ts: u64) -> LedgerInfo {
        LedgerInfo {
            protocol_version:        22,
            sequence_number:         seq,
            timestamp:               ts,
            network_id:              [0u8; 32],
            base_reserve:            10,
            min_temp_entry_ttl:      16,
            min_persistent_entry_ttl: 16,
            max_entry_ttl:           100_000,
        }
    }

    fn setup() -> (Env, NebulaGenClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set(ledger_info(1, 1_000));
        let id     = env.register(NebulaGen, ());
        let client = NebulaGenClient::new(&env, &id);
        let admin  = Address::generate(&env);
        client.init(&admin, &5u32, &1u32, &10u32, &SHORT_TTL);
        (env, client, admin)
    }

    fn zero_seed(env: &Env) -> BytesN<32> {
        BytesN::from_array(env, &[0u8; 32])
    }

    fn valid_seed(env: &Env) -> BytesN<32> {
        BytesN::from_array(env, &[
            1,  2,  3,  4,  5,  6,  7,  8,
            9,  10, 11, 12, 13, 14, 15, 16,
            17, 18, 19, 20, 21, 22, 23, 24,
            25, 26, 27, 28, 29, 30, 31, 32,
        ])
    }

    fn gen_layout(env: &Env, client: &NebulaGenClient, ship_id: u64) {
        let caller = Address::generate(env);
        client.generate_validated_nebula_layout(&caller, &ship_id, &1u64, &valid_seed(env));
    }

    // ── ship_id validation (Issue #170) ──────────────────────

    #[test]
    fn test_ship_id_zero_rejected() {
        let (env, client, _) = setup();
        let caller = Address::generate(&env);
        let result = client.try_generate_validated_nebula_layout(
            &caller, &0u64, &1u64, &valid_seed(&env),
        );
        assert_eq!(result, Err(Ok(NebulaError::InvalidShipId)));
    }

    #[test]
    fn test_ship_id_one_accepted() {
        let (env, client, _) = setup();
        let caller = Address::generate(&env);
        assert!(client.try_generate_validated_nebula_layout(
            &caller, &1u64, &1u64, &valid_seed(&env),
        ).is_ok());
    }

    #[test]
    fn test_ship_id_max_u64_accepted() {
        let (env, client, _) = setup();
        let caller = Address::generate(&env);
        assert!(client.try_generate_validated_nebula_layout(
            &caller, &u64::MAX, &1u64, &valid_seed(&env),
        ).is_ok());
    }

    // ── region_id validation (Issue #170) ────────────────────

    #[test]
    fn test_region_id_zero_rejected() {
        let (env, client, _) = setup();
        let caller = Address::generate(&env);
        let result = client.try_generate_validated_nebula_layout(
            &caller, &1u64, &0u64, &valid_seed(&env),
        );
        assert_eq!(result, Err(Ok(NebulaError::InvalidRegionId)));
    }

    #[test]
    fn test_region_id_one_accepted() {
        let (env, client, _) = setup();
        let caller = Address::generate(&env);
        assert!(client.try_generate_validated_nebula_layout(
            &caller, &1u64, &1u64, &valid_seed(&env),
        ).is_ok());
    }

    #[test]
    fn test_region_id_max_accepted() {
        let (env, client, _) = setup();
        let caller = Address::generate(&env);
        assert!(client.try_generate_validated_nebula_layout(
            &caller, &1u64, &MAX_REGION_ID, &valid_seed(&env),
        ).is_ok());
    }

    #[test]
    fn test_region_id_exceeds_max_rejected() {
        let (env, client, _) = setup();
        let caller = Address::generate(&env);
        let result = client.try_generate_validated_nebula_layout(
            &caller, &1u64, &(MAX_REGION_ID + 1), &valid_seed(&env),
        );
        assert_eq!(result, Err(Ok(NebulaError::InvalidRegionId)));
    }

    #[test]
    fn test_region_id_u64_max_rejected() {
        let (env, client, _) = setup();
        let caller = Address::generate(&env);
        let result = client.try_generate_validated_nebula_layout(
            &caller, &1u64, &u64::MAX, &valid_seed(&env),
        );
        assert_eq!(result, Err(Ok(NebulaError::InvalidRegionId)));
    }

    // ── seed validation (Issue #170) ─────────────────────────

    #[test]
    fn test_all_zero_seed_rejected() {
        let (env, client, _) = setup();
        let caller = Address::generate(&env);
        let result = client.try_generate_validated_nebula_layout(
            &caller, &1u64, &1u64, &zero_seed(&env),
        );
        assert_eq!(result, Err(Ok(NebulaError::InvalidSeed)));
    }

    // ── combined invalid inputs ───────────────────────────────

    #[test]
    fn test_both_ids_invalid_ship_id_error_first() {
        // ship_id is checked before region_id
        let (env, client, _) = setup();
        let caller = Address::generate(&env);
        let result = client.try_generate_validated_nebula_layout(
            &caller, &0u64, &0u64, &valid_seed(&env),
        );
        assert_eq!(result, Err(Ok(NebulaError::InvalidShipId)));
    }

    // ── has_anomaly validation ────────────────────────────────

    #[test]
    fn test_has_anomaly_ship_id_zero_rejected() {
        let (env, client, _) = setup();
        let result = client.try_has_anomaly(&0u64, &0u32);
        assert_eq!(result, Err(Ok(NebulaError::InvalidShipId)));
    }

    #[test]
    fn test_has_anomaly_layout_not_found() {
        let (env, client, _) = setup();
        let result = client.try_has_anomaly(&99u64, &0u32);
        assert_eq!(result, Err(Ok(NebulaError::LayoutNotFound)));
    }

    #[test]
    fn test_has_anomaly_out_of_bounds() {
        let (env, client, _) = setup();
        gen_layout(&env, &client, 5);
        // default_size = 5, so index 5 is OOB
        let result = client.try_has_anomaly(&5u64, &5u32);
        assert_eq!(result, Err(Ok(NebulaError::AnomalyOutOfBounds)));
    }

    #[test]
    fn test_has_anomaly_valid() {
        let (env, client, _) = setup();
        gen_layout(&env, &client, 5);
        assert_eq!(client.has_anomaly(&5u64, &0u32), Ok(true));
    }

    // ── Determinism ───────────────────────────────────────────

    #[test]
    fn test_same_inputs_produce_same_layout_hash() {
        let (env, client, _) = setup();
        let seed = valid_seed(&env);

        let caller1 = Address::generate(&env);
        let caller2 = Address::generate(&env);
        let l1 = client.generate_validated_nebula_layout(&caller1, &42u64, &100u64, &seed);
        let l2 = client.generate_validated_nebula_layout(&caller2, &42u64, &100u64, &seed);

        assert_eq!(l1.layout_hash, l2.layout_hash);
    }

    // ── TTL / lifecycle (from main) ───────────────────────────

    #[test]
    fn layout_available_before_expiry() {
        let (env, client, _) = setup();
        gen_layout(&env, &client, 7);
        assert!(client.get_layout(&7u64).is_some());
    }

    #[test]
    fn layout_auto_cleaned_after_expiry() {
        let (env, client, _) = setup();
        gen_layout(&env, &client, 7);
        env.ledger().set(ledger_info(2, 1_000 + SHORT_TTL + 1));
        assert!(client.get_layout(&7u64).is_none());
        // Entry is physically removed, not just hidden
        assert_eq!(
            client.try_has_anomaly(&7u64, &0u32),
            Err(Ok(NebulaError::LayoutNotFound))
        );
    }

    #[test]
    fn admin_can_clean_expired_layout() {
        let (env, client, _) = setup();
        gen_layout(&env, &client, 7);
        env.ledger().set(ledger_info(2, 1_000 + SHORT_TTL + 1));
        assert_eq!(client.clean_expired_layout(&7u64), Ok(true));
        // Cleaning an already-removed ship reports false
        assert_eq!(client.clean_expired_layout(&7u64), Ok(false));
    }

    #[test]
    fn admin_batch_cleanup_counts_removed() {
        let (env, client, _) = setup();
        for ship in [10u64, 11, 12] {
            gen_layout(&env, &client, ship);
        }
        env.ledger().set(ledger_info(2, 1_000 + SHORT_TTL + 1));
        let mut ships = Vec::new(&env);
        ships.push_back(10u64);
        ships.push_back(11u64);
        ships.push_back(12u64);
        assert_eq!(client.clean_expired_layouts(&ships), Ok(3u32));
    }

    #[test]
    fn admin_can_update_ttl() {
        let (env, client, _) = setup();
        gen_layout(&env, &client, 7);
        // Extend TTL well past elapsed time; layout must stay live
        client.update_layout_ttl(&1_000_000u64).unwrap();
        env.ledger().set(ledger_info(2, 1_000 + SHORT_TTL + 1));
        assert!(client.get_layout(&7u64).is_some());
    }
}