use soroban_sdk::{contracterror, contracttype, symbol_short, Address, BytesN, Env, Vec};

/// Pair lifetime: 30 days in seconds.
pub const PAIR_LIFETIME_SECS: u64 = 30 * 86_400;

/// Maximum messages sent in one burst call.
pub const MAX_MESSAGE_BURST: u32 = 20;

// ─── Storage Keys ──────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum EntanglementKey {
    /// Auto-incrementing pair ID counter.
    Counter,
    /// Entanglement pair by ID.
    Pair(u64),
    /// Message log index per pair (pair_id → total messages sent).
    MsgCount(u64),
}

// ─── Errors ─────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum EntanglementError {
    /// The entanglement pair has expired or was never active.
    PairNotActive = 1,
    /// Caller is not a participant of this pair.
    NotAuthorized = 2,
    /// Pair does not exist.
    PairNotFound = 3,
    /// A ship cannot be entangled with itself.
    SameShip = 4,
    /// Burst size exceeded MAX_MESSAGE_BURST.
    BurstTooLarge = 5,
    /// Message batch is empty.
    EmptyBatch = 6,
}

// ─── Data Types ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct EntanglementPair {
    pub id: u64,
    pub ship_a: u64,
    pub ship_b: u64,
    /// Addresses authorized to send on behalf of each ship.
    pub owner_a: Address,
    pub owner_b: Address,
    pub created_at: u64,
    /// Pair expires at this ledger timestamp.
    pub expires_at: u64,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct EntangledMessage {
    pub pair_id: u64,
    pub sender_ship: u64,
    /// 64-byte encrypted payload.
    pub payload: BytesN<64>,
    pub sent_at: u64,
    pub sequence: u64,
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn require_active_pair(pair: &EntanglementPair, now: u64) -> Result<(), EntanglementError> {
    if !pair.active || now > pair.expires_at {
        return Err(EntanglementError::PairNotActive);
    }
    Ok(())
}

fn require_participant(
    pair: &EntanglementPair,
    caller: &Address,
) -> Result<u64, EntanglementError> {
    if *caller == pair.owner_a {
        return Ok(pair.ship_a);
    }
    if *caller == pair.owner_b {
        return Ok(pair.ship_b);
    }
    Err(EntanglementError::NotAuthorized)
}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Establish a new entanglement pair between two ships.
///
/// `owner_a` initiates and must authorize the call. Both ship IDs must differ.
/// The pair decays after [`PAIR_LIFETIME_SECS`] seconds.
pub fn create_entanglement_pair(
    env: &Env,
    owner_a: &Address,
    ship_a: u64,
    owner_b: &Address,
    ship_b: u64,
) -> Result<u64, EntanglementError> {
    owner_a.require_auth();

    if ship_a == ship_b {
        return Err(EntanglementError::SameShip);
    }

    let pair_id: u64 = env
        .storage()
        .instance()
        .get(&EntanglementKey::Counter)
        .unwrap_or(0);

    let now = env.ledger().timestamp();
    let pair = EntanglementPair {
        id: pair_id,
        ship_a,
        ship_b,
        owner_a: owner_a.clone(),
        owner_b: owner_b.clone(),
        created_at: now,
        expires_at: now + PAIR_LIFETIME_SECS,
        active: true,
    };

    env.storage()
        .persistent()
        .set(&EntanglementKey::Pair(pair_id), &pair);
    env.storage()
        .instance()
        .set(&EntanglementKey::Counter, &(pair_id + 1));
    env.storage()
        .instance()
        .set(&EntanglementKey::MsgCount(pair_id), &0u64);

    env.events().publish(
        (symbol_short!("entangle"), symbol_short!("create")),
        (owner_a.clone(), ship_a, ship_b, pair_id, now),
    );

    Ok(pair_id)
}

/// Transmit an encrypted 64-byte message over an active entanglement pair.
///
/// Only the pair participants (owner_a or owner_b) may send. The message
/// payload is treated as an opaque encrypted blob; the contract does not
/// decrypt it, ensuring end-to-end confidentiality.
pub fn send_entangled_message(
    env: &Env,
    caller: &Address,
    pair_id: u64,
    message: &BytesN<64>,
) -> Result<u64, EntanglementError> {
    caller.require_auth();

    let pair: EntanglementPair = env
        .storage()
        .persistent()
        .get(&EntanglementKey::Pair(pair_id))
        .ok_or(EntanglementError::PairNotFound)?;

    let now = env.ledger().timestamp();
    require_active_pair(&pair, now)?;
    let sender_ship = require_participant(&pair, caller)?;

    let seq: u64 = env
        .storage()
        .instance()
        .get(&EntanglementKey::MsgCount(pair_id))
        .unwrap_or(0);

    let new_seq = seq + 1;
    env.storage()
        .instance()
        .set(&EntanglementKey::MsgCount(pair_id), &new_seq);

    env.events().publish(
        (symbol_short!("entangle"), symbol_short!("msg")),
        (pair_id, sender_ship, new_seq, now),
    );

    Ok(new_seq)
}

/// Send up to [`MAX_MESSAGE_BURST`] messages in a single transaction.
///
/// Returns the sequence number of the last message sent.
pub fn send_entangled_message_batch(
    env: &Env,
    caller: &Address,
    pair_id: u64,
    messages: &Vec<BytesN<64>>,
) -> Result<u64, EntanglementError> {
    caller.require_auth();

    let n = messages.len();
    if n == 0 {
        return Err(EntanglementError::EmptyBatch);
    }
    if n > MAX_MESSAGE_BURST {
        return Err(EntanglementError::BurstTooLarge);
    }

    let mut last_seq = 0u64;
    for msg in messages.iter() {
        last_seq = send_entangled_message(env, caller, pair_id, &msg)?;
    }

    Ok(last_seq)
}

/// Deactivate a pair before it naturally expires.
///
/// Either participant may dissolve the pair.
pub fn dissolve_pair(
    env: &Env,
    caller: &Address,
    pair_id: u64,
) -> Result<(), EntanglementError> {
    caller.require_auth();

    let mut pair: EntanglementPair = env
        .storage()
        .persistent()
        .get(&EntanglementKey::Pair(pair_id))
        .ok_or(EntanglementError::PairNotFound)?;

    require_participant(&pair, caller)?;

    pair.active = false;
    env.storage()
        .persistent()
        .set(&EntanglementKey::Pair(pair_id), &pair);

    env.events().publish(
        (symbol_short!("entangle"), symbol_short!("dissolv")),
        (pair_id, caller.clone()),
    );

    Ok(())
}

/// Get pair data by ID.
pub fn get_entanglement_pair(
    env: &Env,
    pair_id: u64,
) -> Result<EntanglementPair, EntanglementError> {
    env.storage()
        .persistent()
        .get(&EntanglementKey::Pair(pair_id))
        .ok_or(EntanglementError::PairNotFound)
}

/// Return the total messages sent over a pair.
pub fn get_message_count(env: &Env, pair_id: u64) -> u64 {
    env.storage()
        .instance()
        .get(&EntanglementKey::MsgCount(pair_id))
        .unwrap_or(0)
}
