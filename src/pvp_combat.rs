use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, BytesN, Env, Map, String, Symbol, Vec,
};

// ── Error ─────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PvPError {
    /// Player not found.
    PlayerNotFound = 1,
    /// Challenge already exists.
    ChallengeAlreadyExists = 2,
    /// Challenge not found.
    ChallengeNotFound = 3,
    /// Not authorized.
    Unauthorized = 4,
    /// Invalid combat parameters.
    InvalidCombatParams = 5,
    /// Player already in combat.
    AlreadyInCombat = 6,
    /// Combat not found.
    CombatNotFound = 7,
    /// Invalid move.
    InvalidMove = 8,
    /// ELO rating update failed.
    EloUpdateFailed = 9,
    /// Matchmaking queue full.
    QueueFull = 10,
    /// Player not in queue.
    NotInQueue = 11,
    /// Spectator limit reached.
    SpectatorLimitReached = 12,
    /// Admin has already been set; set_admin is a one-time initializer (Issue #237).
    AlreadyInitialized = 13,
}

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum PvPDataKey {
    /// Player combat stats.
    PlayerStats(Address),
    /// Active challenge by ID.
    Challenge(u64),
    /// Challenge counter.
    ChallengeCounter,
    /// Active combat by ID.
    Combat(u64),
    /// Combat counter.
    CombatCounter,
    /// Combat history by (player, index).
    CombatHistory(Address, u64),
    /// Combat history counter.
    CombatHistoryCounter(Address),
    /// Matchmaking queue.
    MatchmakingQueue,
    /// Player ELO rating.
    EloRating(Address),
    /// Spectators for combat.
    Spectators(u64),
    /// Combat rewards config.
    RewardsConfig,
    /// Admin address.
    Admin,
    /// Last active timestamp for a player (Issue #191).
    LastActive(Address),
    /// ELO decay configuration.
    EloDecayConfig,
}

// ── Data Types ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct CombatStats {
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub total_damage_dealt: u64,
    pub total_damage_received: u64,
    pub elo_rating: u32,
    pub combat_count: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Challenge {
    pub challenge_id: u64,
    pub challenger: Address,
    pub opponent: Address,
    pub stake: i128,
    pub status: Symbol, // "pending", "accepted", "declined", "expired"
    pub created_at: u64,
    pub expires_at: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct CombatState {
    pub combat_id: u64,
    pub player1: Address,
    pub player2: Address,
    pub player1_hp: u32,
    pub player2_hp: u32,
    pub player1_energy: u32,
    pub player2_energy: u32,
    pub turn: Address,
    pub status: Symbol, // "active", "finished", "cancelled"
    pub winner: Option<Address>,
    pub started_at: u64,
    pub history: Vec<Symbol>, // Combat log
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct CombatMove {
    pub move_type: Symbol, // "attack", "defend", "special", "heal"
    pub power: u32,
    pub energy_cost: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct CombatHistory {
    pub combat_id: u64,
    pub player1: Address,
    pub player2: Address,
    pub winner: Option<Address>,
    pub rewards: i128,
    pub played_at: u64,
    pub duration: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MatchmakingEntry {
    pub player: Address,
    pub elo_rating: u32,
    pub queued_at: u64,
    pub preferred_stake: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct RewardsConfig {
    pub base_reward: i128,
    pub win_bonus: i128,
    pub elo_bonus_multiplier: u32,
    pub streak_bonus: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct SpectatorInfo {
    pub spectators: Vec<Address>,
    pub max_spectators: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct EloDecayConfig {
    /// Seconds of inactivity before decay starts.
    pub inactivity_secs: u64,
    /// Points lost per decay event.
    pub decay_points: u32,
    /// Minimum ELO (floor).
    pub floor: u32,
}

// ── Constants ────────────────────────────────────────────────────────────────

pub const INITIAL_ELO: u32 = 1200;
pub const K_FACTOR: u32 = 32;
pub const MAX_HP: u32 = 100;
pub const MAX_ENERGY: u32 = 100;
pub const COMBAT_TIMEOUT: u64 = 3600; // 1 hour
pub const CHALLENGE_EXPIRY: u64 = 86400; // 24 hours
pub const MAX_SPECTATORS: u32 = 50;
pub const MAX_QUEUE_SIZE: u32 = 100;

/// Number of seconds of inactivity before ELO decay begins (7 days).
pub const ELO_DECAY_INACTIVITY_SECS: u64 = 604800;
/// Base points lost per decay tick.
pub const ELO_DECAY_BASE_POINTS: u32 = 10;
/// Minimum ELO rating (floor below which decay is not applied).
pub const ELO_DECAY_FLOOR: u32 = 1000;

// ── Admin Functions ──────────────────────────────────────────────────────────

/// Bootstrap the PvP combat admin. Callable exactly once — subsequent calls
/// return `AlreadyInitialized` instead of letting any caller overwrite the
/// admin (Issue #237: this previously let anyone re-appoint themselves).
pub fn set_admin(env: &Env, admin: &Address) -> Result<(), PvPError> {
    admin.require_auth();
    if get_admin(env).is_some() {
        return Err(PvPError::AlreadyInitialized);
    }
    env.storage().persistent().set(&PvPDataKey::Admin, admin);
    env.events().publish(
        (symbol_short!("pvp"), symbol_short!("admin_set")),
        (Option::<Address>::None, admin.clone()),
    );
    Ok(())
}

fn get_admin(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&PvPDataKey::Admin)
}

fn require_admin(env: &Env, caller: &Address) -> Result<(), PvPError> {
    caller.require_auth();
    let admin = get_admin(env).ok_or(PvPError::Unauthorized)?;
    if *caller != admin {
        return Err(PvPError::Unauthorized);
    }
    Ok(())
}

// ── Combat Stats ────────────────────────────────────────────────────────────

pub fn get_or_init_stats(env: &Env, player: &Address) -> CombatStats {
    let key = PvPDataKey::PlayerStats(player.clone());
    let stats = env.storage().persistent().get(&key).unwrap_or(CombatStats {
        wins: 0,
        losses: 0,
        draws: 0,
        total_damage_dealt: 0,
        total_damage_received: 0,
        elo_rating: INITIAL_ELO,
        combat_count: 0,
    });

    // Apply ELO decay on any access
    apply_elo_decay(env, player);
    update_last_active(env, player);

    // Re-read after decay may have modified ELO
    env.storage().persistent().get(&key).unwrap_or(stats)
}

pub fn update_stats(env: &Env, player: &Address, stats: &CombatStats) {
    let key = PvPDataKey::PlayerStats(player.clone());
    env.storage().persistent().set(&key, stats);
    env.events().publish(
        (symbol_short!("pvp"), symbol_short!("stats_upd")),
        (player.clone(), stats.wins, stats.losses, stats.draws),
    );
}

pub fn get_combat_stats(env: &Env, player: &Address) -> CombatStats {
    get_or_init_stats(env, player)
}

// ── ELO Rating ─────────────────────────────────────────────────────────────

pub fn get_elo_rating(env: &Env, player: &Address) -> u32 {
    let key = PvPDataKey::EloRating(player.clone());
    env.storage().persistent().get(&key).unwrap_or(INITIAL_ELO)
}

pub fn update_elo_rating(env: &Env, player: &Address, new_rating: u32) {
    let key = PvPDataKey::EloRating(player.clone());
    let old_elo = env.storage().persistent().get(&key).unwrap_or(INITIAL_ELO);
    env.storage().persistent().set(&key, &new_rating);
    env.events().publish(
        (symbol_short!("pvp"), symbol_short!("elo_upd")),
        (player.clone(), old_elo, new_rating),
    );
}

fn calculate_elo_change(winner_rating: u32, loser_rating: u32) -> (u32, u32) {
    let expected_winner =
        1.0 / (1.0 + 10.0_f64.powf((loser_rating as f64 - winner_rating as f64) / 400.0));
    let expected_loser =
        1.0 / (1.0 + 10.0_f64.powf((winner_rating as f64 - loser_rating as f64) / 400.0));

    let winner_change = (K_FACTOR as f64 * (1.0 - expected_winner)).round() as u32;
    let loser_change = (K_FACTOR as f64 * (0.0 - expected_loser)).round().abs() as u32;

    (winner_change, loser_change)
}

// ── ELO Decay (Issue #191) ─────────────────────────────────────────────────

/// Track the last active timestamp for a player. Called on any PvP action.
pub fn update_last_active(env: &Env, player: &Address) {
    let key = PvPDataKey::LastActive(player.clone());
    let timestamp = env.ledger().timestamp();
    env.storage().persistent().set(&key, &timestamp);
    env.events().publish(
        (symbol_short!("pvp"), symbol_short!("last_act")),
        (player.clone(), timestamp),
    );
}

/// Get the last active timestamp for a player.
pub fn get_last_active(env: &Env, player: &Address) -> u64 {
    let key = PvPDataKey::LastActive(player.clone());
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Set the ELO decay configuration (admin only).
pub fn set_elo_decay_config(
    env: &Env,
    caller: &Address,
    config: EloDecayConfig,
) -> Result<(), PvPError> {
    require_admin(env, caller)?;
    env.storage()
        .persistent()
        .set(&PvPDataKey::EloDecayConfig, &config);
    env.events().publish(
        (symbol_short!("pvp"), symbol_short!("elo_cfg")),
        (
            caller.clone(),
            config.inactivity_secs,
            config.decay_points,
            config.floor,
        ),
    );
    Ok(())
}

/// Get the current ELO decay configuration.
pub fn get_elo_decay_config(env: &Env) -> EloDecayConfig {
    env.storage()
        .persistent()
        .get(&PvPDataKey::EloDecayConfig)
        .unwrap_or(EloDecayConfig {
            inactivity_secs: ELO_DECAY_INACTIVITY_SECS,
            decay_points: ELO_DECAY_BASE_POINTS,
            floor: ELO_DECAY_FLOOR,
        })
}

/// Apply ELO decay for a player based on their last active timestamp.
/// Returns the new ELO rating after decay (or the same if no decay applied).
pub fn apply_elo_decay(env: &Env, player: &Address) -> u32 {
    let now = env.ledger().timestamp();
    let last_active = get_last_active(env, player);

    if last_active == 0 {
        // Never been active — no decay
        return get_elo_rating(env, player);
    }

    let inactive_duration = now.saturating_sub(last_active);
    let config = get_elo_decay_config(env);

    if inactive_duration < config.inactivity_secs {
        return get_elo_rating(env, player);
    }

    // Calculate how many full decay periods have elapsed
    let periods = inactive_duration / config.inactivity_secs;
    let total_decay = (config.decay_points as u64).saturating_mul(periods as u64) as u32;

    let current_elo = get_elo_rating(env, player);
    let new_elo = current_elo.saturating_sub(total_decay).max(config.floor);

    if new_elo != current_elo {
        update_elo_rating(env, player, new_elo);
        // Reset the decay timer
        update_last_active(env, player);

        env.events().publish(
            (symbol_short!("pvp"), symbol_short!("elo_decay")),
            (player.clone(), current_elo, new_elo),
        );
    }

    new_elo
}

/// Apply decay to all players in a batch (admin / cron call).
/// Returns the number of players whose ELO was affected.
pub fn apply_batch_elo_decay(
    env: &Env,
    caller: &Address,
    players: Vec<Address>,
) -> Result<u32, PvPError> {
    require_admin(env, caller)?;

    let mut affected = 0u32;
    for i in 0..players.len() {
        if let Some(player) = players.get(i) {
            let before = get_elo_rating(env, &player);
            let after = apply_elo_decay(env, &player);
            if after != before {
                affected += 1;
            }
        }
    }

    Ok(affected)
}

// ── Challenge System ────────────────────────────────────────────────────────

pub fn create_challenge(
    env: &Env,
    challenger: &Address,
    opponent: &Address,
    stake: i128,
) -> Result<u64, PvPError> {
    challenger.require_auth();

    // Check if opponent exists (has stats)
    let _ = get_or_init_stats(env, opponent);

    // Track last active
    update_last_active(env, challenger);

    // Generate challenge ID
    let counter: u64 = env
        .storage()
        .persistent()
        .get(&PvPDataKey::ChallengeCounter)
        .unwrap_or(0);
    let challenge_id = counter + 1;
    env.storage()
        .persistent()
        .set(&PvPDataKey::ChallengeCounter, &challenge_id);

    let now = env.ledger().timestamp();
    let challenge = Challenge {
        challenge_id,
        challenger: challenger.clone(),
        opponent: opponent.clone(),
        stake,
        status: symbol_short!("pending"),
        created_at: now,
        expires_at: now + CHALLENGE_EXPIRY,
    };

    env.storage()
        .persistent()
        .set(&PvPDataKey::Challenge(challenge_id), &challenge);

    env.events().publish(
        (symbol_short!("pvp"), symbol_short!("chall")),
        (challenger.clone(), opponent.clone(), challenge_id),
    );

    Ok(challenge_id)
}

pub fn accept_challenge(env: &Env, caller: &Address, challenge_id: u64) -> Result<u64, PvPError> {
    caller.require_auth();

    let key = PvPDataKey::Challenge(challenge_id);
    let mut challenge: Challenge = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(PvPError::ChallengeNotFound)?;

    if challenge.opponent != *caller {
        return Err(PvPError::Unauthorized);
    }

    if challenge.status != symbol_short!("pending") {
        return Err(PvPError::InvalidCombatParams);
    }

    // Check expiry
    if env.ledger().timestamp() > challenge.expires_at {
        challenge.status = symbol_short!("expired");
        env.storage().persistent().set(&key, &challenge);
        return Err(PvPError::ChallengeNotFound);
    }

    challenge.status = symbol_short!("accepted");
    env.storage().persistent().set(&key, &challenge);

    // Track last active
    update_last_active(env, caller);
    update_last_active(env, &challenge.challenger);

    // Start combat
    start_combat(env, &challenge.challenger, caller, challenge_id)
}

pub fn decline_challenge(env: &Env, caller: &Address, challenge_id: u64) -> Result<(), PvPError> {
    caller.require_auth();

    let key = PvPDataKey::Challenge(challenge_id);
    let mut challenge: Challenge = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(PvPError::ChallengeNotFound)?;

    if challenge.opponent != *caller && challenge.challenger != *caller {
        return Err(PvPError::Unauthorized);
    }

    challenge.status = symbol_short!("declined");
    env.storage().persistent().set(&key, &challenge);

    env.events().publish(
        (symbol_short!("pvp"), symbol_short!("decline")),
        (caller.clone(), challenge_id),
    );

    Ok(())
}

pub fn get_challenge(env: &Env, challenge_id: u64) -> Result<Challenge, PvPError> {
    let key = PvPDataKey::Challenge(challenge_id);
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(PvPError::ChallengeNotFound)
}

// ── Combat Engine ──────────────────────────────────────────────────────────

pub fn start_combat(
    env: &Env,
    player1: &Address,
    player2: &Address,
    challenge_id: u64,
) -> Result<u64, PvPError> {
    // Generate combat ID
    let counter: u64 = env
        .storage()
        .persistent()
        .get(&PvPDataKey::CombatCounter)
        .unwrap_or(0);
    let combat_id = counter + 1;
    env.storage()
        .persistent()
        .set(&PvPDataKey::CombatCounter, &combat_id);

    // Determine first turn randomly
    let now = env.ledger().timestamp();
    let seed = now % 2;
    let turn = if seed == 0 {
        player1.clone()
    } else {
        player2.clone()
    };

    let combat = CombatState {
        combat_id,
        player1: player1.clone(),
        player2: player2.clone(),
        player1_hp: MAX_HP,
        player2_hp: MAX_HP,
        player1_energy: MAX_ENERGY,
        player2_energy: MAX_ENERGY,
        turn,
        status: symbol_short!("active"),
        winner: None,
        started_at: now,
        history: Vec::new(env),
    };

    env.storage()
        .persistent()
        .set(&PvPDataKey::Combat(combat_id), &combat);

    // Update stats
    let mut stats1 = get_or_init_stats(env, player1);
    stats1.combat_count += 1;
    update_stats(env, player1, &stats1);

    let mut stats2 = get_or_init_stats(env, player2);
    stats2.combat_count += 1;
    update_stats(env, player2, &stats2);

    env.events().publish(
        (symbol_short!("pvp"), symbol_short!("start")),
        (combat_id, player1.clone(), player2.clone(), challenge_id),
    );

    Ok(combat_id)
}

pub fn execute_move(
    env: &Env,
    player: &Address,
    combat_id: u64,
    move_type: Symbol,
    power: u32,
) -> Result<(), PvPError> {
    player.require_auth();

    let key = PvPDataKey::Combat(combat_id);
    let mut combat: CombatState = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(PvPError::CombatNotFound)?;

    if combat.status != symbol_short!("active") {
        return Err(PvPError::InvalidCombatParams);
    }

    // Verify it's player's turn
    if combat.turn != *player {
        return Err(PvPError::InvalidMove);
    }

    // Track last active
    update_last_active(env, player);

    // Check timeout
    if env.ledger().timestamp() > combat.started_at + COMBAT_TIMEOUT {
        combat.status = symbol_short!("cancelled");
        env.storage().persistent().set(&key, &combat);
        return Err(PvPError::CombatNotFound);
    }

    let energy_cost = power / 2;
    let is_player1 = *player == combat.player1;

    // Verify energy
    if is_player1 {
        if combat.player1_energy < energy_cost {
            return Err(PvPError::InvalidMove);
        }
        combat.player1_energy -= energy_cost;
    } else {
        if combat.player2_energy < energy_cost {
            return Err(PvPError::InvalidMove);
        }
        combat.player2_energy -= energy_cost;
    }

    // Process move
    if move_type == symbol_short!("attack") {
        let damage = power;
        if is_player1 {
            combat.player2_hp = combat.player2_hp.saturating_sub(damage);
        } else {
            combat.player1_hp = combat.player1_hp.saturating_sub(damage);
        }

        // Log
        let log_entry = if is_player1 {
            Symbol::new(env, "p1_attack")
        } else {
            Symbol::new(env, "p2_attack")
        };
        combat.history.push_back(log_entry);
    } else if move_type == symbol_short!("defend") {
        // Defend restores some energy
        if is_player1 {
            combat.player1_energy = combat.player1_energy.saturating_add(10);
        } else {
            combat.player2_energy = combat.player2_energy.saturating_add(10);
        }

        let log_entry = if is_player1 {
            Symbol::new(env, "p1_defend")
        } else {
            Symbol::new(env, "p2_defend")
        };
        combat.history.push_back(log_entry);
    } else if move_type == symbol_short!("special") {
        let damage = power * 2;
        if is_player1 {
            combat.player2_hp = combat.player2_hp.saturating_sub(damage);
        } else {
            combat.player1_hp = combat.player1_hp.saturating_sub(damage);
        }

        let log_entry = if is_player1 {
            Symbol::new(env, "p1_special")
        } else {
            Symbol::new(env, "p2_special")
        };
        combat.history.push_back(log_entry);
    } else if move_type == symbol_short!("heal") {
        let heal = power / 2;
        if is_player1 {
            combat.player1_hp = (combat.player1_hp + heal).min(MAX_HP);
        } else {
            combat.player2_hp = (combat.player2_hp + heal).min(MAX_HP);
        }

        let log_entry = if is_player1 {
            Symbol::new(env, "p1_heal")
        } else {
            Symbol::new(env, "p2_heal")
        };
        combat.history.push_back(log_entry);
    } else {
        return Err(PvPError::InvalidMove);
    }

    // Check for winner
    if combat.player1_hp == 0 {
        combat.winner = Some(combat.player2.clone());
        combat.status = symbol_short!("finished");
        end_combat(env, &mut combat)?;
    } else if combat.player2_hp == 0 {
        combat.winner = Some(combat.player1.clone());
        combat.status = symbol_short!("finished");
        end_combat(env, &mut combat)?;
    } else {
        // Switch turn
        combat.turn = if is_player1 {
            combat.player2.clone()
        } else {
            combat.player1.clone()
        };
    }

    env.storage().persistent().set(&key, &combat);

    Ok(())
}

fn end_combat(env: &Env, combat: &mut CombatState) -> Result<(), PvPError> {
    let now = env.ledger().timestamp();
    let duration = now - combat.started_at;

    // Update stats
    let mut stats1 = get_or_init_stats(env, &combat.player1);
    let mut stats2 = get_or_init_stats(env, &combat.player2);

    let winner = combat.winner.clone();

    if let Some(ref w) = winner {
        if *w == combat.player1 {
            stats1.wins += 1;
            stats2.losses += 1;
        } else {
            stats2.wins += 1;
            stats1.losses += 1;
        }

        // Update ELO
        let elo1 = get_elo_rating(env, &combat.player1);
        let elo2 = get_elo_rating(env, &combat.player2);

        let (change1, change2) = calculate_elo_change(elo1, elo2);

        if *w == combat.player1 {
            update_elo_rating(env, &combat.player1, elo1.saturating_add(change1));
            update_elo_rating(env, &combat.player2, elo2.saturating_sub(change2.min(elo2)));
        } else {
            update_elo_rating(env, &combat.player2, elo2.saturating_add(change1));
            update_elo_rating(env, &combat.player1, elo1.saturating_sub(change2.min(elo1)));
        }
    } else {
        // Draw
        stats1.draws += 1;
        stats2.draws += 1;
    }

    update_stats(env, &combat.player1, &stats1);
    update_stats(env, &combat.player2, &stats2);

    // Record history
    let history = CombatHistory {
        combat_id: combat.combat_id,
        player1: combat.player1.clone(),
        player2: combat.player2.clone(),
        winner: winner.clone(),
        rewards: 0, // TODO: implement rewards
        played_at: combat.started_at,
        duration,
    };

    record_combat_history(env, &combat.player1, &history);
    record_combat_history(env, &combat.player2, &history);

    env.events().publish(
        (symbol_short!("pvp"), symbol_short!("end")),
        (combat.combat_id, winner, duration),
    );

    Ok(())
}

fn record_combat_history(env: &Env, player: &Address, history: &CombatHistory) {
    let counter_key = PvPDataKey::CombatHistoryCounter(player.clone());
    let counter: u64 = env.storage().persistent().get(&counter_key).unwrap_or(0);

    let history_key = PvPDataKey::CombatHistory(player.clone(), counter);
    env.storage().persistent().set(&history_key, history);

    env.storage().persistent().set(&counter_key, &(counter + 1));
}

pub fn get_combat(env: &Env, combat_id: u64) -> Result<CombatState, PvPError> {
    let key = PvPDataKey::Combat(combat_id);
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(PvPError::CombatNotFound)
}

pub fn get_player_combat_history(env: &Env, player: &Address, limit: u32) -> Vec<CombatHistory> {
    let counter_key = PvPDataKey::CombatHistoryCounter(player.clone());
    let counter: u64 = env.storage().persistent().get(&counter_key).unwrap_or(0);

    let mut history = Vec::new(env);
    let start = if counter > limit as u64 {
        counter - limit as u64
    } else {
        0
    };

    for i in start..counter {
        let key = PvPDataKey::CombatHistory(player.clone(), i);
        if let Some(h) = env.storage().persistent().get::<_, CombatHistory>(&key) {
            history.push_back(h);
        }
    }

    history
}

// ── Matchmaking ────────────────────────────────────────────────────────────

pub fn join_matchmaking(
    env: &Env,
    player: &Address,
    preferred_stake: i128,
) -> Result<(), PvPError> {
    player.require_auth();

    // Track last active
    update_last_active(env, player);

    let mut queue: Vec<MatchmakingEntry> = env
        .storage()
        .persistent()
        .get(&PvPDataKey::MatchmakingQueue)
        .unwrap_or_else(|| Vec::new(env));

    if queue.len() >= MAX_QUEUE_SIZE {
        return Err(PvPError::QueueFull);
    }

    // Check if already in queue
    for i in 0..queue.len() {
        if let Some(entry) = queue.get(i) {
            if entry.player == *player {
                return Err(PvPError::AlreadyInCombat);
            }
        }
    }

    let elo = get_elo_rating(env, player);
    let entry = MatchmakingEntry {
        player: player.clone(),
        elo_rating: elo,
        queued_at: env.ledger().timestamp(),
        preferred_stake,
    };

    queue.push_back(entry);
    env.storage()
        .persistent()
        .set(&PvPDataKey::MatchmakingQueue, &queue);

    env.events().publish(
        (symbol_short!("pvp"), symbol_short!("queue")),
        (player.clone(), elo),
    );

    Ok(())
}

pub fn leave_matchmaking(env: &Env, player: &Address) -> Result<(), PvPError> {
    player.require_auth();

    let key = PvPDataKey::MatchmakingQueue;
    let mut queue: Vec<MatchmakingEntry> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));

    let mut found = false;
    let mut new_queue = Vec::new(env);
    for i in 0..queue.len() {
        if let Some(entry) = queue.get(i) {
            if entry.player != *player {
                new_queue.push_back(entry);
            } else {
                found = true;
            }
        }
    }

    if !found {
        return Err(PvPError::NotInQueue);
    }

    env.storage().persistent().set(&key, &new_queue);

    env.events().publish(
        (symbol_short!("pvp"), symbol_short!("mq_leave")),
        (player.clone(),),
    );

    Ok(())
}

pub fn process_matchmaking(env: &Env) -> Result<Option<(Address, Address)>, PvPError> {
    let key = PvPDataKey::MatchmakingQueue;
    let mut queue: Vec<MatchmakingEntry> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));

    if queue.len() < 2 {
        return Ok(None);
    }

    // Simple matchmaking: match first two players
    let entry1 = queue.get(0).ok_or(PvPError::NotInQueue)?;
    let entry2 = queue.get(1).ok_or(PvPError::NotInQueue)?;

    let player1 = entry1.player.clone();
    let player2 = entry2.player.clone();

    // Remove them from queue
    let mut new_queue = Vec::new(env);
    for i in 2..queue.len() {
        if let Some(entry) = queue.get(i) {
            new_queue.push_back(entry);
        }
    }
    env.storage().persistent().set(&key, &new_queue);

    Ok(Some((player1, player2)))
}

pub fn get_matchmaking_queue_size(env: &Env) -> u32 {
    let key = PvPDataKey::MatchmakingQueue;
    let queue: Vec<MatchmakingEntry> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    queue.len()
}

// ── Spectator Mode ────────────────────────────────────────────────────────

pub fn add_spectator(env: &Env, spectator: &Address, combat_id: u64) -> Result<(), PvPError> {
    spectator.require_auth();

    let combat_key = PvPDataKey::Combat(combat_id);
    let combat: CombatState = env
        .storage()
        .persistent()
        .get(&combat_key)
        .ok_or(PvPError::CombatNotFound)?;

    if combat.status != symbol_short!("active") {
        return Err(PvPError::InvalidCombatParams);
    }

    let spec_key = PvPDataKey::Spectators(combat_id);
    let mut info: SpectatorInfo =
        env.storage()
            .persistent()
            .get(&spec_key)
            .unwrap_or(SpectatorInfo {
                spectators: Vec::new(env),
                max_spectators: MAX_SPECTATORS,
            });

    if info.spectators.len() >= info.max_spectators {
        return Err(PvPError::SpectatorLimitReached);
    }

    // Check if already spectating
    for i in 0..info.spectators.len() {
        if let Some(s) = info.spectators.get(i) {
            if s == *spectator {
                return Ok(()); // Already spectating
            }
        }
    }

    info.spectators.push_back(spectator.clone());
    env.storage().persistent().set(&spec_key, &info);

    env.events().publish(
        (symbol_short!("pvp"), symbol_short!("spectate")),
        (spectator.clone(), combat_id),
    );

    Ok(())
}

pub fn remove_spectator(env: &Env, spectator: &Address, combat_id: u64) -> Result<(), PvPError> {
    spectator.require_auth();

    let spec_key = PvPDataKey::Spectators(combat_id);
    let mut info: SpectatorInfo = env
        .storage()
        .persistent()
        .get(&spec_key)
        .ok_or(PvPError::CombatNotFound)?;

    let mut new_spectators = Vec::new(env);
    for i in 0..info.spectators.len() {
        if let Some(s) = info.spectators.get(i) {
            if s != *spectator {
                new_spectators.push_back(s);
            }
        }
    }

    info.spectators = new_spectators;
    env.storage().persistent().set(&spec_key, &info);

    env.events().publish(
        (symbol_short!("pvp"), symbol_short!("spec_rm")),
        (spectator.clone(), combat_id),
    );

    Ok(())
}

pub fn get_spectators(env: &Env, combat_id: u64) -> Vec<Address> {
    let spec_key = PvPDataKey::Spectators(combat_id);
    let info: SpectatorInfo = env
        .storage()
        .persistent()
        .get(&spec_key)
        .unwrap_or(SpectatorInfo {
            spectators: Vec::new(env),
            max_spectators: MAX_SPECTATORS,
        });
    info.spectators
}

// ── Combat Rewards ────────────────────────────────────────────────────────

pub fn set_rewards_config(
    env: &Env,
    caller: &Address,
    config: RewardsConfig,
) -> Result<(), PvPError> {
    require_admin(env, caller)?;

    env.storage()
        .persistent()
        .set(&PvPDataKey::RewardsConfig, &config);

    Ok(())
}

pub fn get_rewards_config(env: &Env) -> RewardsConfig {
    env.storage()
        .persistent()
        .get(&PvPDataKey::RewardsConfig)
        .unwrap_or(RewardsConfig {
            base_reward: 100,
            win_bonus: 50,
            elo_bonus_multiplier: 1,
            streak_bonus: 10,
        })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env, Symbol};

    use soroban_sdk::{contract, contractimpl};
    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    fn make_env() -> (Env, soroban_sdk::Address) {
        let env = Env::default();
        let id = env.register_contract(None, Stub);
        (env, id)
    }

    #[test]
    fn test_create_challenge() {
        let (env, _contract_id) = make_env();
        let challenger = Address::generate(&env);
        let opponent = Address::generate(&env);

        env.as_contract(&_contract_id, || {
            let challenge_id = create_challenge(&env, &challenger, &opponent, 100).unwrap();
            assert!(challenge_id > 0);

            let challenge = get_challenge(&env, challenge_id).unwrap();
            assert_eq!(challenge.challenger, challenger);
        });
    }

    #[test]
    fn test_accept_challenge_and_combat() {
        let (env, _contract_id) = make_env();
        let challenger = Address::generate(&env);
        let opponent = Address::generate(&env);

        env.as_contract(&_contract_id, || {
            let challenge_id = create_challenge(&env, &challenger, &opponent, 100).unwrap();
            let combat_id = accept_challenge(&env, &opponent, challenge_id).unwrap();

            let combat = get_combat(&env, combat_id).unwrap();
            assert_eq!(combat.status, symbol_short!("active"));
        });
    }

    #[test]
    fn test_execute_move() {
        let (env, _contract_id) = make_env();
        let challenger = Address::generate(&env);
        let opponent = Address::generate(&env);

        env.as_contract(&_contract_id, || {
            let challenge_id = create_challenge(&env, &challenger, &opponent, 100).unwrap();
            let combat_id = accept_challenge(&env, &opponent, challenge_id).unwrap();

            let combat = get_combat(&env, combat_id).unwrap();
            let first_player = combat.turn.clone();

            execute_move(
                &env,
                &first_player,
                combat_id,
                Symbol::new(&env, "attack"),
                10,
            )
            .unwrap();

            let updated = get_combat(&env, combat_id).unwrap();
            assert!(updated.player2_hp < 100 || updated.player1_hp < 100);
        });
    }

    #[test]
    fn test_matchmaking() {
        let (env, _contract_id) = make_env();
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);

        env.as_contract(&_contract_id, || {
            join_matchmaking(&env, &player1, 100).unwrap();
            join_matchmaking(&env, &player2, 100).unwrap();

            let queue_size = get_matchmaking_queue_size(&env);
            assert_eq!(queue_size, 2);

            let matched = process_matchmaking(&env).unwrap();
            assert!(matched.is_some());
        });
    }

    #[test]
    fn test_set_admin_cannot_be_hijacked_after_init() {
        // Issue #237: set_admin previously let ANY caller overwrite the
        // admin at any time. Now it is a one-time initializer.
        let (env, _contract_id) = make_env();
        let admin = Address::generate(&env);
        let attacker = Address::generate(&env);

        env.as_contract(&_contract_id, || {
            set_admin(&env, &admin).unwrap();

            let result = set_admin(&env, &attacker);
            assert_eq!(result, Err(PvPError::AlreadyInitialized));

            let result = set_rewards_config(
                &env,
                &attacker,
                RewardsConfig {
                    base_reward: 999,
                    win_bonus: 999,
                    elo_bonus_multiplier: 1,
                    streak_bonus: 0,
                },
            );
            assert_eq!(result, Err(PvPError::Unauthorized));
        });
    }
}
