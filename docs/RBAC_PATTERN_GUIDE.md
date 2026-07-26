# Role-Based Access Control (RBAC) Pattern Guide

This guide provides practical examples for using the Role-Based Access Control (RBAC) system in the Stellar Nebula Nomad contract suite.

## Quick Start

### 1. Initialize RBAC at Deployment

First, call `init_rbac` exactly once to create the default roles and set up the admin:

```rust
// In deployment sequence:
ribac_client.init_rbac(&deployer)
    .expect("RBAC initialization failed");
```

This creates three default roles:
- `admin`: Full administrative privileges
- `nomad`: Regular player role
- `indexer`: Data indexer role

The deployer becomes the admin and can grant/revoke roles and permissions.

### 2. Define a New Action

When you want to protect a function with RBAC, define the action as a Symbol constant:

```rust
// In your contract module or lib.rs
const SCAN_ACTION: Symbol = symbol_short!("scan");
const HARVEST_ACTION: Symbol = symbol_short!("harvest");
const PAUSE_ACTION: Symbol = symbol_short!("pause");
```

### 3. Protect a Function with RBAC

Add an early-exit RBAC check as the first statement in any mutating function:

```rust
pub fn scan_nebula(
    env: Env,
    player: Address,
    seed: BytesN<32>,
) -> Result<(NebulaLayout, Rarity), SomeError> {
    player.require_auth();
    
    // ✓ Add this line
    access_control::check_permission(&env, &player, &symbol_short!("scan"))?;
    
    // ... rest of function
    let layout = nebula_explorer::generate_nebula_layout(&env, &seed, &player);
    // ...
    Ok((layout, rarity))
}
```

### 4. Grant Permissions to Roles

As the admin, grant permissions so roles can perform actions:

```rust
// Allow nomad role to scan
rbac_client.grant_permission(&admin, &symbol_short!("nomad"), &symbol_short!("scan"))
    .expect("Failed to grant permission");

// Allow admin role to pause
rbac_client.grant_permission(&admin, &symbol_short!("admin"), &symbol_short!("pause"))
    .expect("Failed to grant permission");
```

### 5. Grant Roles to Players

Assign roles to players so they can perform actions:

```rust
let player = Address::generate(&env);

// Grant nomad role to player (permanent)
rbac_client.grant_role(&admin, &symbol_short!("nomad"), &player, None)
    .expect("Failed to grant role");

// Grant indexer role to player with expiry (expires at ledger 1000)
rbac_client.grant_role(&admin, &symbol_short!("indexer"), &player, Some(1000))
    .expect("Failed to grant role with expiry");
```

### 6. Check Roles and Permissions

Use read-only functions to inspect RBAC state:

```rust
// Check if player holds nomad role
let is_nomad = rbac_client.has_role(&symbol_short!("nomad"), &player);

// Check if nomad role can scan
let can_scan = rbac_client.has_permission(&symbol_short!("nomad"), &symbol_short!("scan"));
```

---

## Patterns

### Pattern 1: Role-Based Access Gateway

Protect admin-only operations:

```rust
pub fn dangerous_operation(env: Env, caller: Address) -> Result<(), MyError> {
    caller.require_auth();
    access_control::check_permission(&env, &caller, &symbol_short!("admin_op"))?;
    // ... operation
}

// Setup:
rbac_client.grant_permission(&admin, &symbol_short!("admin"), &symbol_short!("admin_op"))?;
```

Result: Only the admin role can call `dangerous_operation`.

### Pattern 2: Multiple Roles with Different Permissions

Define different permission sets for different roles:

```rust
const SCAN: Symbol = symbol_short!("scan");
const HARVEST: Symbol = symbol_short!("harvest");
const TRANSFER: Symbol = symbol_short!("transfer");

// nomad can only scan and harvest
rbac_client.grant_permission(&admin, &symbol_short!("nomad"), &SCAN)?;
rbac_client.grant_permission(&admin, &symbol_short!("nomad"), &HARVEST)?;

// indexer can only transfer
rbac_client.grant_permission(&admin, &symbol_short!("indexer"), &TRANSFER)?;

// admin can do everything
rbac_client.grant_permission(&admin, &symbol_short!("admin"), &SCAN)?;
rbac_client.grant_permission(&admin, &symbol_short!("admin"), &HARVEST)?;
rbac_client.grant_permission(&admin, &symbol_short!("admin"), &TRANSFER)?;
```

Result: Each role has distinct capabilities.

### Pattern 3: Temporary Access with Expiry

Grant time-limited permissions for special events:

```rust
let current_ledger = env.ledger().sequence();
let event_duration = 1000; // ~83 minutes (ledgers)
let event_end = current_ledger + event_duration;

// Grant special "event_player" role to moderator for limited time
rbac_client.grant_role(&admin, &symbol_short!("event_p"), &moderator, Some(event_end))?;

// After event_end ledger, role automatically expires
// check_permission will return false
```

Result: Roles automatically expire without manual revocation.

### Pattern 4: Batch Role Grants for Events

Quickly onboard multiple players to a role:

```rust
let mut players = Vec::new(&env);
players.push_back(player1.clone());
players.push_back(player2.clone());
players.push_back(player3.clone());
players.push_back(player4.clone());
players.push_back(player5.clone());

// Grant all 5 players the same role in one transaction
rbac_client.grant_role_batch(&admin, &symbol_short!("event_p"), players, Some(event_end))?;
```

**Constraint**: Maximum 5 addresses per batch. For >5 players, call batch multiple times.

### Pattern 5: Admin Transfer for Multi-Sig Setup

Transfer admin to a contract that implements voting:

```rust
// Original admin transfers to multi-sig voter contract
rbac_client.transfer_admin(&initial_admin, &voter_contract_address)?;

// Now only voter_contract_address can grant/revoke roles
// Initial admin has no privileges
```

Result: Admin responsibilities transferred to a DAO or multi-sig contract.

### Pattern 6: Immediate Privilege Check

Check if a caller can perform an action without making the actual call:

```rust
// From a frontend or helper function
pub fn can_user_transfer(env: &Env, user: &Address) -> bool {
    access_control::check_permission(env, user, &symbol_short!("transfer")).is_ok()
}
```

Result: Frontend can disable buttons or show warnings for actions the user can't perform.

---

## Hardening Ad-Hoc Admin Setters (Issue #237)

Some modules (`leaderboards`, `content_tools`, `pvp_combat`) predate the RBAC
system above and use a simpler single-admin pattern instead of
`access_control`. Their `set_admin(env, admin)` functions used to only
require the *incoming* admin's own signature — never the *current* admin's —
which let any caller call `set_admin` at any time and silently hijack the
role.

They are now one-time initializers, matching the idempotency guard already
used by `access_control::init_roles` and `emergency_controls::initialize_admins`:

```rust
pub fn set_admin(env: &Env, admin: &Address) -> Result<(), MyError> {
    admin.require_auth();
    if get_admin(env).is_some() {
        return Err(MyError::AlreadyInitialized);
    }
    env.storage().persistent().set(&DataKey::Admin, admin);
    Ok(())
}
```

Once set, admin can only change via an explicit transfer function gated on
the *current* admin's signature (see `access_control::transfer_admin` for the
reference implementation) — never via a bare re-call of `set_admin`.

## Common Mistakes to Avoid

### ❌ Mistake 1: Forgetting to Initialize

```rust
// DON'T: Forget init_rbac
// rbac_client.init_rbac(&admin)?;  // MISSING!

// Try to grant role (will fail because admin not set)
rbac_client.grant_role(&admin, &symbol_short!("nomad"), &player)?;
// Error: InitializationFailed
```

✓ **Always call `init_rbac` once at deployment.**

### ❌ Mistake 2: Forgetting the Permission Grant

```rust
// User has role "nomad"
rbac_client.grant_role(&admin, &symbol_short!("nomad"), &player)?;

// But nomad role has no permissions
// check_permission(player, "scan") returns Unauth

// DON'T: Assume the role has permissions
// Player can't scan!

// ✓ Grant permission
rbac_client.grant_permission(&admin, &symbol_short!("nomad"), &symbol_short!("scan"))?;
// Now check_permission(player, "scan") returns OK
```

✓ **Always pair role grants with permission grants.**

### ❌ Mistake 3: Exceeding Batch Limit

```rust
let mut grantees = Vec::new(&env);
for i in 0..10 {  // 10 > 5!
    grantees.push_back(Address::generate(&env));
}

rbac_client.grant_role_batch(&admin, &symbol_short!("nomad"), grantees)?;
// Error: BatchLimitExceeded
// AND no addresses were granted (atomic failure)

// ✓ Split into multiple batches
let batch1: Vec<Address> = /* first 5 */;
let batch2: Vec<Address> = /* next 5 */;

rbac_client.grant_role_batch(&admin, &symbol_short!("nomad"), batch1)?;
rbac_client.grant_role_batch(&admin, &symbol_short!("nomad"), batch2)?;
```

✓ **Keep batch size ≤ 5.**

### ❌ Mistake 4: Using Expired Roles

```rust
let role_expiry = env.ledger().sequence() + 5;
rbac_client.grant_role(&admin, &symbol_short!("nomad"), &player, Some(role_expiry))?;

// Advance ledger 5 steps
env.ledger().set(LedgerInfo { sequence_number: current + 5, ... });

// NOW role is expired
let can_scan = rbac_client.has_role(&symbol_short!("nomad"), &player);
// false! (role expired)

// check_permission(...) returns Unauth
```

✓ **Monitor role expiry and re-grant before expiration if continued access is needed.**

### ❌ Mistake 5: Missing Authorization

```rust
pub fn my_guarded_function(env: Env, caller: Address) -> Result<(), MyError> {
    // DON'T: Forget caller.require_auth()
    access_control::check_permission(&env, &caller, &symbol_short!("action"))?;
    // ...
}

// Attacker can call as anyone (no signature requirement)
```

✓ **Always call `caller.require_auth()` before RBAC checks.**

---

## Advanced Usage

### Scenario: Implement a Pause/Unpause System

```rust
const PAUSE_ACTION: Symbol = symbol_short!("pause");

pub fn pause_contract(env: Env, caller: Address) -> Result<(), MyError> {
    caller.require_auth();
    access_control::check_permission(&env, &caller, &PAUSE_ACTION)?;
    
    // Perform pause logic
    env.storage().instance().set(&PauseKey, &true);
    Ok(())
}

// Setup:
rbac_client.grant_permission(&admin, &symbol_short!("admin"), &PAUSE_ACTION)?;

// Only admin can pause
```

### Scenario: Time-Limited Testing Access

```rust
let test_end = env.ledger().sequence() + 1000; // ~1 hour

rbac_client.grant_role(&admin, &symbol_short!("tester"), &test_account, Some(test_end))?;
rbac_client.grant_permission(&admin, &symbol_short!("tester"), &symbol_short!("debug"))?;

// test_account can call debug functions until test_end
// After blocklisting, permission automatically fails
```

### Scenario: Multi-Tier Governance

Create a hierarchy of roles with different permissions:

```rust
// Contributors can submit proposals
rbac_client.grant_permission(&admin, &symbol_short!("contributor"), &symbol_short!("propose"))?;

// Mods can vote on proposals
rbac_client.grant_permission(&admin, &symbol_short!("mod"), &symbol_short!("vote"))?;

// Mods can also propose
rbac_client.grant_permission(&admin, &symbol_short!("mod"), &symbol_short!("propose"))?;

// Admin has all permissions
rbac_client.grant_permission(&admin, &symbol_short!("admin"), &symbol_short!("propose"))?;
rbac_client.grant_permission(&admin, &symbol_short!("admin"), &symbol_short!("vote"))?;
rbac_client.grant_permission(&admin, &symbol_short!("admin"), &symbol_short!("execute"))?;
```

---

## Testing RBAC

### Unit Test Template

```rust
#[test]
fn test_my_guarded_function_requires_permission() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    // Initialize RBAC
    access_control::init_roles(&env, admin.clone()).unwrap();
    
    // Setup: Grant user the nomad role
    access_control::grant_role(&env, admin.clone(), symbol_short!("nomad"), user.clone(), None).unwrap();
    
    // Grant permission to nomad for the action
    access_control::grant_permission(&env, admin, symbol_short!("nomad"), symbol_short!("scan")).unwrap();
    
    // User can now perform action
    let result = check_permission(&env, &user, &symbol_short!("scan"));
    assert!(result.is_ok());
}
```

### Integration Test Template

```rust
#[test]
fn test_full_rbac_workflow() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let player = Address::generate(&env);
    
    // 1. Initialize
    access_control::init_roles(&env, admin.clone()).unwrap();
    assert!(access_control::has_role(&env, &symbol_short!("admin"), &admin));
    
    // 2. Grant role to player
    access_control::grant_role(&env, admin.clone(), symbol_short!("nomad"), player.clone(), None).unwrap();
    assert!(access_control::has_role(&env, &symbol_short!("nomad"), &player));
    
    // 3. Grant permission
    access_control::grant_permission(&env, admin, symbol_short!("nomad"), symbol_short!("scan")).unwrap();
    assert!(access_control::has_permission(&env, &symbol_short!("nomad"), &symbol_short!("scan")));
    
    // 4. Check permission succeeds
    let result = access_control::check_permission(&env, &player, &symbol_short!("scan"));
    assert!(result.is_ok());
}
```

---

## Performance Considerations

### Role Lookup Performance

- `has_role()`: O(1) storage read + ledger sequence check
- `check_permission()`: O(R) where R = number of known roles (~3-10 for MVP)
- `grant_role()`: O(1) storage write + known_roles update

**Implication**: For large role sets (100+), consider reverse index optimization in future PR.

### Storage Costs

- Each role grant: 1 storage entry (~128 bytes)
- Each permission: 1 storage entry (~64 bytes)
- Batch grant of N: N storage entries

**Implication**: Batch grants are efficient; prefer over N individual calls.

### Event Overhead

- Each mutating operation emits 1 event
- `check_permission` emits on success/failure (provides audit trail)

**Implication**: Monitor log storage if security monitoring is critical; consider off-chain indexing.

---

## Troubleshooting

### Problem: `AdminRequired` error

**Cause**: Non-admin user tried to call grant_role, revoke_role, etc.

**Solution**: 
1. Verify you're calling as the current admin
2. Check if admin was transferred to new address
3. Call `has_role(admin, caller)` to verify admin status

### Problem: `UnauthorizedRole` error

**Cause**: User doesn't hold required role, or role expired/was revoked

**Solution**:
1. Verify user was granted the role: `has_role(role, user)`
2. Check role hasn't expired: compare expiry vs `env.ledger().sequence()`
3. Verify role wasn't revoked: `has_role` returns false for revoked roles
4. Grant/re-grant role if needed

### Problem: `BatchLimitExceeded` error

**Cause**: Attempted batch grant with >5 addresses

**Solution**: Split into multiple batch calls, each with ≤5 addresses

### Problem: `InvalidExpiry` error

**Cause**: Provided expiry is at or before current ledger sequence

**Solution**: Use future ledger sequence: `env.ledger().sequence() + N` where N > 0

---

## Next Steps

For DAO-driven role governance, follow the extensibility pattern:

1. Implement DAO module with proposals and voting
2. Create `execute_dao_proposal(proposal_id)` that calls RBAC functions
3. Link proposals to role changes (grant_role, revoke_role, etc.)

See access_control module doc-comment for interface design.

---

**Questions?** Refer to the access_control.rs module documentation or open an issue.

