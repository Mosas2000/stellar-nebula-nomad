use soroban_sdk::{contracterror, contracttype, symbol_short, Address, BytesN, Env, Symbol, Vec};

// ─── Constants ────────────────────────────────────────────────────────────────

/// Seconds a proposed config change must wait before it can be applied.
pub const CONFIG_DELAY_SECONDS: u64 = 3600; // 1 hour default
/// Maximum parameters accepted in a single batch-update call.
pub const MAX_BATCH_PARAMS: u32 = 5;
/// Minimum number of signer approvals required to apply a change.
pub const DEFAULT_MIN_APPROVALS: u32 = 2;

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ConfigError {
    NotInitialized     = 1,
    AlreadyInitialized = 2,
    /// Param name not in the allowed-parameter whitelist
    InvalidParam       = 3,
    Unauthorized       = 4,
    /// Change is proposed but waiting for enough approvals
    PendingApproval    = 5,
    /// Change has approvals but the time lock has not yet expired
    TimeLockActive     = 6,
    /// Batch exceeds MAX_BATCH_PARAMS
    BatchTooLarge      = 7,
    /// No pending update for this param
    NoPendingUpdate    = 8,
    /// Signer has already approved this param update
    AlreadyApproved    = 9,
}

// ─── Data types ───────────────────────────────────────────────────────────────

/// A proposed — but not yet live — config change.
#[derive(Clone, Debug)]
#[contracttype]
pub struct PendingConfig {
    pub param: Symbol,
    pub new_value: BytesN<64>,
    /// Timestamp when the proposal was created
    pub proposed_at: u64,
    /// Earliest ledger timestamp at which apply is allowed
    pub apply_after: u64,
    /// Current approval count
    pub approvals: u32,
}

/// A confirmed, live config entry.
#[derive(Clone, Debug)]
#[contracttype]
pub struct LiveConfig {
    pub param: Symbol,
    pub value: BytesN<64>,
    /// Timestamp when this value became active
    pub applied_at: u64,
    /// Number of approvals it received before going live
    pub approvals: u32,
}

/// One entry in a batch-update request.
#[derive(Clone, Debug)]
#[contracttype]
pub struct ConfigUpdate {
    pub param: Symbol,
    pub value: BytesN<64>,
}

/// Contract-level configuration for the config updater itself.
#[derive(Clone, Debug)]
#[contracttype]
pub struct UpdaterConfig {
    pub admin: Address,
    /// Seconds between proposal and earliest apply time
    pub delay_seconds: u64,
    /// Approvals required before apply is possible
    pub min_approvals: u32,
}

/// Storage keys.
#[derive(Clone)]
#[contracttype]
pub enum ConfigKey {
    /// Global updater configuration
    Meta,
    /// Whether an address is a registered signer
    Signer(Address),
    /// Active (live) value for a named parameter
    Live(Symbol),
    /// Pending (proposed, not yet applied) change for a parameter
    Pending(Symbol),
    /// Per-(param, signer) approval flag
    Approval(Symbol, Address),
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

fn require_meta(env: &Env) -> Result<UpdaterConfig, ConfigError> {
    env.storage()
        .instance()
        .get(&ConfigKey::Meta)
        .ok_or(ConfigError::NotInitialized)
}

fn is_signer(env: &Env, addr: &Address) -> bool {
    env.storage()
        .persistent()
        .get::<ConfigKey, bool>(&ConfigKey::Signer(addr.clone()))
        .unwrap_or(false)
}

// Param validation uses an open allowlist in this implementation.
// Production deployments should store an explicit allowlist in persistent
// storage and check it inside update_config.  Governance can gate new params
// via the existing proposal / approval flow.

// ─── Public API ───────────────────────────────────────────────────────────────

/// Initialise the config updater. Must be called once.
///
/// - `admin`         – address allowed to add/remove signers and propose changes
/// - `signers`       – initial set of multi-sig approvers (can be empty list,
///                     admin can add more later)
/// - `delay_seconds` – time lock between proposal and apply (0 = no delay)
/// - `min_approvals` – number of signer approvals required before apply
pub fn initialize_config(
    env: &Env,
    admin: &Address,
    signers: Vec<Address>,
    delay_seconds: u64,
    min_approvals: u32,
) -> Result<(), ConfigError> {
    if env.storage().instance().has(&ConfigKey::Meta) {
        return Err(ConfigError::AlreadyInitialized);
    }
    admin.require_auth();
    let cfg = UpdaterConfig {
        admin: admin.clone(),
        delay_seconds,
        min_approvals,
    };
    env.storage().instance().set(&ConfigKey::Meta, &cfg);
    for i in 0..signers.len() {
        let s = signers.get(i).unwrap();
        env.storage()
            .persistent()
            .set(&ConfigKey::Signer(s), &true);
    }
    Ok(())
}

/// Add a new authorised signer. Admin only.
pub fn add_signer(env: &Env, admin: &Address, signer: &Address) -> Result<(), ConfigError> {
    let cfg = require_meta(env)?;
    if &cfg.admin != admin {
        return Err(ConfigError::Unauthorized);
    }
    admin.require_auth();
    env.storage()
        .persistent()
        .set(&ConfigKey::Signer(signer.clone()), &true);
    Ok(())
}

/// Remove a signer. Admin only.
pub fn remove_signer(env: &Env, admin: &Address, signer: &Address) -> Result<(), ConfigError> {
    let cfg = require_meta(env)?;
    if &cfg.admin != admin {
        return Err(ConfigError::Unauthorized);
    }
    admin.require_auth();
    env.storage()
        .persistent()
        .remove(&ConfigKey::Signer(signer.clone()));
    Ok(())
}

/// Propose a config change.  The caller must be the admin or a registered signer.
///
/// Creates a `PendingConfig` entry with `apply_after = now + delay_seconds`.
/// The change cannot be applied until:
///   1. `approvals >= min_approvals`
///   2. `ledger_timestamp >= apply_after`
///
/// Emits `ConfigUpdated` event with topics `("cfg", "proposed")`.
pub fn update_config(
    env: &Env,
    caller: &Address,
    param: Symbol,
    value: BytesN<64>,
) -> Result<(), ConfigError> {
    let cfg = require_meta(env)?;
    let is_admin = &cfg.admin == caller;
    if !is_admin && !is_signer(env, caller) {
        return Err(ConfigError::Unauthorized);
    }
    caller.require_auth();

    let now = env.ledger().timestamp();
    let apply_after = now + cfg.delay_seconds;

    let pending = PendingConfig {
        param: param.clone(),
        new_value: value,
        proposed_at: now,
        apply_after,
        approvals: 0,
    };
    env.storage()
        .persistent()
        .set(&ConfigKey::Pending(param.clone()), &pending);

    env.events().publish(
        (symbol_short!("cfg"), symbol_short!("proposed")),
        (caller.clone(), param.clone(), now, apply_after),
    );
    Ok(())
}

/// Approve a pending config change.  Must be a registered signer.
/// Each signer may approve a given param only once per pending cycle.
pub fn approve_config_update(
    env: &Env,
    signer: &Address,
    param: Symbol,
) -> Result<u32, ConfigError> {
    if !is_signer(env, signer) {
        return Err(ConfigError::Unauthorized);
    }
    signer.require_auth();

    let approval_key = ConfigKey::Approval(param.clone(), signer.clone());
    if env
        .storage()
        .persistent()
        .get::<ConfigKey, bool>(&approval_key)
        .unwrap_or(false)
    {
        return Err(ConfigError::AlreadyApproved);
    }

    let mut pending: PendingConfig = env
        .storage()
        .persistent()
        .get(&ConfigKey::Pending(param.clone()))
        .ok_or(ConfigError::NoPendingUpdate)?;

    pending.approvals += 1;
    env.storage()
        .persistent()
        .set(&ConfigKey::Pending(param.clone()), &pending);
    env.storage()
        .persistent()
        .set(&approval_key, &true);

    env.events().publish(
        (symbol_short!("cfg"), symbol_short!("approved")),
        (signer.clone(), param, pending.approvals),
    );
    Ok(pending.approvals)
}

/// Apply a pending config change, making it the live value.
///
/// Requires:
/// - A `PendingConfig` entry for `param`
/// - `approvals >= min_approvals`
/// - `ledger_timestamp >= apply_after`
///
/// Emits `ConfigApplied` event with topics `("cfg", "applied")`.
pub fn apply_config_update(env: &Env, param: Symbol) -> Result<LiveConfig, ConfigError> {
    let cfg = require_meta(env)?;

    let pending: PendingConfig = env
        .storage()
        .persistent()
        .get(&ConfigKey::Pending(param.clone()))
        .ok_or(ConfigError::NoPendingUpdate)?;

    if pending.approvals < cfg.min_approvals {
        return Err(ConfigError::PendingApproval);
    }
    if env.ledger().timestamp() < pending.apply_after {
        return Err(ConfigError::TimeLockActive);
    }

    let live = LiveConfig {
        param: param.clone(),
        value: pending.new_value,
        applied_at: env.ledger().timestamp(),
        approvals: pending.approvals,
    };
    env.storage()
        .persistent()
        .set(&ConfigKey::Live(param.clone()), &live);
    env.storage()
        .persistent()
        .remove(&ConfigKey::Pending(param.clone()));

    env.events().publish(
        (symbol_short!("cfg"), symbol_short!("applied")),
        (param, live.applied_at, live.approvals),
    );
    Ok(live)
}

/// Batch-propose up to `MAX_BATCH_PARAMS` config changes in a single call.
pub fn batch_update_config(
    env: &Env,
    caller: &Address,
    updates: Vec<ConfigUpdate>,
) -> Result<u32, ConfigError> {
    if updates.len() > MAX_BATCH_PARAMS {
        return Err(ConfigError::BatchTooLarge);
    }
    caller.require_auth();
    let count = updates.len();
    for i in 0..count {
        let u = updates.get(i).unwrap();
        update_config(env, caller, u.param, u.value)?;
    }
    Ok(count)
}

/// Return the currently live value for `param`, or `None`.
pub fn get_config_value(env: &Env, param: Symbol) -> Option<BytesN<64>> {
    env.storage()
        .persistent()
        .get::<ConfigKey, LiveConfig>(&ConfigKey::Live(param))
        .map(|lc| lc.value)
}

/// Return the pending (proposed but not yet applied) entry for `param`, or `None`.
pub fn get_pending_update(env: &Env, param: Symbol) -> Option<PendingConfig> {
    env.storage()
        .persistent()
        .get(&ConfigKey::Pending(param))
}

/// Return the full live config entry for `param`, or `None`.
pub fn get_live_config(env: &Env, param: Symbol) -> Option<LiveConfig> {
    env.storage()
        .persistent()
        .get(&ConfigKey::Live(param))
}

/// Cancel a pending update.  Admin only.
pub fn rollback_config(env: &Env, admin: &Address, param: Symbol) -> Result<(), ConfigError> {
    let cfg = require_meta(env)?;
    if &cfg.admin != admin {
        return Err(ConfigError::Unauthorized);
    }
    admin.require_auth();
    if !env
        .storage()
        .persistent()
        .has(&ConfigKey::Pending(param.clone()))
    {
        return Err(ConfigError::NoPendingUpdate);
    }
    env.storage()
        .persistent()
        .remove(&ConfigKey::Pending(param.clone()));

    env.events().publish(
        (symbol_short!("cfg"), symbol_short!("rolled")),
        (admin.clone(), param),
    );
    Ok(())
}

pub fn get_updater_config(env: &Env) -> Option<UpdaterConfig> {
    env.storage().instance().get(&ConfigKey::Meta)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger, LedgerInfo},
        Address, BytesN, Env, Symbol,
    };

    fn make_value(env: &Env, v: u8) -> BytesN<64> {
        BytesN::from_array(env, &[v; 64])
    }

    fn param(env: &Env, s: &str) -> Symbol {
        Symbol::new(env, s)
    }

    fn advance_time(env: &Env, secs: u64) {
        let ts = env.ledger().timestamp();
        env.ledger().set(LedgerInfo {
            timestamp: ts + secs,
            sequence_number: env.ledger().sequence() + 1,
            protocol_version: 22,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 16,
            min_persistent_entry_ttl: 4096,
            max_entry_ttl: 6_312_000,
        });
    }

    /// Setup: admin + 2 signers, 1h delay, 2 approvals required
    fn setup(env: &Env) -> (Address, Address, Address) {
        env.mock_all_auths();
        let admin   = Address::generate(env);
        let signer1 = Address::generate(env);
        let signer2 = Address::generate(env);
        let mut signers: Vec<Address> = Vec::new(env);
        signers.push_back(signer1.clone());
        signers.push_back(signer2.clone());
        initialize_config(
            env,
            &admin,
            signers,
            CONFIG_DELAY_SECONDS,
            DEFAULT_MIN_APPROVALS,
        )
        .unwrap();
        (admin, signer1, signer2)
    }

    // ── Init ────────────────────────────────────────────────────────────────

    #[test]
    fn test_init_stores_meta() {
        let env = Env::default();
        let (admin, _, _) = setup(&env);
        let meta = get_updater_config(&env).unwrap();
        assert_eq!(meta.admin, admin);
        assert_eq!(meta.delay_seconds, CONFIG_DELAY_SECONDS);
        assert_eq!(meta.min_approvals, DEFAULT_MIN_APPROVALS);
    }

    #[test]
    fn test_double_init_rejected() {
        let env = Env::default();
        let (admin, s1, s2) = setup(&env);
        let mut signers: Vec<Address> = Vec::new(&env);
        signers.push_back(s1);
        signers.push_back(s2);
        let err = initialize_config(&env, &admin, signers, 0, 1).unwrap_err();
        assert_eq!(err, ConfigError::AlreadyInitialized);
    }

    #[test]
    fn test_init_registers_signers() {
        let env = Env::default();
        let (_, s1, s2) = setup(&env);
        assert!(is_signer(&env, &s1));
        assert!(is_signer(&env, &s2));
    }

    // ── Signer management ───────────────────────────────────────────────────

    #[test]
    fn test_add_and_remove_signer() {
        let env = Env::default();
        let (admin, _, _) = setup(&env);
        let new_signer = Address::generate(&env);
        add_signer(&env, &admin, &new_signer).unwrap();
        assert!(is_signer(&env, &new_signer));
        remove_signer(&env, &admin, &new_signer).unwrap();
        assert!(!is_signer(&env, &new_signer));
    }

    #[test]
    fn test_non_admin_cannot_add_signer() {
        let env = Env::default();
        let (_, s1, _) = setup(&env);
        let outsider = Address::generate(&env);
        let err = add_signer(&env, &s1, &outsider).unwrap_err();
        assert_eq!(err, ConfigError::Unauthorized);
    }

    // ── Propose (update_config) ──────────────────────────────────────────────

    #[test]
    fn test_admin_can_propose() {
        let env = Env::default();
        let (admin, _, _) = setup(&env);
        let p = param(&env, "harvest_cap");
        update_config(&env, &admin, p.clone(), make_value(&env, 1)).unwrap();
        assert!(get_pending_update(&env, p).is_some());
    }

    #[test]
    fn test_signer_can_propose() {
        let env = Env::default();
        let (_, s1, _) = setup(&env);
        let p = param(&env, "base_apy");
        update_config(&env, &s1, p.clone(), make_value(&env, 5)).unwrap();
        assert!(get_pending_update(&env, p).is_some());
    }

    #[test]
    fn test_non_signer_cannot_propose() {
        let env = Env::default();
        setup(&env);
        let outsider = Address::generate(&env);
        let err = update_config(&env, &outsider, param(&env, "scan_range"), make_value(&env, 9)).unwrap_err();
        assert_eq!(err, ConfigError::Unauthorized);
    }

    #[test]
    fn test_pending_has_correct_apply_after() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set(LedgerInfo {
            timestamp: 1_000_000,
            sequence_number: 1,
            protocol_version: 22,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 16,
            min_persistent_entry_ttl: 4096,
            max_entry_ttl: 6_312_000,
        });
        let admin = Address::generate(&env);
        let mut signers: Vec<Address> = Vec::new(&env);
        initialize_config(&env, &admin, signers, 7200, 1).unwrap();

        let p = param(&env, "harvest_cap");
        update_config(&env, &admin, p.clone(), make_value(&env, 1)).unwrap();
        let pending = get_pending_update(&env, p).unwrap();
        assert_eq!(pending.apply_after, 1_000_000 + 7200);
    }

    // ── Approve ─────────────────────────────────────────────────────────────

    #[test]
    fn test_signer_can_approve() {
        let env = Env::default();
        let (admin, s1, _) = setup(&env);
        let p = param(&env, "harvest_cap");
        update_config(&env, &admin, p.clone(), make_value(&env, 1)).unwrap();
        let count = approve_config_update(&env, &s1, p).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_double_approve_rejected() {
        let env = Env::default();
        let (admin, s1, _) = setup(&env);
        let p = param(&env, "base_apy");
        update_config(&env, &admin, p.clone(), make_value(&env, 2)).unwrap();
        approve_config_update(&env, &s1, p.clone()).unwrap();
        let err = approve_config_update(&env, &s1, p).unwrap_err();
        assert_eq!(err, ConfigError::AlreadyApproved);
    }

    #[test]
    fn test_approve_with_no_pending_fails() {
        let env = Env::default();
        let (_, s1, _) = setup(&env);
        let err = approve_config_update(&env, &s1, param(&env, "scan_range")).unwrap_err();
        assert_eq!(err, ConfigError::NoPendingUpdate);
    }

    #[test]
    fn test_non_signer_cannot_approve() {
        let env = Env::default();
        let (admin, _, _) = setup(&env);
        let p = param(&env, "tx_fee_bps");
        update_config(&env, &admin, p.clone(), make_value(&env, 3)).unwrap();
        let outsider = Address::generate(&env);
        let err = approve_config_update(&env, &outsider, p).unwrap_err();
        assert_eq!(err, ConfigError::Unauthorized);
    }

    // ── Apply ────────────────────────────────────────────────────────────────

    #[test]
    fn test_apply_requires_enough_approvals() {
        let env = Env::default();
        let (admin, s1, _) = setup(&env);
        let p = param(&env, "harvest_cap");
        update_config(&env, &admin, p.clone(), make_value(&env, 7)).unwrap();
        // Only 1 approval, need 2
        approve_config_update(&env, &s1, p.clone()).unwrap();
        advance_time(&env, CONFIG_DELAY_SECONDS + 1);
        let err = apply_config_update(&env, p).unwrap_err();
        assert_eq!(err, ConfigError::PendingApproval);
    }

    #[test]
    fn test_apply_requires_time_lock_elapsed() {
        let env = Env::default();
        let (admin, s1, s2) = setup(&env);
        let p = param(&env, "harvest_cap");
        update_config(&env, &admin, p.clone(), make_value(&env, 8)).unwrap();
        approve_config_update(&env, &s1, p.clone()).unwrap();
        approve_config_update(&env, &s2, p.clone()).unwrap();
        // NOT advancing time — time lock still active
        let err = apply_config_update(&env, p).unwrap_err();
        assert_eq!(err, ConfigError::TimeLockActive);
    }

    #[test]
    fn test_successful_apply() {
        let env = Env::default();
        let (admin, s1, s2) = setup(&env);
        let p = param(&env, "base_apy");
        let val = make_value(&env, 42);
        update_config(&env, &admin, p.clone(), val.clone()).unwrap();
        approve_config_update(&env, &s1, p.clone()).unwrap();
        approve_config_update(&env, &s2, p.clone()).unwrap();
        advance_time(&env, CONFIG_DELAY_SECONDS + 1);
        let live = apply_config_update(&env, p.clone()).unwrap();
        assert_eq!(live.value, val);
        assert_eq!(live.approvals, 2);
        // Pending must be cleared
        assert!(get_pending_update(&env, p.clone()).is_none());
        // Live must be set
        assert_eq!(get_config_value(&env, p).unwrap(), val);
    }

    #[test]
    fn test_apply_no_pending_fails() {
        let env = Env::default();
        setup(&env);
        let err = apply_config_update(&env, param(&env, "max_hops")).unwrap_err();
        assert_eq!(err, ConfigError::NoPendingUpdate);
    }

    // ── Batch update ────────────────────────────────────────────────────────

    #[test]
    fn test_batch_update_proposes_all() {
        let env = Env::default();
        let (admin, _, _) = setup(&env);
        let mut updates: Vec<ConfigUpdate> = Vec::new(&env);
        for i in 0..5u8 {
            updates.push_back(ConfigUpdate {
                param: param(&env, if i == 0 { "harvest_cap" }
                              else if i == 1 { "base_apy" }
                              else if i == 2 { "scan_range" }
                              else if i == 3 { "max_hops" }
                              else { "tx_fee_bps" }),
                value: make_value(&env, i),
            });
        }
        let count = batch_update_config(&env, &admin, updates).unwrap();
        assert_eq!(count, 5);
    }

    #[test]
    fn test_batch_too_large_rejected() {
        let env = Env::default();
        let (admin, _, _) = setup(&env);
        let mut updates: Vec<ConfigUpdate> = Vec::new(&env);
        for i in 0..(MAX_BATCH_PARAMS + 1) {
            updates.push_back(ConfigUpdate {
                param: Symbol::new(&env, "harvest_cap"),
                value: make_value(&env, i as u8),
            });
        }
        let err = batch_update_config(&env, &admin, updates).unwrap_err();
        assert_eq!(err, ConfigError::BatchTooLarge);
    }

    // ── Rollback ─────────────────────────────────────────────────────────────

    #[test]
    fn test_admin_can_rollback() {
        let env = Env::default();
        let (admin, _, _) = setup(&env);
        let p = param(&env, "scan_range");
        update_config(&env, &admin, p.clone(), make_value(&env, 1)).unwrap();
        rollback_config(&env, &admin, p.clone()).unwrap();
        assert!(get_pending_update(&env, p).is_none());
    }

    #[test]
    fn test_rollback_with_no_pending_fails() {
        let env = Env::default();
        let (admin, _, _) = setup(&env);
        let err = rollback_config(&env, &admin, param(&env, "scan_range")).unwrap_err();
        assert_eq!(err, ConfigError::NoPendingUpdate);
    }

    #[test]
    fn test_non_admin_cannot_rollback() {
        let env = Env::default();
        let (admin, s1, _) = setup(&env);
        let p = param(&env, "energy_rate");
        update_config(&env, &admin, p.clone(), make_value(&env, 1)).unwrap();
        let err = rollback_config(&env, &s1, p).unwrap_err();
        assert_eq!(err, ConfigError::Unauthorized);
    }

    // ── Full lifecycle ───────────────────────────────────────────────────────

    #[test]
    fn test_full_config_lifecycle() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set(LedgerInfo {
            timestamp: 5_000_000,
            sequence_number: 50,
            protocol_version: 22,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 16,
            min_persistent_entry_ttl: 4096,
            max_entry_ttl: 6_312_000,
        });
        let admin   = Address::generate(&env);
        let signer1 = Address::generate(&env);
        let signer2 = Address::generate(&env);
        let mut signers: Vec<Address> = Vec::new(&env);
        signers.push_back(signer1.clone());
        signers.push_back(signer2.clone());
        initialize_config(&env, &admin, signers, 3600, 2).unwrap();

        let p   = param(&env, "prize_share");
        let val = make_value(&env, 25);

        // 1. Propose
        assert!(get_config_value(&env, p.clone()).is_none());
        update_config(&env, &admin, p.clone(), val.clone()).unwrap();
        let pending = get_pending_update(&env, p.clone()).unwrap();
        assert_eq!(pending.approvals, 0);

        // 2. Approve (1 of 2)
        approve_config_update(&env, &signer1, p.clone()).unwrap();

        // 3. Try apply — not enough approvals yet
        advance_time(&env, 3601);
        let err = apply_config_update(&env, p.clone()).unwrap_err();
        assert_eq!(err, ConfigError::PendingApproval);

        // 4. Second approval
        approve_config_update(&env, &signer2, p.clone()).unwrap();

        // 5. Apply succeeds
        let live = apply_config_update(&env, p.clone()).unwrap();
        assert_eq!(live.value, val);
        assert!(get_config_value(&env, p.clone()).is_some());
        assert!(get_pending_update(&env, p).is_none());
    }

    #[test]
    fn test_zero_delay_immediate_apply() {
        let env = Env::default();
        env.mock_all_auths();
        let admin   = Address::generate(&env);
        let signer1 = Address::generate(&env);
        let mut signers: Vec<Address> = Vec::new(&env);
        signers.push_back(signer1.clone());
        initialize_config(&env, &admin, signers, 0, 1).unwrap(); // no delay, 1 approval

        let p = param(&env, "min_stake");
        update_config(&env, &admin, p.clone(), make_value(&env, 100)).unwrap();
        approve_config_update(&env, &signer1, p.clone()).unwrap();
        // No time advance needed — delay = 0
        let live = apply_config_update(&env, p.clone()).unwrap();
        assert_eq!(live.value, make_value(&env, 100));
    }
}
