use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Vec};

use crate::alliance_manager::{
    add_alliance_xp, credit_alliance_treasury, get_alliance,
    get_alliance_treasury, get_player_alliance,
};

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum WarError {
    WarNotFound = 1,
    WarNotActive = 2,
    NotAllianceMember = 3,
    AlreadyAtWar = 4,
    InvalidAlliance = 5,
    NotEnoughVotes = 6,
    CooldownActive = 7,
    TerritoryNotOwned = 8,
    AttackTooFrequent = 9,
    WarAlreadyEnded = 10,
    NotWarParticipant = 11,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum WarStatus {
    Declared,
    Active,
    Finished,
    Ceasefire,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct WarDeclaration {
    pub war_id: u64,
    pub attacker_alliance: u64,
    pub defender_alliance: u64,
    pub declared_at: u64,
    pub status: WarStatus,
    pub attacker_score: u32,
    pub defender_score: u32,
    pub victory_threshold: u32,
    pub ends_at: u64,
    pub territory_stake: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct WarTerritory {
    pub territory_id: u64,
    pub owner_alliance: u64,
    pub captured_at: u64,
    pub defense_bonus: u32,
    pub resource_output: i128,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct BattleRecord {
    pub battle_id: u64,
    pub war_id: u64,
    pub attacker: Address,
    pub defender: Address,
    pub attacker_alliance: u64,
    pub defender_alliance: u64,
    pub result: u32,
    pub points: u32,
    pub fought_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct WarRewards {
    pub winner_alliance: u64,
    pub loser_alliance: u64,
    pub essence_reward: i128,
    pub xp_reward: u64,
    pub captured_territory: Option<u64>,
}

#[contracttype]
#[derive(Clone)]
pub enum WarKey {
    War(u64),
    WarCount,
    AllianceWar(u64),
    Territory(u64),
    TerritoryCount,
    AllianceTerritories(u64),
    Battle(u64),
    BattleCount,
    WarParticipants(u64),
    CeasefireCooldown(u64),
    AllianceAtWar(u64),
}

const WAR_VICTORY_THRESHOLD: u32 = 100;
const WAR_DURATION_SECS: u64 = 604800;
const BATTLE_POINTS_WIN: u32 = 10;
const TERRITORY_CAPTURE_COST: i128 = 2000;

pub fn declare_war(
    env: &Env,
    declarer: Address,
    defender_alliance: u64,
    territory_stake: u64,
) -> Result<u64, WarError> {
    declarer.require_auth();
    let attacker_id = get_player_alliance(env, declarer.clone())
        .ok_or(WarError::NotAllianceMember)?;

    if attacker_id == defender_alliance {
        return Err(WarError::InvalidAlliance);
    }

    let _ = get_alliance(env, defender_alliance).map_err(|_| WarError::InvalidAlliance)?;

    if env.storage().persistent().has(&WarKey::AllianceAtWar(attacker_id))
        || env.storage().persistent().has(&WarKey::AllianceAtWar(defender_alliance))
    {
        return Err(WarError::AlreadyAtWar);
    }

    if env.storage().persistent().has(&WarKey::CeasefireCooldown(defender_alliance)) {
        return Err(WarError::CooldownActive);
    }

    let treasury = get_alliance_treasury(env, attacker_id);
    if treasury < TERRITORY_CAPTURE_COST {
        return Err(WarError::NotEnoughVotes);
    }

    let war_id: u64 = env.storage().persistent()
        .get(&WarKey::WarCount).unwrap_or(0) + 1;
    let now = env.ledger().timestamp();

    let war = WarDeclaration {
        war_id,
        attacker_alliance: attacker_id,
        defender_alliance,
        declared_at: now,
        status: WarStatus::Declared,
        attacker_score: 0,
        defender_score: 0,
        victory_threshold: WAR_VICTORY_THRESHOLD,
        ends_at: now + WAR_DURATION_SECS,
        territory_stake,
    };

    env.storage().persistent().set(&WarKey::War(war_id), &war);
    env.storage().persistent().set(&WarKey::WarCount, &war_id);
    env.storage().persistent().set(&WarKey::AllianceAtWar(attacker_id), &war_id);
    env.storage().persistent().set(&WarKey::AllianceAtWar(defender_alliance), &war_id);

    env.events().publish(
        (symbol_short!("war"), symbol_short!("declared")),
        (war_id, attacker_id, defender_alliance, territory_stake),
    );

    Ok(war_id)
}

pub fn fight_battle(
    env: &Env,
    attacker: Address,
    defender: Address,
    war_id: u64,
    attacker_power: u32,
    defender_power: u32,
) -> Result<BattleRecord, WarError> {
    attacker.require_auth();
    defender.require_auth();

    let mut war: WarDeclaration = env.storage().persistent()
        .get(&WarKey::War(war_id))
        .ok_or(WarError::WarNotFound)?;

    if war.status == WarStatus::Finished || war.status == WarStatus::Ceasefire {
        return Err(WarError::WarAlreadyEnded);
    }

    let now = env.ledger().timestamp();
    if now >= war.ends_at {
        war.status = WarStatus::Finished;
        env.storage().persistent().set(&WarKey::War(war_id), &war);
        return Err(WarError::WarNotActive);
    }

    let attacker_alliance = get_player_alliance(env, attacker.clone())
        .ok_or(WarError::NotAllianceMember)?;
    let defender_alliance = get_player_alliance(env, defender.clone())
        .ok_or(WarError::NotAllianceMember)?;

    if (attacker_alliance != war.attacker_alliance && attacker_alliance != war.defender_alliance)
        || (defender_alliance != war.attacker_alliance && defender_alliance != war.defender_alliance)
    {
        return Err(WarError::NotWarParticipant);
    }

    let battle_id: u64 = env.storage().persistent()
        .get(&WarKey::BattleCount).unwrap_or(0) + 1;
    let result = if attacker_power > defender_power { 1u32 } else { 0u32 };
    let points = if result == 1 { BATTLE_POINTS_WIN } else { 0 };

    if result == 1 {
        if attacker_alliance == war.attacker_alliance {
            war.attacker_score = war.attacker_score.saturating_add(points);
        } else {
            war.defender_score = war.defender_score.saturating_add(points);
        }
    } else {
        if defender_alliance == war.defender_alliance {
            war.defender_score = war.defender_score.saturating_add(points);
        } else {
            war.attacker_score = war.attacker_score.saturating_add(points);
        }
    }

    if war.attacker_score >= war.victory_threshold || war.defender_score >= war.victory_threshold {
        war.status = WarStatus::Finished;
        let rewards = settle_war_rewards(env, &war);
        env.events().publish(
            (symbol_short!("war"), symbol_short!("settled")),
            (war_id, rewards.winner_alliance, rewards.essence_reward),
        );
    }

    let battle = BattleRecord {
        battle_id,
        war_id,
        attacker: attacker.clone(),
        defender: defender.clone(),
        attacker_alliance,
        defender_alliance,
        result,
        points,
        fought_at: now,
    };

    env.storage().persistent().set(&WarKey::Battle(battle_id), &battle);
    env.storage().persistent().set(&WarKey::BattleCount, &battle_id);
    env.storage().persistent().set(&WarKey::War(war_id), &war);

    env.events().publish(
        (symbol_short!("war"), symbol_short!("battle")),
        (war_id, battle_id, attacker, defender, result, points),
    );

    Ok(battle)
}

fn settle_war_rewards(env: &Env, war: &WarDeclaration) -> WarRewards {
    let (winner_id, loser_id) = if war.attacker_score >= war.victory_threshold {
        (war.attacker_alliance, war.defender_alliance)
    } else {
        (war.defender_alliance, war.attacker_alliance)
    };

    let essence_reward: i128 = 5000;
    let xp_reward: u64 = 2500;

    let _ = credit_alliance_treasury(env, winner_id, essence_reward);
    let _ = add_alliance_xp(env, winner_id, xp_reward);

    let captured = if !env.storage().persistent().has(&WarKey::Territory(war.territory_stake)) {
        let territory = WarTerritory {
            territory_id: war.territory_stake,
            owner_alliance: winner_id,
            captured_at: env.ledger().timestamp(),
            defense_bonus: 50,
            resource_output: 100,
        };
        env.storage().persistent().set(&WarKey::Territory(war.territory_stake), &territory);
        env.storage().persistent().set(&WarKey::AllianceTerritories(winner_id), &war.territory_stake);
        Some(war.territory_stake)
    } else {
        None
    };

    env.storage().persistent().remove(&WarKey::AllianceAtWar(war.attacker_alliance));
    env.storage().persistent().remove(&WarKey::AllianceAtWar(war.defender_alliance));
    env.storage().persistent().set(&WarKey::CeasefireCooldown(loser_id), &env.ledger().timestamp());

    WarRewards {
        winner_alliance: winner_id,
        loser_alliance: loser_id,
        essence_reward,
        xp_reward,
        captured_territory: captured,
    }
}

pub fn claim_ceasefire(env: &Env, caller: Address) -> Result<(), WarError> {
    caller.require_auth();
    let alliance_id = get_player_alliance(env, caller.clone())
        .ok_or(WarError::NotAllianceMember)?;

    if !env.storage().persistent().has(&WarKey::AllianceAtWar(alliance_id)) {
        return Err(WarError::WarNotFound);
    }

    let war_id: u64 = env.storage().persistent()
        .get(&WarKey::AllianceAtWar(alliance_id)).unwrap();
    let mut war: WarDeclaration = env.storage().persistent()
        .get(&WarKey::War(war_id)).ok_or(WarError::WarNotFound)?;

    war.status = WarStatus::Ceasefire;
    env.storage().persistent().set(&WarKey::War(war_id), &war);
    env.storage().persistent().remove(&WarKey::AllianceAtWar(war.attacker_alliance));
    env.storage().persistent().remove(&WarKey::AllianceAtWar(war.defender_alliance));
    env.storage().persistent().set(&WarKey::CeasefireCooldown(alliance_id), &env.ledger().timestamp());

    env.events().publish(
        (symbol_short!("war"), symbol_short!("ceasefire")),
        (war_id, alliance_id),
    );
    Ok(())
}

pub fn get_war(env: &Env, war_id: u64) -> Option<WarDeclaration> {
    env.storage().persistent().get(&WarKey::War(war_id))
}

pub fn get_alliance_active_war(env: &Env, alliance_id: u64) -> Option<u64> {
    env.storage().persistent().get(&WarKey::AllianceAtWar(alliance_id))
}

pub fn get_territory(env: &Env, territory_id: u64) -> Option<WarTerritory> {
    env.storage().persistent().get(&WarKey::Territory(territory_id))
}

pub fn get_alliance_territories(env: &Env, alliance_id: u64) -> Vec<u64> {
    let t: Option<u64> = env.storage().persistent().get(&WarKey::AllianceTerritories(alliance_id));
    match t {
        Some(id) => {
            let mut v = Vec::new(env);
            v.push_back(id);
            v
        }
        None => Vec::new(env),
    }
}

pub fn get_battle(env: &Env, battle_id: u64) -> Option<BattleRecord> {
    env.storage().persistent().get(&WarKey::Battle(battle_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{contract, contractimpl, testutils::{Address as _, Ledger, LedgerInfo}, BytesN, String};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    fn setup_env() -> (Env, Address, Address) {
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
        let contract_id = env.register(Stub, ());
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);
        (env, player1, player2)
    }

    fn in_contract<T>(env: &Env, contract_id: &Address, f: impl FnOnce() -> T) -> T {
        env.as_contract(contract_id, f)
    }

    fn create_test_alliance(env: &Env, founder: &Address, name: &str) -> u64 {
        crate::alliance_manager::found_alliance(env, founder.clone(), String::from_str(env, name)).unwrap()
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

    fn fund_treasury(ctx: &TestCtx, aid: u64, player: Address) {
        // credit alliance treasury directly via storage to avoid require_auth issues
        let treasury = crate::alliance_manager::get_alliance_treasury(&ctx.env, aid);
        crate::alliance_manager::credit_alliance_treasury(&ctx.env, aid, 5000).unwrap();
    }

    #[test]
    fn test_declare_war_succeeds() {
        let (env, p1, p2) = setup_env();
        let ctx = TestCtx { contract: env.register(Stub, ()), env };
        let aid1 = ctx.run(|| crate::alliance_manager::found_alliance(&ctx.env, p1.clone(), String::from_str(&ctx.env, "Alpha")).unwrap());
        let aid2 = ctx.run(|| crate::alliance_manager::found_alliance(&ctx.env, p2.clone(), String::from_str(&ctx.env, "Beta")).unwrap());
        ctx.run(|| fund_treasury(&ctx, aid1, p1.clone()));
        ctx.run(|| fund_treasury(&ctx, aid2, p2.clone()));

        let war_id = ctx.run(|| declare_war(&ctx.env, p1.clone(), aid2, 1).unwrap());
        let war = ctx.run(|| get_war(&ctx.env, war_id).unwrap());
        assert_eq!(war.attacker_alliance, aid1);
        assert_eq!(war.defender_alliance, aid2);
        assert_eq!(war.status, WarStatus::Declared);
    }

    #[test]
    fn test_fight_battle_updates_score() {
        let (env, p1, p2) = setup_env();
        let ctx = TestCtx { contract: env.register(Stub, ()), env };
        let aid1 = ctx.run(|| crate::alliance_manager::found_alliance(&ctx.env, p1.clone(), String::from_str(&ctx.env, "Alpha")).unwrap());
        let aid2 = ctx.run(|| crate::alliance_manager::found_alliance(&ctx.env, p2.clone(), String::from_str(&ctx.env, "Beta")).unwrap());
        ctx.run(|| fund_treasury(&ctx, aid1, p1.clone()));
        ctx.run(|| fund_treasury(&ctx, aid2, p2.clone()));

        let war_id = ctx.run(|| declare_war(&ctx.env, p1.clone(), aid2, 1).unwrap());
        let battle = ctx.run(|| fight_battle(&ctx.env, p1.clone(), p2.clone(), war_id, 100, 50).unwrap());
        assert_eq!(battle.result, 1);

        let war = ctx.run(|| get_war(&ctx.env, war_id).unwrap());
        assert!(war.attacker_score > 0);
    }

    #[test]
    fn test_war_settles_on_victory() {
        let (env, p1, p2) = setup_env();
        let ctx = TestCtx { contract: env.register(Stub, ()), env };
        let aid1 = ctx.run(|| crate::alliance_manager::found_alliance(&ctx.env, p1.clone(), String::from_str(&ctx.env, "Alpha")).unwrap());
        let aid2 = ctx.run(|| crate::alliance_manager::found_alliance(&ctx.env, p2.clone(), String::from_str(&ctx.env, "Beta")).unwrap());
        ctx.run(|| fund_treasury(&ctx, aid1, p1.clone()));
        ctx.run(|| fund_treasury(&ctx, aid2, p2.clone()));

        let war_id = ctx.run(|| declare_war(&ctx.env, p1.clone(), aid2, 1).unwrap());
        for _ in 0..15 {
            let atk = Address::generate(&ctx.env);
            let def = Address::generate(&ctx.env);
            let _ = ctx.run(|| crate::alliance_manager::join_alliance(&ctx.env, aid1, atk.clone()));
            let _ = ctx.run(|| crate::alliance_manager::join_alliance(&ctx.env, aid2, def.clone()));
            let _ = ctx.run(|| fight_battle(&ctx.env, atk, def, war_id, 200, 50));
        }

        let war = ctx.run(|| get_war(&ctx.env, war_id).unwrap());
        assert_eq!(war.status, WarStatus::Finished);
    }

    #[test]
    fn test_ceasefire_cooldown() {
        let (env, p1, p2) = setup_env();
        let ctx = TestCtx { contract: env.register(Stub, ()), env };
        let aid1 = ctx.run(|| crate::alliance_manager::found_alliance(&ctx.env, p1.clone(), String::from_str(&ctx.env, "Alpha")).unwrap());
        let aid2 = ctx.run(|| crate::alliance_manager::found_alliance(&ctx.env, p2.clone(), String::from_str(&ctx.env, "Beta")).unwrap());
        ctx.run(|| fund_treasury(&ctx, aid1, p1.clone()));
        ctx.run(|| fund_treasury(&ctx, aid2, p2.clone()));

        let war_id = ctx.run(|| declare_war(&ctx.env, p1.clone(), aid2, 1).unwrap());
        ctx.run(|| claim_ceasefire(&ctx.env, p1.clone()).unwrap());

        let cooldown = ctx.run(|| {
            ctx.env.storage().persistent()
                .get::<_, u64>(&WarKey::CeasefireCooldown(aid1))
        });
        assert!(cooldown.is_some());
    }
}
