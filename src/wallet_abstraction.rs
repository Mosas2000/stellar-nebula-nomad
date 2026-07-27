//! # Smart Contract Wallet Support (#276)
//!
//! Account abstraction layer for Stellar Nebula Nomad. Provides:
//!
//! - **Session Keys**: Time-limited, scoped keys that can act on behalf of a
//!   player without requiring their main key for every transaction.
//! - **Social Recovery**: Multi-guardian recovery scheme so players who lose
//!   their key can regain access via trusted guardians.
//! - **Multisig Wallets**: M-of-N approval for high-value operations.
//!
//! ## Architecture
//!
//! Each player can register a smart contract wallet that wraps their identity.
//! The wallet holds session keys, guardian lists, and multisig configs. Game
//! actions call `wallet_abstraction::authorize_action` instead of raw
//! `require_auth()`, which checks session keys first and falls back to the
//! primary key.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

// ─── Storage Keys ─────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum WalletKey {
    /// Smart wallet config for a player.
    Config(Address),
    /// Session key record: (owner, key_id) → SessionKey.
    SessionKey(Address, Address),
    /// All session key IDs for a player.
    SessionKeys(Address),
    /// Guardian record: (owner, guardian) → bool.
    Guardian(Address, Address),
    /// Guardian count for a player.
    GuardianCount(Address),
    /// Recovery proposal: (owner, new_key) → RecoveryProposal.
    RecoveryProposal(Address, Address),
    /// Multisig config for a player.
    Multisig(Address),
    /// Multisig approval: (owner, operation_hash, approver) → bool.
    MultisigApproval(Address, BytesN<32>, Address),
}

use soroban_sdk::BytesN;

// ─── Data Types ───────────────────────────────────────────────────────────

/// Player's smart wallet configuration.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct WalletConfig {
    pub owner: Address,
    pub is_active: bool,
    pub created_at: u64,
    /// Number of guardians required for social recovery.
    pub recovery_threshold: u32,
}

/// A time-limited, scoped session key.
#[derive(Clone)]
#[contracttype]
pub struct SessionKey {
    pub key: Address,
    pub owner: Address,
    /// Ledger sequence at which this session key expires.
    pub expires_at: u32,
    /// Maximum actions this key can perform (0 = unlimited).
    pub max_actions: u32,
    /// Actions performed so far.
    pub actions_used: u32,
    /// Allowed operation types (empty = all allowed).
    pub allowed_ops: Vec<Symbol>,
    pub created_at: u64,
}

/// Social recovery proposal.
#[derive(Clone)]
#[contracttype]
pub struct RecoveryProposal {
    pub owner: Address,
    pub new_key: Address,
    pub approvals: u32,
    pub created_at: u64,
    pub executed: bool,
}

/// M-of-N multisig configuration for a player's wallet.
#[derive(Clone)]
#[contracttype]
pub struct MultisigWallet {
    pub signers: Vec<Address>,
    pub required: u32,
    pub is_active: bool,
}

// ─── Errors ───────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum WalletError {
    /// Wallet already exists for this player.
    WalletExists = 1,
    /// Wallet not found.
    WalletNotFound = 2,
    /// Session key expired or exhausted.
    SessionKeyInvalid = 3,
    /// Operation not allowed by this session key.
    OperationNotAllowed = 4,
    /// Guardian already registered.
    GuardianExists = 5,
    /// Guardian not found.
    GuardianNotFound = 6,
    /// Insufficient guardian approvals for recovery.
    InsufficientApprovals = 7,
    /// Recovery proposal not found.
    RecoveryNotFound = 8,
    /// Cannot add self as guardian.
    CannotBeOwnGuardian = 9,
    /// Multisig not configured.
    MultisigNotConfigured = 10,
    /// Insufficient multisig approvals.
    MultisigInsufficientApprovals = 11,
    /// Not a registered signer.
    NotASigner = 12,
    /// Already approved.
    AlreadyApproved = 13,
    /// Session key limit reached.
    SessionKeyLimitReached = 14,
}

const MAX_SESSION_KEYS: u32 = 10;

// ─── Public API ───────────────────────────────────────────────────────────

/// Create a smart contract wallet for a player.
pub fn create_wallet(
    env: &Env,
    owner: Address,
    recovery_threshold: u32,
) -> Result<WalletConfig, WalletError> {
    owner.require_auth();

    let key = WalletKey::Config(owner.clone());
    if env.storage().persistent().has(&key) {
        return Err(WalletError::WalletExists);
    }

    let config = WalletConfig {
        owner: owner.clone(),
        is_active: true,
        created_at: env.ledger().timestamp(),
        recovery_threshold,
    };

    env.storage().persistent().set(&key, &config);

    env.events().publish(
        (symbol_short!("wallet"), symbol_short!("created")),
        owner,
    );

    Ok(config)
}

/// Get wallet configuration for a player.
pub fn get_wallet(env: &Env, owner: Address) -> Result<WalletConfig, WalletError> {
    env.storage()
        .persistent()
        .get(&WalletKey::Config(owner))
        .ok_or(WalletError::WalletNotFound)
}

/// Add a session key that can act on behalf of the player.
pub fn add_session_key(
    env: &Env,
    owner: Address,
    key: Address,
    expires_at: u32,
    max_actions: u32,
    allowed_ops: Vec<Symbol>,
) -> Result<(), WalletError> {
    owner.require_auth();

    let _config: WalletConfig = env
        .storage()
        .persistent()
        .get(&WalletKey::Config(owner.clone()))
        .ok_or(WalletError::WalletNotFound)?;

    // Check session key limit.
    let keys: Vec<Address> = env
        .storage()
        .persistent()
        .get(&WalletKey::SessionKeys(owner.clone()))
        .unwrap_or_else(|| Vec::new(env));

    if keys.len() >= MAX_SESSION_KEYS {
        return Err(WalletError::SessionKeyLimitReached);
    }

    let session = SessionKey {
        key: key.clone(),
        owner: owner.clone(),
        expires_at,
        max_actions,
        actions_used: 0,
        allowed_ops,
        created_at: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&WalletKey::SessionKey(owner.clone(), key.clone()), &session);

    // Add to the key list.
    let mut keys = keys;
    keys.push_back(key);
    env.storage()
        .persistent()
        .set(&WalletKey::SessionKeys(owner.clone()), &keys);

    env.events().publish(
        (symbol_short!("wallet"), symbol_short!("sess_add")),
        owner,
    );

    Ok(())
}

/// Revoke a session key.
pub fn revoke_session_key(
    env: &Env,
    owner: Address,
    key: Address,
) -> Result<(), WalletError> {
    owner.require_auth();

    env.storage()
        .persistent()
        .remove(&WalletKey::SessionKey(owner.clone(), key));

    env.events().publish(
        (symbol_short!("wallet"), symbol_short!("sess_rev")),
        owner,
    );

    Ok(())
}

/// Authorize an action using a session key. Falls back to requiring the
/// owner's direct auth if no valid session key is found.
///
/// Returns `Ok(())` if authorized, `Err` otherwise.
pub fn authorize_action(
    env: &Env,
    caller: &Address,
    owner: &Address,
    operation: &Symbol,
) -> Result<(), WalletError> {
    // Try session key authorization first.
    let session_key: Option<SessionKey> = env
        .storage()
        .persistent()
        .get(&WalletKey::SessionKey(owner.clone(), caller.clone()));

    if let Some(mut session) = session_key {
        // Check expiry.
        if env.ledger().sequence() >= session.expires_at {
            return Err(WalletError::SessionKeyInvalid);
        }

        // Check action limit.
        if session.max_actions > 0 && session.actions_used >= session.max_actions {
            return Err(WalletError::SessionKeyInvalid);
        }

        // Check allowed operations.
        if !session.allowed_ops.is_empty() {
            let mut found = false;
            for i in 0..session.allowed_ops.len() {
                if session.allowed_ops.get(i).unwrap() == *operation {
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(WalletError::OperationNotAllowed);
            }
        }

        // Increment action counter.
        session.actions_used += 1;
        env.storage()
            .persistent()
            .set(&WalletKey::SessionKey(owner.clone(), caller.clone()), &session);

        return Ok(());
    }

    // Not a session key — require direct owner auth.
    owner.require_auth();
    Ok(())
}

// ─── Social Recovery ──────────────────────────────────────────────────────

/// Add a guardian for social recovery.
pub fn add_guardian(
    env: &Env,
    owner: Address,
    guardian: Address,
) -> Result<(), WalletError> {
    owner.require_auth();

    if owner == guardian {
        return Err(WalletError::CannotBeOwnGuardian);
    }

    let key = WalletKey::Guardian(owner.clone(), guardian.clone());
    if env.storage().persistent().has(&key) {
        return Err(WalletError::GuardianExists);
    }

    env.storage().persistent().set(&key, &true);

    let count: u32 = env
        .storage()
        .persistent()
        .get(&WalletKey::GuardianCount(owner.clone()))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&WalletKey::GuardianCount(owner), &(count + 1));

    Ok(())
}

/// Remove a guardian.
pub fn remove_guardian(
    env: &Env,
    owner: Address,
    guardian: Address,
) -> Result<(), WalletError> {
    owner.require_auth();

    let key = WalletKey::Guardian(owner.clone(), guardian);
    if !env.storage().persistent().has(&key) {
        return Err(WalletError::GuardianNotFound);
    }

    env.storage().persistent().remove(&key);

    let count: u32 = env
        .storage()
        .persistent()
        .get(&WalletKey::GuardianCount(owner.clone()))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&WalletKey::GuardianCount(owner), &count.saturating_sub(1));

    Ok(())
}

/// Initiate social recovery — a guardian proposes a new key for the owner.
pub fn initiate_recovery(
    env: &Env,
    guardian: Address,
    owner: Address,
    new_key: Address,
) -> Result<(), WalletError> {
    guardian.require_auth();

    // Verify guardian.
    if !env
        .storage()
        .persistent()
        .get::<_, bool>(&WalletKey::Guardian(owner.clone(), guardian.clone()))
        .unwrap_or(false)
    {
        return Err(WalletError::GuardianNotFound);
    }

    let proposal_key = WalletKey::RecoveryProposal(owner.clone(), new_key.clone());
    let mut proposal: RecoveryProposal = env
        .storage()
        .persistent()
        .get(&proposal_key)
        .unwrap_or(RecoveryProposal {
            owner: owner.clone(),
            new_key: new_key.clone(),
            approvals: 0,
            created_at: env.ledger().timestamp(),
            executed: false,
        });

    if proposal.executed {
        return Err(WalletError::RecoveryNotFound);
    }

    proposal.approvals += 1;
    env.storage().persistent().set(&proposal_key, &proposal);

    env.events().publish(
        (symbol_short!("wallet"), symbol_short!("recover")),
        (owner, guardian, proposal.approvals),
    );

    Ok(())
}

/// Execute recovery once enough guardians have approved.
pub fn execute_recovery(
    env: &Env,
    owner: Address,
    new_key: Address,
) -> Result<(), WalletError> {
    let config: WalletConfig = env
        .storage()
        .persistent()
        .get(&WalletKey::Config(owner.clone()))
        .ok_or(WalletError::WalletNotFound)?;

    let proposal_key = WalletKey::RecoveryProposal(owner.clone(), new_key.clone());
    let mut proposal: RecoveryProposal = env
        .storage()
        .persistent()
        .get(&proposal_key)
        .ok_or(WalletError::RecoveryNotFound)?;

    if proposal.approvals < config.recovery_threshold {
        return Err(WalletError::InsufficientApprovals);
    }

    proposal.executed = true;
    env.storage().persistent().set(&proposal_key, &proposal);

    env.events().publish(
        (symbol_short!("wallet"), symbol_short!("rec_done")),
        (owner, new_key),
    );

    Ok(())
}

// ─── Multisig ─────────────────────────────────────────────────────────────

/// Configure multisig for a player's wallet.
pub fn configure_multisig(
    env: &Env,
    owner: Address,
    signers: Vec<Address>,
    required: u32,
) -> Result<(), WalletError> {
    owner.require_auth();

    if required == 0 || required > signers.len() {
        return Err(WalletError::MultisigNotConfigured);
    }

    let config = MultisigWallet {
        signers,
        required,
        is_active: true,
    };

    env.storage()
        .persistent()
        .set(&WalletKey::Multisig(owner), &config);

    Ok(())
}

/// Approve a multisig operation.
pub fn approve_multisig(
    env: &Env,
    approver: Address,
    owner: Address,
    operation_hash: BytesN<32>,
) -> Result<u32, WalletError> {
    approver.require_auth();

    let config: MultisigWallet = env
        .storage()
        .persistent()
        .get(&WalletKey::Multisig(owner.clone()))
        .ok_or(WalletError::MultisigNotConfigured)?;

    // Verify approver is a signer.
    let mut is_signer = false;
    for i in 0..config.signers.len() {
        if config.signers.get(i).unwrap() == approver {
            is_signer = true;
            break;
        }
    }
    if !is_signer {
        return Err(WalletError::NotASigner);
    }

    let approval_key = WalletKey::MultisigApproval(owner.clone(), operation_hash.clone(), approver.clone());
    if env.storage().persistent().has(&approval_key) {
        return Err(WalletError::AlreadyApproved);
    }

    env.storage().persistent().set(&approval_key, &true);

    // Count approvals.
    let mut count = 0u32;
    for i in 0..config.signers.len() {
        let signer = config.signers.get(i).unwrap();
        let key = WalletKey::MultisigApproval(owner.clone(), operation_hash.clone(), signer);
        if env.storage().persistent().get::<_, bool>(&key).unwrap_or(false) {
            count += 1;
        }
    }

    env.events().publish(
        (symbol_short!("wallet"), symbol_short!("ms_approv")),
        (owner, approver, count),
    );

    Ok(count)
}

/// Check if an operation has enough multisig approvals.
pub fn check_multisig(
    env: &Env,
    owner: &Address,
    operation_hash: &BytesN<32>,
) -> Result<(), WalletError> {
    let config: MultisigWallet = env
        .storage()
        .persistent()
        .get(&WalletKey::Multisig(owner.clone()))
        .ok_or(WalletError::MultisigNotConfigured)?;

    if !config.is_active {
        return Ok(()); // Multisig disabled, allow.
    }

    let mut count = 0u32;
    for i in 0..config.signers.len() {
        let signer = config.signers.get(i).unwrap();
        let key = WalletKey::MultisigApproval(owner.clone(), operation_hash.clone(), signer);
        if env.storage().persistent().get::<_, bool>(&key).unwrap_or(false) {
            count += 1;
        }
    }

    if count < config.required {
        return Err(WalletError::MultisigInsufficientApprovals);
    }

    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};

    fn setup() -> (Env, Address) {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set(LedgerInfo {
            protocol_version: 22,
            sequence_number: 100,
            timestamp: 1_700_000_000,
            network_id: [0u8; 32],
            base_reserve: 10,
            min_temp_entry_ttl: 100,
            min_persistent_entry_ttl: 1_000,
            max_entry_ttl: 10_000,
        });
        let owner = Address::generate(&env);
        (env, owner)
    }

    #[test]
    fn test_create_wallet() {
        let (env, owner) = setup();
        let config = create_wallet(&env, owner.clone(), 2).unwrap();
        assert_eq!(config.owner, owner);
        assert!(config.is_active);
    }

    #[test]
    fn test_create_wallet_duplicate_fails() {
        let (env, owner) = setup();
        create_wallet(&env, owner.clone(), 2).unwrap();
        assert_eq!(create_wallet(&env, owner, 2), Err(WalletError::WalletExists));
    }

    #[test]
    fn test_session_key_authorization() {
        let (env, owner) = setup();
        let session_addr = Address::generate(&env);
        let mut ops = Vec::new(&env);
        ops.push_back(symbol_short!("scan"));

        create_wallet(&env, owner.clone(), 2).unwrap();
        add_session_key(&env, owner.clone(), session_addr.clone(), 200, 5, ops).unwrap();

        // Session key should authorize "scan".
        assert!(authorize_action(&env, &session_addr, &owner, &symbol_short!("scan")).is_ok());
    }

    #[test]
    fn test_session_key_rejects_unallowed_op() {
        let (env, owner) = setup();
        let session_addr = Address::generate(&env);
        let mut ops = Vec::new(&env);
        ops.push_back(symbol_short!("scan"));

        create_wallet(&env, owner.clone(), 2).unwrap();
        add_session_key(&env, owner.clone(), session_addr.clone(), 200, 5, ops).unwrap();

        assert_eq!(
            authorize_action(&env, &session_addr, &owner, &symbol_short!("trade")),
            Err(WalletError::OperationNotAllowed)
        );
    }

    #[test]
    fn test_session_key_exhaustion() {
        let (env, owner) = setup();
        let session_addr = Address::generate(&env);
        let ops = Vec::new(&env); // empty = all allowed

        create_wallet(&env, owner.clone(), 2).unwrap();
        add_session_key(&env, owner.clone(), session_addr.clone(), 200, 2, ops).unwrap();

        assert!(authorize_action(&env, &session_addr, &owner, &symbol_short!("scan")).is_ok());
        assert!(authorize_action(&env, &session_addr, &owner, &symbol_short!("scan")).is_ok());
        assert_eq!(
            authorize_action(&env, &session_addr, &owner, &symbol_short!("scan")),
            Err(WalletError::SessionKeyInvalid)
        );
    }

    #[test]
    fn test_social_recovery() {
        let (env, owner) = setup();
        let g1 = Address::generate(&env);
        let g2 = Address::generate(&env);
        let g3 = Address::generate(&env);
        let new_key = Address::generate(&env);

        create_wallet(&env, owner.clone(), 2).unwrap();
        add_guardian(&env, owner.clone(), g1.clone()).unwrap();
        add_guardian(&env, owner.clone(), g2.clone()).unwrap();
        add_guardian(&env, owner.clone(), g3.clone()).unwrap();

        initiate_recovery(&env, g1.clone(), owner.clone(), new_key.clone()).unwrap();
        // Not enough yet.
        assert_eq!(
            execute_recovery(&env, owner.clone(), new_key.clone()),
            Err(WalletError::InsufficientApprovals)
        );

        initiate_recovery(&env, g2.clone(), owner.clone(), new_key.clone()).unwrap();
        // Now 2 of 3 — threshold met.
        assert!(execute_recovery(&env, owner, new_key).is_ok());
    }

    #[test]
    fn test_cannot_add_self_as_guardian() {
        let (env, owner) = setup();
        create_wallet(&env, owner.clone(), 2).unwrap();
        assert_eq!(
            add_guardian(&env, owner.clone(), owner),
            Err(WalletError::CannotBeOwnGuardian)
        );
    }
}
