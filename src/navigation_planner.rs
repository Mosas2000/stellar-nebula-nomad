use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Vec};

// ─── Constants ────────────────────────────────────────────────────────────────

/// Maximum hops allowed in a single route (configurable via admin).
pub const MAX_ROUTE_HOPS: u32 = 12;
/// Maximum edges accepted in a single batch-add call.
pub const MAX_CONNECTIONS_PER_BATCH: u32 = 20;

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum NavError {
    NotInitialized        = 1,
    AlreadyInitialized    = 2,
    /// start == dest
    SameNebula            = 3,
    /// No path exists within the hop limit
    NoValidRoute          = 4,
    /// Provided route exceeds max_hops
    TooManyHops           = 5,
    /// Route Vec is empty
    RouteEmpty            = 6,
    /// Nebula ID referenced but has no registered connections
    InvalidNebula         = 7,
    /// Batch size exceeds MAX_CONNECTIONS_PER_BATCH
    BatchTooLarge         = 8,
}

// ─── Data types ───────────────────────────────────────────────────────────────

/// A directed edge in the nebula navigation graph.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct RouteEdge {
    pub from: u64,
    /// Destination nebula
    pub to: u64,
    /// Fuel units required to traverse this edge
    pub fuel_cost: u32,
    /// Hazard level 0–100 (higher = more dangerous)
    pub hazard_level: u32,
}

/// The result of a successful pathfinding call.
#[derive(Clone, Debug)]
#[contracttype]
pub struct NavPath {
    /// Ordered list of nebula IDs: [start, …, dest]
    pub hops: Vec<u64>,
    /// Sum of fuel_cost along every traversed edge
    pub total_fuel: u32,
    /// Average hazard across the route (0–100)
    pub risk_score: u32,
    /// Number of edges traversed (hops.len() - 1)
    pub hop_count: u32,
}

/// Persistent storage keys for the navigation graph.
#[derive(Clone)]
#[contracttype]
pub enum NavKey {
    Config,
    /// Adjacency list: nebula_id → Vec<RouteEdge>
    Neighbors(u64),
}

/// Contract-level configuration for the navigation planner.
#[derive(Clone)]
#[contracttype]
pub struct NavConfig {
    pub admin: Address,
    pub max_hops: u32,
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

/// Linear scan to find the index of `id` in a `Vec<u64>`.
fn find_idx(nodes: &Vec<u64>, id: u64) -> Option<u32> {
    for i in 0..nodes.len() {
        if nodes.get(i).unwrap_or(u64::MAX) == id {
            return Some(i);
        }
    }
    None
}

/// Check whether `id` appears in `visited`.
fn is_visited(visited: &Vec<u64>, id: u64) -> bool {
    find_idx(visited, id).is_some()
}

/// Average hazard across consecutive edges in `path`.
fn path_risk(env: &Env, path: &Vec<u64>) -> u32 {
    if path.len() <= 1 {
        return 0;
    }
    let mut total: u32 = 0;
    let mut edges: u32 = 0;
    for i in 0..(path.len() - 1) {
        let from = path.get(i).unwrap();
        let to   = path.get(i + 1).unwrap();
        let nb: Vec<RouteEdge> = env
            .storage()
            .persistent()
            .get(&NavKey::Neighbors(from))
            .unwrap_or_else(|| Vec::new(env));
        for j in 0..nb.len() {
            let e = nb.get(j).unwrap();
            if e.to == to {
                total += e.hazard_level;
                edges += 1;
                break;
            }
        }
    }
    if edges == 0 { 0 } else { total / edges }
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Initialise the nebula navigation graph. Must be called once by the admin.
pub fn initialize_nav_graph(env: &Env, admin: &Address) -> Result<(), NavError> {
    if env.storage().instance().has(&NavKey::Config) {
        return Err(NavError::AlreadyInitialized);
    }
    admin.require_auth();
    env.storage().instance().set(
        &NavKey::Config,
        &NavConfig {
            admin: admin.clone(),
            max_hops: MAX_ROUTE_HOPS,
        },
    );
    Ok(())
}

/// Register a directed edge between two nebulae.
/// If the edge already exists it is updated in-place.
pub fn add_nebula_connection(
    env: &Env,
    admin: &Address,
    from: u64,
    to: u64,
    fuel_cost: u32,
    hazard_level: u32,
) -> Result<(), NavError> {
    let _cfg: NavConfig = env
        .storage()
        .instance()
        .get(&NavKey::Config)
        .ok_or(NavError::NotInitialized)?;
    admin.require_auth();
    if from == to {
        return Err(NavError::SameNebula);
    }
    let edge = RouteEdge {
        from,
        to,
        fuel_cost,
        hazard_level: hazard_level.min(100),
    };
    let mut nb: Vec<RouteEdge> = env
        .storage()
        .persistent()
        .get(&NavKey::Neighbors(from))
        .unwrap_or_else(|| Vec::new(env));

    let mut updated = false;
    for i in 0..nb.len() {
        if nb.get(i).unwrap().to == to {
            nb.set(i, edge.clone());
            updated = true;
            break;
        }
    }
    if !updated {
        nb.push_back(edge);
    }
    env.storage().persistent().set(&NavKey::Neighbors(from), &nb);
    Ok(())
}

/// Add up to `MAX_CONNECTIONS_PER_BATCH` edges in a single call.
pub fn add_nebula_connections_batch(
    env: &Env,
    admin: &Address,
    edges: Vec<RouteEdge>,
) -> Result<u32, NavError> {
    if edges.len() > MAX_CONNECTIONS_PER_BATCH {
        return Err(NavError::BatchTooLarge);
    }
    admin.require_auth();
    let count = edges.len();
    for i in 0..count {
        let e = edges.get(i).unwrap();
        add_nebula_connection(env, admin, e.from, e.to, e.fuel_cost, e.hazard_level)?;
    }
    Ok(count)
}

/// Return the adjacency list for a nebula (empty Vec if none registered).
pub fn get_neighbors(env: &Env, nebula_id: u64) -> Vec<RouteEdge> {
    env.storage()
        .persistent()
        .get(&NavKey::Neighbors(nebula_id))
        .unwrap_or_else(|| Vec::new(env))
}

/// Return the single edge from `from` to `to`, if it exists.
pub fn get_connection(env: &Env, from: u64, to: u64) -> Option<RouteEdge> {
    let nb: Vec<RouteEdge> = env
        .storage()
        .persistent()
        .get(&NavKey::Neighbors(from))?;
    for i in 0..nb.len() {
        let e = nb.get(i).unwrap();
        if e.to == to {
            return Some(e);
        }
    }
    None
}

/// Dijkstra-style shortest-fuel-cost pathfinding between two nebulae.
///
/// ## Algorithm
/// Implemented with parallel `Vec<u64>` / `Vec<u32>` arrays (no HashMap) to
/// satisfy Soroban's `no_std` constraints.  All vectors live in WASM memory for
/// the duration of the call; nothing is stored until the final event emit.
///
/// Complexity: O(V²) in the worst case, acceptable for game-scale graphs
/// (≤ 100 nodes, ≤ 12 hops).
///
/// ## Emits
/// `RouteCalculated` event: topics `("nav", "route")`, data `(start, dest, total_fuel, hop_count)`
pub fn calculate_optimal_route(
    env: &Env,
    start: u64,
    dest: u64,
) -> Result<NavPath, NavError> {
    let cfg: NavConfig = env
        .storage()
        .instance()
        .get(&NavKey::Config)
        .ok_or(NavError::NotInitialized)?;

    if start == dest {
        return Err(NavError::SameNebula);
    }

    let max_hops = cfg.max_hops;

    // Parallel arrays — index i stores data for the same discovered node.
    // `dist_nodes[i]` = nebula id
    // `dist_costs[i]` = best known fuel cost to reach that node
    // `prev_nodes[i]` = predecessor on the best path (start is its own prev)
    // `hop_arr[i]`    = hop depth on the best path
    let mut dist_nodes: Vec<u64> = Vec::new(env);
    let mut dist_costs: Vec<u32> = Vec::new(env);
    let mut prev_nodes: Vec<u64> = Vec::new(env); // parallel to dist_nodes
    let mut hop_arr:   Vec<u32> = Vec::new(env);
    let mut visited:   Vec<u64> = Vec::new(env);

    // Seed with start node
    dist_nodes.push_back(start);
    dist_costs.push_back(0u32);
    prev_nodes.push_back(start);
    hop_arr.push_back(0u32);

    loop {
        // ── Extract minimum-cost unvisited node ──────────────────────────────
        let mut min_cost: u32 = u32::MAX;
        let mut cur: u64 = u64::MAX;
        let mut cur_idx: u32 = 0;

        for i in 0..dist_nodes.len() {
            let nid  = dist_nodes.get(i).unwrap();
            let cost = dist_costs.get(i).unwrap();
            if !is_visited(&visited, nid) && cost < min_cost {
                min_cost = cost;
                cur = nid;
                cur_idx = i;
            }
        }

        // No reachable unvisited node left
        if cur == u64::MAX {
            break;
        }

        // Reached destination
        if cur == dest {
            break;
        }

        let cur_hops = hop_arr.get(cur_idx).unwrap();
        visited.push_back(cur);

        // Hop limit reached — mark visited, skip expansion
        if cur_hops >= max_hops {
            continue;
        }

        // ── Relax outgoing edges ─────────────────────────────────────────────
        let nb: Vec<RouteEdge> = env
            .storage()
            .persistent()
            .get(&NavKey::Neighbors(cur))
            .unwrap_or_else(|| Vec::new(env));

        for j in 0..nb.len() {
            let edge     = nb.get(j).unwrap();
            let next     = edge.to;
            let new_cost = min_cost.saturating_add(edge.fuel_cost);
            let new_hops = cur_hops + 1;

            if is_visited(&visited, next) || new_hops > max_hops {
                continue;
            }

            if let Some(idx) = find_idx(&dist_nodes, next) {
                // Update if we found a cheaper path
                if new_cost < dist_costs.get(idx).unwrap() {
                    dist_costs.set(idx, new_cost);
                    prev_nodes.set(idx, cur);
                    hop_arr.set(idx, new_hops);
                }
            } else {
                // First time we see this node
                dist_nodes.push_back(next);
                dist_costs.push_back(new_cost);
                prev_nodes.push_back(cur);
                hop_arr.push_back(new_hops);
            }
        }
    }

    // ── Check destination was reached ────────────────────────────────────────
    let dest_idx = find_idx(&dist_nodes, dest).ok_or(NavError::NoValidRoute)?;
    let total_fuel = dist_costs.get(dest_idx).unwrap();
    if total_fuel == u32::MAX {
        return Err(NavError::NoValidRoute);
    }

    // ── Reconstruct path (dest → start, then reverse) ────────────────────────
    let mut rev_path: Vec<u64> = Vec::new(env);
    let mut node = dest;
    let mut steps: u32 = 0;

    loop {
        rev_path.push_back(node);
        steps += 1;
        let idx = find_idx(&dist_nodes, node).ok_or(NavError::NoValidRoute)?;
        let prev = prev_nodes.get(idx).unwrap();
        if prev == node || steps > max_hops + 1 {
            break; // start node's predecessor is itself
        }
        node = prev;
    }

    if steps > max_hops + 1 {
        return Err(NavError::TooManyHops);
    }

    // Reverse to get start → dest order
    let total_nodes = rev_path.len();
    let mut path: Vec<u64> = Vec::new(env);
    for i in (0..total_nodes).rev() {
        path.push_back(rev_path.get(i as u32).unwrap());
    }

    let hop_count = total_nodes.saturating_sub(1);
    let risk = path_risk(env, &path);

    // Emit RouteCalculated event
    env.events().publish(
        (symbol_short!("nav"), symbol_short!("route")),
        (start, dest, total_fuel, hop_count as u32),
    );

    Ok(NavPath {
        hops: path,
        total_fuel,
        risk_score: risk,
        hop_count: hop_count as u32,
    })
}

/// Validate an existing route Vec and return its aggregate risk score (0–100).
///
/// Checks:
/// - Route is non-empty
/// - Route length ≤ max_hops + 1
/// - Every consecutive pair (from, to) has a registered edge
pub fn validate_route_safety(env: &Env, route: Vec<u64>) -> Result<u32, NavError> {
    if route.len() == 0 {
        return Err(NavError::RouteEmpty);
    }
    let cfg: NavConfig = env
        .storage()
        .instance()
        .get(&NavKey::Config)
        .ok_or(NavError::NotInitialized)?;

    let max_len = cfg.max_hops + 1; // nodes = hops + 1
    if route.len() > max_len {
        return Err(NavError::TooManyHops);
    }

    // Verify every edge exists in the graph
    for i in 0..(route.len().saturating_sub(1)) {
        let from = route.get(i).unwrap();
        let to   = route.get(i + 1).unwrap();
        if get_connection(env, from, to).is_none() {
            return Err(NavError::InvalidNebula);
        }
    }

    Ok(path_risk(env, &route))
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup() -> (Env, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        initialize_nav_graph(&env, &admin).unwrap();
        (env, admin)
    }

    // ── Init ────────────────────────────────────────────────────────────────

    #[test]
    fn test_init_stores_config() {
        let (env, _) = setup();
        let cfg: NavConfig = env.storage().instance().get(&NavKey::Config).unwrap();
        assert_eq!(cfg.max_hops, MAX_ROUTE_HOPS);
    }

    #[test]
    fn test_double_init_rejected() {
        let (env, admin) = setup();
        let err = initialize_nav_graph(&env, &admin).unwrap_err();
        assert_eq!(err, NavError::AlreadyInitialized);
    }

    // ── Edge management ─────────────────────────────────────────────────────

    #[test]
    fn test_add_and_retrieve_connection() {
        let (env, admin) = setup();
        add_nebula_connection(&env, &admin, 1, 2, 10, 20).unwrap();
        let e = get_connection(&env, 1, 2).unwrap();
        assert_eq!(e.fuel_cost, 10);
        assert_eq!(e.hazard_level, 20);
    }

    #[test]
    fn test_same_nebula_rejected() {
        let (env, admin) = setup();
        let err = add_nebula_connection(&env, &admin, 5, 5, 10, 0).unwrap_err();
        assert_eq!(err, NavError::SameNebula);
    }

    #[test]
    fn test_hazard_clamped_at_100() {
        let (env, admin) = setup();
        add_nebula_connection(&env, &admin, 1, 2, 5, 200).unwrap();
        assert_eq!(get_connection(&env, 1, 2).unwrap().hazard_level, 100);
    }

    #[test]
    fn test_edge_update_in_place() {
        let (env, admin) = setup();
        add_nebula_connection(&env, &admin, 1, 2, 10, 20).unwrap();
        add_nebula_connection(&env, &admin, 1, 2, 99, 50).unwrap();
        let nb = get_neighbors(&env, 1);
        assert_eq!(nb.len(), 1); // still one edge
        assert_eq!(nb.get(0).unwrap().fuel_cost, 99);
    }

    #[test]
    fn test_batch_add_connections() {
        let (env, admin) = setup();
        let mut edges: Vec<RouteEdge> = Vec::new(&env);
        edges.push_back(RouteEdge { from: 1, to: 2, fuel_cost: 5, hazard_level: 10 });
        edges.push_back(RouteEdge { from: 2, to: 3, fuel_cost: 7, hazard_level: 15 });
        edges.push_back(RouteEdge { from: 3, to: 4, fuel_cost: 3, hazard_level: 5  });
        let count = add_nebula_connections_batch(&env, &admin, edges).unwrap();
        assert_eq!(count, 3);
        assert!(get_connection(&env, 1, 2).is_some());
        assert!(get_connection(&env, 2, 3).is_some());
        assert!(get_connection(&env, 3, 4).is_some());
    }

    #[test]
    fn test_batch_too_large_rejected() {
        let (env, admin) = setup();
        let mut edges: Vec<RouteEdge> = Vec::new(&env);
        for i in 0..(MAX_CONNECTIONS_PER_BATCH + 1) {
            edges.push_back(RouteEdge {
                from: i as u64,
                to: (i + 100) as u64,
                fuel_cost: 1,
                hazard_level: 0,
            });
        }
        let err = add_nebula_connections_batch(&env, &admin, edges).unwrap_err();
        assert_eq!(err, NavError::BatchTooLarge);
    }

    // ── Pathfinding ─────────────────────────────────────────────────────────

    fn build_linear_graph(env: &Env, admin: &Address, nodes: u64) {
        for i in 0..(nodes - 1) {
            add_nebula_connection(env, admin, i, i + 1, 10, 20).unwrap();
        }
    }

    #[test]
    fn test_direct_route() {
        let (env, admin) = setup();
        add_nebula_connection(&env, &admin, 1, 2, 10, 30).unwrap();
        let path = calculate_optimal_route(&env, 1, 2).unwrap();
        assert_eq!(path.hop_count, 1);
        assert_eq!(path.total_fuel, 10);
        assert_eq!(path.hops.get(0).unwrap(), 1);
        assert_eq!(path.hops.get(1).unwrap(), 2);
    }

    #[test]
    fn test_multi_hop_route() {
        let (env, admin) = setup();
        build_linear_graph(&env, &admin, 6); // 0→1→2→3→4→5
        let path = calculate_optimal_route(&env, 0, 5).unwrap();
        assert_eq!(path.hop_count, 5);
        assert_eq!(path.total_fuel, 50);
    }

    #[test]
    fn test_picks_cheaper_path() {
        let (env, admin) = setup();
        // Direct: 1→3 costs 100
        add_nebula_connection(&env, &admin, 1, 3, 100, 0).unwrap();
        // Via 2: 1→2 costs 5, 2→3 costs 5 → total 10
        add_nebula_connection(&env, &admin, 1, 2, 5, 0).unwrap();
        add_nebula_connection(&env, &admin, 2, 3, 5, 0).unwrap();
        let path = calculate_optimal_route(&env, 1, 3).unwrap();
        assert_eq!(path.total_fuel, 10);
        assert_eq!(path.hop_count, 2);
    }

    #[test]
    fn test_no_valid_route_returns_error() {
        let (env, admin) = setup();
        add_nebula_connection(&env, &admin, 1, 2, 10, 0).unwrap();
        // 3 is completely disconnected
        let err = calculate_optimal_route(&env, 1, 3).unwrap_err();
        assert_eq!(err, NavError::NoValidRoute);
    }

    #[test]
    fn test_same_nebula_returns_error() {
        let (env, _) = setup();
        let err = calculate_optimal_route(&env, 5, 5).unwrap_err();
        assert_eq!(err, NavError::SameNebula);
    }

    #[test]
    fn test_route_respects_max_hops() {
        let (env, admin) = setup();
        // 15-hop chain — only path, exceeds MAX_ROUTE_HOPS (12)
        for i in 0u64..15 {
            add_nebula_connection(&env, &admin, i, i + 1, 1, 0).unwrap();
        }
        let err = calculate_optimal_route(&env, 0, 15).unwrap_err();
        assert_eq!(err, NavError::NoValidRoute);
    }

    #[test]
    fn test_route_within_max_hops() {
        let (env, admin) = setup();
        build_linear_graph(&env, &admin, 13); // 12-hop path: 0→…→12
        let path = calculate_optimal_route(&env, 0, 12).unwrap();
        assert_eq!(path.hop_count, 12);
    }

    #[test]
    fn test_risk_score_computed() {
        let (env, admin) = setup();
        add_nebula_connection(&env, &admin, 1, 2, 10, 40).unwrap();
        add_nebula_connection(&env, &admin, 2, 3, 10, 80).unwrap();
        let path = calculate_optimal_route(&env, 1, 3).unwrap();
        // avg hazard = (40 + 80) / 2 = 60
        assert_eq!(path.risk_score, 60);
    }

    #[test]
    fn test_path_starts_at_start_ends_at_dest() {
        let (env, admin) = setup();
        build_linear_graph(&env, &admin, 5);
        let path = calculate_optimal_route(&env, 0, 4).unwrap();
        assert_eq!(path.hops.get(0).unwrap(), 0);
        assert_eq!(path.hops.get(path.hop_count).unwrap(), 4);
    }

    // ── validate_route_safety ───────────────────────────────────────────────

    #[test]
    fn test_validate_valid_route() {
        let (env, admin) = setup();
        add_nebula_connection(&env, &admin, 1, 2, 10, 30).unwrap();
        add_nebula_connection(&env, &admin, 2, 3, 10, 70).unwrap();
        let mut route: Vec<u64> = Vec::new(&env);
        route.push_back(1);
        route.push_back(2);
        route.push_back(3);
        let risk = validate_route_safety(&env, route).unwrap();
        assert_eq!(risk, 50); // avg(30, 70) = 50
    }

    #[test]
    fn test_validate_empty_route_rejected() {
        let (env, _) = setup();
        let route: Vec<u64> = Vec::new(&env);
        let err = validate_route_safety(&env, route).unwrap_err();
        assert_eq!(err, NavError::RouteEmpty);
    }

    #[test]
    fn test_validate_too_long_route_rejected() {
        let (env, admin) = setup();
        build_linear_graph(&env, &admin, 15);
        let mut route: Vec<u64> = Vec::new(&env);
        for i in 0u64..15 {
            route.push_back(i);
        }
        let err = validate_route_safety(&env, route).unwrap_err();
        assert_eq!(err, NavError::TooManyHops);
    }

    #[test]
    fn test_validate_missing_edge_rejected() {
        let (env, admin) = setup();
        add_nebula_connection(&env, &admin, 1, 2, 10, 0).unwrap();
        // No edge 2→99
        let mut route: Vec<u64> = Vec::new(&env);
        route.push_back(1);
        route.push_back(2);
        route.push_back(99);
        let err = validate_route_safety(&env, route).unwrap_err();
        assert_eq!(err, NavError::InvalidNebula);
    }

    #[test]
    fn test_validate_single_node_route() {
        let (env, _) = setup();
        let mut route: Vec<u64> = Vec::new(&env);
        route.push_back(42);
        let risk = validate_route_safety(&env, route).unwrap();
        assert_eq!(risk, 0);
    }

    // ── Graph correctness ───────────────────────────────────────────────────

    #[test]
    fn test_get_neighbors_empty_for_unknown_node() {
        let (env, _) = setup();
        let nb = get_neighbors(&env, 9999);
        assert_eq!(nb.len(), 0);
    }

    #[test]
    fn test_multiple_neighbors() {
        let (env, admin) = setup();
        add_nebula_connection(&env, &admin, 1, 2, 5,  10).unwrap();
        add_nebula_connection(&env, &admin, 1, 3, 8,  20).unwrap();
        add_nebula_connection(&env, &admin, 1, 4, 12, 30).unwrap();
        assert_eq!(get_neighbors(&env, 1).len(), 3);
    }

    #[test]
    fn test_directed_graph_no_reverse_edge() {
        let (env, admin) = setup();
        add_nebula_connection(&env, &admin, 1, 2, 10, 0).unwrap();
        // Edge 2→1 was NOT added
        let err = calculate_optimal_route(&env, 2, 1).unwrap_err();
        assert_eq!(err, NavError::NoValidRoute);
    }
}
