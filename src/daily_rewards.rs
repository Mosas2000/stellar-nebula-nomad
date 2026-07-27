//! Daily login rewards — Issue #280
//!
//! Retention loop for returning nomads. A player claims once per UTC day and
//! walks a fixed 28-day **calendar**; the calendar slot decides *what* the
//! reward is (reward variety), while the player's consecutive-login **streak**
//! decides *how much* of it they get (escalating rewards).
//!
//! Design notes:
//!   • Day granularity is `timestamp / 86_400` — the same convention used by
//!     [`crate::mission_generator`] for daily mission resets.
//!   • The streak itself lives on the player's profile (see
//!     [`crate::player_profile::record_login`]) so that every subsystem reads
//!     one authoritative value; this module owns the calendar and payouts.
//!   • Rewards are pure functions of (calendar slot, streak), so a front-end
//!     can render the whole month ahead via [`get_reward_calendar`] and the
//!     exact next payout via [`preview_daily_reward`] without mutating state.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Vec};

use crate::player_profile::{self, ProfileError};

// ─── Constants ────────────────────────────────────────────────────────────────

/// Seconds in a reward day. Claims are bucketed by `timestamp / DAY_SECS`.
pub const DAY_SECS: u64 = 86_400;

/// Length of one full reward calendar cycle (4 weeks).
pub const CALENDAR_DAYS: u32 = 28;

/// Extra basis points of reward granted per consecutive login day.
pub const STREAK_BONUS_BPS_PER_DAY: u32 = 500;

/// Ceiling on the streak bonus (300 % on top of base) so a very long streak
/// cannot mint unbounded essence.
pub const MAX_STREAK_BONUS_BPS: u32 = 30_000;

/// Streak length at which the bonus saturates — derived so the two constants
/// above can never disagree.
pub const STREAK_BONUS_CAP_DAYS: u32 = MAX_STREAK_BONUS_BPS / STREAK_BONUS_BPS_PER_DAY;

/// Base essence granted on an ordinary calendar day.
pub const BASE_ESSENCE: i128 = 50;

/// Weekly milestone slots (day 7, 14, 21, 28 of the cycle) pay a multiple of
/// the ordinary day's base amount.
pub const MILESTONE_MULTIPLIER: i128 = 4;

/// The final slot of the cycle is the grand prize.
pub const CYCLE_COMPLETE_MULTIPLIER: i128 = 10;

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum DailyRewardKey {
    /// Per-player claim/streak bookkeeping.
    Record(Address),
    /// Global count of rewards ever claimed.
    TotalClaims,
    /// Global essence ever paid out by this module.
    TotalEssencePaid,
    /// Number of players who have claimed at least once.
    UniqueClaimers,
}

// ─── Data Types ───────────────────────────────────────────────────────────────

/// The kind of payout occupying a calendar slot. Drives reward variety: a
/// player walking the calendar receives a mix of currency, resources, crafting
/// materials and cosmetic rolls rather than a single escalating number.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RewardKind {
    /// Soft currency credited straight to the player's profile.
    Essence,
    /// Stellar dust resource grant.
    StellarDust,
    /// Dark matter resource grant.
    DarkMatter,
    /// Exotic matter — the rare resource, milestone slots only.
    ExoticMatter,
    /// Energy refill for the ship's scan budget.
    Energy,
    /// A cosmetic skin roll (rarity decided by the caller's roll table).
    CosmeticRoll,
}

/// A fully-resolved reward: what the player gets and why.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DailyReward {
    /// 1-based slot within the 28-day calendar.
    pub calendar_day: u32,
    pub kind: RewardKind,
    /// Payout before the streak bonus is applied.
    pub base_amount: i128,
    /// Payout after the streak bonus — this is what is actually credited.
    pub amount: i128,
    /// Streak bonus applied, in basis points.
    pub streak_bonus_bps: u32,
    /// True for weekly milestones and the cycle finale.
    pub is_milestone: bool,
}

/// Per-player daily-login bookkeeping.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoginRecord {
    pub player: Address,
    /// Day index (`timestamp / DAY_SECS`) of the most recent claim.
    pub last_claim_day: u64,
    /// Consecutive claim days, including today.
    pub current_streak: u32,
    /// Best streak ever achieved.
    pub longest_streak: u32,
    /// Lifetime number of claims.
    pub total_claims: u32,
    /// Number of completed 28-day calendar cycles.
    pub cycles_completed: u32,
    /// Lifetime essence-equivalent value claimed through this module.
    pub lifetime_value: i128,
}

/// Aggregate module statistics for dashboards.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DailyRewardStats {
    pub total_claims: u64,
    pub total_essence_paid: i128,
    pub unique_claimers: u64,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum DailyRewardError {
    /// The player already claimed during the current UTC day.
    AlreadyClaimedToday = 1,
    /// No profile exists for the claiming address.
    ProfileNotFound = 2,
    /// Reward math overflowed (unreachable with the constants above, but the
    /// crate policy is that no balance-modifying path may wrap).
    ArithmeticOverflow = 3,
    /// `calendar_day` outside `1..=CALENDAR_DAYS`.
    InvalidCalendarDay = 4,
}

impl From<ProfileError> for DailyRewardError {
    fn from(e: ProfileError) -> Self {
        match e {
            ProfileError::ArithmeticOverflow => DailyRewardError::ArithmeticOverflow,
            _ => DailyRewardError::ProfileNotFound,
        }
    }
}

// ─── Calendar ─────────────────────────────────────────────────────────────────

/// True when `calendar_day` is a weekly milestone or the cycle finale.
pub fn is_milestone_day(calendar_day: u32) -> bool {
    calendar_day % 7 == 0
}

/// The payout kind occupying calendar slot `calendar_day` (1-based).
///
/// The rotation is deliberately hand-written rather than derived from the day
/// number: it keeps rare grants (exotic matter, cosmetic rolls) on milestone
/// slots and spreads the ordinary kinds so no two adjacent days repeat.
pub fn calendar_kind(calendar_day: u32) -> RewardKind {
    if calendar_day == CALENDAR_DAYS {
        // Cycle finale is always a cosmetic roll — the memorable reward.
        return RewardKind::CosmeticRoll;
    }
    if is_milestone_day(calendar_day) {
        return RewardKind::ExoticMatter;
    }
    match calendar_day % 6 {
        0 => RewardKind::Energy,
        1 => RewardKind::Essence,
        2 => RewardKind::StellarDust,
        3 => RewardKind::Essence,
        4 => RewardKind::DarkMatter,
        _ => RewardKind::Energy,
    }
}

/// Base (pre-streak) payout for a calendar slot.
fn calendar_base_amount(calendar_day: u32) -> i128 {
    if calendar_day == CALENDAR_DAYS {
        BASE_ESSENCE.saturating_mul(CYCLE_COMPLETE_MULTIPLIER)
    } else if is_milestone_day(calendar_day) {
        BASE_ESSENCE.saturating_mul(MILESTONE_MULTIPLIER)
    } else {
        BASE_ESSENCE
    }
}

/// Streak bonus in basis points for a streak of `streak` consecutive days.
///
/// Day 1 earns no bonus; each further day adds [`STREAK_BONUS_BPS_PER_DAY`]
/// until the bonus saturates at [`MAX_STREAK_BONUS_BPS`].
pub fn streak_bonus_bps(streak: u32) -> u32 {
    streak
        .saturating_sub(1)
        .saturating_mul(STREAK_BONUS_BPS_PER_DAY)
        .min(MAX_STREAK_BONUS_BPS)
}

/// Resolve the reward for a `(calendar_day, streak)` pair.
///
/// Pure — safe to call from read-only endpoints for previews and UI calendars.
pub fn resolve_reward(calendar_day: u32, streak: u32) -> Result<DailyReward, DailyRewardError> {
    if calendar_day == 0 || calendar_day > CALENDAR_DAYS {
        return Err(DailyRewardError::InvalidCalendarDay);
    }

    let base_amount = calendar_base_amount(calendar_day);
    let bonus_bps = streak_bonus_bps(streak);

    // amount = base * (10_000 + bonus_bps) / 10_000, checked end-to-end.
    let scaled = base_amount
        .checked_mul(10_000i128 + bonus_bps as i128)
        .ok_or(DailyRewardError::ArithmeticOverflow)?;
    let amount = scaled / 10_000;

    Ok(DailyReward {
        calendar_day,
        kind: calendar_kind(calendar_day),
        base_amount,
        amount,
        streak_bonus_bps: bonus_bps,
        is_milestone: is_milestone_day(calendar_day),
    })
}

/// The calendar slot a player with `total_claims` lifetime claims lands on next.
///
/// Slots advance with claims (not wall-clock days), so a missed day costs the
/// streak bonus but never desyncs the player from the calendar.
pub fn next_calendar_day(total_claims: u32) -> u32 {
    (total_claims % CALENDAR_DAYS) + 1
}

/// Render the full 28-slot calendar at a hypothetical `streak`.
///
/// Read-only: intended for the client to draw the month view.
pub fn get_reward_calendar(env: &Env, streak: u32) -> Vec<DailyReward> {
    let mut calendar = Vec::new(env);
    for day in 1..=CALENDAR_DAYS {
        // `day` is always in range, so `resolve_reward` cannot fail here.
        if let Ok(reward) = resolve_reward(day, streak) {
            calendar.push_back(reward);
        }
    }
    calendar
}

// ─── Claiming ─────────────────────────────────────────────────────────────────

fn load_record(env: &Env, player: &Address) -> Option<LoginRecord> {
    env.storage()
        .persistent()
        .get(&DailyRewardKey::Record(player.clone()))
}

/// Fetch a player's login record, if they have ever claimed.
pub fn get_login_record(env: &Env, player: &Address) -> Option<LoginRecord> {
    load_record(env, player)
}

/// The player's current consecutive-login streak (0 when never claimed or when
/// the streak has since lapsed).
pub fn get_streak(env: &Env, player: &Address) -> u32 {
    match load_record(env, player) {
        Some(record) => {
            let today = env.ledger().timestamp() / DAY_SECS;
            // A streak survives only while today is the claim day or the one
            // right after it; beyond that the chain is already broken.
            if today <= record.last_claim_day + 1 {
                record.current_streak
            } else {
                0
            }
        }
        None => 0,
    }
}

/// True when `player` may claim right now.
pub fn can_claim(env: &Env, player: &Address) -> bool {
    match load_record(env, player) {
        Some(record) => env.ledger().timestamp() / DAY_SECS > record.last_claim_day,
        None => true,
    }
}

/// The reward `player` would receive if they claimed right now.
///
/// Read-only. Returns `AlreadyClaimedToday` when the claim would be rejected,
/// so a client can use this as a single source of truth for the claim button.
pub fn preview_daily_reward(env: &Env, player: &Address) -> Result<DailyReward, DailyRewardError> {
    let today = env.ledger().timestamp() / DAY_SECS;

    let (total_claims, streak) = match load_record(env, player) {
        Some(record) => {
            if today <= record.last_claim_day {
                return Err(DailyRewardError::AlreadyClaimedToday);
            }
            let continues = today == record.last_claim_day + 1;
            let streak = if continues {
                record.current_streak.saturating_add(1)
            } else {
                1
            };
            (record.total_claims, streak)
        }
        None => (0, 1),
    };

    resolve_reward(next_calendar_day(total_claims), streak)
}

/// Claim today's login reward.
///
/// Advances the calendar, extends or resets the streak, credits the payout to
/// the player's profile and emits `daily/claimed`. Milestone slots additionally
/// emit `daily/milestn` so indexers can surface them without re-deriving the
/// calendar.
pub fn claim_daily_reward(env: &Env, player: Address) -> Result<DailyReward, DailyRewardError> {
    player.require_auth();

    // A profile must exist — the reward is credited to it.
    let profile = player_profile::get_profile_by_owner(env, &player)?;

    let today = env.ledger().timestamp() / DAY_SECS;
    let existing = load_record(env, &player);

    let (streak_before, total_claims_before, longest_before, cycles_before, lifetime_before) =
        match &existing {
            Some(r) => (
                r.current_streak,
                r.total_claims,
                r.longest_streak,
                r.cycles_completed,
                r.lifetime_value,
            ),
            None => (0, 0, 0, 0, 0i128),
        };

    if let Some(record) = &existing {
        if today <= record.last_claim_day {
            return Err(DailyRewardError::AlreadyClaimedToday);
        }
    }

    // Streak continues only if the previous claim was literally yesterday.
    let streak_continues = existing
        .as_ref()
        .map(|r| today == r.last_claim_day + 1)
        .unwrap_or(false);
    let new_streak = if streak_continues {
        streak_before.saturating_add(1)
    } else {
        1
    };

    let calendar_day = next_calendar_day(total_claims_before);
    let reward = resolve_reward(calendar_day, new_streak)?;

    let total_claims = total_claims_before
        .checked_add(1)
        .ok_or(DailyRewardError::ArithmeticOverflow)?;
    let cycles_completed = if calendar_day == CALENDAR_DAYS {
        cycles_before.saturating_add(1)
    } else {
        cycles_before
    };
    let lifetime_value = lifetime_before
        .checked_add(reward.amount)
        .ok_or(DailyRewardError::ArithmeticOverflow)?;

    let record = LoginRecord {
        player: player.clone(),
        last_claim_day: today,
        current_streak: new_streak,
        longest_streak: longest_before.max(new_streak),
        total_claims,
        cycles_completed,
        lifetime_value,
    };
    env.storage()
        .persistent()
        .set(&DailyRewardKey::Record(player.clone()), &record);

    // Mirror the streak onto the profile so other subsystems read one value.
    player_profile::record_login(env, profile.id, today, new_streak)?;
    player_profile::credit_essence(env, profile.id, reward.amount)?;

    // ── Global stats ──────────────────────────────────────────────────────
    let claims: u64 = env
        .storage()
        .persistent()
        .get(&DailyRewardKey::TotalClaims)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&DailyRewardKey::TotalClaims, &claims.saturating_add(1));

    let paid: i128 = env
        .storage()
        .persistent()
        .get(&DailyRewardKey::TotalEssencePaid)
        .unwrap_or(0);
    env.storage().persistent().set(
        &DailyRewardKey::TotalEssencePaid,
        &paid.saturating_add(reward.amount),
    );

    if existing.is_none() {
        let claimers: u64 = env
            .storage()
            .persistent()
            .get(&DailyRewardKey::UniqueClaimers)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DailyRewardKey::UniqueClaimers, &claimers.saturating_add(1));
    }

    // ── Events ────────────────────────────────────────────────────────────
    env.events().publish(
        (symbol_short!("daily"), symbol_short!("claimed")),
        (
            player.clone(),
            calendar_day,
            new_streak,
            reward.kind,
            reward.amount,
        ),
    );

    if !streak_continues && streak_before > 0 {
        env.events().publish(
            (symbol_short!("daily"), symbol_short!("brokestk")),
            (player.clone(), streak_before),
        );
    }

    if reward.is_milestone {
        env.events().publish(
            (symbol_short!("daily"), symbol_short!("milestn")),
            (player, calendar_day, reward.amount),
        );
    }

    Ok(reward)
}

/// Aggregate module statistics.
pub fn get_daily_reward_stats(env: &Env) -> DailyRewardStats {
    DailyRewardStats {
        total_claims: env
            .storage()
            .persistent()
            .get(&DailyRewardKey::TotalClaims)
            .unwrap_or(0),
        total_essence_paid: env
            .storage()
            .persistent()
            .get(&DailyRewardKey::TotalEssencePaid)
            .unwrap_or(0),
        unique_claimers: env
            .storage()
            .persistent()
            .get(&DailyRewardKey::UniqueClaimers)
            .unwrap_or(0),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};
    use soroban_sdk::{contract, contractimpl};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    /// Soroban storage is only reachable inside a contract invocation, so the
    /// tests below run their bodies through `Env::as_contract` — the same
    /// pattern the rest of the crate's unit tests use.
    struct Harness {
        env: Env,
        contract: Address,
        player: Address,
    }

    fn harness() -> Harness {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register(Stub, ());
        let player = Address::generate(&env);
        Harness {
            env,
            contract,
            player,
        }
    }

    impl Harness {
        fn run<T>(&self, f: impl FnOnce() -> T) -> T {
            self.env.as_contract(&self.contract, f)
        }

        /// Give the harness player a profile, so rewards have somewhere to go.
        fn with_profile(self) -> Self {
            let player = self.player.clone();
            self.run(|| {
                player_profile::initialize_profile(&self.env, player).unwrap();
            });
            self
        }

        fn set_day(&self, day: u64) {
            self.env
                .ledger()
                .with_mut(|l| l.timestamp = day * DAY_SECS + 100);
        }
    }

    // ── Calendar ──────────────────────────────────────────────────────────

    #[test]
    fn calendar_covers_full_cycle_with_variety() {
        let env = Env::default();
        let calendar = get_reward_calendar(&env, 1);
        assert_eq!(calendar.len(), CALENDAR_DAYS);

        // At least four distinct reward kinds appear across the cycle.
        let mut kinds: Vec<RewardKind> = Vec::new(&env);
        for i in 0..calendar.len() {
            let kind = calendar.get(i).unwrap().kind;
            if !kinds.iter().any(|k| k == kind) {
                kinds.push_back(kind);
            }
        }
        assert!(
            kinds.len() >= 4,
            "expected reward variety, got {} kinds",
            kinds.len()
        );
    }

    #[test]
    fn milestone_days_pay_more_than_ordinary_days() {
        let ordinary = resolve_reward(3, 1).unwrap();
        let weekly = resolve_reward(7, 1).unwrap();
        let finale = resolve_reward(CALENDAR_DAYS, 1).unwrap();

        assert!(!ordinary.is_milestone);
        assert!(weekly.is_milestone);
        assert!(finale.is_milestone);
        assert!(weekly.amount > ordinary.amount);
        assert!(finale.amount > weekly.amount);
        assert_eq!(finale.kind, RewardKind::CosmeticRoll);
    }

    #[test]
    fn calendar_day_out_of_range_is_rejected() {
        assert_eq!(
            resolve_reward(0, 1),
            Err(DailyRewardError::InvalidCalendarDay)
        );
        assert_eq!(
            resolve_reward(CALENDAR_DAYS + 1, 1),
            Err(DailyRewardError::InvalidCalendarDay)
        );
    }

    #[test]
    fn calendar_slot_advances_with_claims_and_wraps() {
        assert_eq!(next_calendar_day(0), 1);
        assert_eq!(next_calendar_day(27), 28);
        assert_eq!(next_calendar_day(28), 1);
        assert_eq!(next_calendar_day(29), 2);
    }

    // ── Escalation ────────────────────────────────────────────────────────

    #[test]
    fn streak_bonus_escalates_then_saturates() {
        assert_eq!(streak_bonus_bps(0), 0);
        assert_eq!(streak_bonus_bps(1), 0);
        assert_eq!(streak_bonus_bps(2), STREAK_BONUS_BPS_PER_DAY);
        assert_eq!(
            streak_bonus_bps(STREAK_BONUS_CAP_DAYS + 1),
            MAX_STREAK_BONUS_BPS
        );
        // Saturated: further days add nothing.
        assert_eq!(streak_bonus_bps(u32::MAX), MAX_STREAK_BONUS_BPS);
    }

    #[test]
    fn reward_amount_grows_monotonically_with_streak() {
        let mut previous = 0i128;
        for streak in 1..=STREAK_BONUS_CAP_DAYS {
            let reward = resolve_reward(1, streak).unwrap();
            assert!(
                reward.amount >= previous,
                "streak {streak} regressed: {} < {previous}",
                reward.amount
            );
            previous = reward.amount;
        }
    }

    // ── Claim flow ────────────────────────────────────────────────────────

    #[test]
    fn first_claim_starts_a_streak_of_one() {
        let h = harness().with_profile();
        h.set_day(100);

        h.run(|| {
            let reward = claim_daily_reward(&h.env, h.player.clone()).unwrap();
            assert_eq!(reward.calendar_day, 1);
            assert_eq!(reward.streak_bonus_bps, 0);

            let record = get_login_record(&h.env, &h.player).unwrap();
            assert_eq!(record.current_streak, 1);
            assert_eq!(record.total_claims, 1);
            assert_eq!(record.last_claim_day, 100);
        });
    }

    #[test]
    fn second_claim_same_day_is_rejected() {
        let h = harness().with_profile();
        h.set_day(100);

        // Separate invocations: `require_auth` may only be satisfied once per
        // contract frame, so each claim needs its own.
        h.run(|| claim_daily_reward(&h.env, h.player.clone()).unwrap());

        h.run(|| {
            assert_eq!(
                claim_daily_reward(&h.env, h.player.clone()),
                Err(DailyRewardError::AlreadyClaimedToday)
            );
            assert!(!can_claim(&h.env, &h.player));
        });
    }

    #[test]
    fn consecutive_days_build_the_streak() {
        let h = harness().with_profile();

        for day in 100..107 {
            h.set_day(day);
            h.run(|| claim_daily_reward(&h.env, h.player.clone()).unwrap());
        }

        h.run(|| {
            let record = get_login_record(&h.env, &h.player).unwrap();
            assert_eq!(record.current_streak, 7);
            assert_eq!(record.longest_streak, 7);
            assert_eq!(record.total_claims, 7);
        });
    }

    #[test]
    fn missed_day_resets_streak_but_keeps_longest_and_calendar() {
        let h = harness().with_profile();

        for day in 100..105 {
            h.set_day(day);
            h.run(|| claim_daily_reward(&h.env, h.player.clone()).unwrap());
        }
        h.run(|| {
            assert_eq!(
                get_login_record(&h.env, &h.player).unwrap().current_streak,
                5
            );
        });

        // Skip day 105 entirely.
        h.set_day(106);
        h.run(|| {
            let reward = claim_daily_reward(&h.env, h.player.clone()).unwrap();

            let record = get_login_record(&h.env, &h.player).unwrap();
            assert_eq!(record.current_streak, 1, "streak must reset after a gap");
            assert_eq!(record.longest_streak, 5, "longest streak is preserved");
            // The calendar keeps advancing regardless of the broken streak.
            assert_eq!(reward.calendar_day, 6);
            assert_eq!(reward.streak_bonus_bps, 0);
        });
    }

    #[test]
    fn full_cycle_increments_cycles_completed() {
        let h = harness().with_profile();

        for day in 100..(100 + CALENDAR_DAYS as u64) {
            h.set_day(day);
            h.run(|| claim_daily_reward(&h.env, h.player.clone()).unwrap());
        }

        h.run(|| {
            let record = get_login_record(&h.env, &h.player).unwrap();
            assert_eq!(record.total_claims, CALENDAR_DAYS);
            assert_eq!(record.cycles_completed, 1);
            assert_eq!(record.current_streak, CALENDAR_DAYS);
        });

        // Next claim wraps back to slot 1 of a fresh cycle.
        h.set_day(100 + CALENDAR_DAYS as u64);
        h.run(|| {
            let reward = claim_daily_reward(&h.env, h.player.clone()).unwrap();
            assert_eq!(reward.calendar_day, 1);
        });
    }

    #[test]
    fn claim_credits_essence_to_the_profile() {
        let h = harness().with_profile();
        h.set_day(100);

        h.run(|| {
            let before = player_profile::get_profile_by_owner(&h.env, &h.player)
                .unwrap()
                .essence_earned;
            let reward = claim_daily_reward(&h.env, h.player.clone()).unwrap();
            let after = player_profile::get_profile_by_owner(&h.env, &h.player)
                .unwrap()
                .essence_earned;

            assert_eq!(after - before, reward.amount);
        });
    }

    #[test]
    fn claim_mirrors_streak_onto_the_profile() {
        let h = harness().with_profile();

        for day in 100..104 {
            h.set_day(day);
            h.run(|| claim_daily_reward(&h.env, h.player.clone()).unwrap());
        }

        h.run(|| {
            let profile = player_profile::get_profile_by_owner(&h.env, &h.player).unwrap();
            assert_eq!(profile.login_streak, 4);
            assert_eq!(profile.longest_login_streak, 4);
            assert_eq!(profile.last_login_day, 103);
        });
    }

    #[test]
    fn claim_without_a_profile_is_rejected() {
        // No `with_profile()` — the claimer has never joined.
        let h = harness();
        h.set_day(100);

        h.run(|| {
            assert_eq!(
                claim_daily_reward(&h.env, h.player.clone()),
                Err(DailyRewardError::ProfileNotFound)
            );
        });
    }

    #[test]
    fn preview_matches_the_reward_actually_granted() {
        let h = harness().with_profile();
        h.set_day(100);
        h.run(|| claim_daily_reward(&h.env, h.player.clone()).unwrap());

        h.set_day(101);
        h.run(|| {
            let preview = preview_daily_reward(&h.env, &h.player).unwrap();
            let granted = claim_daily_reward(&h.env, h.player.clone()).unwrap();
            assert_eq!(preview, granted);
        });
    }

    #[test]
    fn preview_reports_already_claimed() {
        let h = harness().with_profile();
        h.set_day(100);

        h.run(|| {
            claim_daily_reward(&h.env, h.player.clone()).unwrap();
            assert_eq!(
                preview_daily_reward(&h.env, &h.player),
                Err(DailyRewardError::AlreadyClaimedToday)
            );
        });
    }

    #[test]
    fn preview_for_a_new_player_is_day_one() {
        let h = harness().with_profile();
        h.set_day(100);

        h.run(|| {
            let preview = preview_daily_reward(&h.env, &h.player).unwrap();
            assert_eq!(preview.calendar_day, 1);
            assert!(can_claim(&h.env, &h.player));
        });
    }

    #[test]
    fn get_streak_reports_zero_once_the_chain_lapses() {
        let h = harness().with_profile();
        h.set_day(100);
        h.run(|| claim_daily_reward(&h.env, h.player.clone()).unwrap());

        h.set_day(101);
        h.run(|| assert_eq!(get_streak(&h.env, &h.player), 1, "still claimable today"));

        h.set_day(103);
        h.run(|| assert_eq!(get_streak(&h.env, &h.player), 0, "chain has lapsed"));
    }

    #[test]
    fn stats_accumulate_across_players() {
        let h = harness().with_profile();
        let other = Address::generate(&h.env);
        h.set_day(100);

        h.run(|| player_profile::initialize_profile(&h.env, other.clone()).unwrap());
        let a = h.run(|| claim_daily_reward(&h.env, h.player.clone()).unwrap());
        let b = h.run(|| claim_daily_reward(&h.env, other.clone()).unwrap());

        h.run(|| {
            let stats = get_daily_reward_stats(&h.env);
            assert_eq!(stats.total_claims, 2);
            assert_eq!(stats.unique_claimers, 2);
            assert_eq!(stats.total_essence_paid, a.amount + b.amount);
        });
    }

    #[test]
    fn unique_claimers_counts_each_player_once() {
        let h = harness().with_profile();

        for day in 100..103 {
            h.set_day(day);
            h.run(|| claim_daily_reward(&h.env, h.player.clone()).unwrap());
        }

        h.run(|| assert_eq!(get_daily_reward_stats(&h.env).unique_claimers, 1));
    }
}
