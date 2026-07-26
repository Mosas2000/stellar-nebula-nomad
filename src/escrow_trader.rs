use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

use crate::reentrancy_guard::{with_guard, ReentrancyError};

const ESCROW_EXPIRY: u64 = 172800;
const MAX_CONCURRENT_ESCROWS: u32 = 5;

#[derive(Clone)]
#[contracttype]
pub enum EscrowKey {
    EscrowCounter,
    EscrowData(u64),
    PlayerEscrowCount(Address),
    EscrowConfirmation(u64, Address),
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum EscrowError {
    TradeExpired = 1,
    AlreadyConfirmed = 2,
    NotParticipant = 3,
    EscrowNotFound = 4,
    MaxEscrowsReached = 5,
    InvalidAssets = 6,
    NotFullyConfirmed = 7,
    /// A guarded section was re-entered (Issue #238).
    Reentrancy = 8,
}

impl From<ReentrancyError> for EscrowError {
    fn from(_: ReentrancyError) -> Self {
        EscrowError::Reentrancy
    }
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct TradeAsset {
    pub asset_type: Symbol,
    pub asset_id: u64,
    pub quantity: i128,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct Escrow {
    pub escrow_id: u64,
    pub trader_a: Address,
    pub trader_b: Address,
    pub assets_a: Vec<TradeAsset>,
    pub assets_b: Vec<TradeAsset>,
    pub confirmed_a: bool,
    pub confirmed_b: bool,
    pub completed: bool,
    pub created_at: u64,
    pub expires_at: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct EscrowResult {
    pub escrow_id: u64,
    pub trader_a: Address,
    pub trader_b: Address,
    pub completed: bool,
}

pub fn initiate_escrow(
    env: &Env,
    trader_a: Address,
    trader_b: Address,
    assets_a: Vec<TradeAsset>,
    assets_b: Vec<TradeAsset>,
) -> Result<Escrow, EscrowError> {
    trader_a.require_auth();

    if assets_a.is_empty() || assets_b.is_empty() {
        return Err(EscrowError::InvalidAssets);
    }

    let escrow_count_a = env
        .storage()
        .persistent()
        .get::<EscrowKey, u32>(&EscrowKey::PlayerEscrowCount(trader_a.clone()))
        .unwrap_or(0);

    if escrow_count_a >= MAX_CONCURRENT_ESCROWS {
        return Err(EscrowError::MaxEscrowsReached);
    }

    let escrow_counter = env
        .storage()
        .persistent()
        .get::<EscrowKey, u64>(&EscrowKey::EscrowCounter)
        .unwrap_or(0)
        + 1;

    env.storage()
        .persistent()
        .set(&EscrowKey::EscrowCounter, &escrow_counter);

    let current_time = env.ledger().timestamp();
    let expires_at = current_time + ESCROW_EXPIRY;

    let escrow = Escrow {
        escrow_id: escrow_counter,
        trader_a: trader_a.clone(),
        trader_b: trader_b.clone(),
        assets_a,
        assets_b,
        confirmed_a: true,
        confirmed_b: false,
        completed: false,
        created_at: current_time,
        expires_at,
    };

    env.storage()
        .persistent()
        .set(&EscrowKey::EscrowData(escrow_counter), &escrow);

    env.storage().persistent().set(
        &EscrowKey::PlayerEscrowCount(trader_a.clone()),
        &(escrow_count_a + 1),
    );

    env.storage().persistent().set(
        &EscrowKey::EscrowConfirmation(escrow_counter, trader_a.clone()),
        &true,
    );

    env.events().publish(
        (symbol_short!("escrow"), symbol_short!("init")),
        (escrow_counter, trader_a, trader_b),
    );

    Ok(escrow)
}

pub fn confirm_escrow(env: &Env, escrow_id: u64, trader: Address) -> Result<Escrow, EscrowError> {
    trader.require_auth();

    let mut escrow = env
        .storage()
        .persistent()
        .get::<EscrowKey, Escrow>(&EscrowKey::EscrowData(escrow_id))
        .ok_or(EscrowError::EscrowNotFound)?;

    if env.ledger().timestamp() > escrow.expires_at {
        return Err(EscrowError::TradeExpired);
    }

    if trader != escrow.trader_a && trader != escrow.trader_b {
        return Err(EscrowError::NotParticipant);
    }

    let already_confirmed = env
        .storage()
        .persistent()
        .get::<EscrowKey, bool>(&EscrowKey::EscrowConfirmation(escrow_id, trader.clone()))
        .unwrap_or(false);

    if already_confirmed {
        return Err(EscrowError::AlreadyConfirmed);
    }

    if trader == escrow.trader_a {
        escrow.confirmed_a = true;
    } else {
        escrow.confirmed_b = true;
    }

    env.storage().persistent().set(
        &EscrowKey::EscrowConfirmation(escrow_id, trader.clone()),
        &true,
    );

    env.storage()
        .persistent()
        .set(&EscrowKey::EscrowData(escrow_id), &escrow);

    Ok(escrow)
}

/// # Security
/// * Holds a reentrancy guard across the confirmation-count decrements so a
///   nested call cannot observe or double-spend the half-updated escrow
///   state before settlement finishes (Issue #238).
pub fn complete_escrow(env: &Env, escrow_id: u64) -> Result<EscrowResult, EscrowError> {
    with_guard(env, || {
        let mut escrow = env
            .storage()
            .persistent()
            .get::<EscrowKey, Escrow>(&EscrowKey::EscrowData(escrow_id))
            .ok_or(EscrowError::EscrowNotFound)?;

        if env.ledger().timestamp() > escrow.expires_at {
            return Err(EscrowError::TradeExpired);
        }

        if !escrow.confirmed_a || !escrow.confirmed_b {
            return Err(EscrowError::NotFullyConfirmed);
        }

        if escrow.completed {
            return Err(EscrowError::AlreadyConfirmed);
        }

        escrow.completed = true;

        env.storage()
            .persistent()
            .set(&EscrowKey::EscrowData(escrow_id), &escrow);

        let count_a = env
            .storage()
            .persistent()
            .get::<EscrowKey, u32>(&EscrowKey::PlayerEscrowCount(escrow.trader_a.clone()))
            .unwrap_or(1);
        env.storage().persistent().set(
            &EscrowKey::PlayerEscrowCount(escrow.trader_a.clone()),
            &count_a.saturating_sub(1),
        );

        let count_b = env
            .storage()
            .persistent()
            .get::<EscrowKey, u32>(&EscrowKey::PlayerEscrowCount(escrow.trader_b.clone()))
            .unwrap_or(1);
        env.storage().persistent().set(
            &EscrowKey::PlayerEscrowCount(escrow.trader_b.clone()),
            &count_b.saturating_sub(1),
        );

        let result = EscrowResult {
            escrow_id,
            trader_a: escrow.trader_a.clone(),
            trader_b: escrow.trader_b.clone(),
            completed: true,
        };

        env.events().publish(
            (symbol_short!("escrow"), symbol_short!("complete")),
            (escrow_id, escrow.trader_a, escrow.trader_b),
        );

        Ok(result)
    })
}

pub fn cancel_escrow(env: &Env, escrow_id: u64, trader: Address) -> Result<(), EscrowError> {
    trader.require_auth();

    let escrow = env
        .storage()
        .persistent()
        .get::<EscrowKey, Escrow>(&EscrowKey::EscrowData(escrow_id))
        .ok_or(EscrowError::EscrowNotFound)?;

    if trader != escrow.trader_a && trader != escrow.trader_b {
        return Err(EscrowError::NotParticipant);
    }

    if escrow.completed {
        return Err(EscrowError::AlreadyConfirmed);
    }

    env.storage()
        .persistent()
        .remove(&EscrowKey::EscrowData(escrow_id));

    let count = env
        .storage()
        .persistent()
        .get::<EscrowKey, u32>(&EscrowKey::PlayerEscrowCount(trader.clone()))
        .unwrap_or(1);
    env.storage().persistent().set(
        &EscrowKey::PlayerEscrowCount(trader.clone()),
        &count.saturating_sub(1),
    );

    env.events().publish(
        (symbol_short!("escrow"), symbol_short!("cancel")),
        (escrow_id, trader),
    );

    Ok(())
}

pub fn get_escrow(env: &Env, escrow_id: u64) -> Option<Escrow> {
    env.storage()
        .persistent()
        .get::<EscrowKey, Escrow>(&EscrowKey::EscrowData(escrow_id))
}

// ── Tests (Issue #238: reentrancy safety) ───────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, testutils::Address as _};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    fn make_env() -> (Env, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(Stub, ());
        (env, contract_id)
    }

    fn open_and_confirm_escrow(env: &Env, trader_a: &Address, trader_b: &Address) -> u64 {
        let mut assets_a = Vec::new(env);
        assets_a.push_back(TradeAsset {
            asset_type: Symbol::new(env, "ship"),
            asset_id: 1,
            quantity: 1,
        });
        let mut assets_b = Vec::new(env);
        assets_b.push_back(TradeAsset {
            asset_type: Symbol::new(env, "essence"),
            asset_id: 0,
            quantity: 100,
        });

        let escrow = initiate_escrow(env, trader_a.clone(), trader_b.clone(), assets_a, assets_b)
            .expect("initiate_escrow should succeed");
        confirm_escrow(env, escrow.escrow_id, trader_b.clone())
            .expect("confirm_escrow should succeed");
        escrow.escrow_id
    }

    #[test]
    fn test_complete_escrow_rejected_while_guard_held() {
        // Simulates a reentrant callback attempting to re-enter
        // complete_escrow while a prior invocation's guard is held.
        let (env, contract_id) = make_env();
        let trader_a = Address::generate(&env);
        let trader_b = Address::generate(&env);

        env.as_contract(&contract_id, || {
            let escrow_id = open_and_confirm_escrow(&env, &trader_a, &trader_b);

            crate::reentrancy_guard::acquire(&env).expect("lock should be free");
            let result = complete_escrow(&env, escrow_id);
            assert_eq!(result, Err(EscrowError::Reentrancy));
            crate::reentrancy_guard::release(&env);

            // Once released, completion succeeds normally.
            let result = complete_escrow(&env, escrow_id).expect("complete_escrow should succeed");
            assert!(result.completed);
        });
    }

    #[test]
    fn test_complete_escrow_twice_rejected() {
        let (env, contract_id) = make_env();
        let trader_a = Address::generate(&env);
        let trader_b = Address::generate(&env);

        env.as_contract(&contract_id, || {
            let escrow_id = open_and_confirm_escrow(&env, &trader_a, &trader_b);

            complete_escrow(&env, escrow_id).expect("first completion should succeed");
            let result = complete_escrow(&env, escrow_id);
            assert_eq!(result, Err(EscrowError::AlreadyConfirmed));
        });
    }
}
