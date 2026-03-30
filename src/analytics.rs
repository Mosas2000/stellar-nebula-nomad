use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Vec};

// ── Error ─────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AnalyticsError {
    /// top_n must be in the range 1..=50.
    InvalidTopN = 1,
}

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum AnalyticsDataKey {
    /// Single-key global counters (total_scans, ships_minted, etc.).
    GlobalStats,
    /// Cumulative cosmic essence earned by a player through scanning.
    PlayerEssence(Address),
    /// Ordered list of known players (capped at MAX_PLAYERS).
    PlayerList,
}

// ── Data Types ────────────────────────────────────────────────────────────────

/// Aggregate on-chain statistics for community dashboards.
#[contracttype]
#[derive(Clone, Default)]
pub struct GlobalStats {
    pub total_scans: u64,
    pub ships_minted: u64,
    pub total_essence_accrued: u64,
}

/// One entry in a leaderboard snapshot, sorted descending by cosmic_essence.
#[contracttype]
#[derive(Clone, Debug)]
pub struct LeaderboardEntry {
    pub player: Address,
    pub cosmic_essence: u64,
}

/// Maximum number of players tracked in the leaderboard — supports top-50
/// without requiring unbounded on-chain iteration.
pub const MAX_PLAYERS: u32 = 50;

// ── Write Helpers (called on every harvest / scan) ────────────────────────────

/// Increment the global scan counter and record essence earned by `player`.
///
/// This is the primary hook called from `scan_nebula`.  It is free to call
/// from any internal path — no auth required (the caller's auth was already
/// enforced upstream).
pub fn record_scan(env: &Env, player: &Address, essence: u64) {
    // Update global counters.
    let mut stats: GlobalStats = env
        .storage()
        .persistent()
        .get(&AnalyticsDataKey::GlobalStats)
        .unwrap_or_default();
    stats.total_scans += 1;
    stats.total_essence_accrued += essence;
    env.storage()
        .persistent()
        .set(&AnalyticsDataKey::GlobalStats, &stats);

    // Update per-player cumulative essence.
    let prev: u64 = env
        .storage()
        .persistent()
        .get(&AnalyticsDataKey::PlayerEssence(player.clone()))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&AnalyticsDataKey::PlayerEssence(player.clone()), &(prev + essence));

    // Ensure the player appears in the leaderboard candidate list.
    register_player(env, player);
}

/// Increment the global ship-minted counter.
///
/// Call this from the ship minting path.
pub fn record_ship_minted(env: &Env) {
    let mut stats: GlobalStats = env
        .storage()
        .persistent()
        .get(&AnalyticsDataKey::GlobalStats)
        .unwrap_or_default();
    stats.ships_minted += 1;
    env.storage()
        .persistent()
        .set(&AnalyticsDataKey::GlobalStats, &stats);
}

/// Add `player` to the leaderboard candidate list if not already present.
///
/// The list is capped at `MAX_PLAYERS` (50) to bound on-chain iteration.
fn register_player(env: &Env, player: &Address) {
    let mut list: Vec<Address> = env
        .storage()
        .persistent()
        .get(&AnalyticsDataKey::PlayerList)
        .unwrap_or_else(|| Vec::new(env));

    // At capacity — no new entrants until an eviction strategy is added.
    if list.len() >= MAX_PLAYERS {
        return;
    }

    // Deduplicate: skip if already tracked.
    for i in 0..list.len() {
        if let Some(p) = list.get(i) {
            if p == *player {
                return;
            }
        }
    }

    list.push_back(player.clone());
    env.storage()
        .persistent()
        .set(&AnalyticsDataKey::PlayerList, &list);
}

// ── Read-Only View Functions (zero state mutation) ────────────────────────────

/// Return the current global statistics aggregate.
///
/// Pure read — no ledger writes.
pub fn get_global_stats(env: &Env) -> GlobalStats {
    env.storage()
        .persistent()
        .get(&AnalyticsDataKey::GlobalStats)
        .unwrap_or_default()
}

/// Build, sort, and return the top-`top_n` leaderboard entries.
///
/// Players are ranked by cumulative cosmic essence earned from scanning.
/// Returns `Err(InvalidTopN)` if `top_n` is 0 or greater than 50.
///
/// Emits a `LeaderboardSnapshot` event so frontends can subscribe via
/// Stellar event streams without additional RPC calls.
pub fn snapshot_leaderboard(
    env: &Env,
    top_n: u32,
) -> Result<Vec<LeaderboardEntry>, AnalyticsError> {
    if top_n == 0 || top_n > MAX_PLAYERS {
        return Err(AnalyticsError::InvalidTopN);
    }

    let players: Vec<Address> = env
        .storage()
        .persistent()
        .get(&AnalyticsDataKey::PlayerList)
        .unwrap_or_else(|| Vec::new(env));

    let total = players.len();

    // Build the unsorted candidate Vec (at most MAX_PLAYERS = 50 entries).
    let mut entries: Vec<LeaderboardEntry> = Vec::new(env);
    for i in 0..total {
        if let Some(player) = players.get(i) {
            let essence: u64 = env
                .storage()
                .persistent()
                .get(&AnalyticsDataKey::PlayerEssence(player.clone()))
                .unwrap_or(0);
            entries.push_back(LeaderboardEntry {
                player,
                cosmic_essence: essence,
            });
        }
    }

    // Selection sort descending by cosmic_essence.
    // O(n²) is acceptable: n ≤ MAX_PLAYERS (50).
    let mut used = [false; 50];
    let mut sorted: Vec<LeaderboardEntry> = Vec::new(env);
    let limit = top_n.min(entries.len());

    for _ in 0..limit {
        let mut best_idx: u32 = 0;
        let mut best_essence: u64 = 0;
        let mut found = false;

        for i in 0..entries.len() {
            if !used[i as usize] {
                if let Some(e) = entries.get(i) {
                    if !found || e.cosmic_essence > best_essence {
                        best_essence = e.cosmic_essence;
                        best_idx = i;
                        found = true;
                    }
                }
            }
        }

        if found {
            used[best_idx as usize] = true;
            if let Some(e) = entries.get(best_idx) {
                sorted.push_back(e);
            }
        }
    }

    emit_leaderboard_snapshot(env, &sorted);
    Ok(sorted)
}

/// Emit a `LeaderboardSnapshot` event for frontend / indexer consumption.
pub fn emit_leaderboard_snapshot(env: &Env, entries: &Vec<LeaderboardEntry>) {
    env.events().publish(
        (symbol_short!("analytics"), symbol_short!("lb_snap")),
        (entries.clone(), env.ledger().timestamp()),
    );
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    // Helper: a minimal contract shell used only to activate contract storage.
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
    fn test_global_stats_zero_on_empty() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let stats = get_global_stats(&env);
            assert_eq!(stats.total_scans, 0);
            assert_eq!(stats.ships_minted, 0);
            assert_eq!(stats.total_essence_accrued, 0);
        });
    }

    #[test]
    fn test_record_scan_increments_counters() {
        let (env, contract_id) = make_env();
        let player = Address::generate(&env);

        env.as_contract(&contract_id, || {
            record_scan(&env, &player, 100);
            record_scan(&env, &player, 50);

            let stats = get_global_stats(&env);
            assert_eq!(stats.total_scans, 2);
            assert_eq!(stats.total_essence_accrued, 150);

            let essence: u64 = env
                .storage()
                .persistent()
                .get(&AnalyticsDataKey::PlayerEssence(player.clone()))
                .unwrap_or(0);
            assert_eq!(essence, 150);
        });
    }

    #[test]
    fn test_record_ship_minted() {
        let (env, contract_id) = make_env();

        env.as_contract(&contract_id, || {
            record_ship_minted(&env);
            record_ship_minted(&env);

            let stats = get_global_stats(&env);
            assert_eq!(stats.ships_minted, 2);
            assert_eq!(stats.total_scans, 0);
        });
    }

    #[test]
    fn test_leaderboard_sorted_descending() {
        let (env, contract_id) = make_env();
        let p1 = Address::generate(&env);
        let p2 = Address::generate(&env);
        let p3 = Address::generate(&env);

        env.as_contract(&contract_id, || {
            record_scan(&env, &p1, 30);
            record_scan(&env, &p2, 100);
            record_scan(&env, &p3, 60);

            let result = snapshot_leaderboard(&env, 3).unwrap();
            assert_eq!(result.len(), 3);
            // First entry must have the highest essence.
            assert_eq!(result.get(0).unwrap().cosmic_essence, 100);
            assert_eq!(result.get(1).unwrap().cosmic_essence, 60);
            assert_eq!(result.get(2).unwrap().cosmic_essence, 30);
        });
    }

    #[test]
    fn test_leaderboard_top_n_respected() {
        let (env, contract_id) = make_env();
        let p1 = Address::generate(&env);
        let p2 = Address::generate(&env);
        let p3 = Address::generate(&env);

        env.as_contract(&contract_id, || {
            record_scan(&env, &p1, 10);
            record_scan(&env, &p2, 20);
            record_scan(&env, &p3, 30);

            let result = snapshot_leaderboard(&env, 2).unwrap();
            assert_eq!(result.len(), 2);
            // Should be the top-2: p3 (30) and p2 (20).
            assert_eq!(result.get(0).unwrap().cosmic_essence, 30);
            assert_eq!(result.get(1).unwrap().cosmic_essence, 20);
        });
    }

    #[test]
    fn test_invalid_top_n_zero() {
        let (env, contract_id) = make_env();

        env.as_contract(&contract_id, || {
            let err = snapshot_leaderboard(&env, 0).unwrap_err();
            assert_eq!(err, AnalyticsError::InvalidTopN);
        });
    }

    #[test]
    fn test_invalid_top_n_exceeds_max() {
        let (env, contract_id) = make_env();

        env.as_contract(&contract_id, || {
            let err = snapshot_leaderboard(&env, MAX_PLAYERS + 1).unwrap_err();
            assert_eq!(err, AnalyticsError::InvalidTopN);
        });
    }

    #[test]
    fn test_player_list_capped_at_max() {
        let (env, contract_id) = make_env();

        env.as_contract(&contract_id, || {
            // Register MAX_PLAYERS + 5 distinct players.
            for _ in 0..(MAX_PLAYERS + 5) {
                let p = Address::generate(&env);
                record_scan(&env, &p, 1);
            }

            let list: Vec<Address> = env
                .storage()
                .persistent()
                .get(&AnalyticsDataKey::PlayerList)
                .unwrap_or_else(|| Vec::new(&env));
            assert_eq!(list.len(), MAX_PLAYERS);
        });
    }

    #[test]
    fn test_no_duplicate_players_in_list() {
        let (env, contract_id) = make_env();
        let player = Address::generate(&env);

        env.as_contract(&contract_id, || {
            record_scan(&env, &player, 10);
            record_scan(&env, &player, 10);
            record_scan(&env, &player, 10);

            let list: Vec<Address> = env
                .storage()
                .persistent()
                .get(&AnalyticsDataKey::PlayerList)
                .unwrap_or_else(|| Vec::new(&env));
            assert_eq!(list.len(), 1);
        });
    }

    #[test]
    fn test_leaderboard_empty_when_no_players() {
        let (env, contract_id) = make_env();

        env.as_contract(&contract_id, || {
            let result = snapshot_leaderboard(&env, 10).unwrap();
            assert_eq!(result.len(), 0);
        });
    }
}
