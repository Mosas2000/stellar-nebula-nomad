use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, Env, String, Symbol, Vec,
};

// ─── Constants ────────────────────────────────────────────────────────────────

/// Maximum number of simultaneous active events.
pub const MAX_SIMULTANEOUS_EVENTS: u32 = 5;
/// Maximum players per event instance.
pub const MAX_PLAYERS_PER_EVENT: u32 = 500;
/// Event synchronization window in seconds.
pub const SYNC_WINDOW_SECS: u64 = 30;
/// Maximum raid boss health.
pub const MAX_RAID_BOSS_HEALTH: u64 = 1_000_000;
/// Default raid boss health.
pub const DEFAULT_RAID_BOSS_HEALTH: u64 = 500_000;
/// Maximum global challenge participants.
pub const MAX_GLOBAL_CHALLENGE_PARTICIPANTS: u32 = 10_000;

// ─── Storage Keys ─�───────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum RealtimeKey {
    /// Active event by ID.
    Event(u64),
    /// Global event counter.
    EventCounter,
    /// Event participants: event_id -> Vec<Address>.
    Participants(u64),
    /// Player event state: (event_id, player) -> PlayerEventState.
    PlayerState(u64, Address),
    /// Raid boss state: event_id -> RaidBossState.
    RaidBoss(u64),
    /// Global challenge: event_id -> GlobalChallenge.
    GlobalChallenge(u64),
    /// Global challenge progress: (event_id, player) -> u64.
    ChallengeProgress(u64, Address),
    /// Admin address.
    Admin,
    /// Event leaderboard: event_id -> Vec<EventLeaderboardEntry>.
    Leaderboard(u64),
}

// ─── Data Types ───────────────────────────────────────────────────────────────

/// Type of realtime event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
pub enum EventType {
    /// Cooperative raid boss fight.
    RaidBoss,
    /// Global challenge (collective goal).
    GlobalChallenge,
    /// Synchronized exploration event.
    SyncedExploration,
    /// Fleet battle event.
    FleetBattle,
}

/// Status of a realtime event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
pub enum EventStatus {
    /// Event is being set up.
    Setup = 0,
    /// Event is actively running.
    Active = 1,
    /// Event is in cooldown/finalization.
    Cooldown = 2,
    /// Event has ended.
    Ended = 3,
}

/// A realtime multiplayer event instance.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct RealtimeEvent {
    /// Unique event identifier.
    pub event_id: u64,
    /// Type of event.
    pub event_type: EventType,
    /// Event display name.
    pub name: String,
    /// Current status.
    pub status: EventStatus,
    /// Creator/admin of the event.
    pub creator: Address,
    /// When the event starts.
    pub start_time: u64,
    /// When the event ends.
    pub end_time: u64,
    /// Current number of participants.
    pub participant_count: u32,
    /// Maximum participants allowed.
    pub max_participants: u32,
    /// Reward pool for the event.
    pub reward_pool: i128,
    /// Timestamp of last sync update.
    pub last_sync: u64,
}

/// State of a player within an event.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct PlayerEventState {
    /// Player address.
    pub player: Address,
    /// Event ID.
    pub event_id: u64,
    /// Player's contribution score.
    pub contribution: u64,
    /// Whether the player has claimed rewards.
    pub reward_claimed: bool,
    /// Timestamp when player joined.
    pub joined_at: u64,
    /// Last action timestamp for sync.
    pub last_action: u64,
}

/// Raid boss state for cooperative events.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct RaidBossState {
    /// Boss identifier.
    pub boss_id: u64,
    /// Current health.
    pub current_health: u64,
    /// Maximum health.
    pub max_health: u64,
    /// Boss level (affects rewards).
    pub level: u32,
    /// Total damage dealt by all players.
    pub total_damage: u64,
    /// Number of players who participated.
    pub participant_count: u32,
    /// Whether the boss has been defeated.
    pub defeated: bool,
    /// Timestamp when defeated.
    pub defeated_at: Option<u64>,
}

/// Global challenge with collective goal.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct GlobalChallenge {
    /// Challenge identifier.
    pub challenge_id: u64,
    /// Challenge name.
    pub name: String,
    /// Target metric (e.g., "total_scans", "boss_damage").
    pub target_metric: Symbol,
    /// Collective target value.
    pub target_value: u64,
    /// Current collective progress.
    pub current_value: u64,
    /// Number of contributors.
    pub contributor_count: u32,
    /// Whether the challenge has been completed.
    pub completed: bool,
    /// Reward pool.
    pub reward_pool: i128,
}

/// Leaderboard entry for an event.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct EventLeaderboardEntry {
    /// Player address.
    pub player: Address,
    /// Player's score/contribution.
    pub score: u64,
    /// Player's rank.
    pub rank: u32,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RealtimeError {
    /// Event not found.
    EventNotFound = 110,
    /// Event is not active.
    EventNotActive = 111,
    /// Event is full.
    EventFull = 112,
    /// Player already in event.
    AlreadyParticipating = 113,
    /// Player not in event.
    NotParticipating = 114,
    /// Unauthorized action.
    Unauthorized = 115,
    /// Too many simultaneous events.
    TooManyEvents = 116,
    /// Invalid event type.
    InvalidEventType = 117,
    /// Boss already defeated.
    BossDefeated = 118,
    /// Challenge already completed.
    ChallengeCompleted = 119,
    /// Invalid contribution amount.
    InvalidContribution = 120,
    /// Reward already claimed.
    RewardAlreadyClaimed = 121,
    /// Event has ended.
    EventEnded = 122,
}

// ─── Helper Functions ─────────────────────────────────────────────────────────

fn require_admin(env: &Env, caller: &Address) -> Result<(), RealtimeError> {
    caller.require_auth();
    let admin: Address = env
        .storage()
        .instance()
        .get(&RealtimeKey::Admin)
        .ok_or(RealtimeError::Unauthorized)?;
    if caller != &admin {
        return Err(RealtimeError::Unauthorized);
    }
    Ok(())
}

fn next_event_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&RealtimeKey::EventCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage()
        .instance()
        .set(&RealtimeKey::EventCounter, &next);
    next
}

fn get_event_status(env: &Env, event_id: u64) -> Result<EventStatus, RealtimeError> {
    let event: RealtimeEvent = env
        .storage()
        .instance()
        .get(&RealtimeKey::Event(event_id))
        .ok_or(RealtimeError::EventNotFound)?;
    Ok(event.status)
}

// ─── Initialization ───────────────────────────────────────────────────────────

/// Initialize the realtime events system with an admin.
pub fn initialize_realtime_events(env: &Env, admin: &Address) -> Result<(), RealtimeError> {
    admin.require_auth();
    env.storage().instance().set(&RealtimeKey::Admin, admin);
    env.storage()
        .instance()
        .set(&RealtimeKey::EventCounter, &0u64);
    Ok(())
}

// ─── Event Creation ──────────────────────────────────────────────────────────

/// Create a new realtime event (admin only).
pub fn create_event(
    env: &Env,
    admin: Address,
    event_type: EventType,
    name: String,
    start_time: u64,
    end_time: u64,
    max_participants: u32,
    reward_pool: i128,
) -> Result<u64, RealtimeError> {
    require_admin(env, &admin)?;

    if start_time >= end_time {
        return Err(RealtimeError::InvalidEventType);
    }

    if max_participants == 0 || max_participants > MAX_PLAYERS_PER_EVENT {
        return Err(RealtimeError::EventFull);
    }

    let event_id = next_event_id(env);

    let event = RealtimeEvent {
        event_id,
        event_type,
        name,
        status: EventStatus::Setup,
        creator: admin.clone(),
        start_time,
        end_time,
        participant_count: 0,
        max_participants,
        reward_pool,
        last_sync: env.ledger().timestamp(),
    };

    env.storage()
        .instance()
        .set(&RealtimeKey::Event(event_id), &event);

    env.events().publish(
        (symbol_short!("rt"), symbol_short!("created")),
        (event_id, event_type as u32, start_time, end_time),
    );

    Ok(event_id)
}

/// Start an event (admin only, transitions from Setup to Active).
pub fn start_event(env: &Env, admin: Address, event_id: u64) -> Result<(), RealtimeError> {
    require_admin(env, &admin)?;

    let mut event: RealtimeEvent = env
        .storage()
        .instance()
        .get(&RealtimeKey::Event(event_id))
        .ok_or(RealtimeError::EventNotFound)?;

    if event.status != EventStatus::Setup {
        return Err(RealtimeError::EventNotActive);
    }

    event.status = EventStatus::Active;
    event.last_sync = env.ledger().timestamp();

    env.storage()
        .instance()
        .set(&RealtimeKey::Event(event_id), &event);

    env.events().publish(
        (symbol_short!("rt"), symbol_short!("started")),
        (event_id,),
    );

    Ok(())
}

/// End an event (admin only, transitions to Ended).
pub fn end_event(env: &Env, admin: Address, event_id: u64) -> Result<(), RealtimeError> {
    require_admin(env, &admin)?;

    let mut event: RealtimeEvent = env
        .storage()
        .instance()
        .get(&RealtimeKey::Event(event_id))
        .ok_or(RealtimeError::EventNotFound)?;

    if event.status == EventStatus::Ended {
        return Err(RealtimeError::EventEnded);
    }

    event.status = EventStatus::Ended;

    env.storage()
        .instance()
        .set(&RealtimeKey::Event(event_id), &event);

    env.events().publish(
        (symbol_short!("rt"), symbol_short!("ended")),
        (event_id, event.participant_count),
    );

    Ok(())
}

// ─── Player Participation ────────────────────────────────────────────────────

/// Join an active realtime event.
pub fn join_event(
    env: &Env,
    player: Address,
    event_id: u64,
) -> Result<(), RealtimeError> {
    player.require_auth();

    let status = get_event_status(env, event_id)?;
    if status != EventStatus::Active {
        return Err(RealtimeError::EventNotActive);
    }

    let state_key = RealtimeKey::PlayerState(event_id, player.clone());
    if env.storage().instance().has(&state_key) {
        return Err(RealtimeError::AlreadyParticipating);
    }

    let mut event: RealtimeEvent = env
        .storage()
        .instance()
        .get(&RealtimeKey::Event(event_id))
        .ok_or(RealtimeError::EventNotFound)?;

    if event.participant_count >= event.max_participants {
        return Err(RealtimeError::EventFull);
    }

    let now = env.ledger().timestamp();
    let player_state = PlayerEventState {
        player: player.clone(),
        event_id,
        contribution: 0,
        reward_claimed: false,
        joined_at: now,
        last_action: now,
    };

    env.storage()
        .instance()
        .set(&state_key, &player_state);

    // Add to participants list
    let mut participants: Vec<Address> = env
        .storage()
        .instance()
        .get(&RealtimeKey::Participants(event_id))
        .unwrap_or(Vec::new(env));
    participants.push_back(player.clone());
    env.storage()
        .instance()
        .set(&RealtimeKey::Participants(event_id), &participants);

    event.participant_count += 1;
    event.last_sync = now;
    env.storage()
        .instance()
        .set(&RealtimeKey::Event(event_id), &event);

    env.events().publish(
        (symbol_short!("rt"), symbol_short!("joined")),
        (event_id, player),
    );

    Ok(())
}

/// Record a player's contribution to an event.
pub fn record_contribution(
    env: &Env,
    player: Address,
    event_id: u64,
    amount: u64,
) -> Result<u64, RealtimeError> {
    player.require_auth();

    let status = get_event_status(env, event_id)?;
    if status != EventStatus::Active {
        return Err(RealtimeError::EventNotActive);
    }

    if amount == 0 {
        return Err(RealtimeError::InvalidContribution);
    }

    let state_key = RealtimeKey::PlayerState(event_id, player.clone());
    let mut state: PlayerEventState = env
        .storage()
        .instance()
        .get(&state_key)
        .ok_or(RealtimeError::NotParticipating)?;

    state.contribution = state.contribution.saturating_add(amount);
    state.last_action = env.ledger().timestamp();

    env.storage().instance().set(&state_key, &state);

    // Update event sync timestamp
    let mut event: RealtimeEvent = env
        .storage()
        .instance()
        .get(&RealtimeKey::Event(event_id))
        .ok_or(RealtimeError::EventNotFound)?;
    event.last_sync = env.ledger().timestamp();
    env.storage()
        .instance()
        .set(&RealtimeKey::Event(event_id), &event);

    env.events().publish(
        (symbol_short!("rt"), symbol_short!("contrib")),
        (event_id, player, amount, state.contribution),
    );

    Ok(state.contribution)
}

// ─── Raid Boss ───────────────────────────────────────────────────────────────

/// Spawn a raid boss for an event (admin only).
pub fn spawn_raid_boss(
    env: &Env,
    admin: Address,
    event_id: u64,
    boss_id: u64,
    max_health: u64,
    level: u32,
) -> Result<(), RealtimeError> {
    require_admin(env, &admin)?;

    let status = get_event_status(env, event_id)?;
    if status != EventStatus::Active {
        return Err(RealtimeError::EventNotActive);
    }

    let health = if max_health == 0 {
        DEFAULT_RAID_BOSS_HEALTH
    } else {
        max_health.min(MAX_RAID_BOSS_HEALTH)
    };

    let boss = RaidBossState {
        boss_id,
        current_health: health,
        max_health: health,
        level,
        total_damage: 0,
        participant_count: 0,
        defeated: false,
        defeated_at: None,
    };

    env.storage()
        .instance()
        .set(&RealtimeKey::RaidBoss(event_id), &boss);

    env.events().publish(
        (symbol_short!("rt"), symbol_short!("boss_spn")),
        (event_id, boss_id, health, level),
    );

    Ok(())
}

/// Deal damage to a raid boss.
pub fn deal_raid_damage(
    env: &Env,
    player: Address,
    event_id: u64,
    damage: u64,
) -> Result<u64, RealtimeError> {
    player.require_auth();

    let status = get_event_status(env, event_id)?;
    if status != EventStatus::Active {
        return Err(RealtimeError::EventNotActive);
    }

    let mut boss: RaidBossState = env
        .storage()
        .instance()
        .get(&RealtimeKey::RaidBoss(event_id))
        .ok_or(RealtimeError::EventNotFound)?;

    if boss.defeated {
        return Err(RealtimeError::BossDefeated);
    }

    boss.current_health = boss.current_health.saturating_sub(damage);
    boss.total_damage = boss.total_damage.saturating_add(damage);

    if boss.current_health == 0 {
        boss.defeated = true;
        boss.defeated_at = Some(env.ledger().timestamp());
    }

    env.storage()
        .instance()
        .set(&RealtimeKey::RaidBoss(event_id), &boss);

    // Record contribution
    let _ = record_contribution(env, player, event_id, damage);

    env.events().publish(
        (symbol_short!("rt"), symbol_short!("boss_dmg")),
        (event_id, damage, boss.current_health, boss.defeated),
    );

    Ok(boss.current_health)
}

/// Get raid boss state.
pub fn get_raid_boss(env: &Env, event_id: u64) -> Result<RaidBossState, RealtimeError> {
    env.storage()
        .instance()
        .get(&RealtimeKey::RaidBoss(event_id))
        .ok_or(RealtimeError::EventNotFound)
}

// ─── Global Challenges ───────────────────────────────────────────────────────

/// Create a global challenge (admin only).
pub fn create_global_challenge(
    env: &Env,
    admin: Address,
    event_id: u64,
    name: String,
    target_metric: Symbol,
    target_value: u64,
    reward_pool: i128,
) -> Result<(), RealtimeError> {
    require_admin(env, &admin)?;

    let status = get_event_status(env, event_id)?;
    if status != EventStatus::Active {
        return Err(RealtimeError::EventNotActive);
    }

    let challenge = GlobalChallenge {
        challenge_id: event_id,
        name,
        target_metric: target_metric.clone(),
        target_value,
        current_value: 0,
        contributor_count: 0,
        completed: false,
        reward_pool,
    };

    env.storage()
        .instance()
        .set(&RealtimeKey::GlobalChallenge(event_id), &challenge);

    env.events().publish(
        (symbol_short!("rt"), symbol_short!("challenge")),
        (event_id, target_metric, target_value),
    );

    Ok(())
}

/// Record progress toward a global challenge.
pub fn record_challenge_progress(
    env: &Env,
    player: Address,
    event_id: u64,
    amount: u64,
) -> Result<u64, RealtimeError> {
    player.require_auth();

    let mut challenge: GlobalChallenge = env
        .storage()
        .instance()
        .get(&RealtimeKey::GlobalChallenge(event_id))
        .ok_or(RealtimeError::EventNotFound)?;

    if challenge.completed {
        return Err(RealtimeError::ChallengeCompleted);
    }

    // Check if this is a new contributor
    let progress_key = RealtimeKey::ChallengeProgress(event_id, player.clone());
    let existing: u64 = env
        .storage()
        .instance()
        .get(&progress_key)
        .unwrap_or(0);

    if existing == 0 && amount > 0 {
        challenge.contributor_count += 1;
    }

    let new_value = existing.saturating_add(amount);
    env.storage().instance().set(&progress_key, &new_value);

    challenge.current_value = challenge.current_value.saturating_add(amount);
    if challenge.current_value >= challenge.target_value {
        challenge.completed = true;
    }

    env.storage()
        .instance()
        .set(&RealtimeKey::GlobalChallenge(event_id), &challenge);

    env.events().publish(
        (symbol_short!("rt"), symbol_short!("chal_prog")),
        (event_id, player, amount, challenge.current_value, challenge.completed),
    );

    Ok(challenge.current_value)
}

/// Get global challenge state.
pub fn get_global_challenge(
    env: &Env,
    event_id: u64,
) -> Result<GlobalChallenge, RealtimeError> {
    env.storage()
        .instance()
        .get(&RealtimeKey::GlobalChallenge(event_id))
        .ok_or(RealtimeError::EventNotFound)
}

// ─── Leaderboard ─────────────────────────────────────────────────────────────

/// Get the leaderboard for an event (sorted by contribution, top 10).
pub fn get_event_leaderboard(
    env: &Env,
    event_id: u64,
) -> Vec<EventLeaderboardEntry> {
    let participants: Vec<Address> = env
        .storage()
        .instance()
        .get(&RealtimeKey::Participants(event_id))
        .unwrap_or(Vec::new(env));

    let mut entries: Vec<EventLeaderboardEntry> = Vec::new(env);

    for i in 0..participants.len() {
        let player = participants.get(i).unwrap();
        let state_key = RealtimeKey::PlayerState(event_id, player.clone());
        if let Some(state) = env
            .storage()
            .instance()
            .get::<RealtimeKey, PlayerEventState>(&state_key)
        {
            entries.push_back(EventLeaderboardEntry {
                player: state.player,
                score: state.contribution,
                rank: 0,
            });
        }
    }

    // Simple bubble sort for top 10 (Soroban-friendly)
    let len = entries.len();
    let mut sorted = Vec::new(env);
    let mut used = Vec::new(env);

    for _ in 0..len.min(10) {
        let mut best_idx: u32 = 0;
        let mut best_score: u64 = 0;
        for j in 0..entries.len() {
            if used.get(j).unwrap_or(false) {
                continue;
            }
            let e = entries.get(j).unwrap();
            if e.score > best_score {
                best_score = e.score;
                best_idx = j;
            }
        }
        if best_score > 0 {
            let mut e = entries.get(best_idx).unwrap();
            e.rank = sorted.len() + 1;
            sorted.push_back(e);
            // Mark as used
            if used.len() <= best_idx {
                for _ in used.len()..=best_idx {
                    used.push_back(false);
                }
            }
            used.set(best_idx, true);
        }
    }

    sorted
}

// ─── Read Queries ─────────────────────────────────────────────────────────────

/// Get event details by ID.
pub fn get_event(env: &Env, event_id: u64) -> Result<RealtimeEvent, RealtimeError> {
    env.storage()
        .instance()
        .get(&RealtimeKey::Event(event_id))
        .ok_or(RealtimeError::EventNotFound)
}

/// Get a player's state in an event.
pub fn get_player_event_state(
    env: &Env,
    event_id: u64,
    player: &Address,
) -> Result<PlayerEventState, RealtimeError> {
    env.storage()
        .instance()
        .get(&RealtimeKey::PlayerState(event_id, player.clone()))
        .ok_or(RealtimeError::NotParticipating)
}

/// Get all active event IDs.
pub fn get_active_events(env: &Env) -> Vec<u64> {
    let counter: u64 = env
        .storage()
        .instance()
        .get(&RealtimeKey::EventCounter)
        .unwrap_or(0);

    let mut active = Vec::new(env);
    for i in 1..=counter {
        if let Some(event) = env
            .storage()
            .instance()
            .get::<RealtimeKey, RealtimeEvent>(&RealtimeKey::Event(i))
        {
            if event.status == EventStatus::Active {
                active.push_back(i);
            }
        }
    }
    active
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn make_env() -> Env {
        let env = Env::default();
        env.mock_all_auths();
        env
    }

    #[test]
    fn test_create_event() {
        let env = make_env();
        let admin = Address::generate(&env);
        initialize_realtime_events(&env, &admin).unwrap();

        let name = String::from_str(&env, "Raid Boss");
        let result = create_event(
            &env,
            admin,
            EventType::RaidBoss,
            name,
            100,
            200,
            100,
            10000,
        );

        assert!(result.is_ok());
        let event_id = result.unwrap();
        let event = get_event(&env, event_id).unwrap();
        assert_eq!(event.status, EventStatus::Setup);
    }

    #[test]
    fn test_join_and_contribute() {
        let env = make_env();
        let admin = Address::generate(&env);
        let player = Address::generate(&env);
        initialize_realtime_events(&env, &admin).unwrap();

        let name = String::from_str(&env, "Raid Boss");
        let event_id = create_event(
            &env,
            admin.clone(),
            EventType::RaidBoss,
            name,
            100,
            200,
            100,
            10000,
        )
        .unwrap();

        start_event(&env, admin, event_id).unwrap();
        join_event(&env, player.clone(), event_id).unwrap();

        let contribution = record_contribution(&env, player.clone(), event_id, 50).unwrap();
        assert_eq!(contribution, 50);

        let state = get_player_event_state(&env, event_id, &player).unwrap();
        assert_eq!(state.contribution, 50);
    }

    #[test]
    fn test_raid_boss_defeat() {
        let env = make_env();
        let admin = Address::generate(&env);
        let player = Address::generate(&env);
        initialize_realtime_events(&env, &admin).unwrap();

        let name = String::from_str(&env, "Raid Boss");
        let event_id = create_event(
            &env,
            admin.clone(),
            EventType::RaidBoss,
            name,
            100,
            200,
            100,
            10000,
        )
        .unwrap();

        start_event(&env, admin.clone(), event_id).unwrap();
        spawn_raid_boss(&env, admin, event_id, 1, 1, 1).unwrap();
        join_event(&env, player.clone(), event_id).unwrap();

        let remaining = deal_raid_damage(&env, player, event_id, 10).unwrap();
        assert_eq!(remaining, 0);

        let boss = get_raid_boss(&env, event_id).unwrap();
        assert!(boss.defeated);
    }
}
