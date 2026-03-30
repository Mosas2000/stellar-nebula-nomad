use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Vec};

/// One week in seconds.
pub const WEEK_SECONDS: u64 = 604_800;

/// Maximum positions paid out in a single distribution call.
pub const MAX_PAYOUT_POSITIONS: u32 = 50;

// ─── Storage Keys ──────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum PrizeKey {
    /// Current prize pool balance.
    Pool,
    /// Admin / pool operator address.
    Admin,
    /// Ledger timestamp of the last weekly reset.
    LastReset,
    /// Total prizes distributed all-time.
    TotalDistributed,
    /// Snapshot entry: maps rank (1-based) to winner address.
    Snapshot(u32),
    /// Number of entries in the current snapshot.
    SnapshotSize,
}

// ─── Errors ─────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PrizeError {
    /// Prize pool has insufficient funds for the requested distribution.
    InsufficientPrizePool = 1,
    /// Caller is not authorized to perform this action.
    NotAuthorized = 2,
    /// Requested top-N exceeds the maximum payout positions.
    TooManyPositions = 3,
    /// No leaderboard snapshot is available.
    NoSnapshot = 4,
    /// Amount must be positive.
    InvalidAmount = 5,
    /// Snapshot rank out of range.
    InvalidRank = 6,
}

// ─── Data Types ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct PrizeRecord {
    pub rank: u32,
    pub winner: Address,
    pub amount: i128,
    pub distributed_at: u64,
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Compute the prize share for rank `rank` out of `top_n`.
///
/// Distribution uses a descending linear weight:
///   weight(i) = top_n + 1 − i   (rank is 1-based)
///   total_weight = top_n * (top_n + 1) / 2
///   share(i) = pool * weight(i) / total_weight
fn prize_share(pool: i128, rank: u32, top_n: u32) -> i128 {
    let weight = (top_n + 1 - rank) as i128;
    let total_weight = (top_n as i128 * (top_n as i128 + 1)) / 2;
    pool * weight / total_weight
}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Initialize the prize distributor with an admin address.
pub fn initialize_prize_distributor(env: &Env, admin: &Address) {
    admin.require_auth();
    env.storage().instance().set(&PrizeKey::Admin, admin);
    env.storage().instance().set(&PrizeKey::Pool, &0i128);
    env.storage()
        .instance()
        .set(&PrizeKey::LastReset, &env.ledger().timestamp());
    env.storage()
        .instance()
        .set(&PrizeKey::TotalDistributed, &0i128);
    env.storage()
        .instance()
        .set(&PrizeKey::SnapshotSize, &0u32);
}

/// Add `amount` tokens to the weekly prize pool. Anyone may fund the pool
/// (enables sponsor-funded pools).
pub fn fund_prize_pool(env: &Env, funder: &Address, amount: i128) -> Result<i128, PrizeError> {
    funder.require_auth();
    if amount <= 0 {
        return Err(PrizeError::InvalidAmount);
    }

    let current: i128 = env
        .storage()
        .instance()
        .get(&PrizeKey::Pool)
        .unwrap_or(0i128);
    let new_total = current + amount;
    env.storage().instance().set(&PrizeKey::Pool, &new_total);

    env.events().publish(
        (symbol_short!("prize"), symbol_short!("funded")),
        (funder.clone(), amount, new_total),
    );

    Ok(new_total)
}

/// Record a leaderboard snapshot (admin-only).
///
/// `winners` is ordered from rank 1 (highest) to rank N.
/// Replaces any existing snapshot for the current week.
pub fn submit_leaderboard_snapshot(
    env: &Env,
    admin: &Address,
    winners: &Vec<Address>,
) -> Result<u32, PrizeError> {
    admin.require_auth();
    let stored_admin: Address = env
        .storage()
        .instance()
        .get(&PrizeKey::Admin)
        .ok_or(PrizeError::NotAuthorized)?;
    if stored_admin != *admin {
        return Err(PrizeError::NotAuthorized);
    }

    let size = winners.len();
    for i in 0..size {
        let rank = i + 1;
        env.storage()
            .persistent()
            .set(&PrizeKey::Snapshot(rank), &winners.get(i).unwrap());
    }
    env.storage()
        .instance()
        .set(&PrizeKey::SnapshotSize, &size);

    Ok(size)
}

/// Distribute weekly prizes to the top `top_n` leaderboard positions.
///
/// - Reads the stored snapshot to determine winners.
/// - Uses verifiable linear-weight math to split the pool.
/// - Emits a `PrizeDistributed` event per payout.
/// - Resets the pool and records the distribution timestamp.
/// - Caps at [`MAX_PAYOUT_POSITIONS`] to fit within a single transaction.
pub fn distribute_weekly_prizes(
    env: &Env,
    caller: &Address,
    top_n: u32,
) -> Result<Vec<PrizeRecord>, PrizeError> {
    caller.require_auth();

    // Only admin may trigger distribution.
    let admin: Address = env
        .storage()
        .instance()
        .get(&PrizeKey::Admin)
        .ok_or(PrizeError::NotAuthorized)?;
    if admin != *caller {
        return Err(PrizeError::NotAuthorized);
    }

    if top_n == 0 || top_n > MAX_PAYOUT_POSITIONS {
        return Err(PrizeError::TooManyPositions);
    }

    let snapshot_size: u32 = env
        .storage()
        .instance()
        .get(&PrizeKey::SnapshotSize)
        .unwrap_or(0);
    if snapshot_size == 0 {
        return Err(PrizeError::NoSnapshot);
    }

    let effective_n = top_n.min(snapshot_size);

    let pool: i128 = env
        .storage()
        .instance()
        .get(&PrizeKey::Pool)
        .unwrap_or(0);

    // Verify pool is large enough (at least 1 unit per position).
    if pool < effective_n as i128 {
        return Err(PrizeError::InsufficientPrizePool);
    }

    let now = env.ledger().timestamp();
    let mut records: Vec<PrizeRecord> = Vec::new(env);
    let mut total_paid: i128 = 0;

    for rank in 1..=effective_n {
        let winner: Address = env
            .storage()
            .persistent()
            .get(&PrizeKey::Snapshot(rank))
            .ok_or(PrizeError::InvalidRank)?;

        let amount = prize_share(pool, rank, effective_n);
        total_paid += amount;

        let record = PrizeRecord {
            rank,
            winner: winner.clone(),
            amount,
            distributed_at: now,
        };

        env.events().publish(
            (symbol_short!("prize"), symbol_short!("paid")),
            (winner, amount, rank, now),
        );

        records.push_back(record);
    }

    // Update pool balance (deduct exactly what was paid).
    let remaining = pool - total_paid;
    env.storage().instance().set(&PrizeKey::Pool, &remaining);

    // Accumulate total distributed.
    let prev_total: i128 = env
        .storage()
        .instance()
        .get(&PrizeKey::TotalDistributed)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&PrizeKey::TotalDistributed, &(prev_total + total_paid));

    // Record weekly reset timestamp.
    env.storage()
        .instance()
        .set(&PrizeKey::LastReset, &now);

    // Clear snapshot.
    env.storage()
        .instance()
        .set(&PrizeKey::SnapshotSize, &0u32);

    Ok(records)
}

/// Return the current prize pool balance.
pub fn get_prize_pool(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&PrizeKey::Pool)
        .unwrap_or(0)
}

/// Return the cumulative total distributed since contract init.
pub fn get_total_distributed(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&PrizeKey::TotalDistributed)
        .unwrap_or(0)
}

/// Return the timestamp of the last weekly reset.
pub fn get_last_reset(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&PrizeKey::LastReset)
        .unwrap_or(0)
}
