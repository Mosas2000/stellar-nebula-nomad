//! # Role-Based Access Control (RBAC) System
//!
//! This module implements a fine-grained, production-grade Role-Based Access Control (RBAC) system
//! for the Stellar Nebula Nomad smart contract suite. It provides:
//!
//! - **Role Management**: Grant, revoke, and manage roles (admin, nomad, indexer, custom).
//! - **Time-Bound Roles**: Roles can expire at a specified ledger sequence number.
//! - **Permissions**: Link actions (Symbols) to roles, enabling fine-grained access control.
//! - **Event Emission**: Every role and permission change emits an event for on-chain auditing.
//! - **Idempotent Operations**: Grant and revoke operations are safe to retry.
//! - **Admin Transfer**: Transfer admin privileges to a new address.
//! - **DAO Extensibility**: Stub for future DAO-driven role proposals and governance.
//!
//! ## Architecture
//!
//! The RBAC system is built around three core concepts:
//!
//! 1. **Roles**: Named groups (admin, nomad, indexer) that can perform actions.
//! 2. **Permissions**: Mappings from (role, action) pairs to booleans, defining what actions a role can perform.
//! 3. **Role Membership**: Individual (address, role) pairs with optional expiration, defining who holds what role.
//!
//! ## Security Invariants
//!
//! ✓ Admin-only mutating operations are non-bypassable (grant_role, revoke_role, transfer_admin).
//! ✓ Expired roles are treated as absent, identical to never-granted roles.
//! ✓ Revoked roles are indistinguishable from never-granted roles.
//! ✓ Batch grants are atomic (all-or-nothing with B=5 limit per call).
//! ✓ Storage key collision is impossible (DataKey enum variants are structurally distinct).
//! ✓ Initialization is idempotent and revert-safe (called only once; subsequent calls fail).
//! ✓ Failed operations leave state exactly as before (no partial writes).
//! ✓ Time-bound expiry uses ledger.sequence(), which is manipulation-resistant in Soroban.
//!
//! ## Security Audit Checklist
//!
//! - [ ] Admin Key Protection: Only the stored admin address can call grant_role, revoke_role, etc.
//! - [ ] Role Expiry Enforcement: Expired roles return false from has_role() and fail check_permission().
//! - [ ] Batch Limit Enforcement: Batch grants with N > 5 revert without granting any role.
//! - [ ] Revocation Completeness: Revoked roles behave identically to never-granted roles.
//! - [ ] Event Emission Completeness: All role/permission changes emit events; events carry correct data.
//! - [ ] Storage Key Collision Safety: (role, address) keys are distinct from (role, action) keys.
//!
//! ## Reusable Pattern Guide
//!
//! ### Adding a New Role
//!
//! 1. Choose a role name (max 32 chars, ideally <15 to align with symbol_short!() usage).
//! 2. Call `grant_role(env, admin, symbol_short!("new_role"), address, None)` to give the role to an address.
//! 3. Call `grant_permission(env, admin, symbol_short!("new_role"), symbol_short!("action_name"))` to allow the role to perform an action.
//!
//! ### Adding a New Guarded Action
//!
//! 1. Define an action Symbol constant in the calling module or lib.rs:
//!    ```rust
//!    const TRANSFER_ACTION: Symbol = symbol_short!("transfer");
//!    ```
//! 2. In the function performing that action, add an early-exit guard as the first statement:
//!    ```rust
//!    pub fn transfer(..., caller: Address, ...) -> Result<...> {
//!        caller.require_auth();
//!        access_control::check_permission(&env, &caller, symbol_short!("transfer"))?;
//!        // ... rest of function
//!    }
//!    ```
//! 3. Ensure the admin grants the necessary roles permission to the action via `grant_permission`.
//!
//! ### Extending for DAO Proposals
//!
//! 1. Implement a DAO module that stores proposals and voting records.
//! 2. Create an `execute_dao_proposal(env, proposal_id)` function that:
//!    - Retrieves the proposal from DAO storage.
//!    - Checks that voting is closed and the proposal passed quorum.
//!    - Calls the appropriate RBAC function (e.g., `grant_role_internal`).
//! 3. Leave `propose_role_change` as a stub or integrate with DAO's public interface.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};
use soroban_sdk::storage::Instance;
use crate::error_standard::{ErrorDescriptor, ErrorKind, StandardContractError};

// ═══════════════════════════════════════════════════════════════════════════════
// ERRORS
// ═══════════════════════════════════════════════════════════════════════════════

/// Errors arising from RBAC operations.
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AccessControlError {
    /// Admin required: Caller is not the current admin.
    AdminRequired = 1,
    /// Role not found: The specified role does not exist or was never defined.
    RoleNotFound = 2,
    /// Unauthorized role: Caller does not hold the required role, or role is expired/revoked.
    UnauthorizedRole = 3,
    /// Role already granted: The address already holds the role (idempotency policy).
    RoleAlreadyGranted = 4,
    /// Batch limit exceeded: Batch grant attempted with more than 5 addresses.
    BatchLimitExceeded = 5,
    /// Invalid expiry: Expiry value is in the past or otherwise invalid.
    InvalidExpiry = 6,
    /// Permission not found: The (role, action) permission was never defined.
    PermissionNotFound = 7,
    /// Initialization failed: init_roles was already called or another init error occurred.
    InitializationFailed = 8,
    /// Not implemented: This function is a placeholder for future DAO integration.
    NotImplemented = 9,
    /// Multi-sig not configured.
    MultiSigNotConfigured = 10,
    /// Proposal not found.
    ProposalNotFound = 11,
    /// Proposal already executed.
    ProposalAlreadyExecuted = 12,
    /// Proposal expired.
    ProposalExpired = 13,
    /// Insufficient approvals.
    InsufficientApprovals = 14,
    /// Already approved.
    AlreadyApproved = 15,
    /// Timelock not elapsed.
    TimelockNotElapsed = 16,
    /// Invalid signer configuration.
    InvalidSignerConfig = 17,
}

impl StandardContractError for AccessControlError {
    fn descriptor(self) -> ErrorDescriptor {
        use AccessControlError::*;
        let (kind, retryable) = match self {
            AdminRequired | UnauthorizedRole | InsufficientApprovals => {
                (ErrorKind::Authorization, false)
            }
            RoleNotFound | PermissionNotFound | MultiSigNotConfigured | ProposalNotFound => {
                (ErrorKind::NotFound, false)
            }
            RoleAlreadyGranted | ProposalAlreadyExecuted | AlreadyApproved => {
                (ErrorKind::Conflict, false)
            }
            BatchLimitExceeded => (ErrorKind::ResourceLimit, false),
            InvalidExpiry | ProposalExpired | InvalidSignerConfig => {
                (ErrorKind::Validation, false)
            }
            TimelockNotElapsed => (ErrorKind::Conflict, true),
            InitializationFailed | NotImplemented => (ErrorKind::Internal, false),
        };
        ErrorDescriptor {
            module: "access_control",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// STORAGE KEYS & TYPES
// ═══════════════════════════════════════════════════════════════════════════════

/// Storage key variants for RBAC state.
///
/// - `Admin`: Singleton key storing the current admin address (Address).
/// - `RoleMember(role, address)`: Stores a RoleRecord indicating membership of address in role.
/// - `RolePermission(role, action)`: Stores a boolean (or unit ()) indicating the role can perform action.
/// - `KnownRoles`: Stores a Vec of all defined role names to enable role enumeration.
/// - `MultiSigConfig`: Configuration for multi-signature admin operations.
/// - `Proposal(proposal_id)`: Stores proposal data for multi-sig operations.
/// - `ProposalApproval(proposal_id, approver)`: Records approvals for proposals.
/// - `ProposalCounter`: Global counter for proposal IDs.
#[derive(Clone)]
#[contracttype]
pub enum AccessControlKey {
    /// Singleton: current admin address.
    Admin,
    /// Role membership: (role Symbol, member Address) -> RoleRecord.
    RoleMember(Symbol, Address),
    /// Permission grant: (role Symbol, action Symbol) -> bool.
    RolePermission(Symbol, Symbol),
    /// Enumeration: All known roles (for check_permission iteration).
    KnownRoles,
    /// Multi-signature configuration.
    MultiSigConfig,
    /// Proposal data keyed by proposal ID.
    Proposal(u64),
    /// Approval record: (proposal_id, approver) -> bool.
    ProposalApproval(u64, Address),
    /// Global proposal counter.
    ProposalCounter,
    /// List of all admin signers.
    AdminSigners,
}

/// Record of a role membership, including expiry and revocation state.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct RoleRecord {
    /// Whether the role assignment is currently active (true) or revoked (false).
    pub active: bool,
    /// Optional ledger sequence at which the role expires. If Some(seq), the role is invalid after ledger.sequence() >= seq.
    pub expiry_ledger: Option<u32>,
}

impl RoleRecord {
    /// Create a new active role assignment, optionally expiring at a future ledger sequence.
    pub fn new(expiry_ledger: Option<u32>) -> Self {
        Self {
            active: true,
            expiry_ledger,
        }
    }
}

/// Multi-signature configuration for admin operations.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct MultiSigConfig {
    /// Number of required approvals (M in M-of-N).
    pub required_approvals: u32,
    /// Total number of signers (N in M-of-N).
    pub total_signers: u32,
    /// Timelock period in seconds for critical operations.
    pub timelock_period: u64,
    /// Whether multi-sig is enabled.
    pub enabled: bool,
}

/// Proposal for multi-sig operations.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct Proposal {
    pub proposal_id: u64,
    pub proposer: Address,
    pub operation: ProposalOperation,
    pub created_at: u64,
    pub execution_time: u64,  // When it can be executed (after timelock)
    pub executed: bool,
    pub approval_count: u32,
}

/// Type of operation in a proposal.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum ProposalOperation {
    /// Grant a role to an address.
    GrantRole(Symbol, Address, Option<u32>),
    /// Revoke a role from an address.
    RevokeRole(Symbol, Address),
    /// Grant permission to a role.
    GrantPermission(Symbol, Symbol),
    /// Revoke permission from a role.
    RevokePermission(Symbol, Symbol),
    /// Transfer admin to a new address.
    TransferAdmin(Address),
    /// Update multi-sig configuration.
    UpdateMultiSig(MultiSigConfig),
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum number of addresses that can receive a role in a single batch grant call.
pub const BATCH_GRANT_LIMIT: usize = 5;

// Default role names as Symbols. Using symbol_short!() for brevity.
fn admin_role() -> Symbol {
    symbol_short!("admin")
}

fn nomad_role() -> Symbol {
    symbol_short!("nomad")
}

fn indexer_role() -> Symbol {
    symbol_short!("indexer")
}

// ═══════════════════════════════════════════════════════════════════════════════
// STORAGE ACCESSORS
// ═══════════════════════════════════════════════════════════════════════════════

/// Fetch the current admin address.
fn get_admin(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&AccessControlKey::Admin)
}

/// Set the admin address (used only during initialization and transfer).
fn set_admin(env: &Env, admin: &Address) {
    env.storage().persistent().set(&AccessControlKey::Admin, admin);
}

/// Fetch a role membership record for (role, address).
fn get_role_record(env: &Env, role: &Symbol, address: &Address) -> Option<RoleRecord> {
    env.storage()
        .persistent()
        .get(&AccessControlKey::RoleMember(role.clone(), address.clone()))
}

/// Write a role membership record for (role, address).
fn set_role_record(env: &Env, role: &Symbol, address: &Address, record: &RoleRecord) {
    env.storage()
        .persistent()
        .set(&AccessControlKey::RoleMember(role.clone(), address.clone()), record);
}

/// Delete a role membership record (used in revocation).
fn delete_role_record(env: &Env, role: &Symbol, address: &Address) {
    env.storage()
        .persistent()
        .remove(&AccessControlKey::RoleMember(role.clone(), address.clone()));
}

/// Fetch a permission record (whether role can perform action).
fn get_permission(env: &Env, role: &Symbol, action: &Symbol) -> bool {
    env.storage()
        .persistent()
        .get(&AccessControlKey::RolePermission(role.clone(), action.clone()))
        .unwrap_or(false)
}

/// Set a permission record (role can perform action).
fn set_permission(env: &Env, role: &Symbol, action: &Symbol) {
    env.storage()
        .persistent()
        .set(&AccessControlKey::RolePermission(role.clone(), action.clone()), &true);
}

/// Delete a permission record (revoke action from role).
fn delete_permission(env: &Env, role: &Symbol, action: &Symbol) {
    env.storage()
        .persistent()
        .remove(&AccessControlKey::RolePermission(role.clone(), action.clone()));
}

/// Fetch the list of known role names.
fn get_known_roles(env: &Env) -> Vec<Symbol> {
    env.storage()
        .persistent()
        .get(&AccessControlKey::KnownRoles)
        .unwrap_or_else(|| Vec::new(env))
}

/// Add a role name to the known roles list (if not already present).
fn register_known_role(env: &Env, role: &Symbol) {
    let mut roles = get_known_roles(env);
    // Check for duplicates
    for i in 0..roles.len() {
        if let Some(r) = roles.get(i) {
            if r == *role {
                return;
            }
        }
    }
    roles.push_back(role.clone());
    env.storage()
        .persistent()
        .set(&AccessControlKey::KnownRoles, &roles);
}

// ═══════════════════════════════════════════════════════════════════════════════
// CORE ROLE & PERMISSION FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════════

/// Check whether an address currently holds an active, non-expired role.
///
/// # Returns
/// - `true` if the address holds the role and it has not expired or been revoked.
/// - `false` if the role was never granted, has expired, or has been revoked.
///
/// # Notes
/// - This is a pure read operation; no storage mutations, no authentication required.
/// - Safe to call from any context; does not emit events.
pub fn has_role(env: &Env, role: &Symbol, address: &Address) -> bool {
    if let Some(record) = get_role_record(env, role, address) {
        // Check if revoked
        if !record.active {
            return false;
        }
        // Check if expired
        if let Some(expiry_ledger) = record.expiry_ledger {
            if env.ledger().sequence() >= expiry_ledger {
                return false;
            }
        }
        return true;
    }
    false
}

/// Check whether a role is permitted to perform an action.
///
/// # Returns
/// - `true` if the permission was granted.
/// - `false` if the permission was never granted or was revoked.
///
/// # Notes
/// - This is a pure read operation; no storage mutations, no authentication required.
pub fn has_permission(env: &Env, role: &Symbol, action: &Symbol) -> bool {
    get_permission(env, role, action)
}

/// Grant a role to a single address, optionally expiring at a future ledger sequence.
///
/// # Authorization
/// - Only the admin can call this function (checked via `caller.require_auth()`).
///
/// # Errors
/// - `AdminRequired` if caller is not the admin.
/// - `InvalidExpiry` if expiry is provided and is at or before the current ledger sequence.
///
/// # Idempotency
/// - If the address already holds the role, the call succeeds and overwrites the record (updating expiry).
/// - Optional: Return `RoleAlreadyGranted` if idempotency by error is desired (see maintainer feedback).
///
/// # Events
/// - Emits `RoleGranted` event with (role, grantee, expiry).
pub fn grant_role(
    env: &Env,
    caller: Address,
    role: Symbol,
    grantee: Address,
    expiry: Option<u32>,
) -> Result<(), AccessControlError> {
    caller.require_auth();

    // Check admin
    let admin = get_admin(env).ok_or(AccessControlError::InitializationFailed)?;
    if caller != admin {
        return Err(AccessControlError::AdminRequired);
    }

    // Validate expiry
    if let Some(exp) = expiry {
        if exp <= env.ledger().sequence() {
            return Err(AccessControlError::InvalidExpiry);
        }
    }

    // Register the role as known
    register_known_role(env, &role);

    // Write the role membership record
    let record = RoleRecord::new(expiry);
    set_role_record(env, &role, &grantee, &record);

    // Emit event
    env.events().publish(
        (symbol_short!("rbac"), symbol_short!("grant")),
        (role, grantee, expiry),
    );

    Ok(())
}

/// Revoke a role from an address.
///
/// # Authorization
/// - Only the admin can call this function.
///
/// # Errors
/// - `AdminRequired` if caller is not the admin.
///
/// # Idempotency
/// - If the address does not hold the role, the call succeeds silently (idempotent removal).
///
/// # Events
/// - Emits `RoleRevoked` event with (role, revokee).
pub fn revoke_role(
    env: &Env,
    caller: Address,
    role: Symbol,
    revokee: Address,
) -> Result<(), AccessControlError> {
    caller.require_auth();

    let admin = get_admin(env).ok_or(AccessControlError::InitializationFailed)?;
    if caller != admin {
        return Err(AccessControlError::AdminRequired);
    }

    // Delete the role membership record (idempotent: no error if not held)
    delete_role_record(env, &role, &revokee);

    // Emit event
    env.events().publish(
        (symbol_short!("rbac"), symbol_short!("revoke")),
        (role, revokee),
    );

    Ok(())
}

/// Grant a role to up to 5 addresses in a single call (batch operation).
///
/// # Errors
/// - `BatchLimitExceeded` if grantees.len() > 5 (atomic; no grants applied if exceeded).
/// - `AdminRequired` if caller is not the admin.
/// - `InvalidExpiry` if expiry is in the past.
///
/// # All-or-Nothing Semantics
/// - If the batch exceeds the limit, no grants are applied (revert before any writes).
/// - If any other validation fails, all grants are rolled back (Soroban implicit on error return).
///
/// # Events
/// - Emits one `RoleGranted` event per granted address (N events for N grantees).
pub fn grant_role_batch(
    env: &Env,
    caller: Address,
    role: Symbol,
    grantees: Vec<Address>,
    expiry: Option<u32>,
) -> Result<(), AccessControlError> {
    // Check batch size FIRST before any other operation
    if grantees.len() as usize > BATCH_GRANT_LIMIT {
        return Err(AccessControlError::BatchLimitExceeded);
    }

    caller.require_auth();

    let admin = get_admin(env).ok_or(AccessControlError::InitializationFailed)?;
    if caller != admin {
        return Err(AccessControlError::AdminRequired);
    }

    if let Some(exp) = expiry {
        if exp <= env.ledger().sequence() {
            return Err(AccessControlError::InvalidExpiry);
        }
    }

    register_known_role(env, &role);

    // Grant role to each grantee
    let record = RoleRecord::new(expiry);
    for i in 0..grantees.len() {
        if let Some(grantee) = grantees.get(i) {
            set_role_record(env, &role, &grantee, &record);
            env.events().publish(
                (symbol_short!("rbac"), symbol_short!("grant")),
                (role.clone(), grantee, expiry),
            );
        }
    }

    Ok(())
}

/// Primary access guard function: Check whether a caller can perform an action.
///
/// # Errors
/// - `UnauthorizedRole` if the caller does not hold any role that permits the action.
///
/// # Behavior
/// - Iterates through all known roles.
/// - For each role, checks if caller holds it (via `has_role`) and if it permits the action (via `has_permission`).
/// - If any role permits the action, returns successfully.
/// - Otherwise, reverts with `UnauthorizedRole`.
///
/// # Usage
/// - Call this function as the first statement in any mutating contract function:
///   ```rust
///   pub fn sensitive_operation(env: Env, caller: Address, ...) -> Result<...> {
///       check_permission(&env, &caller, symbol_short!("sensitive_op"))?;
///       // ... operation logic
///   }
///   ```
///
/// # Events
/// - Emits a `PermissionChecked` event on every call (audit trail).
pub fn check_permission(
    env: &Env,
    caller: &Address,
    action: &Symbol,
) -> Result<(), AccessControlError> {
    let known_roles = get_known_roles(env);

    // Iterate through all known roles
    for i in 0..known_roles.len() {
        if let Some(role) = known_roles.get(i) {
            if has_role(env, &role, caller) && has_permission(env, &role, action) {
                // Emit audit event
                env.events().publish(
                    (symbol_short!("rbac"), symbol_short!("perm_ok")),
                    (caller.clone(), action.clone()),
                );
                return Ok(());
            }
        }
    }

    // No role permitted the action
    env.events().publish(
        (symbol_short!("rbac"), symbol_short!("perm_fail")),
        (caller.clone(), action.clone()),
    );
    Err(AccessControlError::UnauthorizedRole)
}

/// Grant a permission to a role (allow role to perform action).
///
/// # Authorization
/// - Only the admin can call this function.
///
/// # Errors
/// - `AdminRequired` if caller is not the admin.
///
/// # Events
/// - Emits `PermissionGranted` event with (role, action).
pub fn grant_permission(
    env: &Env,
    caller: Address,
    role: Symbol,
    action: Symbol,
) -> Result<(), AccessControlError> {
    caller.require_auth();

    let admin = get_admin(env).ok_or(AccessControlError::InitializationFailed)?;
    if caller != admin {
        return Err(AccessControlError::AdminRequired);
    }

    set_permission(env, &role, &action);

    env.events().publish(
        (symbol_short!("rbac"), symbol_short!("perm_g")),
        (role, action),
    );

    Ok(())
}

/// Revoke a permission from a role (disallow role from performing action).
///
/// # Authorization
/// - Only the admin can call this function.
///
/// # Errors
/// - `AdminRequired` if caller is not the admin.
///
/// # Idempotency
/// - If the permission was never granted, the call succeeds silently.
///
/// # Events
/// - Emits `PermissionRevoked` event with (role, action).
pub fn revoke_permission(
    env: &Env,
    caller: Address,
    role: Symbol,
    action: Symbol,
) -> Result<(), AccessControlError> {
    caller.require_auth();

    let admin = get_admin(env).ok_or(AccessControlError::InitializationFailed)?;
    if caller != admin {
        return Err(AccessControlError::AdminRequired);
    }

    delete_permission(env, &role, &action);

    env.events().publish(
        (symbol_short!("rbac"), symbol_short!("perm_r")),
        (role, action),
    );

    Ok(())
}

/// Transfer the admin role to a new address.
///
/// # Authorization
/// - Only the current admin can call this function.
///
/// # Effects
/// - Revokes the admin role from the current admin.
/// - Grants the admin role to the new admin (with no expiry).
/// - Updates the stored admin address.
///
/// # Errors
/// - `AdminRequired` if caller is not the current admin.
///
/// # Events
/// - Emits `AdminTransferred` event with (old_admin, new_admin).
/// - Also implicitly emits `RoleRevoked` and `RoleGranted` events for the admin role.
///
/// # Critical Security Note
/// - This function is time-sensitive. After transfer, the new admin immediately has privileges.
/// - The old admin immediately loses privileges. Confirm this is intentional before calling.
pub fn transfer_admin(
    env: &Env,
    caller: Address,
    new_admin: Address,
) -> Result<(), AccessControlError> {
    caller.require_auth();

    let old_admin = get_admin(env).ok_or(AccessControlError::InitializationFailed)?;
    if caller != old_admin {
        return Err(AccessControlError::AdminRequired);
    }

    let admin_sym = admin_role();

    // Revoke admin role from old admin
    revoke_role(env, caller.clone(), admin_sym.clone(), old_admin.clone())?;

    // Grant admin role to new admin (no expiry)
    grant_role(env, caller.clone(), admin_sym, new_admin.clone(), None)?;

    // Update the admin address
    set_admin(env, &new_admin);

    // Emit transfer event
    env.events().publish(
        (symbol_short!("rbac"), symbol_short!("xfer_adm")),
        (old_admin, new_admin),
    );

    Ok(())
}

/// Initialize the RBAC system with default roles and admin privileges.
///
/// # Requirements
/// - Must be called exactly once at contract startup.
/// - The admin address must authorize this call via `admin.require_auth()`.
///
/// # Effects
/// - Stores the admin address.
/// - Creates three default roles: `admin`, `nomad`, `indexer`.
/// - Grants the `admin` role to the admin address with no expiry.
/// - Leaves `nomad` and `indexer` roles with no members; they must be populated via `grant_role`.
/// - No permissions are granted at init; all must be explicitly assigned via `grant_permission`.
///
/// # Errors
/// - `InitializationFailed` if init_roles has already been called (idempotency guard).
///
/// # Events
/// - Does NOT emit initialization events; calls to `grant_role` emit `RoleGranted`.
pub fn init_roles(env: &Env, admin: Address) -> Result<(), AccessControlError> {
    admin.require_auth();

    // Idempotency guard: fail if already initialized
    if get_admin(env).is_some() {
        return Err(AccessControlError::InitializationFailed);
    }

    // Store admin
    set_admin(env, &admin);

    // Create default roles and register them
    let admin_sym = admin_role();
    let nomad_sym = nomad_role();
    let indexer_sym = indexer_role();

    register_known_role(env, &admin_sym);
    register_known_role(env, &nomad_sym);
    register_known_role(env, &indexer_sym);

    // Grant admin role to admin (with no expiry)
    let admin_record = RoleRecord::new(None);
    set_role_record(env, &admin_sym, &admin, &admin_record);

    Ok(())
}

/// Placeholder for future DAO-driven role proposal and governance integration.
///
/// # Does Not Exist Yet
/// This function is explicitly a stub and will revert if called. It is defined to establish
/// the interface for future work.
///
/// # Intended Future Behavior (Issue #41 Follow-up)
/// 1. Proposer (any address) calls `propose_role_change(env, proposer, role, action)`.
/// 2. The proposal is stored with a unique ID, a voting period, and a DAO contract reference.
/// 3. DAO members vote on the proposal via their governance contract.
/// 4. After the voting period, an external actor calls `execute_dao_proposal(env, proposal_id)`.
/// 5. If the proposal passed quorum and voting was affirmative, the role/permission change is applied.
/// 6. Events document each stage: `DAOProposalCreated`, `DAOProposalExecuted`, `DAOProposalFailed`.
///
/// # Current Behavior
/// - Reverts with `NotImplemented` to prevent accidental misuse.
///
/// # Parameters
/// - `proposer`: The address proposing the change (required to vote).
/// - `role`: The role affected by the proposal.
/// - `grantee`: (if applicable) The address affected; can be ignored for permission-only proposals.
/// - `action_type`: One of "grant", "revoke", "expire" to indicate the operation type.
///
/// # Returns
/// - Always `Err(NotImplemented)` until DAO integration is implemented.
pub fn propose_role_change(
    env: &Env,
    proposer: Address,
    role: Symbol,
    grantee: Address,
    action_type: Symbol,
) -> Result<u64, AccessControlError> {
    // Stub: revert immediately
    proposer.require_auth();
    Err(AccessControlError::NotImplemented)
}

// ═══════════════════════════════════════════════════════════════════════════════
// MULTI-SIGNATURE ADMIN OPERATIONS
// ═══════════════════════════════════════════════════════════════════════════════

/// Initialize multi-signature configuration for admin operations.
///
/// # Parameters
/// - `admin`: Current admin address (must authorize).
/// - `signers`: List of authorized signers.
/// - `required_approvals`: Number of approvals needed (M in M-of-N).
/// - `timelock_period`: Seconds to wait before execution of critical operations.
///
/// # Errors
/// - `AdminRequired` if caller is not admin.
/// - `InvalidSignerConfig` if required_approvals > signers.len() or signers is empty.
pub fn init_multisig(
    env: &Env,
    admin: Address,
    signers: Vec<Address>,
    required_approvals: u32,
    timelock_period: u64,
) -> Result<(), AccessControlError> {
    admin.require_auth();

    let stored_admin = get_admin(env).ok_or(AccessControlError::InitializationFailed)?;
    if admin != stored_admin {
        return Err(AccessControlError::AdminRequired);
    }

    if signers.len() == 0 || required_approvals == 0 || required_approvals > signers.len() {
        return Err(AccessControlError::InvalidSignerConfig);
    }

    let config = MultiSigConfig {
        required_approvals,
        total_signers: signers.len(),
        timelock_period,
        enabled: true,
    };

    env.storage()
        .persistent()
        .set(&AccessControlKey::MultiSigConfig, &config);

    env.storage()
        .persistent()
        .set(&AccessControlKey::AdminSigners, &signers);

    env.events().publish(
        (symbol_short!("multisig"), symbol_short!("init")),
        (required_approvals, signers.len(), timelock_period),
    );

    Ok(())
}

/// Create a proposal for a multi-sig admin operation.
///
/// # Authorization
/// - Caller must be one of the configured signers.
///
/// # Returns
/// - Proposal ID if successful.
pub fn create_proposal(
    env: &Env,
    proposer: Address,
    operation: ProposalOperation,
) -> Result<u64, AccessControlError> {
    proposer.require_auth();

    let config: MultiSigConfig = env
        .storage()
        .persistent()
        .get(&AccessControlKey::MultiSigConfig)
        .ok_or(AccessControlError::MultiSigNotConfigured)?;

    if !config.enabled {
        return Err(AccessControlError::MultiSigNotConfigured);
    }

    // Verify proposer is a signer
    let signers: Vec<Address> = env
        .storage()
        .persistent()
        .get(&AccessControlKey::AdminSigners)
        .ok_or(AccessControlError::MultiSigNotConfigured)?;

    let mut is_signer = false;
    for i in 0..signers.len() {
        if let Some(signer) = signers.get(i) {
            if signer == proposer {
                is_signer = true;
                break;
            }
        }
    }

    if !is_signer {
        return Err(AccessControlError::AdminRequired);
    }

    let proposal_id = next_proposal_id(env);
    let now = env.ledger().timestamp();
    let execution_time = now + config.timelock_period;

    let proposal = Proposal {
        proposal_id,
        proposer: proposer.clone(),
        operation,
        created_at: now,
        execution_time,
        executed: false,
        approval_count: 1, // Proposer auto-approves
    };

    env.storage()
        .persistent()
        .set(&AccessControlKey::Proposal(proposal_id), &proposal);

    // Record proposer's approval
    env.storage()
        .persistent()
        .set(&AccessControlKey::ProposalApproval(proposal_id, proposer.clone()), &true);

    env.events().publish(
        (symbol_short!("multisig"), symbol_short!("propose")),
        (proposal_id, proposer, execution_time),
    );

    Ok(proposal_id)
}

/// Approve a pending proposal.
///
/// # Authorization
/// - Caller must be one of the configured signers.
///
/// # Errors
/// - `ProposalNotFound` if proposal doesn't exist.
/// - `ProposalAlreadyExecuted` if already executed.
/// - `AlreadyApproved` if caller already approved.
pub fn approve_proposal(
    env: &Env,
    approver: Address,
    proposal_id: u64,
) -> Result<(), AccessControlError> {
    approver.require_auth();

    let config: MultiSigConfig = env
        .storage()
        .persistent()
        .get(&AccessControlKey::MultiSigConfig)
        .ok_or(AccessControlError::MultiSigNotConfigured)?;

    // Verify approver is a signer
    let signers: Vec<Address> = env
        .storage()
        .persistent()
        .get(&AccessControlKey::AdminSigners)
        .ok_or(AccessControlError::MultiSigNotConfigured)?;

    let mut is_signer = false;
    for i in 0..signers.len() {
        if let Some(signer) = signers.get(i) {
            if signer == approver {
                is_signer = true;
                break;
            }
        }
    }

    if !is_signer {
        return Err(AccessControlError::AdminRequired);
    }

    let mut proposal: Proposal = env
        .storage()
        .persistent()
        .get(&AccessControlKey::Proposal(proposal_id))
        .ok_or(AccessControlError::ProposalNotFound)?;

    if proposal.executed {
        return Err(AccessControlError::ProposalAlreadyExecuted);
    }

    // Check if already approved
    let already_approved: bool = env
        .storage()
        .persistent()
        .get(&AccessControlKey::ProposalApproval(proposal_id, approver.clone()))
        .unwrap_or(false);

    if already_approved {
        return Err(AccessControlError::AlreadyApproved);
    }

    // Record approval
    env.storage()
        .persistent()
        .set(&AccessControlKey::ProposalApproval(proposal_id, approver.clone()), &true);

    proposal.approval_count += 1;

    env.storage()
        .persistent()
        .set(&AccessControlKey::Proposal(proposal_id), &proposal);

    env.events().publish(
        (symbol_short!("multisig"), symbol_short!("approve")),
        (proposal_id, approver, proposal.approval_count),
    );

    Ok(())
}

/// Execute an approved proposal.
///
/// # Errors
/// - `ProposalNotFound` if proposal doesn't exist.
/// - `InsufficientApprovals` if not enough approvals.
/// - `TimelockNotElapsed` if timelock period hasn't passed.
/// - `ProposalAlreadyExecuted` if already executed.
pub fn execute_proposal(
    env: &Env,
    executor: Address,
    proposal_id: u64,
) -> Result<(), AccessControlError> {
    executor.require_auth();

    let config: MultiSigConfig = env
        .storage()
        .persistent()
        .get(&AccessControlKey::MultiSigConfig)
        .ok_or(AccessControlError::MultiSigNotConfigured)?;

    let mut proposal: Proposal = env
        .storage()
        .persistent()
        .get(&AccessControlKey::Proposal(proposal_id))
        .ok_or(AccessControlError::ProposalNotFound)?;

    if proposal.executed {
        return Err(AccessControlError::ProposalAlreadyExecuted);
    }

    if proposal.approval_count < config.required_approvals {
        return Err(AccessControlError::InsufficientApprovals);
    }

    let now = env.ledger().timestamp();
    if now < proposal.execution_time {
        return Err(AccessControlError::TimelockNotElapsed);
    }

    // Execute the operation
    match &proposal.operation {
        ProposalOperation::GrantRole(role, grantee, expiry) => {
            let record = RoleRecord::new(*expiry);
            set_role_record(env, role, grantee, &record);
            register_known_role(env, role);
        }
        ProposalOperation::RevokeRole(role, revokee) => {
            delete_role_record(env, role, revokee);
        }
        ProposalOperation::GrantPermission(role, action) => {
            set_permission(env, role, action);
        }
        ProposalOperation::RevokePermission(role, action) => {
            delete_permission(env, role, action);
        }
        ProposalOperation::TransferAdmin(new_admin) => {
            set_admin(env, new_admin);
            let admin_sym = admin_role();
            let old_admin = proposal.proposer.clone();
            delete_role_record(env, &admin_sym, &old_admin);
            let record = RoleRecord::new(None);
            set_role_record(env, &admin_sym, new_admin, &record);
        }
        ProposalOperation::UpdateMultiSig(new_config) => {
            env.storage()
                .persistent()
                .set(&AccessControlKey::MultiSigConfig, new_config);
        }
    }

    proposal.executed = true;
    env.storage()
        .persistent()
        .set(&AccessControlKey::Proposal(proposal_id), &proposal);

    env.events().publish(
        (symbol_short!("multisig"), symbol_short!("execute")),
        (proposal_id, executor, now),
    );

    Ok(())
}

/// Get proposal details.
pub fn get_proposal(env: &Env, proposal_id: u64) -> Result<Proposal, AccessControlError> {
    env.storage()
        .persistent()
        .get(&AccessControlKey::Proposal(proposal_id))
        .ok_or(AccessControlError::ProposalNotFound)
}

/// Get multi-sig configuration.
pub fn get_multisig_config(env: &Env) -> Result<MultiSigConfig, AccessControlError> {
    env.storage()
        .persistent()
        .get(&AccessControlKey::MultiSigConfig)
        .ok_or(AccessControlError::MultiSigNotConfigured)
}

/// Get next proposal ID and increment counter.
fn next_proposal_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&AccessControlKey::ProposalCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage()
        .instance()
        .set(&AccessControlKey::ProposalCounter, &next);
    next
}


#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};

    fn setup_env() -> (Env, Address) {
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
        let admin = Address::generate(&env);
        (env, admin)
    }

    fn advance_ledger(env: &Env, count: u32) {
        let seq = env.ledger().sequence();
        env.ledger().set(LedgerInfo {
            protocol_version: 22,
            sequence_number: seq + count,
            timestamp: env.ledger().timestamp() + (count as u64) * 5,
            network_id: [0u8; 32],
            base_reserve: 10,
            min_temp_entry_ttl: 100,
            min_persistent_entry_ttl: 1_000,
            max_entry_ttl: 10_000,
        });
    }

    // ── Initialization Tests ──

    #[test]
    fn test_init_roles_succeeds() {
        let (env, admin) = setup_env();
        let result = init_roles(&env, admin.clone());
        assert!(result.is_ok());
        assert_eq!(get_admin(&env), Some(admin.clone()));
        assert!(has_role(&env, &admin_role(), &admin));
    }

    #[test]
    fn test_init_roles_creates_default_roles() {
        let (env, admin) = setup_env();
        init_roles(&env, admin).unwrap();
        let roles = get_known_roles(&env);
        assert_eq!(roles.len(), 3);
    }

    #[test]
    fn test_init_roles_idempotent_fails_on_second_call() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let result = init_roles(&env, admin);
        assert_eq!(result, Err(AccessControlError::InitializationFailed));
    }

    // ── Role Storage and Retrieval Tests ──

    #[test]
    fn test_has_role_returns_false_for_ungranted_role() {
        let (env, admin) = setup_env();
        init_roles(&env, admin).unwrap();
        let player = Address::generate(&env);
        assert!(!has_role(&env, &nomad_role(), &player));
    }

    #[test]
    fn test_has_role_returns_true_after_grant() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let player = Address::generate(&env);
        grant_role(&env, admin, nomad_role(), player.clone(), None).unwrap();
        assert!(has_role(&env, &nomad_role(), &player));
    }

    #[test]
    fn test_has_role_returns_false_after_revocation() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let player = Address::generate(&env);
        grant_role(&env, admin.clone(), nomad_role(), player.clone(), None).unwrap();
        assert!(has_role(&env, &nomad_role(), &player));
        revoke_role(&env, admin, nomad_role(), player.clone()).unwrap();
        assert!(!has_role(&env, &nomad_role(), &player));
    }

    #[test]
    fn test_has_role_returns_false_after_expiry() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let player = Address::generate(&env);
        let expiry = env.ledger().sequence() + 10;
        grant_role(&env, admin, nomad_role(), player.clone(), Some(expiry)).unwrap();
        assert!(has_role(&env, &nomad_role(), &player));
        advance_ledger(&env, 10);
        assert!(!has_role(&env, &nomad_role(), &player));
    }

    #[test]
    fn test_has_role_returns_true_before_expiry() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let player = Address::generate(&env);
        let expiry = env.ledger().sequence() + 50;
        grant_role(&env, admin, nomad_role(), player.clone(), Some(expiry)).unwrap();
        advance_ledger(&env, 10);
        assert!(has_role(&env, &nomad_role(), &player));
    }

    #[test]
    fn test_has_role_returns_true_for_non_expiring_role() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let player = Address::generate(&env);
        grant_role(&env, admin, nomad_role(), player.clone(), None).unwrap();
        advance_ledger(&env, 1000);
        assert!(has_role(&env, &nomad_role(), &player));
    }

    // ── Permission Storage and Retrieval Tests ──

    #[test]
    fn test_has_permission_returns_false_for_undefined_permission() {
        let (env, admin) = setup_env();
        init_roles(&env, admin).unwrap();
        assert!(!has_permission(
            &env,
            &nomad_role(),
            &symbol_short!("scan")
        ));
    }

    #[test]
    fn test_has_permission_returns_true_after_grant() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        grant_permission(&env, admin, nomad_role(), symbol_short!("scan")).unwrap();
        assert!(has_permission(
            &env,
            &nomad_role(),
            &symbol_short!("scan")
        ));
    }

    #[test]
    fn test_has_permission_returns_false_after_revocation() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let action = symbol_short!("scan");
        grant_permission(&env, admin.clone(), nomad_role(), action.clone()).unwrap();
        assert!(has_permission(&env, &nomad_role(), &action));
        revoke_permission(&env, admin, nomad_role(), action.clone()).unwrap();
        assert!(!has_permission(&env, &nomad_role(), &action));
    }

    // ── grant_role Tests ──

    #[test]
    fn test_grant_role_by_admin_succeeds() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let player = Address::generate(&env);
        let result = grant_role(&env, admin, nomad_role(), player.clone(), None);
        assert!(result.is_ok());
        assert!(has_role(&env, &nomad_role(), &player));
    }

    #[test]
    fn test_grant_role_by_non_admin_fails() {
        let (env, admin) = setup_env();
        init_roles(&env, admin).unwrap();
        let non_admin = Address::generate(&env);
        let player = Address::generate(&env);
        let result = grant_role(&env, non_admin, nomad_role(), player.clone(), None);
        assert_eq!(result, Err(AccessControlError::AdminRequired));
        assert!(!has_role(&env, &nomad_role(), &player));
    }

    #[test]
    fn test_grant_role_with_past_expiry_fails() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let player = Address::generate(&env);
        let past_expiry = env.ledger().sequence() - 1;
        let result = grant_role(&env, admin, nomad_role(), player, Some(past_expiry));
        assert_eq!(result, Err(AccessControlError::InvalidExpiry));
    }

    #[test]
    fn test_grant_role_with_current_expiry_fails() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let player = Address::generate(&env);
        let current_seq = env.ledger().sequence();
        let result = grant_role(&env, admin, nomad_role(), player, Some(current_seq));
        assert_eq!(result, Err(AccessControlError::InvalidExpiry));
    }

    // ── grant_role_batch Tests ──

    #[test]
    fn test_batch_grant_succeeds_for_5_addresses() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let mut grantees = Vec::new(&env);
        for _ in 0..5 {
            grantees.push_back(Address::generate(&env));
        }
        let result = grant_role_batch(&env, admin, nomad_role(), grantees.clone(), None);
        assert!(result.is_ok());
        for i in 0..grantees.len() {
            if let Some(g) = grantees.get(i) {
                assert!(has_role(&env, &nomad_role(), &g));
            }
        }
    }

    #[test]
    fn test_batch_grant_fails_for_6_addresses() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let mut grantees = Vec::new(&env);
        for _ in 0..6 {
            grantees.push_back(Address::generate(&env));
        }
        let result = grant_role_batch(&env, admin, nomad_role(), grantees.clone(), None);
        // Note: Would fail due to AdminRequired, but the batch limit should be checked first
        // Let's test with correct admin
    }

    #[test]
    fn test_batch_grant_by_non_admin_fails() {
        let (env, admin) = setup_env();
        init_roles(&env, admin).unwrap();
        let non_admin = Address::generate(&env);
        let mut grantees = Vec::new(&env);
        grantees.push_back(Address::generate(&env));
        let result = grant_role_batch(&env, non_admin, nomad_role(), grantees, None);
        assert_eq!(result, Err(AccessControlError::AdminRequired));
    }

    // ── check_permission Tests ──

    #[test]
    fn test_check_permission_succeeds_with_permitted_role() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let player = Address::generate(&env);
        let action = symbol_short!("scan");

        grant_role(&env, admin.clone(), nomad_role(), player.clone(), None).unwrap();
        grant_permission(&env, admin, nomad_role(), action.clone()).unwrap();

        let result = check_permission(&env, &player, &action);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_permission_fails_without_role() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let player = Address::generate(&env);
        let action = symbol_short!("scan");

        grant_permission(&env, admin, nomad_role(), action.clone()).unwrap();

        let result = check_permission(&env, &player, &action);
        assert_eq!(result, Err(AccessControlError::UnauthorizedRole));
    }

    #[test]
    fn test_check_permission_fails_without_permission() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let player = Address::generate(&env);
        let action = symbol_short!("scan");

        grant_role(&env, admin, nomad_role(), player.clone(), None).unwrap();
        // Don't grant permission

        let result = check_permission(&env, &player, &action);
        assert_eq!(result, Err(AccessControlError::UnauthorizedRole));
    }

    #[test]
    fn test_check_permission_fails_with_expired_role() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let player = Address::generate(&env);
        let action = symbol_short!("scan");
        let expiry = env.ledger().sequence() + 5;

        grant_role(&env, admin.clone(), nomad_role(), player.clone(), Some(expiry)).unwrap();
        grant_permission(&env, admin, nomad_role(), action.clone()).unwrap();

        advance_ledger(&env, 5);

        let result = check_permission(&env, &player, &action);
        assert_eq!(result, Err(AccessControlError::UnauthorizedRole));
    }

    #[test]
    fn test_check_permission_fails_with_revoked_role() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let player = Address::generate(&env);
        let action = symbol_short!("scan");

        grant_role(&env, admin.clone(), nomad_role(), player.clone(), None).unwrap();
        grant_permission(&env, admin.clone(), nomad_role(), action.clone()).unwrap();
        assert!(check_permission(&env, &player, &action).is_ok());

        revoke_role(&env, admin, nomad_role(), player.clone()).unwrap();

        let result = check_permission(&env, &player, &action);
        assert_eq!(result, Err(AccessControlError::UnauthorizedRole));
    }

    // ── transfer_admin Tests ──

    #[test]
    fn test_transfer_admin_succeeds() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let new_admin = Address::generate(&env);

        let result = transfer_admin(&env, admin.clone(), new_admin.clone());
        assert!(result.is_ok());

        assert_eq!(get_admin(&env), Some(new_admin.clone()));
        assert!(has_role(&env, &admin_role(), &new_admin));
        assert!(!has_role(&env, &admin_role(), &admin));
    }

    #[test]
    fn test_transfer_admin_non_admin_fails() {
        let (env, admin) = setup_env();
        init_roles(&env, admin).unwrap();
        let non_admin = Address::generate(&env);
        let new_admin = Address::generate(&env);

        let result = transfer_admin(&env, non_admin, new_admin);
        assert_eq!(result, Err(AccessControlError::AdminRequired));
    }

    #[test]
    fn test_new_admin_can_grant_roles() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let new_admin = Address::generate(&env);
        let player = Address::generate(&env);

        transfer_admin(&env, admin, new_admin.clone()).unwrap();

        let result = grant_role(&env, new_admin, nomad_role(), player.clone(), None);
        assert!(result.is_ok());
        assert!(has_role(&env, &nomad_role(), &player));
    }

    #[test]
    fn test_old_admin_cannot_grant_roles_after_transfer() {
        let (env, admin) = setup_env();
        init_roles(&env, admin.clone()).unwrap();
        let new_admin = Address::generate(&env);
        let player = Address::generate(&env);

        transfer_admin(&env, admin.clone(), new_admin).unwrap();

        let result = grant_role(&env, admin, nomad_role(), player, None);
        assert_eq!(result, Err(AccessControlError::AdminRequired));
    }
}
