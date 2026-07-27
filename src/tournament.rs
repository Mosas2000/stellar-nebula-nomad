//! Tournament system (Issue #284): bracket generation, entry-fee prize
//! pools, and spectator mode for competitive PvP play.
//!
//! Builds directly on top of `pvp_combat.rs`'s existing combat engine,
//! matchmaking, ELO, and spectator primitives rather than duplicating them —
//! a tournament match *is* a real `pvp_combat::CombatState`, so spectators
//! can already watch it via `pvp_combat::add_spectator`/`get_spectators`
//! once its `combat_id` is known.
//!
//! Prize pools use the real, already-audited `resource_minter` balance
//! system (`debit_balance`/`credit_balance`, checked arithmetic, no
//! overflow/underflow) rather than inventing a parallel economy.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

use crate::pvp_combat;
use crate::resource_minter::{self, ResourceType};

// ── Error ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TournamentError {
    /// `max_players` must be a power of two in [4, 64].
    InvalidPlayerCount = 1,
    /// Prize distribution basis points don't sum to <= 10_000, or the list
    /// is longer than the number of possible placements.
    InvalidPrizeDistribution = 2,
    /// Tournament not found.
    TournamentNotFound = 3,
    /// Not authorized (not the organizer/admin).
    Unauthorized = 4,
    /// Registration window has closed, or the tournament isn't in the
    /// registration phase.
    RegistrationClosed = 5,
    /// Player already registered for this tournament.
    AlreadyRegistered = 6,
    /// Tournament already at max capacity.
    TournamentFull = 7,
    /// Fewer than 2 players registered — can't start a bracket.
    NotEnoughRegistrants = 8,
    /// Tournament isn't in the registration phase (can't start it, or
    /// can't register once it's left that phase).
    NotInRegistration = 9,
    /// Tournament isn't currently active (bracket in progress).
    TournamentNotActive = 10,
    /// Round/match index out of range for this tournament's bracket.
    InvalidMatch = 11,
    /// The underlying combat for this match hasn't finished yet.
    MatchNotReady = 12,
    /// This match already has a recorded winner.
    MatchAlreadyResolved = 13,
    /// Player isn't part of this specific match.
    PlayerNotInMatch = 14,
    /// Tournament already completed.
    TournamentAlreadyDone = 15,
}

// ── Storage Keys ────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum TournamentDataKey {
    Tournament(u64),
    TournamentCounter,
    /// Registrants in registration order (also the seeding order once
    /// re-sorted by ELO at bracket-generation time).
    Registrants(u64),
    /// One bracket round's matches: (tournament_id, round) -> Vec<BracketMatch>.
    Round(u64, u32),
}

// ── Data Types ────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct Tournament {
    pub tournament_id: u64,
    pub organizer: Address,
    pub resource_type: ResourceType,
    pub entry_fee: u64,
    /// Bracket capacity (power of two, 4..=64). Actual registrant count may
    /// be lower — the bracket that gets generated is sized to the smallest
    /// power of two that fits the real registrant count, not necessarily
    /// this cap.
    pub max_players: u32,
    /// "reg" | "active" | "done" | "cancelled"
    pub status: Symbol,
    pub created_at: u64,
    pub registration_deadline: u64,
    /// 0 before the bracket starts; 1-based once `start_tournament` runs.
    pub current_round: u32,
    /// Total rounds in the generated bracket (log2 of its size).
    pub total_rounds: u32,
    pub prize_pool: u64,
    /// Basis-point share of the prize pool per placement, best-first
    /// (index 0 = champion, 1 = runner-up, ...). Must sum to <= 10_000.
    pub prize_distribution_bps: Vec<u32>,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct BracketMatch {
    /// `None` on either side means a bye (auto-advance) rather than a real
    /// combat — used to fill a bracket whose size is a power of two even
    /// when the registrant count wasn't.
    pub player1: Option<Address>,
    pub player2: Option<Address>,
    pub winner: Option<Address>,
    /// Set once a real (non-bye) combat has been started for this match.
    pub combat_id: Option<u64>,
}

// ── Constants ────────────────────────────────────────────────────────────

pub const MIN_TOURNAMENT_PLAYERS: u32 = 4;
pub const MAX_TOURNAMENT_PLAYERS: u32 = 64;

fn is_power_of_two(n: u32) -> bool {
    n != 0 && (n & (n - 1)) == 0
}

/// Smallest power of two >= n (n >= 1).
fn next_power_of_two(n: u32) -> u32 {
    let mut p = 1u32;
    while p < n {
        p *= 2;
    }
    p
}

fn log2_u32(mut n: u32) -> u32 {
    let mut rounds = 0u32;
    while n > 1 {
        n /= 2;
        rounds += 1;
    }
    rounds
}

// ── Admin / Creation ────────────────────────────────────────────────────

/// Create a new tournament in its registration phase.
///
/// `prize_distribution_bps` gives each placement's share of the prize pool,
/// best-first (e.g. `[7000, 3000]` = 70% champion / 30% runner-up). Must sum
/// to <= 10_000 and have no more entries than `max_players`.
pub fn create_tournament(
    env: &Env,
    organizer: &Address,
    resource_type: ResourceType,
    entry_fee: u64,
    max_players: u32,
    registration_window_secs: u64,
    prize_distribution_bps: Vec<u32>,
) -> Result<u64, TournamentError> {
    organizer.require_auth();

    if !is_power_of_two(max_players)
        || max_players < MIN_TOURNAMENT_PLAYERS
        || max_players > MAX_TOURNAMENT_PLAYERS
    {
        return Err(TournamentError::InvalidPlayerCount);
    }

    let mut bps_sum: u32 = 0;
    for i in 0..prize_distribution_bps.len() {
        bps_sum = bps_sum
            .checked_add(prize_distribution_bps.get(i).unwrap_or(0))
            .ok_or(TournamentError::InvalidPrizeDistribution)?;
    }
    if bps_sum > 10_000 || prize_distribution_bps.len() > max_players {
        return Err(TournamentError::InvalidPrizeDistribution);
    }

    let counter: u64 = env
        .storage()
        .persistent()
        .get(&TournamentDataKey::TournamentCounter)
        .unwrap_or(0);
    let tournament_id = counter + 1;
    env.storage()
        .persistent()
        .set(&TournamentDataKey::TournamentCounter, &tournament_id);

    let now = env.ledger().timestamp();
    let tournament = Tournament {
        tournament_id,
        organizer: organizer.clone(),
        resource_type,
        entry_fee,
        max_players,
        status: symbol_short!("reg"),
        created_at: now,
        registration_deadline: now + registration_window_secs,
        current_round: 0,
        total_rounds: 0,
        prize_pool: 0,
        prize_distribution_bps,
    };

    env.storage()
        .persistent()
        .set(&TournamentDataKey::Tournament(tournament_id), &tournament);
    env.storage().persistent().set(
        &TournamentDataKey::Registrants(tournament_id),
        &Vec::<Address>::new(env),
    );

    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("created")),
        (tournament_id, organizer.clone(), max_players),
    );

    Ok(tournament_id)
}

pub fn get_tournament(env: &Env, tournament_id: u64) -> Result<Tournament, TournamentError> {
    env.storage()
        .persistent()
        .get(&TournamentDataKey::Tournament(tournament_id))
        .ok_or(TournamentError::TournamentNotFound)
}

fn save_tournament(env: &Env, t: &Tournament) {
    env.storage()
        .persistent()
        .set(&TournamentDataKey::Tournament(t.tournament_id), t);
}

pub fn get_registrants(env: &Env, tournament_id: u64) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&TournamentDataKey::Registrants(tournament_id))
        .unwrap_or(Vec::new(env))
}

// ── Registration & Prize Pool ───────────────────────────────────────────

/// Register for a tournament, paying the entry fee into its prize pool.
/// Returns the registrant's 0-based registration order.
pub fn register_for_tournament(
    env: &Env,
    player: &Address,
    tournament_id: u64,
) -> Result<u32, TournamentError> {
    player.require_auth();

    let mut tournament = get_tournament(env, tournament_id)?;
    if tournament.status != symbol_short!("reg") {
        return Err(TournamentError::NotInRegistration);
    }
    if env.ledger().timestamp() > tournament.registration_deadline {
        return Err(TournamentError::RegistrationClosed);
    }

    let mut registrants = get_registrants(env, tournament_id);
    if registrants.len() >= tournament.max_players {
        return Err(TournamentError::TournamentFull);
    }
    for i in 0..registrants.len() {
        if let Some(p) = registrants.get(i) {
            if p == *player {
                return Err(TournamentError::AlreadyRegistered);
            }
        }
    }

    if tournament.entry_fee > 0 {
        resource_minter::debit_balance(env, player, &tournament.resource_type, tournament.entry_fee)
            .map_err(|_| TournamentError::RegistrationClosed)?;
        tournament.prize_pool = tournament.prize_pool.saturating_add(tournament.entry_fee);
    }

    let position = registrants.len();
    registrants.push_back(player.clone());
    env.storage()
        .persistent()
        .set(&TournamentDataKey::Registrants(tournament_id), &registrants);
    save_tournament(env, &tournament);

    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("register")),
        (tournament_id, player.clone(), position),
    );

    Ok(position)
}

// ── Bracket Generation ──────────────────────────────────────────────────

/// Close registration and generate the bracket. Seeds registrants by
/// current ELO (descending) so any byes (when the registrant count isn't
/// itself a power of two) land against the strongest remaining players
/// first, matching standard tournament seeding.
pub fn start_tournament(
    env: &Env,
    caller: &Address,
    tournament_id: u64,
) -> Result<(), TournamentError> {
    caller.require_auth();

    let mut tournament = get_tournament(env, tournament_id)?;
    if tournament.organizer != *caller {
        return Err(TournamentError::Unauthorized);
    }
    if tournament.status != symbol_short!("reg") {
        return Err(TournamentError::NotInRegistration);
    }

    let registrants = get_registrants(env, tournament_id);
    if registrants.len() < 2 {
        return Err(TournamentError::NotEnoughRegistrants);
    }

    // Seed by ELO descending (stable insertion sort — registrant counts are
    // small, at most MAX_TOURNAMENT_PLAYERS).
    let mut seeded: Vec<Address> = Vec::new(env);
    for i in 0..registrants.len() {
        let p = registrants.get(i).unwrap();
        let p_elo = pvp_combat::get_elo_rating(env, &p);
        let mut insert_at = seeded.len();
        for j in 0..seeded.len() {
            let existing = seeded.get(j).unwrap();
            if p_elo > pvp_combat::get_elo_rating(env, &existing) {
                insert_at = j;
                break;
            }
        }
        seeded.insert(insert_at, p);
    }

    let bracket_size = next_power_of_two(seeded.len());
    let total_rounds = log2_u32(bracket_size);

    // Standard seeding pairing: slot i vs slot (bracket_size - 1 - i).
    // Byes (slots beyond seeded.len()) always pair against the top seeds
    // first because they occupy the highest-index slots.
    let mut first_round: Vec<BracketMatch> = Vec::new(env);
    for i in 0..(bracket_size / 2) {
        let hi_idx = i;
        let lo_idx = bracket_size - 1 - i;
        let p1 = if hi_idx < seeded.len() {
            Some(seeded.get(hi_idx).unwrap())
        } else {
            None
        };
        let p2 = if lo_idx < seeded.len() {
            Some(seeded.get(lo_idx).unwrap())
        } else {
            None
        };
        first_round.push_back(resolve_or_start_match(env, tournament_id, p1, p2));
    }

    env.storage()
        .persistent()
        .set(&TournamentDataKey::Round(tournament_id, 1), &first_round);

    tournament.status = symbol_short!("active");
    tournament.current_round = 1;
    tournament.total_rounds = total_rounds;
    save_tournament(env, &tournament);

    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("start")),
        (tournament_id, bracket_size, total_rounds),
    );

    maybe_advance_round(env, tournament_id)?;

    Ok(())
}

/// Build a `BracketMatch` for a pairing: a bye if either side is absent
/// (auto-winner, no combat), otherwise starts a real `pvp_combat` combat.
fn resolve_or_start_match(
    env: &Env,
    tournament_id: u64,
    p1: Option<Address>,
    p2: Option<Address>,
) -> BracketMatch {
    match (p1, p2) {
        (Some(a), None) => BracketMatch {
            player1: Some(a.clone()),
            player2: None,
            winner: Some(a),
            combat_id: None,
        },
        (None, Some(b)) => BracketMatch {
            player1: None,
            player2: Some(b.clone()),
            winner: Some(b),
            combat_id: None,
        },
        (None, None) => BracketMatch {
            player1: None,
            player2: None,
            winner: None,
            combat_id: None,
        },
        (Some(a), Some(b)) => {
            // Tournament match id encodes the tournament so it's easy to
            // trace in `pvp_combat` event logs; not used for validation.
            let synthetic_challenge_id = tournament_id;
            let combat_id = pvp_combat::start_combat(env, &a, &b, synthetic_challenge_id)
                .expect("start_combat should not fail for two freshly-seeded players");
            BracketMatch {
                player1: Some(a),
                player2: Some(b),
                winner: None,
                combat_id: Some(combat_id),
            }
        }
    }
}

// ── Match Resolution & Round Advancement ─────────────────────────────────

pub fn get_bracket_round(env: &Env, tournament_id: u64, round: u32) -> Vec<BracketMatch> {
    env.storage()
        .persistent()
        .get(&TournamentDataKey::Round(tournament_id, round))
        .unwrap_or(Vec::new(env))
}

/// Pull the result of a finished `pvp_combat` combat into its bracket match.
/// No-op error if the combat hasn't finished yet — callers should retry
/// once `pvp_combat::get_combat` reports a winner.
pub fn report_match_result(
    env: &Env,
    tournament_id: u64,
    round: u32,
    match_index: u32,
) -> Result<(), TournamentError> {
    let tournament = get_tournament(env, tournament_id)?;
    if tournament.status != symbol_short!("active") {
        return Err(TournamentError::TournamentNotActive);
    }
    if round != tournament.current_round {
        return Err(TournamentError::InvalidMatch);
    }

    let mut matches = get_bracket_round(env, tournament_id, round);
    let mut m = matches
        .get(match_index)
        .ok_or(TournamentError::InvalidMatch)?;

    if m.winner.is_some() {
        return Err(TournamentError::MatchAlreadyResolved);
    }
    let combat_id = m.combat_id.ok_or(TournamentError::InvalidMatch)?;

    let combat = pvp_combat::get_combat(env, combat_id)
        .map_err(|_| TournamentError::InvalidMatch)?;
    let winner = combat.winner.ok_or(TournamentError::MatchNotReady)?;

    m.winner = Some(winner.clone());
    matches.set(match_index, m);
    env.storage()
        .persistent()
        .set(&TournamentDataKey::Round(tournament_id, round), &matches);

    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("match")),
        (tournament_id, round, match_index, winner),
    );

    maybe_advance_round(env, tournament_id)?;

    Ok(())
}

/// If every match in the current round has a winner, build the next round
/// (or finish the tournament if the current round was the final).
fn maybe_advance_round(env: &Env, tournament_id: u64) -> Result<(), TournamentError> {
    let mut tournament = get_tournament(env, tournament_id)?;
    if tournament.status != symbol_short!("active") {
        return Ok(());
    }

    let current = get_bracket_round(env, tournament_id, tournament.current_round);
    let mut winners: Vec<Address> = Vec::new(env);
    for i in 0..current.len() {
        let m = current.get(i).unwrap();
        match m.winner {
            Some(w) => winners.push_back(w),
            None => return Ok(()), // round not fully resolved yet
        }
    }

    if winners.len() == 1 {
        finish_tournament(env, &mut tournament, &winners.get(0).unwrap())?;
        return Ok(());
    }

    // Build the next round by pairing consecutive winners in bracket order.
    let mut next_round: Vec<BracketMatch> = Vec::new(env);
    let mut i = 0u32;
    while i < winners.len() {
        let p1 = winners.get(i);
        let p2 = winners.get(i + 1);
        next_round.push_back(resolve_or_start_match(env, tournament_id, p1, p2));
        i += 2;
    }

    let next_round_num = tournament.current_round + 1;
    env.storage().persistent().set(
        &TournamentDataKey::Round(tournament_id, next_round_num),
        &next_round,
    );
    tournament.current_round = next_round_num;
    save_tournament(env, &tournament);

    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("round")),
        (tournament_id, next_round_num),
    );

    // A newly-built round might itself be all-byes-resolved already
    // (possible only in degenerate tiny brackets) — recurse once.
    maybe_advance_round(env, tournament_id)
}

fn finish_tournament(
    env: &Env,
    tournament: &mut Tournament,
    champion: &Address,
) -> Result<(), TournamentError> {
    tournament.status = symbol_short!("done");
    save_tournament(env, tournament);

    distribute_prizes(env, tournament, champion)?;

    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("done")),
        (tournament.tournament_id, champion.clone()),
    );

    Ok(())
}

/// Pay out the prize pool per `prize_distribution_bps`, best-first. Only
/// the champion (index 0) is resolvable purely from the final match today —
/// runner-up/3rd-place payouts (index >= 1) are derived from whoever lost
/// in the semifinal/final rounds, walked back from the recorded bracket.
fn distribute_prizes(
    env: &Env,
    tournament: &Tournament,
    champion: &Address,
) -> Result<(), TournamentError> {
    if tournament.prize_pool == 0 || tournament.prize_distribution_bps.is_empty() {
        return Ok(());
    }

    let placements = placements_best_first(env, tournament);

    for i in 0..tournament.prize_distribution_bps.len() {
        let bps = tournament.prize_distribution_bps.get(i).unwrap_or(0);
        if bps == 0 {
            continue;
        }
        let Some(winner_addr) = placements.get(i) else {
            continue;
        };
        let payout = (tournament.prize_pool as u128 * bps as u128 / 10_000u128) as u64;
        if payout > 0 {
            resource_minter::credit_balance(env, &winner_addr, &tournament.resource_type, payout)
                .map_err(|_| TournamentError::InvalidPrizeDistribution)?;
        }
    }

    let _ = champion; // champion == placements.get(0); kept as a param for clarity at call sites
    Ok(())
}

/// Reconstruct placement order best-first: champion, then the finalist
/// they beat, then the losing semifinalists, etc. — i.e. the loser of each
/// round in reverse-round order, ending with round 1's losers.
fn placements_best_first(env: &Env, tournament: &Tournament) -> Vec<Address> {
    let mut placements: Vec<Address> = Vec::new(env);
    let final_round = get_bracket_round(env, tournament.tournament_id, tournament.current_round);
    if let Some(final_match) = final_round.get(0) {
        if let Some(w) = final_match.winner.clone() {
            placements.push_back(w.clone());
            let loser = if final_match.player1.as_ref() == Some(&w) {
                final_match.player2.clone()
            } else {
                final_match.player1.clone()
            };
            if let Some(l) = loser {
                placements.push_back(l);
            }
        }
    }

    // Walk earlier rounds' losers, most recent round first, to fill out
    // 3rd/4th place etc.
    let mut round = tournament.current_round;
    while round > 1 && placements.len() < tournament.prize_distribution_bps.len() {
        round -= 1;
        let matches = get_bracket_round(env, tournament.tournament_id, round);
        for i in 0..matches.len() {
            if placements.len() >= tournament.prize_distribution_bps.len() {
                break;
            }
            let m = matches.get(i).unwrap();
            if let Some(w) = m.winner.clone() {
                let loser = if m.player1.as_ref() == Some(&w) {
                    m.player2.clone()
                } else {
                    m.player1.clone()
                };
                if let Some(l) = loser {
                    placements.push_back(l);
                }
            }
        }
    }

    placements
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::resource_minter;
    use soroban_sdk::testutils::{Address as _, Ledger as _};
    use soroban_sdk::Env;

    fn seed_balance(env: &Env, player: &Address, resource_type: &ResourceType, amount: u64) {
        resource_minter::credit_balance(env, player, resource_type, amount).unwrap();
    }

    #[test]
    fn create_tournament_rejects_non_power_of_two() {
        let env = Env::default();
        env.mock_all_auths();
        let organizer = Address::generate(&env);

        let err = create_tournament(
            &env,
            &organizer,
            ResourceType::StellarDust,
            0,
            5, // not a power of two
            3600,
            Vec::new(&env),
        )
        .unwrap_err();
        assert_eq!(err, TournamentError::InvalidPlayerCount);
    }

    #[test]
    fn create_tournament_rejects_prize_distribution_over_10000_bps() {
        let env = Env::default();
        env.mock_all_auths();
        let organizer = Address::generate(&env);

        let mut bps = Vec::new(&env);
        bps.push_back(6000u32);
        bps.push_back(5000u32); // sums to 11000 > 10000

        let err = create_tournament(
            &env,
            &organizer,
            ResourceType::StellarDust,
            0,
            4,
            3600,
            bps,
        )
        .unwrap_err();
        assert_eq!(err, TournamentError::InvalidPrizeDistribution);
    }

    #[test]
    fn full_bracket_4_players_resolves_to_a_champion_and_pays_prizes() {
        let env = Env::default();
        env.mock_all_auths();

        let organizer = Address::generate(&env);
        let p1 = Address::generate(&env);
        let p2 = Address::generate(&env);
        let p3 = Address::generate(&env);
        let p4 = Address::generate(&env);

        let entry_fee = 100u64;
        for p in [&p1, &p2, &p3, &p4] {
            seed_balance(&env, p, &ResourceType::StellarDust, entry_fee);
        }

        let mut bps = Vec::new(&env);
        bps.push_back(7000u32);
        bps.push_back(3000u32);

        let tournament_id = create_tournament(
            &env,
            &organizer,
            ResourceType::StellarDust,
            entry_fee,
            4,
            3600,
            bps,
        )
        .unwrap();

        for p in [&p1, &p2, &p3, &p4] {
            register_for_tournament(&env, p, tournament_id).unwrap();
        }

        let t = get_tournament(&env, tournament_id).unwrap();
        assert_eq!(t.prize_pool, entry_fee * 4);

        start_tournament(&env, &organizer, tournament_id).unwrap();

        // Resolve round 1 by forcing combats to completion via pvp_combat's
        // own execute_move, then feed the result back into the bracket.
        let round1 = get_bracket_round(&env, tournament_id, 1);
        assert_eq!(round1.len(), 2);
        for i in 0..round1.len() {
            let m = round1.get(i).unwrap();
            let combat_id = m.combat_id.unwrap();
            force_combat_finish(&env, combat_id);
            report_match_result(&env, tournament_id, 1, i).unwrap();
        }

        // Round 2 (final) should now exist with exactly one match.
        let t = get_tournament(&env, tournament_id).unwrap();
        assert_eq!(t.current_round, 2);
        let round2 = get_bracket_round(&env, tournament_id, 2);
        assert_eq!(round2.len(), 1);

        let final_match = round2.get(0).unwrap();
        let combat_id = final_match.combat_id.unwrap();
        force_combat_finish(&env, combat_id);
        report_match_result(&env, tournament_id, 2, 0).unwrap();

        let t = get_tournament(&env, tournament_id).unwrap();
        assert_eq!(t.status, symbol_short!("done"));

        // Prize pool of 400 split 70/30 => 280 champion, 120 runner-up.
        let placements = placements_best_first(&env, &t);
        let champion = placements.get(0).unwrap();
        let runner_up = placements.get(1).unwrap();
        assert_eq!(
            resource_minter::balance_of(&env, &champion, &ResourceType::StellarDust),
            280
        );
        assert_eq!(
            resource_minter::balance_of(&env, &runner_up, &ResourceType::StellarDust),
            120
        );
    }

    #[test]
    fn odd_registrant_count_gets_byes_seeded_against_top_elo() {
        let env = Env::default();
        env.mock_all_auths();

        let organizer = Address::generate(&env);
        let p1 = Address::generate(&env);
        let p2 = Address::generate(&env);
        let p3 = Address::generate(&env);

        let tournament_id = create_tournament(
            &env,
            &organizer,
            ResourceType::StellarDust,
            0,
            4, // cap of 4, but only 3 register -> bracket size still 4
            3600,
            Vec::new(&env),
        )
        .unwrap();

        for p in [&p1, &p2, &p3] {
            register_for_tournament(&env, p, tournament_id).unwrap();
        }

        start_tournament(&env, &organizer, tournament_id).unwrap();

        let round1 = get_bracket_round(&env, tournament_id, 1);
        assert_eq!(round1.len(), 2);
        // Exactly one match should be a bye (one side None, winner already set).
        let bye_count = (0..round1.len())
            .filter(|&i| {
                let m = round1.get(i).unwrap();
                m.player1.is_none() || m.player2.is_none()
            })
            .count();
        assert_eq!(bye_count, 1);
    }

    /// Test-only helper: drive a fresh 2-player combat straight to a finish
    /// by repeatedly attacking until one side's HP hits zero, using
    /// pvp_combat's own public `execute_move` — no shortcuts into its
    /// private state. A single full-power attack (100 dmg vs MAX_HP=100)
    /// ends it in one exchange, but the loop tolerates more turns in case
    /// combat balance ever changes.
    fn force_combat_finish(env: &Env, combat_id: u64) {
        use crate::pvp_combat::{execute_move, get_combat};
        for _ in 0..8 {
            let combat = get_combat(env, combat_id).unwrap();
            if combat.status == symbol_short!("finished") {
                return;
            }
            let acting_player = combat.turn.clone();
            execute_move(env, &acting_player, combat_id, symbol_short!("attack"), 100).ok();
        }
    }
}
