use soroban_sdk::{contracttype, contracterror, symbol_short, Address, Env, String, Symbol, Vec};

pub const MIN_REPUTATION: u32 = 1;
pub const MAX_REPUTATION: u32 = 100;
pub const INITIAL_REPUTATION: u32 = 50;
pub const MAX_ACTIVE_REPORTS: u32 = 1000;
pub const REPORT_RESOLUTION_DAYS: u64 = 604_800; // 7 days in seconds

#[derive(Clone)]
#[contracttype]
pub enum ReputationKey {
    Score(Address),
    ReputationHistory(Address),
    Behavior(Address),
    ReportCount(Address),
    DisputeList,
    AdminList,
    BanList(Address),
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ReputationError {
    Unauthorized = 1,
    ReputationNotFound = 2,
    InvalidScore = 3,
    ReportNotFound = 4,
    MaxReportsExceeded = 5,
    InvalidBehavior = 6,
    AlreadyBanned = 7,
    NotInitialized = 8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
#[repr(u32)]
pub enum BehaviorType {
    Positive = 0,
    Negative = 1,
    Neutral = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
#[repr(u32)]
pub enum ReportStatus {
    Pending = 0,
    Resolved = 1,
    Dismissed = 2,
    Appealed = 3,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct ReputationScore {
    pub player: Address,
    pub score: u32,
    pub total_reports: u32,
    pub positive_actions: u32,
    pub negative_actions: u32,
    pub last_updated: u64,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct BehaviorRecord {
    pub id: u64,
    pub player: Address,
    pub behavior_type: BehaviorType,
    pub description: String,
    pub points_change: i32,
    pub timestamp: u64,
    pub reporter: Address,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct DisputeReport {
    pub id: u64,
    pub reporter: Address,
    pub accused: Address,
    pub reason: String,
    pub evidence: String,
    pub status: ReportStatus,
    pub created_at: u64,
    pub resolved_at: u64,
}

pub fn initialize_reputation(env: &Env, admin: &Address) -> Result<(), ReputationError> {
    admin.require_auth();

    let mut admins: Vec<Address> = env
        .storage()
        .persistent()
        .get(&ReputationKey::AdminList)
        .unwrap_or_else(|| Vec::new(env));

    admins.push_back(admin.clone());
    env.storage()
        .persistent()
        .set(&ReputationKey::AdminList, &admins);

    env.events().publish(
        (symbol_short!("rep"), symbol_short!("init")),
        admin.clone(),
    );

    Ok(())
}

pub fn create_player_reputation(env: &Env, player: &Address) -> Result<(), ReputationError> {
    if env
        .storage()
        .persistent()
        .has(&ReputationKey::Score(player.clone()))
    {
        return Ok(());
    }

    let score = ReputationScore {
        player: player.clone(),
        score: INITIAL_REPUTATION,
        total_reports: 0,
        positive_actions: 0,
        negative_actions: 0,
        last_updated: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&ReputationKey::Score(player.clone()), &score);

    env.events().publish(
        (symbol_short!("rep"), symbol_short!("create")),
        (player.clone(), INITIAL_REPUTATION),
    );

    Ok(())
}

pub fn get_reputation_score(env: &Env, player: &Address) -> Result<u32, ReputationError> {
    let score: ReputationScore = env
        .storage()
        .persistent()
        .get(&ReputationKey::Score(player.clone()))
        .ok_or(ReputationError::ReputationNotFound)?;

    Ok(score.score)
}

pub fn get_reputation_details(env: &Env, player: &Address) -> Result<ReputationScore, ReputationError> {
    env.storage()
        .persistent()
        .get(&ReputationKey::Score(player.clone()))
        .ok_or(ReputationError::ReputationNotFound)
}

pub fn record_behavior(
    env: &Env,
    player: &Address,
    behavior_type: BehaviorType,
    description: String,
    points: i32,
    reporter: Address,
) -> Result<(), ReputationError> {
    reporter.require_auth();

    if points.abs() > 20 {
        return Err(ReputationError::InvalidBehavior);
    }

    let mut score: ReputationScore = env
        .storage()
        .persistent()
        .get(&ReputationKey::Score(player.clone()))
        .ok_or(ReputationError::ReputationNotFound)?;

    let new_score = if points > 0 {
        ((score.score as i32) + points).max(MIN_REPUTATION as i32) as u32
    } else {
        ((score.score as i32) + points).min(MAX_REPUTATION as i32).max(MIN_REPUTATION as i32) as u32
    };

    if new_score > MAX_REPUTATION || new_score < MIN_REPUTATION {
        return Err(ReputationError::InvalidScore);
    }

    score.score = new_score;
    score.last_updated = env.ledger().timestamp();

    match behavior_type {
        BehaviorType::Positive => score.positive_actions += 1,
        BehaviorType::Negative => score.negative_actions += 1,
        BehaviorType::Neutral => {}
    }

    let record = BehaviorRecord {
        id: env.ledger().sequence(),
        player: player.clone(),
        behavior_type,
        description: description.clone(),
        points_change: points,
        timestamp: env.ledger().timestamp(),
        reporter: reporter.clone(),
    };

    let mut history: Vec<BehaviorRecord> = env
        .storage()
        .persistent()
        .get(&ReputationKey::ReputationHistory(player.clone()))
        .unwrap_or_else(|| Vec::new(env));

    history.push_back(record);
    if history.len() > 100 {
        history.pop_front();
    }

    env.storage()
        .persistent()
        .set(&ReputationKey::Score(player.clone()), &score);
    env.storage()
        .persistent()
        .set(&ReputationKey::ReputationHistory(player.clone()), &history);

    env.events().publish(
        (symbol_short!("rep"), symbol_short!("update")),
        (player.clone(), score.score, behavior_type),
    );

    Ok(())
}

pub fn submit_report(
    env: &Env,
    reporter: &Address,
    accused: &Address,
    reason: String,
    evidence: String,
) -> Result<u64, ReputationError> {
    reporter.require_auth();

    let mut disputes: Vec<DisputeReport> = env
        .storage()
        .persistent()
        .get(&ReputationKey::DisputeList)
        .unwrap_or_else(|| Vec::new(env));

    if disputes.len() >= MAX_ACTIVE_REPORTS {
        return Err(ReputationError::MaxReportsExceeded);
    }

    let report_id = env.ledger().sequence();
    let report = DisputeReport {
        id: report_id,
        reporter: reporter.clone(),
        accused: accused.clone(),
        reason: reason.clone(),
        evidence: evidence.clone(),
        status: ReportStatus::Pending,
        created_at: env.ledger().timestamp(),
        resolved_at: 0,
    };

    disputes.push_back(report);
    env.storage()
        .persistent()
        .set(&ReputationKey::DisputeList, &disputes);

    let mut report_count: u32 = env
        .storage()
        .persistent()
        .get(&ReputationKey::ReportCount(accused.clone()))
        .unwrap_or(0);

    report_count += 1;
    env.storage()
        .persistent()
        .set(&ReputationKey::ReportCount(accused.clone()), &report_count);

    env.events().publish(
        (symbol_short!("rep"), symbol_short!("report")),
        (reporter.clone(), accused.clone(), report_id),
    );

    Ok(report_id)
}

pub fn resolve_report(
    env: &Env,
    admin: &Address,
    report_id: u64,
    resolved: bool,
) -> Result<(), ReputationError> {
    admin.require_auth();

    let admins: Vec<Address> = env
        .storage()
        .persistent()
        .get(&ReputationKey::AdminList)
        .ok_or(ReputationError::Unauthorized)?;

    if !admins.iter().any(|a| a == admin) {
        return Err(ReputationError::Unauthorized);
    }

    let mut disputes: Vec<DisputeReport> = env
        .storage()
        .persistent()
        .get(&ReputationKey::DisputeList)
        .ok_or(ReputationError::ReportNotFound)?;

    for report in disputes.iter_mut() {
        if report.id == report_id {
            report.status = if resolved {
                ReportStatus::Resolved
            } else {
                ReportStatus::Dismissed
            };
            report.resolved_at = env.ledger().timestamp();

            if resolved {
                let _ = record_behavior(
                    env,
                    &report.accused,
                    BehaviorType::Negative,
                    String::from_small_str(env, "Report resolved with sanctions"),
                    -5,
                    admin.clone(),
                );
            }

            env.storage()
                .persistent()
                .set(&ReputationKey::DisputeList, &disputes);

            env.events().publish(
                (symbol_short!("rep"), symbol_short!("resolve")),
                (report_id, resolved),
            );

            return Ok(());
        }
    }

    Err(ReputationError::ReportNotFound)
}

pub fn ban_player(env: &Env, admin: &Address, player: &Address) -> Result<(), ReputationError> {
    admin.require_auth();

    let admins: Vec<Address> = env
        .storage()
        .persistent()
        .get(&ReputationKey::AdminList)
        .ok_or(ReputationError::Unauthorized)?;

    if !admins.iter().any(|a| a == admin) {
        return Err(ReputationError::Unauthorized);
    }

    if env
        .storage()
        .persistent()
        .has(&ReputationKey::BanList(player.clone()))
    {
        return Err(ReputationError::AlreadyBanned);
    }

    env.storage()
        .persistent()
        .set(&ReputationKey::BanList(player.clone()), &true);

    let mut score: ReputationScore = env
        .storage()
        .persistent()
        .get(&ReputationKey::Score(player.clone()))
        .ok_or(ReputationError::ReputationNotFound)?;

    score.score = MIN_REPUTATION;
    env.storage()
        .persistent()
        .set(&ReputationKey::Score(player.clone()), &score);

    env.events().publish(
        (symbol_short!("rep"), symbol_short!("ban")),
        player.clone(),
    );

    Ok(())
}

pub fn is_player_banned(env: &Env, player: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&ReputationKey::BanList(player.clone()))
        .unwrap_or(false)
}

pub fn get_player_history(env: &Env, player: &Address) -> Vec<BehaviorRecord> {
    env.storage()
        .persistent()
        .get(&ReputationKey::ReputationHistory(player.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn get_player_report_count(env: &Env, player: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&ReputationKey::ReportCount(player.clone()))
        .unwrap_or(0)
}

pub fn get_all_reports(env: &Env) -> Vec<DisputeReport> {
    env.storage()
        .persistent()
        .get(&ReputationKey::DisputeList)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn claim_reputation_reward(env: &Env, player: &Address) -> Result<i128, ReputationError> {
    player.require_auth();

    let score: ReputationScore = env
        .storage()
        .persistent()
        .get(&ReputationKey::Score(player.clone()))
        .ok_or(ReputationError::ReputationNotFound)?;

    let reward = (score.score as i128) * 100; // 1 XLM per reputation point

    env.events().publish(
        (symbol_short!("rep"), symbol_short!("reward")),
        (player.clone(), reward),
    );

    Ok(reward)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn test_reputation_initialization() {
        let env = soroban_sdk::testing::Env::default();
        let admin = soroban_sdk::testing::Address::generate(&env);

        assert!(initialize_reputation(&env, &admin).is_ok());
    }

    #[test]
    fn test_player_reputation_creation() {
        let env = soroban_sdk::testing::Env::default();
        let admin = soroban_sdk::testing::Address::generate(&env);
        let player = soroban_sdk::testing::Address::generate(&env);

        let _ = initialize_reputation(&env, &admin);
        assert!(create_player_reputation(&env, &player).is_ok());

        let score = get_reputation_score(&env, &player);
        assert_eq!(score.unwrap(), INITIAL_REPUTATION);
    }

    #[test]
    fn test_behavior_recording() {
        let env = soroban_sdk::testing::Env::default();
        let admin = soroban_sdk::testing::Address::generate(&env);
        let player = soroban_sdk::testing::Address::generate(&env);
        let reporter = soroban_sdk::testing::Address::generate(&env);

        let _ = initialize_reputation(&env, &admin);
        let _ = create_player_reputation(&env, &player);

        let description = String::from_small_str(&env, "Test behavior");
        let result = record_behavior(
            &env,
            &player,
            BehaviorType::Positive,
            description,
            5,
            reporter,
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_ban_player() {
        let env = soroban_sdk::testing::Env::default();
        let admin = soroban_sdk::testing::Address::generate(&env);
        let player = soroban_sdk::testing::Address::generate(&env);

        let _ = initialize_reputation(&env, &admin);
        let _ = create_player_reputation(&env, &player);

        assert!(ban_player(&env, &admin, &player).is_ok());
        assert!(is_player_banned(&env, &player));
    }
}
