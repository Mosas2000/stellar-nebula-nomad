use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MiniGameError {
    GameNotFound = 1,
    GameFull = 2,
    NotActive = 3,
    AlreadyPlayed = 4,
    CooldownActive = 5,
    NotEnoughResources = 6,
    DailyLimitReached = 7,
    InvalidMove = 8,
    LeaderboardFull = 9,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum MiniGameType {
    MemoryChain,
    ResourceRush,
    AnomalyArena,
    TradingBlitz,
    NebulaPuzzle,
    SpeedHarvest,
    CosmicQuiz,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct MiniGameConfig {
    pub game_type: MiniGameType,
    pub name: Symbol,
    pub description: Symbol,
    pub entry_fee: i128,
    pub max_players: u32,
    pub cooldown_secs: u64,
    pub daily_limit: u32,
    pub base_reward: i128,
    pub time_limit_secs: u64,
    pub difficulty_multiplier: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct MiniGameSession {
    pub session_id: u64,
    pub game_type: MiniGameType,
    pub players: Vec<Address>,
    pub scores: Vec<u32>,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub state: MiniGameState,
    pub rewards: Vec<i128>,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum MiniGameState {
    Waiting,
    InProgress,
    Completed,
    Expired,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct LeaderboardEntry {
    pub player: Address,
    pub score: u32,
    pub game_type: MiniGameType,
    pub achieved_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct DailyChallenge {
    pub challenge_id: u64,
    pub game_type: MiniGameType,
    pub target_score: u32,
    pub reward: i128,
    pub bonus_reward: i128,
    pub active_date: u64,
    pub completed_players: Vec<Address>,
    pub expires_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerMiniGameStats {
    pub games_played: u32,
    pub total_score: u64,
    pub best_score: u32,
    pub total_rewards: i128,
    pub games_won: u32,
    pub daily_plays: u32,
    pub last_play_time: u64,
}

#[contracttype]
#[derive(Clone)]
pub enum MiniGameKey {
    Session(u64),
    SessionCount,
    PlayerStats(Address),
    DailyChallenge(u64),
    ActiveChallenge,
    Leaderboard(MiniGameType, u64),
    DailyPlays(Address),
    GameConfig(MiniGameType),
    PlayerGames(Address, u64),
}

const LEADERBOARD_SIZE: u64 = 100;
const SECONDS_IN_DAY: u64 = 86400;

pub fn get_default_game_configs(env: &Env) -> Vec<MiniGameConfig> {
    let mut configs = Vec::new(env);
    configs.push_back(MiniGameConfig {
        game_type: MiniGameType::MemoryChain,
        name: symbol_short!("m_chain"),
        description: symbol_short!("desc_mc"),
        entry_fee: 50,
        max_players: 4,
        cooldown_secs: 300,
        daily_limit: 10,
        base_reward: 200,
        time_limit_secs: 120,
        difficulty_multiplier: 100,
    });
    configs.push_back(MiniGameConfig {
        game_type: MiniGameType::ResourceRush,
        name: symbol_short!("r_rush"),
        description: symbol_short!("desc_rr"),
        entry_fee: 100,
        max_players: 2,
        cooldown_secs: 600,
        daily_limit: 5,
        base_reward: 500,
        time_limit_secs: 180,
        difficulty_multiplier: 150,
    });
    configs.push_back(MiniGameConfig {
        game_type: MiniGameType::AnomalyArena,
        name: symbol_short!("a_arena"),
        description: symbol_short!("desc_aa"),
        entry_fee: 200,
        max_players: 6,
        cooldown_secs: 900,
        daily_limit: 3,
        base_reward: 1000,
        time_limit_secs: 300,
        difficulty_multiplier: 200,
    });
    configs.push_back(MiniGameConfig {
        game_type: MiniGameType::TradingBlitz,
        name: symbol_short!("t_blitz"),
        description: symbol_short!("desc_tb"),
        entry_fee: 150,
        max_players: 8,
        cooldown_secs: 450,
        daily_limit: 8,
        base_reward: 750,
        time_limit_secs: 240,
        difficulty_multiplier: 125,
    });
    configs.push_back(MiniGameConfig {
        game_type: MiniGameType::NebulaPuzzle,
        name: symbol_short!("n_puzzle"),
        description: symbol_short!("desc_np"),
        entry_fee: 75,
        max_players: 1,
        cooldown_secs: 120,
        daily_limit: 15,
        base_reward: 300,
        time_limit_secs: 90,
        difficulty_multiplier: 80,
    });
    configs.push_back(MiniGameConfig {
        game_type: MiniGameType::SpeedHarvest,
        name: symbol_short!("s_harvest"),
        description: symbol_short!("desc_sh"),
        entry_fee: 125,
        max_players: 4,
        cooldown_secs: 750,
        daily_limit: 6,
        base_reward: 600,
        time_limit_secs: 200,
        difficulty_multiplier: 175,
    });
    configs.push_back(MiniGameConfig {
        game_type: MiniGameType::CosmicQuiz,
        name: symbol_short!("c_quiz"),
        description: symbol_short!("desc_cq"),
        entry_fee: 25,
        max_players: 10,
        cooldown_secs: 60,
        daily_limit: 20,
        base_reward: 100,
        time_limit_secs: 60,
        difficulty_multiplier: 50,
    });
    configs
}

pub fn start_mini_game(
    env: &Env,
    player: Address,
    game_type: MiniGameType,
) -> Result<u64, MiniGameError> {
    player.require_auth();

    let config = get_game_config(env, game_type.clone());
    let stats = get_player_stats(env, player.clone());

    if stats.daily_plays >= config.daily_limit {
        return Err(MiniGameError::DailyLimitReached);
    }

    let now = env.ledger().timestamp();
    if now - stats.last_play_time < config.cooldown_secs {
        return Err(MiniGameError::CooldownActive);
    }

    let session_id: u64 = env.storage().persistent()
        .get(&MiniGameKey::SessionCount).unwrap_or(0) + 1;

    let mut players = Vec::new(env);
    players.push_back(player.clone());

    let session = MiniGameSession {
        session_id,
        game_type: game_type.clone(),
        players,
        scores: Vec::new(env),
        started_at: now,
        ended_at: None,
        state: MiniGameState::InProgress,
        rewards: Vec::new(env),
    };

    env.storage().persistent().set(&MiniGameKey::Session(session_id), &session);
    env.storage().persistent().set(&MiniGameKey::SessionCount, &session_id);

    let mut updated_stats = stats;
    updated_stats.games_played = updated_stats.games_played.saturating_add(1);
    updated_stats.last_play_time = now;
    updated_stats.daily_plays = updated_stats.daily_plays.saturating_add(1);
    env.storage().persistent().set(&MiniGameKey::PlayerStats(player.clone()), &updated_stats);
    env.storage().persistent().set(&MiniGameKey::DailyPlays(player.clone()), &updated_stats.daily_plays);

    env.events().publish(
        (symbol_short!("minigame"), symbol_short!("started")),
        (session_id, game_type, player, now),
    );

    Ok(session_id)
}

pub fn submit_mini_game_score(
    env: &Env,
    player: Address,
    session_id: u64,
    score: u32,
) -> Result<i128, MiniGameError> {
    player.require_auth();

    let mut session: MiniGameSession = env.storage().persistent()
        .get(&MiniGameKey::Session(session_id))
        .ok_or(MiniGameError::GameNotFound)?;

    if session.state != MiniGameState::InProgress {
        return Err(MiniGameError::NotActive);
    }

    let config = get_game_config(env, session.game_type.clone());
    let now = env.ledger().timestamp();

    if now - session.started_at > config.time_limit_secs {
        session.state = MiniGameState::Expired;
        env.storage().persistent().set(&MiniGameKey::Session(session_id), &session);
        return Err(MiniGameError::NotActive);
    }

    let weighted_score = score * config.difficulty_multiplier / 100;
    session.scores.push_back(weighted_score);
    session.state = MiniGameState::Completed;
    session.ended_at = Some(now);

    let reward = config.base_reward * (weighted_score as i128) / 1000;
    session.rewards.push_back(reward);

    env.storage().persistent().set(&MiniGameKey::Session(session_id), &session);

    let mut stats = get_player_stats(env, player.clone());
    stats.total_score = stats.total_score.saturating_add(weighted_score as u64);
    if weighted_score > stats.best_score {
        stats.best_score = weighted_score;
    }
    stats.games_won = stats.games_won.saturating_add(1);
    stats.total_rewards = stats.total_rewards.saturating_add(reward);
    env.storage().persistent().set(&MiniGameKey::PlayerStats(player.clone()), &stats);

    update_leaderboard(env, player.clone(), session.game_type.clone(), weighted_score);

    env.events().publish(
        (symbol_short!("minigame"), symbol_short!("completed")),
        (session_id, player.clone(), weighted_score, reward),
    );

    Ok(reward)
}

pub fn join_multiplayer_game(
    env: &Env,
    player: Address,
    session_id: u64,
) -> Result<(), MiniGameError> {
    player.require_auth();

    let mut session: MiniGameSession = env.storage().persistent()
        .get(&MiniGameKey::Session(session_id))
        .ok_or(MiniGameError::GameNotFound)?;

    if session.state != MiniGameState::Waiting && session.state != MiniGameState::InProgress {
        return Err(MiniGameError::NotActive);
    }

    let config = get_game_config(env, session.game_type.clone());
    if session.players.len() >= config.max_players {
        return Err(MiniGameError::GameFull);
    }

    for p in session.players.iter() {
        if p == player {
            return Err(MiniGameError::AlreadyPlayed);
        }
    }

    session.players.push_back(player.clone());
    env.storage().persistent().set(&MiniGameKey::Session(session_id), &session);

    env.events().publish(
        (symbol_short!("minigame"), symbol_short!("joined")),
        (session_id, player, session.players.len()),
    );

    Ok(())
}

pub fn create_daily_challenge(env: &Env, admin: Address) -> Result<u64, MiniGameError> {
    admin.require_auth();

    let now = env.ledger().timestamp();
    let challenge_id: u64 = env.storage().persistent()
        .get(&MiniGameKey::ActiveChallenge).unwrap_or(0) + 1;

    let today = now - (now % SECONDS_IN_DAY);
    let expiry = today + SECONDS_IN_DAY;

    let mut game_types = Vec::new(env);
    game_types.push_back(MiniGameType::MemoryChain);
    game_types.push_back(MiniGameType::ResourceRush);
    game_types.push_back(MiniGameType::AnomalyArena);
    game_types.push_back(MiniGameType::TradingBlitz);
    game_types.push_back(MiniGameType::NebulaPuzzle);
    game_types.push_back(MiniGameType::SpeedHarvest);
    game_types.push_back(MiniGameType::CosmicQuiz);
    let total_types = game_types.len();
    let idx = challenge_id % (total_types as u64);
    let gt = game_types.get(idx as u32).unwrap();

    let challenge = DailyChallenge {
        challenge_id,
        game_type: gt,
        target_score: 500 + (challenge_id as u32 * 50) % 2000,
        reward: 1000,
        bonus_reward: 2500,
        active_date: today,
        completed_players: Vec::new(env),
        expires_at: expiry,
    };

    env.storage().persistent().set(&MiniGameKey::DailyChallenge(challenge_id), &challenge);
    env.storage().persistent().set(&MiniGameKey::ActiveChallenge, &challenge_id);

    env.events().publish(
        (symbol_short!("minigame"), symbol_short!("challenge")),
        (challenge_id, challenge.game_type, challenge.target_score, challenge.reward),
    );

    Ok(challenge_id)
}

pub fn claim_daily_challenge(
    env: &Env,
    player: Address,
    challenge_id: u64,
    score: u32,
) -> Result<i128, MiniGameError> {
    player.require_auth();

    let mut challenge: DailyChallenge = env.storage().persistent()
        .get(&MiniGameKey::DailyChallenge(challenge_id))
        .ok_or(MiniGameError::GameNotFound)?;

    if env.ledger().timestamp() >= challenge.expires_at {
        return Err(MiniGameError::NotActive);
    }

    for p in challenge.completed_players.iter() {
        if p == player {
            return Err(MiniGameError::AlreadyPlayed);
        }
    }

    let reward = if score >= challenge.target_score {
        challenge.reward.saturating_add(challenge.bonus_reward)
    } else {
        challenge.reward
    };

    challenge.completed_players.push_back(player.clone());
    env.storage().persistent().set(&MiniGameKey::DailyChallenge(challenge_id), &challenge);

    let mut stats = get_player_stats(env, player.clone());
    stats.total_rewards = stats.total_rewards.saturating_add(reward);
    env.storage().persistent().set(&MiniGameKey::PlayerStats(player.clone()), &stats);

    env.events().publish(
        (symbol_short!("minigame"), symbol_short!("ch_clm")),
        (challenge_id, player, reward, score >= challenge.target_score),
    );

    Ok(reward)
}

fn update_leaderboard(env: &Env, player: Address, game_type: MiniGameType, score: u32) {
    let leaderboard_key = MiniGameKey::Leaderboard(game_type.clone(), 0);
    let entries: Vec<LeaderboardEntry> = env.storage().persistent()
        .get(&leaderboard_key).unwrap_or_else(|| Vec::new(env));

    let now = env.ledger().timestamp();
    let new_entry = LeaderboardEntry {
        player: player.clone(),
        score,
        game_type: game_type.clone(),
        achieved_at: now,
    };

    let mut inserted = false;
    let mut updated = Vec::new(env);
    for e in entries.iter() {
        if updated.len() < LEADERBOARD_SIZE as u32 {
            if !inserted && score > e.score {
                updated.push_back(new_entry.clone());
                inserted = true;
            }
            if updated.len() < LEADERBOARD_SIZE as u32 {
                updated.push_back(e);
            }
        }
    }
    if !inserted && updated.len() < LEADERBOARD_SIZE as u32 {
        updated.push_back(new_entry);
    }

    env.storage().persistent().set(&leaderboard_key, &updated);
}

pub fn get_leaderboard(env: &Env, game_type: MiniGameType) -> Vec<LeaderboardEntry> {
    env.storage().persistent()
        .get(&MiniGameKey::Leaderboard(game_type, 0))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn reset_daily_plays(env: &Env, player: Address) {
    let mut stats = get_player_stats(env, player.clone());
    stats.daily_plays = 0;
    env.storage().persistent().set(&MiniGameKey::PlayerStats(player.clone()), &stats);
    env.storage().persistent().set(&MiniGameKey::DailyPlays(player.clone()), &0u32);
}

fn get_game_config(env: &Env, game_type: MiniGameType) -> MiniGameConfig {
    env.storage().persistent()
        .get(&MiniGameKey::GameConfig(game_type.clone()))
        .unwrap_or_else(|| {
            let configs = get_default_game_configs(env);
            for c in configs.iter() {
                if c.game_type == game_type {
                    return c;
                }
            }
            configs.get(0).unwrap()
        })
}

fn get_player_stats(env: &Env, player: Address) -> PlayerMiniGameStats {
    env.storage().persistent()
        .get(&MiniGameKey::PlayerStats(player))
        .unwrap_or(PlayerMiniGameStats {
            games_played: 0,
            total_score: 0,
            best_score: 0,
            total_rewards: 0,
            games_won: 0,
            daily_plays: 0,
            last_play_time: 0,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{contract, contractimpl, testutils::{Address as _, Ledger, LedgerInfo}};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    fn setup() -> (Env, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set(LedgerInfo {
            protocol_version: 22,
            sequence_number: 100,
            timestamp: 1_000_000,
            network_id: [0u8; 32],
            base_reserve: 10,
            min_temp_entry_ttl: 100,
            min_persistent_entry_ttl: 1_000,
            max_entry_ttl: 10_000,
        });
        let contract = env.register(Stub, ());
        let player = Address::generate(&env);
        (env, player, contract)
    }

    struct TestCtx {
        env: Env,
        contract: Address,
    }

    impl TestCtx {
        fn run<T>(&self, f: impl FnOnce() -> T) -> T {
            self.env.as_contract(&self.contract, f)
        }
    }

    #[test]
    fn test_start_and_complete_single_player_game() {
        let (env, player, contract) = setup();
        let ctx = TestCtx { env, contract };
        let session_id = ctx.run(|| start_mini_game(&ctx.env, player.clone(), MiniGameType::NebulaPuzzle).unwrap());
        let reward = ctx.run(|| submit_mini_game_score(&ctx.env, player.clone(), session_id, 800).unwrap());
        assert!(reward > 0);
    }

    #[test]
    fn test_daily_limit_enforced() {
        let (env, player, contract) = setup();
        let ctx = TestCtx { env, contract };
        let configs = get_default_game_configs(&ctx.env);
        let limit = configs.get(4).unwrap().daily_limit;
        let mut ts = 1_000_000u64;
        for _ in 0..limit {
            ctx.env.ledger().set(LedgerInfo {
                protocol_version: 22,
                sequence_number: 100,
                timestamp: ts,
                network_id: [0u8; 32],
                base_reserve: 10,
                min_temp_entry_ttl: 100,
                min_persistent_entry_ttl: 1_000,
                max_entry_ttl: 10_000,
            });
            let sid = ctx.run(|| start_mini_game(&ctx.env, player.clone(), MiniGameType::NebulaPuzzle).unwrap());
            let _ = ctx.run(|| submit_mini_game_score(&ctx.env, player.clone(), sid, 500));
            ts += 1000;
        }
        ctx.env.ledger().set(LedgerInfo {
            protocol_version: 22,
            sequence_number: 100,
            timestamp: ts,
            network_id: [0u8; 32],
            base_reserve: 10,
            min_temp_entry_ttl: 100,
            min_persistent_entry_ttl: 1_000,
            max_entry_ttl: 10_000,
        });
        let result = ctx.run(|| start_mini_game(&ctx.env, player.clone(), MiniGameType::NebulaPuzzle));
        assert_eq!(result, Err(MiniGameError::DailyLimitReached));
    }

    #[test]
    fn test_leaderboard_updates() {
        let (env, player, contract) = setup();
        let ctx = TestCtx { env, contract };
        let session_id = ctx.run(|| start_mini_game(&ctx.env, player.clone(), MiniGameType::MemoryChain).unwrap());
        let _ = ctx.run(|| submit_mini_game_score(&ctx.env, player.clone(), session_id, 900).unwrap());
        let lb = ctx.run(|| get_leaderboard(&ctx.env, MiniGameType::MemoryChain));
        assert_eq!(lb.len(), 1);
        assert_eq!(lb.get(0).unwrap().player, player);
    }

    #[test]
    fn test_multiplayer_join() {
        let (env, host, contract) = setup();
        let guest = Address::generate(&env);
        let ctx = TestCtx { env, contract };
        let session_id = ctx.run(|| start_mini_game(&ctx.env, host.clone(), MiniGameType::CosmicQuiz).unwrap());
        assert!(ctx.run(|| join_multiplayer_game(&ctx.env, guest.clone(), session_id)).is_ok());
    }

    #[test]
    fn test_daily_challenge() {
        let (env, player, contract) = setup();
        let admin = Address::generate(&env);
        let ctx = TestCtx { env, contract };
        let challenge_id = ctx.run(|| create_daily_challenge(&ctx.env, admin.clone()).unwrap());
        let reward = ctx.run(|| claim_daily_challenge(&ctx.env, player.clone(), challenge_id, 1500).unwrap());
        assert!(reward >= 1000);
    }
}
