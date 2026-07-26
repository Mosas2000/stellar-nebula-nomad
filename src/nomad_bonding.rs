use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env};

use crate::reentrancy_guard::{with_guard, ReentrancyError};

/// ── Errors ────────────────────────────────────────────────────────────────
///
/// Yield delegation moves financial value between bonded players, so every
/// fallible path returns a typed contract error instead of panicking. This
/// keeps overflow and authorization failures observable to callers.
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum BondError {
    /// A player attempted to bond with themselves.
    SelfBond = 1,
    /// No bond exists for the supplied id.
    BondNotFound = 2,
    /// Caller is not the bond's designated partner.
    NotDesignatedPartner = 3,
    /// Bond is not in `Pending` status.
    BondNotPending = 4,
    /// Delegation percentage is outside the 1..=100 range.
    InvalidPercentage = 5,
    /// Bond is not in `Active` status.
    BondNotActive = 6,
    /// Caller is not a member of the bond.
    NotBondMember = 7,
    /// No yield delegation is configured for the bond.
    NoDelegation = 8,
    /// Caller is not the delegation's beneficiary.
    NotBeneficiary = 9,
    /// Bond has already been dissolved.
    AlreadyDissolved = 10,
    /// Caller is not a party to the bond.
    NotBondParty = 11,
    /// A checked arithmetic operation overflowed.
    ArithmeticOverflow = 12,
    /// A guarded section was re-entered (Issue #238).
    Reentrancy = 13,
}

impl From<ReentrancyError> for BondError {
    fn from(_: ReentrancyError) -> Self {
        BondError::Reentrancy
    }
}

/// ── Storage Keys ──────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    /// Auto-incrementing bond counter.
    BondCounter,
    /// Bond metadata keyed by bond_id.
    Bond(u64),
    /// Yield delegation config keyed by bond_id.
    YieldDel(u64),
    /// Cosmic essence balance for a player address.
    Essence(Address),
}

/// ── Bond Status ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum BondStatus {
    /// Bond created, waiting for partner to accept.
    Pending,
    /// Both parties confirmed — bond is live.
    Active,
    /// Bond dissolved by one of the parties.
    Dissolved,
}

/// ── Nomad Bond ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct NomadBond {
    pub bond_id: u64,
    pub initiator: Address,
    pub partner: Address,
    pub ship_id: u64,
    pub status: BondStatus,
    pub created_at: u64,
}

/// ── Yield Delegation ──────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct YieldDelegation {
    pub bond_id: u64,
    pub delegator: Address,
    pub beneficiary: Address,
    pub percentage: u32,
    pub total_yielded: u64,
}

/// ── Helper: next bond id ──────────────────────────────────────────────────

fn next_bond_id(env: &Env) -> Result<u64, BondError> {
    let current: u64 = env
        .storage()
        .instance()
        .get(&DataKey::BondCounter)
        .unwrap_or(0);
    let next = current
        .checked_add(1)
        .ok_or(BondError::ArithmeticOverflow)?;
    env.storage().instance().set(&DataKey::BondCounter, &next);
    Ok(next)
}

/// ── Helper: yield amount ──────────────────────────────────────────────────
///
/// Pure arithmetic for a yield share: `balance * percentage / 100` using
/// `checked_mul`/`checked_div`. Returns `None` on overflow so callers can
/// surface [`BondError::ArithmeticOverflow`] instead of panicking. Kept as a
/// standalone, side-effect-free function so it can be property-tested directly.
fn calculate_yield_amount(balance: u64, percentage: u32) -> Option<u64> {
    balance.checked_mul(percentage as u64)?.checked_div(100)
}

/// ── create_bond ───────────────────────────────────────────────────────────
///
/// Creates a new Nomad Bond between the caller (`initiator`) and a
/// `partner`.  The bond starts in `Pending` status until the partner
/// accepts it via `accept_bond`.
///
/// # Arguments
/// * `initiator` – The player who proposes the bond (must authorize).
/// * `ship_id`   – The ship NFT the bond is attached to.
/// * `partner`   – The address invited to bond.
///
/// # Errors
/// * [`BondError::SelfBond`] if `initiator` and `partner` are the same address.
/// * [`BondError::ArithmeticOverflow`] if the bond counter overflows.
pub fn create_bond(
    env: &Env,
    initiator: &Address,
    ship_id: u64,
    partner: &Address,
) -> Result<NomadBond, BondError> {
    initiator.require_auth();

    if initiator == partner {
        return Err(BondError::SelfBond);
    }

    let bond_id = next_bond_id(env)?;
    let bond = NomadBond {
        bond_id,
        initiator: initiator.clone(),
        partner: partner.clone(),
        ship_id,
        status: BondStatus::Pending,
        created_at: env.ledger().timestamp(),
    };

    env.storage().instance().set(&DataKey::Bond(bond_id), &bond);

    env.events().publish(
        (symbol_short!("bond"), symbol_short!("created")),
        (bond_id, initiator.clone(), partner.clone()),
    );

    Ok(bond)
}

/// ── accept_bond ───────────────────────────────────────────────────────────
///
/// The invited partner accepts a pending bond, moving it to `Active`.
///
/// # Errors
/// * [`BondError::BondNotFound`] if the bond does not exist.
/// * [`BondError::NotDesignatedPartner`] if the caller is not the partner.
/// * [`BondError::BondNotPending`] if the bond is not in `Pending` status.
pub fn accept_bond(env: &Env, partner: &Address, bond_id: u64) -> Result<NomadBond, BondError> {
    partner.require_auth();

    let mut bond: NomadBond = env
        .storage()
        .instance()
        .get(&DataKey::Bond(bond_id))
        .ok_or(BondError::BondNotFound)?;

    if bond.partner != *partner {
        return Err(BondError::NotDesignatedPartner);
    }
    if bond.status != BondStatus::Pending {
        return Err(BondError::BondNotPending);
    }

    bond.status = BondStatus::Active;
    env.storage().instance().set(&DataKey::Bond(bond_id), &bond);

    env.events().publish(
        (symbol_short!("bond"), symbol_short!("accepted")),
        (bond_id, partner.clone()),
    );

    Ok(bond)
}

/// ── delegate_yield ────────────────────────────────────────────────────────
///
/// Allows one bonded party (the `delegator`) to share a percentage of
/// their accrued cosmic essence with the other bonded party.
///
/// # Arguments
/// * `delegator`  – Must be either `initiator` or `partner` of the bond.
/// * `bond_id`    – An active bond.
/// * `percentage` – 1–100 inclusive.
///
/// # Errors
/// * [`BondError::InvalidPercentage`] if `percentage` is out of the 1..=100 range.
/// * [`BondError::BondNotFound`] if the bond does not exist.
/// * [`BondError::BondNotActive`] if the bond is not `Active`.
/// * [`BondError::NotBondMember`] if the caller is not part of the bond.
pub fn delegate_yield(
    env: &Env,
    delegator: &Address,
    bond_id: u64,
    percentage: u32,
) -> Result<YieldDelegation, BondError> {
    delegator.require_auth();

    if percentage == 0 || percentage > 100 {
        return Err(BondError::InvalidPercentage);
    }

    let bond: NomadBond = env
        .storage()
        .instance()
        .get(&DataKey::Bond(bond_id))
        .ok_or(BondError::BondNotFound)?;

    if bond.status != BondStatus::Active {
        return Err(BondError::BondNotActive);
    }

    let beneficiary = if *delegator == bond.initiator {
        bond.partner.clone()
    } else if *delegator == bond.partner {
        bond.initiator.clone()
    } else {
        return Err(BondError::NotBondMember);
    };

    let delegation = YieldDelegation {
        bond_id,
        delegator: delegator.clone(),
        beneficiary: beneficiary.clone(),
        percentage,
        total_yielded: 0,
    };

    env.storage()
        .instance()
        .set(&DataKey::YieldDel(bond_id), &delegation);

    env.events().publish(
        (symbol_short!("yield"), symbol_short!("delegatd")),
        (bond_id, delegator.clone(), percentage),
    );

    Ok(delegation)
}

/// ── accrue_essence ────────────────────────────────────────────────────────
///
/// Award cosmic essence to a player.  Called by game logic (e.g. after a
/// successful nebula scan).  In a production setup this would be an
/// internal / cross-contract call from the `NebulaExplorer` contract.
///
/// # Errors
/// * [`BondError::ArithmeticOverflow`] if the new balance overflows `u64`.
pub fn accrue_essence(env: &Env, player: &Address, amount: u64) -> Result<(), BondError> {
    player.require_auth();

    let balance: u64 = env
        .storage()
        .instance()
        .get(&DataKey::Essence(player.clone()))
        .unwrap_or(0);
    let new_balance = balance
        .checked_add(amount)
        .ok_or(BondError::ArithmeticOverflow)?;
    env.storage()
        .instance()
        .set(&DataKey::Essence(player.clone()), &new_balance);
    Ok(())
}

/// ── claim_yield ───────────────────────────────────────────────────────────
///
/// The beneficiary of a yield delegation claims their share.  The
/// delegator's cosmic essence is reduced by the delegated percentage and
/// transferred to the beneficiary.
///
/// # Security
/// * Only the beneficiary address recorded in the delegation can claim.
/// * The bond must still be `Active`.
/// * If the delegator has zero balance, nothing is transferred.
///
/// Returns the amount transferred.
///
/// # Errors
/// * [`BondError::BondNotFound`] if the bond does not exist.
/// * [`BondError::BondNotActive`] if the bond is not `Active`.
/// * [`BondError::NoDelegation`] if no delegation is configured for the bond.
/// * [`BondError::NotBeneficiary`] if the caller is not the beneficiary.
/// * [`BondError::ArithmeticOverflow`] if any balance calculation overflows.
/// # Security
/// * Holds a reentrancy guard across the debit/credit sequence so a
///   malicious `require_auth` callback (e.g. a custom-account contract)
///   cannot re-enter `claim_yield` mid-transfer (Issue #238).
pub fn claim_yield(env: &Env, claimer: &Address, bond_id: u64) -> Result<u64, BondError> {
    claimer.require_auth();

    with_guard(env, || {
        let bond: NomadBond = env
            .storage()
            .instance()
            .get(&DataKey::Bond(bond_id))
            .ok_or(BondError::BondNotFound)?;

        if bond.status != BondStatus::Active {
            return Err(BondError::BondNotActive);
        }

        let mut delegation: YieldDelegation = env
            .storage()
            .instance()
            .get(&DataKey::YieldDel(bond_id))
            .ok_or(BondError::NoDelegation)?;

        if *claimer != delegation.beneficiary {
            return Err(BondError::NotBeneficiary);
        }

        let delegator_balance: u64 = env
            .storage()
            .instance()
            .get(&DataKey::Essence(delegation.delegator.clone()))
            .unwrap_or(0);

        if delegator_balance == 0 {
            return Ok(0);
        }

        // Checked: balance * percentage / 100 cannot overflow silently.
        let yield_amount = calculate_yield_amount(delegator_balance, delegation.percentage)
            .ok_or(BondError::ArithmeticOverflow)?;

        if yield_amount == 0 {
            return Ok(0);
        }

        // Debit delegator (checked: yield_amount <= delegator_balance, but verified).
        let new_delegator_balance = delegator_balance
            .checked_sub(yield_amount)
            .ok_or(BondError::ArithmeticOverflow)?;
        env.storage().instance().set(
            &DataKey::Essence(delegation.delegator.clone()),
            &new_delegator_balance,
        );

        // Credit beneficiary (checked add).
        let claimer_balance: u64 = env
            .storage()
            .instance()
            .get(&DataKey::Essence(claimer.clone()))
            .unwrap_or(0);
        let new_claimer_balance = claimer_balance
            .checked_add(yield_amount)
            .ok_or(BondError::ArithmeticOverflow)?;
        env.storage()
            .instance()
            .set(&DataKey::Essence(claimer.clone()), &new_claimer_balance);

        delegation.total_yielded = delegation
            .total_yielded
            .checked_add(yield_amount)
            .ok_or(BondError::ArithmeticOverflow)?;
        env.storage()
            .instance()
            .set(&DataKey::YieldDel(bond_id), &delegation);

        env.events().publish(
            (symbol_short!("yield"), symbol_short!("claimed")),
            (bond_id, claimer.clone(), yield_amount),
        );

        Ok(yield_amount)
    })
}

/// ── dissolve_bond ─────────────────────────────────────────────────────────
///
/// Either the initiator or partner can dissolve an active bond.
/// Once dissolved, no further yield claims can be made.
///
/// # Errors
/// * [`BondError::BondNotFound`] if the bond does not exist.
/// * [`BondError::AlreadyDissolved`] if the bond is already dissolved.
/// * [`BondError::NotBondParty`] if the caller is not a bonded party.
pub fn dissolve_bond(env: &Env, caller: &Address, bond_id: u64) -> Result<NomadBond, BondError> {
    caller.require_auth();

    let mut bond: NomadBond = env
        .storage()
        .instance()
        .get(&DataKey::Bond(bond_id))
        .ok_or(BondError::BondNotFound)?;

    if bond.status == BondStatus::Dissolved {
        return Err(BondError::AlreadyDissolved);
    }

    if *caller != bond.initiator && *caller != bond.partner {
        return Err(BondError::NotBondParty);
    }

    bond.status = BondStatus::Dissolved;
    env.storage().instance().set(&DataKey::Bond(bond_id), &bond);

    env.events().publish(
        (symbol_short!("bond"), symbol_short!("dissolve")),
        (bond_id, caller.clone()),
    );

    Ok(bond)
}

/// ── get_bond ──────────────────────────────────────────────────────────────
///
/// Read-only view of a bond by its ID.
pub fn get_bond(env: &Env, bond_id: u64) -> Result<NomadBond, BondError> {
    env.storage()
        .instance()
        .get(&DataKey::Bond(bond_id))
        .ok_or(BondError::BondNotFound)
}

/// ── get_yield_delegation ──────────────────────────────────────────────────
///
/// Read-only view of a yield delegation by its bond ID.
pub fn get_yield_delegation(env: &Env, bond_id: u64) -> Result<YieldDelegation, BondError> {
    env.storage()
        .instance()
        .get(&DataKey::YieldDel(bond_id))
        .ok_or(BondError::NoDelegation)
}

/// ── get_essence_balance ───────────────────────────────────────────────────
///
/// Read-only view of a player's cosmic essence balance.
pub fn get_essence_balance(env: &Env, player: &Address) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::Essence(player.clone()))
        .unwrap_or(0)
}

/// ── Property-based tests ──────────────────────────────────────────────────
///
/// These exercise the arithmetic edge cases of yield delegation against the
/// pure [`calculate_yield_amount`] helper so the invariants hold for every
/// input, not just a handful of examples.
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Largest balance for which `balance * 100` still fits in u64.
    const MAX_NON_OVERFLOWING: u64 = u64::MAX / 100;

    proptest! {
        /// A yield share is always <= the originating balance for any valid
        /// percentage, and therefore the debit can never underflow.
        #[test]
        fn yield_never_exceeds_balance(
            balance in 0u64..=MAX_NON_OVERFLOWING,
            percentage in 1u32..=100u32,
        ) {
            let amount = calculate_yield_amount(balance, percentage)
                .expect("must not overflow within the non-overflowing range");
            prop_assert!(amount <= balance);
        }

        /// A 100% delegation returns exactly the full balance.
        #[test]
        fn full_percentage_returns_balance(balance in 0u64..=MAX_NON_OVERFLOWING) {
            prop_assert_eq!(calculate_yield_amount(balance, 100), Some(balance));
        }

        /// Overflow is detected (not silently wrapped) when balance * 100 would
        /// exceed u64::MAX.
        #[test]
        fn overflow_is_detected(balance in (MAX_NON_OVERFLOWING + 1)..=u64::MAX) {
            prop_assert_eq!(calculate_yield_amount(balance, 100), None);
        }

        /// The helper never panics for any input in the full u64/percentage space.
        #[test]
        fn never_panics(balance in any::<u64>(), percentage in 0u32..=100u32) {
            let _ = calculate_yield_amount(balance, percentage);
        }
    }

    #[test]
    fn zero_balance_yields_zero() {
        assert_eq!(calculate_yield_amount(0, 50), Some(0));
    }

    #[test]
    fn max_balance_one_percent_does_not_overflow() {
        // u64::MAX * 1 fits, so 1% is well-defined at the extreme.
        assert_eq!(calculate_yield_amount(u64::MAX, 1), Some(u64::MAX / 100));
    }

    #[test]
    fn max_balance_full_percentage_overflows() {
        // u64::MAX * 100 overflows and must be reported, not wrapped.
        assert_eq!(calculate_yield_amount(u64::MAX, 100), None);
    }

    // ── Storage & auth tests ──────────────────────────────────────────────────
    use soroban_sdk::{contract, contractimpl, testutils::Address as _};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    fn make_env() -> (Env, Address) {
        let env = Env::default();
        let contract_id = env.register(Stub, ());
        (env, contract_id)
    }

    #[test]
    fn test_get_bond_not_found_returns_error() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let result = get_bond(&env, 999);
            assert_eq!(result, Err(BondError::BondNotFound));
        });
    }

    #[test]
    fn test_get_yield_delegation_not_found_returns_error() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let result = get_yield_delegation(&env, 999);
            assert_eq!(result, Err(BondError::NoDelegation));
        });
    }

    #[test]
    fn test_get_essence_balance_defaults_to_zero() {
        let (env, contract_id) = make_env();
        let player = Address::generate(&env);
        env.as_contract(&contract_id, || {
            let balance = get_essence_balance(&env, &player);
            assert_eq!(balance, 0);
        });
    }

    #[test]
    fn test_create_bond_success() {
        let (env, contract_id) = make_env();
        let initiator = Address::generate(&env);
        let partner = Address::generate(&env);

        env.mock_all_auths();

        env.as_contract(&contract_id, || {
            let bond =
                create_bond(&env, &initiator, 1, &partner).expect("create_bond should succeed");

            assert_eq!(bond.initiator, initiator);
            assert_eq!(bond.partner, partner);
            assert_eq!(bond.status, BondStatus::Pending);
            assert_eq!(bond.ship_id, 1);
        });
    }

    #[test]
    fn test_create_bond_self_bond_rejected() {
        let (env, contract_id) = make_env();
        let player = Address::generate(&env);

        env.mock_all_auths();

        env.as_contract(&contract_id, || {
            let result = create_bond(&env, &player, 1, &player);
            assert_eq!(result, Err(BondError::SelfBond));
        });
    }

    #[test]
    fn test_accept_bond_invalid_partner_rejected() {
        let (env, contract_id) = make_env();
        let initiator = Address::generate(&env);
        let partner = Address::generate(&env);
        let stranger = Address::generate(&env);

        env.mock_all_auths();

        env.as_contract(&contract_id, || {
            let bond =
                create_bond(&env, &initiator, 1, &partner).expect("create_bond should succeed");

            let result = accept_bond(&env, &stranger, bond.bond_id);
            assert_eq!(result, Err(BondError::NotDesignatedPartner));
        });
    }

    #[test]
    fn test_dissolve_bond_non_member_rejected() {
        let (env, contract_id) = make_env();
        let initiator = Address::generate(&env);
        let partner = Address::generate(&env);
        let stranger = Address::generate(&env);

        env.mock_all_auths();

        env.as_contract(&contract_id, || {
            let bond =
                create_bond(&env, &initiator, 1, &partner).expect("create_bond should succeed");
            let _ = accept_bond(&env, &partner, bond.bond_id);

            let result = dissolve_bond(&env, &stranger, bond.bond_id);
            assert_eq!(result, Err(BondError::NotBondParty));
        });
    }

    #[test]
    fn test_claim_yield_no_delegation_rejected() {
        let (env, contract_id) = make_env();
        let initiator = Address::generate(&env);
        let partner = Address::generate(&env);

        env.mock_all_auths();

        env.as_contract(&contract_id, || {
            let bond =
                create_bond(&env, &initiator, 1, &partner).expect("create_bond should succeed");
            let _ = accept_bond(&env, &partner, bond.bond_id);

            let result = claim_yield(&env, &partner, bond.bond_id);
            assert_eq!(result, Err(BondError::NoDelegation));
        });
    }

    #[test]
    fn test_delegate_yield_invalid_percentage_rejected() {
        let (env, contract_id) = make_env();
        let initiator = Address::generate(&env);
        let partner = Address::generate(&env);

        env.mock_all_auths();

        env.as_contract(&contract_id, || {
            let bond =
                create_bond(&env, &initiator, 1, &partner).expect("create_bond should succeed");
            let _ = accept_bond(&env, &partner, bond.bond_id);

            let result = delegate_yield(&env, &initiator, bond.bond_id, 0);
            assert_eq!(result, Err(BondError::InvalidPercentage));

            let result = delegate_yield(&env, &initiator, bond.bond_id, 101);
            assert_eq!(result, Err(BondError::InvalidPercentage));
        });
    }

    #[test]
    fn test_claim_yield_rejected_while_guard_held() {
        // Simulates a reentrant callback (e.g. a malicious require_auth
        // callback) attempting to re-enter claim_yield while a prior
        // invocation's guard is still held (Issue #238).
        let (env, contract_id) = make_env();
        let initiator = Address::generate(&env);
        let partner = Address::generate(&env);

        env.mock_all_auths();

        env.as_contract(&contract_id, || {
            let bond =
                create_bond(&env, &initiator, 1, &partner).expect("create_bond should succeed");
            let _ = accept_bond(&env, &partner, bond.bond_id);
            let _ = delegate_yield(&env, &initiator, bond.bond_id, 50);
            let _ = accrue_essence(&env, &initiator, 1000);

            crate::reentrancy_guard::acquire(&env).expect("lock should be free");
            let result = claim_yield(&env, &partner, bond.bond_id);
            assert_eq!(result, Err(BondError::Reentrancy));
            crate::reentrancy_guard::release(&env);

            // Once released, the call succeeds normally.
            let amount = claim_yield(&env, &partner, bond.bond_id)
                .expect("claim_yield should succeed once unlocked");
            assert_eq!(amount, 500);
        });
    }

    #[test]
    fn test_full_bond_lifecycle_with_yield() {
        let (env, contract_id) = make_env();
        let initiator = Address::generate(&env);
        let partner = Address::generate(&env);

        env.mock_all_auths();

        env.as_contract(&contract_id, || {
            let bond =
                create_bond(&env, &initiator, 1, &partner).expect("create_bond should succeed");
            assert_eq!(bond.status, BondStatus::Pending);

            let bond =
                accept_bond(&env, &partner, bond.bond_id).expect("accept_bond should succeed");
            assert_eq!(bond.status, BondStatus::Active);

            let delegation = delegate_yield(&env, &initiator, bond.bond_id, 50)
                .expect("delegate_yield should succeed");
            assert_eq!(delegation.percentage, 50);
            assert_eq!(delegation.beneficiary, partner);

            let _ = accrue_essence(&env, &initiator, 1000).expect("accrue_essence should succeed");

            let amount =
                claim_yield(&env, &partner, bond.bond_id).expect("claim_yield should succeed");
            assert_eq!(amount, 500);

            let bond = dissolve_bond(&env, &initiator, bond.bond_id)
                .expect("dissolve_bond should succeed");
            assert_eq!(bond.status, BondStatus::Dissolved);
        });
    }
}
