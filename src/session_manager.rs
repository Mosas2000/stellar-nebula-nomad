use soroban_sdk::{contracttype, contracterror, symbol_short, Address, Env};

/// Session time-to-live: 24 hours in seconds.
pub const SESSION_TTL: u64 = 86_400;
/// Maximum concurrent active sessions per player.
pub const MAX_SESSIONS_PER_PLAYER: u32 = 3;

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum SessionKey {
    /// Individual session data keyed by session ID.
    Session(u64),
    /// Active session count for a player (enforces 3-session cap).
    PlayerSessionCount(Address),
    /// Global auto-increment counter for session IDs.
    SessionCount,
}

// ─── Data Types ───────────────────────────────────────────────────────────────

/// A timed nebula exploration session tied to a ship.
#[derive(Clone)]
#[contracttype]
pub struct Session {
    pub id: u64,
    pub ship_id: u64,
    pub owner: Address,
    pub started_at: u64,
    pub expires_at: u64,
    pub active: bool,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SessionError {
    SessionNotFound = 1,
    SessionExpired = 2,
    TooManySessions = 3,
    NotOwner = 4,
}

// ─── Functions ────────────────────────────────────────────────────────────────

/// Start a timed nebula exploration session for `owner` using `ship_id`.
///
/// Enforces a cap of `MAX_SESSIONS_PER_PLAYER` concurrent active sessions.
/// TTL is pulled from the `SESSION_TTL` constant (24 h). Emits `SessionStarted`.
pub fn start_session(env: &Env, owner: Address, ship_id: u64) -> Result<u64, SessionError> {
    owner.require_auth();

    let count_key = SessionKey::PlayerSessionCount(owner.clone());
    let active_count: u32 = env
        .storage()
        .persistent()
        .get(&count_key)
        .unwrap_or(0u32);

    if active_count >= MAX_SESSIONS_PER_PLAYER {
        return Err(SessionError::TooManySessions);
    }

    let id: u64 = env
        .storage()
        .instance()
        .get(&SessionKey::SessionCount)
        .unwrap_or(0u64)
        + 1;
    env.storage().instance().set(&SessionKey::SessionCount, &id);

    let now = env.ledger().timestamp();
    let session = Session {
        id,
        ship_id,
        owner: owner.clone(),
        started_at: now,
        expires_at: now + SESSION_TTL,
        active: true,
    };

    env.storage()
        .persistent()
        .set(&SessionKey::Session(id), &session);
    env.storage()
        .persistent()
        .set(&count_key, &(active_count + 1));

    env.events().publish(
        (symbol_short!("session"), symbol_short!("started")),
        (owner, id, ship_id),
    );

    Ok(id)
}

/// Close a session. Anyone may close a session that has passed its TTL.
/// Only the owner may force-close an active session.
///
/// Decrements the player's active session counter. Emits `SessionExpired`.
pub fn expire_session(env: &Env, caller: Address, session_id: u64) -> Result<(), SessionError> {
    caller.require_auth();

    let mut session: Session = env
        .storage()
        .persistent()
        .get(&SessionKey::Session(session_id))
        .ok_or(SessionError::SessionNotFound)?;

    if !session.active {
        return Err(SessionError::SessionExpired);
    }

    let now = env.ledger().timestamp();
    // Owner can close any time; others only after TTL has elapsed.
    if session.owner != caller && session.expires_at > now {
        return Err(SessionError::NotOwner);
    }

    session.active = false;
    env.storage()
        .persistent()
        .set(&SessionKey::Session(session_id), &session);

    // Decrement active session counter for the owner.
    let count_key = SessionKey::PlayerSessionCount(session.owner.clone());
    let count: u32 = env
        .storage()
        .persistent()
        .get(&count_key)
        .unwrap_or(0u32);
    if count > 0 {
        env.storage().persistent().set(&count_key, &(count - 1));
    }

    env.events().publish(
        (symbol_short!("session"), symbol_short!("expired")),
        (caller, session_id),
    );

    Ok(())
}

/// Retrieve session data by ID.
pub fn get_session(env: &Env, session_id: u64) -> Result<Session, SessionError> {
    env.storage()
        .persistent()
        .get(&SessionKey::Session(session_id))
        .ok_or(SessionError::SessionNotFound)
}
