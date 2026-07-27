//! Quest system — Issue #282
//!
//! Guided, multi-step content on top of the one-shot dailies produced by
//! [`crate::mission_generator`]. Where a mission is a single objective that
//! expires in a day, a quest is a **node in a chain**: completing it unlocks
//! one of several successors, so a chain describes a branching narrative rather
//! than a fixed list.
//!
//! ```text
//!                       ┌── choice 1 ──► node 2 ──► node 4
//!   chain root: node 1 ─┤
//!                       └── choice 2 ──► node 3 ──► (end)
//! ```
//!
//! Lifecycle of one node for one player:
//!
//! ```text
//!   Locked ──start/choose──► Active ──progress reaches target──► Completed
//!                              │                                    │
//!                              └────────── expires ──────► Expired  ├─claim─► Claimed
//!                                                                   │
//!                                                       choose_branch └──► next node Active
//! ```
//!
//! Progress can be recorded directly or fed in from the mission system: a
//! completed mission calls [`on_mission_completed`], which advances every
//! active quest whose objective matches the mission type. That means normal
//! play drives quest chains forward without the player tracking two systems.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, String, Symbol, Vec};

use crate::player_profile::{self, ProfileError};
use crate::resource_minter::{self, ResourceType};

// ─── Constants ────────────────────────────────────────────────────────────────

/// Maximum nodes in a single chain — bounds the storage a chain can occupy.
pub const MAX_CHAIN_LENGTH: u32 = 20;

/// Maximum branch choices offered by one node.
pub const MAX_BRANCHES_PER_NODE: u32 = 4;

/// Maximum quests a player may have active at once. Also bounds the iteration
/// in [`on_mission_completed`], keeping that call's cost constant.
pub const MAX_ACTIVE_QUESTS: u32 = 10;

/// Sentinel used by [`QuestBranch::next_quest_id`] to mean "this path ends
/// here" — chains terminate on a leaf rather than needing an explicit end node.
pub const CHAIN_TERMINUS: u64 = 0;

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum QuestKey {
    /// Chain definition by chain ID.
    Chain(u64),
    /// Node definition by (globally unique) quest ID.
    Node(u64),
    /// Auto-increment chain ID counter.
    ChainCounter,
    /// Auto-increment quest (node) ID counter.
    NodeCounter,
    /// Per-player state for one node.
    State(Address, u64),
    /// Per-player progress through one chain.
    Progress(Address, u64),
    /// Quest IDs currently active for a player.
    ActiveQuests(Address),
}

// ─── Data Types ───────────────────────────────────────────────────────────────

/// Where a player stands on a single quest node.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuestStatus {
    /// Defined but not yet reachable for this player.
    Locked,
    /// Accepted and accruing progress.
    Active,
    /// Target met; reward not yet taken.
    Completed,
    /// Reward taken.
    Claimed,
    /// Deadline passed before the target was met.
    Expired,
}

/// A quest payout. Multiple currencies so chains can mix incentives instead of
/// paying escalating essence at every step.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuestReward {
    /// Essence credited to the player's profile.
    pub essence: i128,
    /// Resource type granted (ignored when `resource_amount` is 0).
    pub resource_type: ResourceType,
    pub resource_amount: u64,
    /// Experience toward account progression.
    pub xp: u64,
    /// Whether the node grants a cosmetic roll.
    pub cosmetic_roll: bool,
}

/// One outgoing edge from a quest node — the unit of branching narrative.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuestBranch {
    /// Identifier the player passes to [`choose_branch`]. Must be non-zero.
    pub choice_id: u32,
    /// Short label for the choice, e.g. `sabotage` / `negotiate`.
    pub label: Symbol,
    /// Node unlocked by this choice, or [`CHAIN_TERMINUS`] to end the chain.
    pub next_quest_id: u64,
}

/// A quest node: one objective plus the choices that follow it.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuestNode {
    pub quest_id: u64,
    pub chain_id: u64,
    /// 0-based position in the order the node was added to its chain.
    pub step_index: u32,
    /// Objective tag, matched against `Mission::mission_type` when mission
    /// completions are forwarded in (`scan`, `harvest`, `explore`, `trade`, …).
    pub objective: Symbol,
    pub target_count: u32,
    pub reward: QuestReward,
    /// Narrative bucket for flavour text selection.
    pub narrative_tier: Symbol,
    pub title: String,
    pub description: String,
    pub branches: Vec<QuestBranch>,
    /// Seconds the player has to finish once active; 0 means no deadline.
    pub duration_secs: u64,
}

/// A named chain of quest nodes.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuestChain {
    pub chain_id: u64,
    pub name: Symbol,
    pub creator: Address,
    /// First node added to the chain; where [`start_chain`] begins.
    pub root_quest_id: u64,
    pub node_count: u32,
}

/// Per-player state for one quest node.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuestState {
    pub player: Address,
    pub quest_id: u64,
    pub chain_id: u64,
    pub status: QuestStatus,
    pub progress: u32,
    pub target: u32,
    pub started_at: u64,
    /// 0 when the node has no deadline.
    pub expires_at: u64,
    /// 0 until the target is met.
    pub completed_at: u64,
    /// Branch taken after completion; 0 while none has been chosen.
    pub chosen_branch: u32,
}

/// Per-player progress through a chain, including the narrative path walked.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChainProgress {
    pub player: Address,
    pub chain_id: u64,
    /// Node the player is currently on, or the last one they finished.
    pub current_quest_id: u64,
    pub steps_completed: u32,
    pub total_essence_earned: i128,
    /// Branch choice IDs in the order they were taken.
    pub path: Vec<u32>,
    /// True once a terminal node has been claimed.
    pub finished: bool,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum QuestError {
    /// No chain with the given ID.
    ChainNotFound = 1,
    /// No node with the given quest ID.
    QuestNotFound = 2,
    /// The player has no state for this quest.
    QuestNotStarted = 3,
    /// The quest is not in a state that allows this operation.
    InvalidStatus = 4,
    /// The quest deadline has passed.
    QuestExpired = 5,
    /// The reward for this quest was already taken.
    AlreadyClaimed = 6,
    /// Chain already holds [`MAX_CHAIN_LENGTH`] nodes.
    ChainFull = 7,
    /// Node declares more than [`MAX_BRANCHES_PER_NODE`] branches.
    TooManyBranches = 8,
    /// `choice_id` is not offered by this node.
    InvalidBranch = 9,
    /// The player already has [`MAX_ACTIVE_QUESTS`] quests active.
    TooManyActiveQuests = 10,
    /// Only the chain's creator may extend it.
    Unauthorized = 11,
    /// The player already started this chain.
    ChainAlreadyStarted = 12,
    /// `target_count` must be greater than zero.
    InvalidTarget = 13,
    /// No profile exists for the player, so rewards cannot be credited.
    ProfileNotFound = 14,
    /// Reward accounting overflowed.
    ArithmeticOverflow = 15,
    /// A branch points at a node that does not exist.
    DanglingBranch = 16,
}

impl From<ProfileError> for QuestError {
    fn from(e: ProfileError) -> Self {
        match e {
            ProfileError::ArithmeticOverflow => QuestError::ArithmeticOverflow,
            _ => QuestError::ProfileNotFound,
        }
    }
}

// ─── Authoring ────────────────────────────────────────────────────────────────

/// Create an empty quest chain owned by `creator`.
///
/// Nodes are added afterwards with [`add_quest_node`]; the first node added
/// becomes the chain's root.
pub fn define_chain(env: &Env, creator: Address, name: Symbol) -> Result<u64, QuestError> {
    creator.require_auth();

    let chain_id: u64 = env
        .storage()
        .persistent()
        .get::<QuestKey, u64>(&QuestKey::ChainCounter)
        .unwrap_or(0)
        .saturating_add(1);
    env.storage()
        .persistent()
        .set(&QuestKey::ChainCounter, &chain_id);

    let chain = QuestChain {
        chain_id,
        name: name.clone(),
        creator: creator.clone(),
        root_quest_id: CHAIN_TERMINUS,
        node_count: 0,
    };
    env.storage()
        .persistent()
        .set(&QuestKey::Chain(chain_id), &chain);

    env.events().publish(
        (symbol_short!("quest"), symbol_short!("chaindef")),
        (creator, chain_id, name),
    );

    Ok(chain_id)
}

/// Append a node to `chain_id` and return its quest ID.
///
/// `branches` may reference quest IDs that do not exist yet — chains are
/// authored root-first, so forward edges are resolved when the target node is
/// added. [`validate_chain`] checks that no dangling edge remains.
#[allow(clippy::too_many_arguments)]
pub fn add_quest_node(
    env: &Env,
    creator: Address,
    chain_id: u64,
    objective: Symbol,
    target_count: u32,
    reward: QuestReward,
    narrative_tier: Symbol,
    title: String,
    description: String,
    branches: Vec<QuestBranch>,
    duration_secs: u64,
) -> Result<u64, QuestError> {
    creator.require_auth();

    let mut chain: QuestChain = env
        .storage()
        .persistent()
        .get(&QuestKey::Chain(chain_id))
        .ok_or(QuestError::ChainNotFound)?;

    if chain.creator != creator {
        return Err(QuestError::Unauthorized);
    }
    if chain.node_count >= MAX_CHAIN_LENGTH {
        return Err(QuestError::ChainFull);
    }
    if branches.len() > MAX_BRANCHES_PER_NODE {
        return Err(QuestError::TooManyBranches);
    }
    if target_count == 0 {
        return Err(QuestError::InvalidTarget);
    }
    // A zero choice_id would collide with the "no branch chosen" sentinel in
    // `QuestState::chosen_branch`.
    for branch in branches.iter() {
        if branch.choice_id == 0 {
            return Err(QuestError::InvalidBranch);
        }
    }

    let quest_id: u64 = env
        .storage()
        .persistent()
        .get::<QuestKey, u64>(&QuestKey::NodeCounter)
        .unwrap_or(0)
        .saturating_add(1);
    env.storage()
        .persistent()
        .set(&QuestKey::NodeCounter, &quest_id);

    let node = QuestNode {
        quest_id,
        chain_id,
        step_index: chain.node_count,
        objective: objective.clone(),
        target_count,
        reward,
        narrative_tier,
        title,
        description,
        branches,
        duration_secs,
    };
    env.storage()
        .persistent()
        .set(&QuestKey::Node(quest_id), &node);

    chain.node_count = chain.node_count.saturating_add(1);
    if chain.root_quest_id == CHAIN_TERMINUS {
        chain.root_quest_id = quest_id;
    }
    env.storage()
        .persistent()
        .set(&QuestKey::Chain(chain_id), &chain);

    env.events().publish(
        (symbol_short!("quest"), symbol_short!("nodeadd")),
        (chain_id, quest_id, objective),
    );

    Ok(quest_id)
}

/// Check that every branch in `chain_id` points at an existing node.
///
/// Authors should call this once a chain is fully written; it is not enforced at
/// node-insert time because forward references are legitimate while authoring.
pub fn validate_chain(env: &Env, chain_id: u64) -> Result<(), QuestError> {
    let chain: QuestChain = env
        .storage()
        .persistent()
        .get(&QuestKey::Chain(chain_id))
        .ok_or(QuestError::ChainNotFound)?;

    // Node IDs are globally sequential, so walking from the root over the
    // chain's node count covers exactly this chain's nodes.
    let first = chain.root_quest_id;
    for quest_id in first..first.saturating_add(chain.node_count as u64) {
        let node: QuestNode = match env.storage().persistent().get(&QuestKey::Node(quest_id)) {
            Some(node) => node,
            None => continue,
        };
        if node.chain_id != chain_id {
            continue;
        }
        for branch in node.branches.iter() {
            if branch.next_quest_id != CHAIN_TERMINUS
                && !env
                    .storage()
                    .persistent()
                    .has(&QuestKey::Node(branch.next_quest_id))
            {
                return Err(QuestError::DanglingBranch);
            }
        }
    }

    Ok(())
}

// ─── Active-quest bookkeeping ─────────────────────────────────────────────────

fn active_quests(env: &Env, player: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&QuestKey::ActiveQuests(player.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

fn push_active(env: &Env, player: &Address, quest_id: u64) -> Result<(), QuestError> {
    let mut active = active_quests(env, player);
    if active.len() >= MAX_ACTIVE_QUESTS {
        return Err(QuestError::TooManyActiveQuests);
    }
    active.push_back(quest_id);
    env.storage()
        .persistent()
        .set(&QuestKey::ActiveQuests(player.clone()), &active);
    Ok(())
}

fn remove_active(env: &Env, player: &Address, quest_id: u64) {
    let active = active_quests(env, player);
    let mut remaining = Vec::new(env);
    for id in active.iter() {
        if id != quest_id {
            remaining.push_back(id);
        }
    }
    env.storage()
        .persistent()
        .set(&QuestKey::ActiveQuests(player.clone()), &remaining);
}

fn save_state(env: &Env, state: &QuestState) {
    env.storage().persistent().set(
        &QuestKey::State(state.player.clone(), state.quest_id),
        state,
    );
}

fn load_state(env: &Env, player: &Address, quest_id: u64) -> Result<QuestState, QuestError> {
    env.storage()
        .persistent()
        .get(&QuestKey::State(player.clone(), quest_id))
        .ok_or(QuestError::QuestNotStarted)
}

fn load_node(env: &Env, quest_id: u64) -> Result<QuestNode, QuestError> {
    env.storage()
        .persistent()
        .get(&QuestKey::Node(quest_id))
        .ok_or(QuestError::QuestNotFound)
}

/// Activate `node` for `player`, writing state and progress records.
fn activate(env: &Env, player: &Address, node: &QuestNode) -> Result<QuestState, QuestError> {
    let now = env.ledger().timestamp();
    let expires_at = if node.duration_secs == 0 {
        0
    } else {
        now.saturating_add(node.duration_secs)
    };

    let state = QuestState {
        player: player.clone(),
        quest_id: node.quest_id,
        chain_id: node.chain_id,
        status: QuestStatus::Active,
        progress: 0,
        target: node.target_count,
        started_at: now,
        expires_at,
        completed_at: 0,
        chosen_branch: 0,
    };

    push_active(env, player, node.quest_id)?;
    save_state(env, &state);

    env.events().publish(
        (symbol_short!("quest"), symbol_short!("started")),
        (
            player.clone(),
            node.quest_id,
            node.chain_id,
            node.step_index,
        ),
    );

    Ok(state)
}

// ─── Player flow ──────────────────────────────────────────────────────────────

/// Begin `chain_id` for `player`, activating its root node.
pub fn start_chain(env: &Env, player: Address, chain_id: u64) -> Result<QuestState, QuestError> {
    player.require_auth();

    let chain: QuestChain = env
        .storage()
        .persistent()
        .get(&QuestKey::Chain(chain_id))
        .ok_or(QuestError::ChainNotFound)?;

    if chain.root_quest_id == CHAIN_TERMINUS {
        return Err(QuestError::QuestNotFound);
    }
    if env
        .storage()
        .persistent()
        .has(&QuestKey::Progress(player.clone(), chain_id))
    {
        return Err(QuestError::ChainAlreadyStarted);
    }

    let node = load_node(env, chain.root_quest_id)?;
    let state = activate(env, &player, &node)?;

    let progress = ChainProgress {
        player: player.clone(),
        chain_id,
        current_quest_id: node.quest_id,
        steps_completed: 0,
        total_essence_earned: 0,
        path: Vec::new(env),
        finished: false,
    };
    env.storage()
        .persistent()
        .set(&QuestKey::Progress(player.clone(), chain_id), &progress);

    env.events().publish(
        (symbol_short!("quest"), symbol_short!("chainstr")),
        (player, chain_id, node.quest_id),
    );

    Ok(state)
}

/// Add `amount` to a player's progress on an active quest.
///
/// Transitions the quest to `Completed` once the target is met, and to `Expired`
/// if the deadline has already passed. Idempotent once the quest leaves
/// `Active`: further calls return `InvalidStatus` rather than over-counting.
pub fn record_progress(
    env: &Env,
    player: Address,
    quest_id: u64,
    amount: u32,
) -> Result<QuestState, QuestError> {
    player.require_auth();
    record_progress_unchecked(env, &player, quest_id, amount)
}

/// Progress without an auth check, for callers that already authorized `player`.
fn record_progress_unchecked(
    env: &Env,
    player: &Address,
    quest_id: u64,
    amount: u32,
) -> Result<QuestState, QuestError> {
    let mut state = load_state(env, player, quest_id)?;

    if state.status != QuestStatus::Active {
        return Err(QuestError::InvalidStatus);
    }

    let now = env.ledger().timestamp();
    if state.expires_at != 0 && now > state.expires_at {
        state.status = QuestStatus::Expired;
        save_state(env, &state);
        remove_active(env, player, quest_id);

        env.events().publish(
            (symbol_short!("quest"), symbol_short!("expired")),
            (player.clone(), quest_id),
        );
        return Err(QuestError::QuestExpired);
    }

    state.progress = state.progress.saturating_add(amount);

    if state.progress >= state.target {
        state.progress = state.target;
        state.status = QuestStatus::Completed;
        state.completed_at = now;
        remove_active(env, player, quest_id);

        env.events().publish(
            (symbol_short!("quest"), symbol_short!("complete")),
            (player.clone(), quest_id, state.chain_id),
        );
    } else {
        env.events().publish(
            (symbol_short!("quest"), symbol_short!("progress")),
            (player.clone(), quest_id, state.progress, state.target),
        );
    }

    save_state(env, &state);
    Ok(state)
}

/// Claim the reward for a completed quest.
///
/// Credits essence to the player's profile and resources to their balance, then
/// marks the quest `Claimed`. A node whose only branches are
/// [`CHAIN_TERMINUS`] — or which has no branches at all — finishes the chain
/// here.
pub fn claim_quest_reward(
    env: &Env,
    player: Address,
    quest_id: u64,
) -> Result<QuestReward, QuestError> {
    player.require_auth();

    let mut state = load_state(env, &player, quest_id)?;

    match state.status {
        QuestStatus::Completed => {}
        QuestStatus::Claimed => return Err(QuestError::AlreadyClaimed),
        QuestStatus::Expired => return Err(QuestError::QuestExpired),
        _ => return Err(QuestError::InvalidStatus),
    }

    let node = load_node(env, quest_id)?;
    let reward = node.reward.clone();

    // ── Pay out ───────────────────────────────────────────────────────────
    let profile = player_profile::get_profile_by_owner(env, &player)?;
    if reward.essence > 0 {
        player_profile::credit_essence(env, profile.id, reward.essence)?;
    }
    if reward.resource_amount > 0 {
        resource_minter::credit_balance(
            env,
            &player,
            &reward.resource_type,
            reward.resource_amount,
        )
        .map_err(|_| QuestError::ArithmeticOverflow)?;
    }

    state.status = QuestStatus::Claimed;
    save_state(env, &state);

    // ── Chain progress ────────────────────────────────────────────────────
    let mut progress =
        get_chain_progress(env, &player, state.chain_id).ok_or(QuestError::QuestNotStarted)?;
    progress.steps_completed = progress.steps_completed.saturating_add(1);
    progress.total_essence_earned = progress
        .total_essence_earned
        .checked_add(reward.essence)
        .ok_or(QuestError::ArithmeticOverflow)?;
    progress.current_quest_id = quest_id;

    // No onward edge means this path of the narrative ends here.
    let has_successor = node
        .branches
        .iter()
        .any(|b| b.next_quest_id != CHAIN_TERMINUS);
    if !has_successor {
        progress.finished = true;
        env.events().publish(
            (symbol_short!("quest"), symbol_short!("chainend")),
            (player.clone(), state.chain_id, progress.steps_completed),
        );
    }
    env.storage().persistent().set(
        &QuestKey::Progress(player.clone(), state.chain_id),
        &progress,
    );

    env.events().publish(
        (symbol_short!("quest"), symbol_short!("claimed")),
        (player, quest_id, reward.essence, reward.xp),
    );

    Ok(reward)
}

/// Take a branch out of a claimed quest, activating the successor node.
///
/// Returns the newly activated state, or `None` when the chosen branch is a
/// [`CHAIN_TERMINUS`] leaf. The choice is appended to
/// [`ChainProgress::path`], which is the record of the narrative walked.
pub fn choose_branch(
    env: &Env,
    player: Address,
    quest_id: u64,
    choice_id: u32,
) -> Result<Option<QuestState>, QuestError> {
    player.require_auth();

    let mut state = load_state(env, &player, quest_id)?;

    // Branching happens after the reward is taken, so the player cannot skip
    // past a node's payout by racing ahead in the narrative.
    if state.status != QuestStatus::Claimed {
        return Err(QuestError::InvalidStatus);
    }
    if state.chosen_branch != 0 {
        return Err(QuestError::InvalidStatus);
    }

    let node = load_node(env, quest_id)?;
    let branch = node
        .branches
        .iter()
        .find(|b| b.choice_id == choice_id)
        .ok_or(QuestError::InvalidBranch)?;

    state.chosen_branch = choice_id;
    save_state(env, &state);

    let mut progress =
        get_chain_progress(env, &player, state.chain_id).ok_or(QuestError::QuestNotStarted)?;
    progress.path.push_back(choice_id);

    env.events().publish(
        (symbol_short!("quest"), symbol_short!("branch")),
        (
            player.clone(),
            quest_id,
            choice_id,
            branch.label.clone(),
            branch.next_quest_id,
        ),
    );

    if branch.next_quest_id == CHAIN_TERMINUS {
        progress.finished = true;
        env.storage().persistent().set(
            &QuestKey::Progress(player.clone(), state.chain_id),
            &progress,
        );

        env.events().publish(
            (symbol_short!("quest"), symbol_short!("chainend")),
            (player, state.chain_id, progress.steps_completed),
        );
        return Ok(None);
    }

    let next_node = load_node(env, branch.next_quest_id)?;
    let next_state = activate(env, &player, &next_node)?;

    progress.current_quest_id = next_node.quest_id;
    env.storage()
        .persistent()
        .set(&QuestKey::Progress(player, state.chain_id), &progress);

    Ok(Some(next_state))
}

// ─── Mission bridge ───────────────────────────────────────────────────────────

/// Forward a completed mission into the quest system.
///
/// Advances every active quest whose `objective` equals `objective` by
/// `amount`, and returns how many quests were advanced. Errors on individual
/// quests (expired, already complete) are swallowed: a mission completion must
/// never fail because of unrelated quest state.
pub fn on_mission_completed(env: &Env, player: &Address, objective: Symbol, amount: u32) -> u32 {
    let mut advanced = 0u32;

    for quest_id in active_quests(env, player).iter() {
        let node = match env
            .storage()
            .persistent()
            .get::<QuestKey, QuestNode>(&QuestKey::Node(quest_id))
        {
            Some(node) => node,
            None => continue,
        };
        if node.objective != objective {
            continue;
        }
        if record_progress_unchecked(env, player, quest_id, amount).is_ok() {
            advanced = advanced.saturating_add(1);
        }
    }

    if advanced > 0 {
        env.events().publish(
            (symbol_short!("quest"), symbol_short!("misfeed")),
            (player.clone(), objective, advanced),
        );
    }

    advanced
}

// ─── Queries ──────────────────────────────────────────────────────────────────

/// Fetch a quest node definition.
pub fn get_quest_node(env: &Env, quest_id: u64) -> Option<QuestNode> {
    env.storage().persistent().get(&QuestKey::Node(quest_id))
}

/// Fetch a chain definition.
pub fn get_chain(env: &Env, chain_id: u64) -> Option<QuestChain> {
    env.storage().persistent().get(&QuestKey::Chain(chain_id))
}

/// Fetch a player's state for one quest.
pub fn get_quest_state(env: &Env, player: &Address, quest_id: u64) -> Option<QuestState> {
    env.storage()
        .persistent()
        .get(&QuestKey::State(player.clone(), quest_id))
}

/// Fetch a player's progress through one chain.
pub fn get_chain_progress(env: &Env, player: &Address, chain_id: u64) -> Option<ChainProgress> {
    env.storage()
        .persistent()
        .get(&QuestKey::Progress(player.clone(), chain_id))
}

/// Quest IDs the player currently has active.
pub fn get_active_quests(env: &Env, player: &Address) -> Vec<u64> {
    active_quests(env, player)
}

/// Full state records for the player's active quests.
pub fn get_active_quest_states(env: &Env, player: &Address) -> Vec<QuestState> {
    let mut states = Vec::new(env);
    for quest_id in active_quests(env, player).iter() {
        if let Some(state) = get_quest_state(env, player, quest_id) {
            states.push_back(state);
        }
    }
    states
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

    /// Quest state lives in contract storage, so test bodies run through
    /// `Env::as_contract`. Each `run` is a separate invocation frame, matching
    /// how the flow works in practice (start, progress, claim and branch are
    /// distinct transactions) and satisfying the rule that `require_auth` may
    /// only be met once per frame.
    struct Fixture {
        env: Env,
        contract: Address,
        author: Address,
        player: Address,
    }

    impl Fixture {
        fn run<T>(&self, f: impl FnOnce() -> T) -> T {
            self.env.as_contract(&self.contract, f)
        }
    }

    fn fixture() -> Fixture {
        let f = bare_fixture();
        let player = f.player.clone();
        f.run(|| player_profile::initialize_profile(&f.env, player).unwrap());
        f
    }

    /// Like [`fixture`] but the player has no profile, so rewards cannot land.
    fn bare_fixture() -> Fixture {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register(Stub, ());
        let author = Address::generate(&env);
        let player = Address::generate(&env);
        Fixture {
            env,
            contract,
            author,
            player,
        }
    }

    fn reward(essence: i128) -> QuestReward {
        QuestReward {
            essence,
            resource_type: ResourceType::StellarDust,
            resource_amount: 0,
            xp: 10,
            cosmetic_roll: false,
        }
    }

    fn branch(env: &Env, choice_id: u32, label: &str, next: u64) -> QuestBranch {
        QuestBranch {
            choice_id,
            label: Symbol::new(env, label),
            next_quest_id: next,
        }
    }

    fn branches(env: &Env, items: &[QuestBranch]) -> Vec<QuestBranch> {
        let mut v = Vec::new(env);
        for b in items {
            v.push_back(b.clone());
        }
        v
    }

    /// Add a node with sensible defaults for the fields a test does not care
    /// about, so call sites stay readable.
    fn add_node(
        f: &Fixture,
        chain_id: u64,
        objective: Symbol,
        target: u32,
        essence: i128,
        node_branches: Vec<QuestBranch>,
        duration_secs: u64,
    ) -> Result<u64, QuestError> {
        f.run(|| {
            add_quest_node(
                &f.env,
                f.author.clone(),
                chain_id,
                objective,
                target,
                reward(essence),
                symbol_short!("tier"),
                String::from_str(&f.env, "Title"),
                String::from_str(&f.env, "Description"),
                node_branches,
                duration_secs,
            )
        })
    }

    /// A three-node chain that branches after the root:
    ///
    /// ```text
    ///   node1 ─┬─ choice 1 ─► node2 ─► (end)
    ///          └─ choice 2 ─► node3 ─► (end)
    /// ```
    fn branching_chain(f: &Fixture) -> (u64, u64, u64, u64) {
        let chain_id =
            f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("prologue")).unwrap());

        // The root is added first (that is what makes it the root) and carries
        // forward references to nodes 2 and 3, which do not exist yet.
        let node1 = add_node(
            f,
            chain_id,
            symbol_short!("scan"),
            3,
            100,
            branches(
                &f.env,
                &[branch(&f.env, 1, "ally", 2), branch(&f.env, 2, "raid", 3)],
            ),
            0,
        )
        .unwrap();

        let node2 = add_node(
            f,
            chain_id,
            symbol_short!("trade"),
            2,
            250,
            branches(&f.env, &[branch(&f.env, 1, "finish", CHAIN_TERMINUS)]),
            0,
        )
        .unwrap();

        let node3 = add_node(
            f,
            chain_id,
            symbol_short!("harvest"),
            5,
            400,
            Vec::new(&f.env),
            0,
        )
        .unwrap();

        (chain_id, node1, node2, node3)
    }

    // ── Authoring ─────────────────────────────────────────────────────────

    #[test]
    fn chain_records_its_root_and_node_count() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);

        f.run(|| {
            let chain = get_chain(&f.env, chain_id).unwrap();
            assert_eq!(chain.root_quest_id, node1);
            assert_eq!(chain.node_count, 3);
            assert_eq!(chain.creator, f.author);
        });
    }

    #[test]
    fn node_step_indexes_follow_insertion_order() {
        let f = fixture();
        let (_, node1, node2, node3) = branching_chain(&f);

        f.run(|| {
            assert_eq!(get_quest_node(&f.env, node1).unwrap().step_index, 0);
            assert_eq!(get_quest_node(&f.env, node2).unwrap().step_index, 1);
            assert_eq!(get_quest_node(&f.env, node3).unwrap().step_index, 2);
        });
    }

    #[test]
    fn only_the_creator_may_extend_a_chain() {
        let f = fixture();
        let (chain_id, _, _, _) = branching_chain(&f);

        let result = f.run(|| {
            add_quest_node(
                &f.env,
                f.player.clone(),
                chain_id,
                symbol_short!("scan"),
                1,
                reward(1),
                symbol_short!("tier"),
                String::from_str(&f.env, "t"),
                String::from_str(&f.env, "d"),
                Vec::new(&f.env),
                0,
            )
        });
        assert_eq!(result, Err(QuestError::Unauthorized));
    }

    #[test]
    fn node_rejects_zero_target_and_zero_choice_ids() {
        let f = fixture();
        let chain_id =
            f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("c")).unwrap());

        assert_eq!(
            add_node(
                &f,
                chain_id,
                symbol_short!("scan"),
                0,
                1,
                Vec::new(&f.env),
                0
            ),
            Err(QuestError::InvalidTarget)
        );
        assert_eq!(
            add_node(
                &f,
                chain_id,
                symbol_short!("scan"),
                1,
                1,
                branches(&f.env, &[branch(&f.env, 0, "bad", CHAIN_TERMINUS)]),
                0
            ),
            Err(QuestError::InvalidBranch)
        );
    }

    #[test]
    fn node_rejects_too_many_branches() {
        let f = fixture();
        let chain_id =
            f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("c")).unwrap());

        let mut too_many = Vec::new(&f.env);
        for i in 1..=(MAX_BRANCHES_PER_NODE + 1) {
            too_many.push_back(branch(&f.env, i, "b", CHAIN_TERMINUS));
        }

        assert_eq!(
            add_node(&f, chain_id, symbol_short!("scan"), 1, 1, too_many, 0),
            Err(QuestError::TooManyBranches)
        );
    }

    #[test]
    fn chain_length_is_capped() {
        let f = fixture();
        let chain_id =
            f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("long")).unwrap());

        for _ in 0..MAX_CHAIN_LENGTH {
            add_node(
                &f,
                chain_id,
                symbol_short!("scan"),
                1,
                1,
                Vec::new(&f.env),
                0,
            )
            .unwrap();
        }

        assert_eq!(
            add_node(
                &f,
                chain_id,
                symbol_short!("scan"),
                1,
                1,
                Vec::new(&f.env),
                0
            ),
            Err(QuestError::ChainFull)
        );
    }

    #[test]
    fn adding_a_node_to_a_missing_chain_fails() {
        let f = fixture();
        assert_eq!(
            add_node(&f, 999, symbol_short!("scan"), 1, 1, Vec::new(&f.env), 0),
            Err(QuestError::ChainNotFound)
        );
    }

    #[test]
    fn validate_chain_accepts_a_complete_chain_and_rejects_dangling_edges() {
        let f = fixture();
        let (chain_id, _, _, _) = branching_chain(&f);
        f.run(|| validate_chain(&f.env, chain_id).unwrap());

        // A chain whose only node points at a node that was never authored.
        let broken =
            f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("broken")).unwrap());
        add_node(
            &f,
            broken,
            symbol_short!("scan"),
            1,
            1,
            branches(&f.env, &[branch(&f.env, 1, "nowhere", 9_999)]),
            0,
        )
        .unwrap();

        f.run(|| {
            assert_eq!(
                validate_chain(&f.env, broken),
                Err(QuestError::DanglingBranch)
            );
            assert_eq!(
                validate_chain(&f.env, 12_345),
                Err(QuestError::ChainNotFound)
            );
        });
    }

    // ── Starting ──────────────────────────────────────────────────────────

    #[test]
    fn starting_a_chain_activates_the_root_node() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);

        let state = f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        assert_eq!(state.quest_id, node1);
        assert_eq!(state.status, QuestStatus::Active);
        assert_eq!(state.target, 3);
        assert_eq!(state.progress, 0);

        f.run(|| {
            assert_eq!(get_active_quests(&f.env, &f.player).len(), 1);
            let progress = get_chain_progress(&f.env, &f.player, chain_id).unwrap();
            assert_eq!(progress.current_quest_id, node1);
            assert_eq!(progress.steps_completed, 0);
            assert!(!progress.finished);
        });
    }

    #[test]
    fn a_chain_cannot_be_started_twice() {
        let f = fixture();
        let (chain_id, _, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());

        f.run(|| {
            assert_eq!(
                start_chain(&f.env, f.player.clone(), chain_id),
                Err(QuestError::ChainAlreadyStarted)
            );
        });
    }

    #[test]
    fn starting_an_unknown_or_empty_chain_fails() {
        let f = fixture();
        f.run(|| {
            assert_eq!(
                start_chain(&f.env, f.player.clone(), 42),
                Err(QuestError::ChainNotFound)
            );
        });

        let empty =
            f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("empty")).unwrap());
        f.run(|| {
            assert_eq!(
                start_chain(&f.env, f.player.clone(), empty),
                Err(QuestError::QuestNotFound)
            );
        });
    }

    #[test]
    fn active_quests_are_capped_per_player() {
        let f = fixture();

        // Each single-node chain contributes one active quest.
        for _ in 0..MAX_ACTIVE_QUESTS {
            let chain_id =
                f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("c")).unwrap());
            add_node(
                &f,
                chain_id,
                symbol_short!("scan"),
                1,
                1,
                Vec::new(&f.env),
                0,
            )
            .unwrap();
            f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        }

        let extra =
            f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("extra")).unwrap());
        add_node(&f, extra, symbol_short!("scan"), 1, 1, Vec::new(&f.env), 0).unwrap();
        f.run(|| {
            assert_eq!(
                start_chain(&f.env, f.player.clone(), extra),
                Err(QuestError::TooManyActiveQuests)
            );
        });
    }

    // ── Progress tracking ─────────────────────────────────────────────────

    #[test]
    fn progress_accumulates_then_completes_at_the_target() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());

        let s1 = f.run(|| record_progress(&f.env, f.player.clone(), node1, 1).unwrap());
        assert_eq!(s1.progress, 1);
        assert_eq!(s1.status, QuestStatus::Active);

        let s2 = f.run(|| record_progress(&f.env, f.player.clone(), node1, 2).unwrap());
        assert_eq!(s2.progress, 3);
        assert_eq!(s2.status, QuestStatus::Completed);
        assert_eq!(s2.completed_at, f.env.ledger().timestamp());

        // Completing removes the quest from the active list.
        f.run(|| assert_eq!(get_active_quests(&f.env, &f.player).len(), 0));
    }

    #[test]
    fn progress_is_clamped_to_the_target() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());

        let state = f.run(|| record_progress(&f.env, f.player.clone(), node1, 99).unwrap());
        assert_eq!(state.progress, 3, "progress never exceeds the target");
    }

    #[test]
    fn progress_on_a_completed_quest_is_rejected() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());

        f.run(|| {
            assert_eq!(
                record_progress(&f.env, f.player.clone(), node1, 1),
                Err(QuestError::InvalidStatus)
            );
        });
    }

    #[test]
    fn progress_on_an_unstarted_quest_is_rejected() {
        let f = fixture();
        let (_, node1, _, _) = branching_chain(&f);
        f.run(|| {
            assert_eq!(
                record_progress(&f.env, f.player.clone(), node1, 1),
                Err(QuestError::QuestNotStarted)
            );
        });
    }

    #[test]
    fn a_quest_past_its_deadline_expires() {
        let f = fixture();
        let chain_id =
            f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("timed")).unwrap());
        let node = add_node(
            &f,
            chain_id,
            symbol_short!("scan"),
            5,
            10,
            Vec::new(&f.env),
            3_600,
        )
        .unwrap();
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());

        f.env.ledger().with_mut(|l| l.timestamp += 3_601);

        f.run(|| {
            assert_eq!(
                record_progress(&f.env, f.player.clone(), node, 1),
                Err(QuestError::QuestExpired)
            );
        });
        f.run(|| {
            let state = get_quest_state(&f.env, &f.player, node).unwrap();
            assert_eq!(state.status, QuestStatus::Expired);
            assert_eq!(get_active_quests(&f.env, &f.player).len(), 0);
        });
    }

    // ── Rewards ───────────────────────────────────────────────────────────

    #[test]
    fn claiming_pays_essence_and_marks_the_quest_claimed() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());

        let before = f.run(|| {
            player_profile::get_profile_by_owner(&f.env, &f.player)
                .unwrap()
                .essence_earned
        });
        let reward = f.run(|| claim_quest_reward(&f.env, f.player.clone(), node1).unwrap());
        let after = f.run(|| {
            player_profile::get_profile_by_owner(&f.env, &f.player)
                .unwrap()
                .essence_earned
        });

        assert_eq!(reward.essence, 100);
        assert_eq!(after - before, 100);
        f.run(|| {
            assert_eq!(
                get_quest_state(&f.env, &f.player, node1).unwrap().status,
                QuestStatus::Claimed
            );
        });
    }

    #[test]
    fn claiming_grants_resource_rewards() {
        let f = fixture();
        let chain_id =
            f.run(|| define_chain(&f.env, f.author.clone(), symbol_short!("res")).unwrap());
        let node = f
            .run(|| {
                add_quest_node(
                    &f.env,
                    f.author.clone(),
                    chain_id,
                    symbol_short!("harvest"),
                    1,
                    QuestReward {
                        essence: 0,
                        resource_type: ResourceType::DarkMatter,
                        resource_amount: 75,
                        xp: 5,
                        cosmetic_roll: true,
                    },
                    symbol_short!("tier"),
                    String::from_str(&f.env, "t"),
                    String::from_str(&f.env, "d"),
                    Vec::new(&f.env),
                    0,
                )
            })
            .unwrap();
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node, 1).unwrap());

        let reward = f.run(|| claim_quest_reward(&f.env, f.player.clone(), node).unwrap());
        assert!(reward.cosmetic_roll);
        f.run(|| {
            assert_eq!(
                resource_minter::balance_of(&f.env, &f.player, &ResourceType::DarkMatter),
                75
            );
        });
    }

    #[test]
    fn a_reward_cannot_be_claimed_twice() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());
        f.run(|| claim_quest_reward(&f.env, f.player.clone(), node1).unwrap());

        f.run(|| {
            assert_eq!(
                claim_quest_reward(&f.env, f.player.clone(), node1),
                Err(QuestError::AlreadyClaimed)
            );
        });
    }

    #[test]
    fn an_incomplete_quest_cannot_be_claimed() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 1).unwrap());

        f.run(|| {
            assert_eq!(
                claim_quest_reward(&f.env, f.player.clone(), node1),
                Err(QuestError::InvalidStatus)
            );
        });
    }

    #[test]
    fn a_player_without_a_profile_cannot_claim() {
        let f = bare_fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);

        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());
        f.run(|| {
            assert_eq!(
                claim_quest_reward(&f.env, f.player.clone(), node1),
                Err(QuestError::ProfileNotFound)
            );
        });
    }

    // ── Branching narrative ───────────────────────────────────────────────

    #[test]
    fn choosing_a_branch_activates_the_successor_and_records_the_path() {
        let f = fixture();
        let (chain_id, node1, node2, _node3) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());
        f.run(|| claim_quest_reward(&f.env, f.player.clone(), node1).unwrap());

        let next = f
            .run(|| choose_branch(&f.env, f.player.clone(), node1, 1).unwrap())
            .expect("choice 1 has a successor");
        assert_eq!(next.quest_id, node2);
        assert_eq!(next.status, QuestStatus::Active);
        assert_eq!(next.target, 2);

        f.run(|| {
            let progress = get_chain_progress(&f.env, &f.player, chain_id).unwrap();
            assert_eq!(progress.current_quest_id, node2);
            assert_eq!(progress.path.len(), 1);
            assert_eq!(progress.path.get(0).unwrap(), 1);
            assert_eq!(progress.steps_completed, 1);
            assert_eq!(progress.total_essence_earned, 100);
        });
    }

    #[test]
    fn the_other_branch_leads_somewhere_else() {
        let f = fixture();
        let (chain_id, node1, _node2, node3) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());
        f.run(|| claim_quest_reward(&f.env, f.player.clone(), node1).unwrap());

        let next = f
            .run(|| choose_branch(&f.env, f.player.clone(), node1, 2).unwrap())
            .expect("choice 2 has a successor");
        assert_eq!(next.quest_id, node3);
        assert_eq!(next.target, 5);
    }

    #[test]
    fn a_terminus_branch_finishes_the_chain() {
        let f = fixture();
        let (chain_id, node1, node2, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());
        f.run(|| claim_quest_reward(&f.env, f.player.clone(), node1).unwrap());
        f.run(|| choose_branch(&f.env, f.player.clone(), node1, 1).unwrap());

        f.run(|| record_progress(&f.env, f.player.clone(), node2, 2).unwrap());
        f.run(|| claim_quest_reward(&f.env, f.player.clone(), node2).unwrap());
        let end = f.run(|| choose_branch(&f.env, f.player.clone(), node2, 1).unwrap());

        assert!(end.is_none(), "a terminus branch has no successor");
        f.run(|| {
            let progress = get_chain_progress(&f.env, &f.player, chain_id).unwrap();
            assert!(progress.finished);
            assert_eq!(progress.steps_completed, 2);
            assert_eq!(progress.total_essence_earned, 350);
            assert_eq!(progress.path.len(), 2);
        });
    }

    #[test]
    fn a_branchless_node_finishes_the_chain_on_claim() {
        let f = fixture();
        let (chain_id, node1, _, node3) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());
        f.run(|| claim_quest_reward(&f.env, f.player.clone(), node1).unwrap());
        f.run(|| choose_branch(&f.env, f.player.clone(), node1, 2).unwrap());

        f.run(|| record_progress(&f.env, f.player.clone(), node3, 5).unwrap());
        f.run(|| claim_quest_reward(&f.env, f.player.clone(), node3).unwrap());

        f.run(|| {
            let progress = get_chain_progress(&f.env, &f.player, chain_id).unwrap();
            assert!(progress.finished, "node3 has no branches, so the path ends");
            assert_eq!(progress.total_essence_earned, 500);
        });
    }

    #[test]
    fn branching_before_claiming_is_rejected() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());

        f.run(|| {
            assert_eq!(
                choose_branch(&f.env, f.player.clone(), node1, 1),
                Err(QuestError::InvalidStatus)
            );
        });
    }

    #[test]
    fn a_branch_cannot_be_taken_twice() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());
        f.run(|| claim_quest_reward(&f.env, f.player.clone(), node1).unwrap());
        f.run(|| choose_branch(&f.env, f.player.clone(), node1, 1).unwrap());

        f.run(|| {
            assert_eq!(
                choose_branch(&f.env, f.player.clone(), node1, 2),
                Err(QuestError::InvalidStatus),
                "the narrative path is immutable once walked"
            );
        });
    }

    #[test]
    fn an_unknown_choice_is_rejected() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());
        f.run(|| record_progress(&f.env, f.player.clone(), node1, 3).unwrap());
        f.run(|| claim_quest_reward(&f.env, f.player.clone(), node1).unwrap());

        f.run(|| {
            assert_eq!(
                choose_branch(&f.env, f.player.clone(), node1, 99),
                Err(QuestError::InvalidBranch)
            );
        });
    }

    // ── Mission bridge ────────────────────────────────────────────────────

    #[test]
    fn mission_completion_advances_matching_quests_only() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());

        // node1's objective is `scan`; a `trade` mission must not advance it.
        f.run(|| {
            assert_eq!(
                on_mission_completed(&f.env, &f.player, symbol_short!("trade"), 1),
                0
            );
            assert_eq!(
                get_quest_state(&f.env, &f.player, node1).unwrap().progress,
                0
            );
        });

        f.run(|| {
            assert_eq!(
                on_mission_completed(&f.env, &f.player, symbol_short!("scan"), 2),
                1
            );
            assert_eq!(
                get_quest_state(&f.env, &f.player, node1).unwrap().progress,
                2
            );
        });
    }

    #[test]
    fn mission_completion_can_finish_a_quest() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());

        f.run(|| on_mission_completed(&f.env, &f.player, symbol_short!("scan"), 3));
        f.run(|| {
            assert_eq!(
                get_quest_state(&f.env, &f.player, node1).unwrap().status,
                QuestStatus::Completed
            );
        });
    }

    #[test]
    fn mission_completion_is_a_no_op_for_a_player_with_no_quests() {
        let f = fixture();
        f.run(|| {
            assert_eq!(
                on_mission_completed(&f.env, &f.player, symbol_short!("scan"), 5),
                0
            );
        });
    }

    // ── Queries ───────────────────────────────────────────────────────────

    #[test]
    fn active_quest_states_are_listed() {
        let f = fixture();
        let (chain_id, node1, _, _) = branching_chain(&f);
        f.run(|| start_chain(&f.env, f.player.clone(), chain_id).unwrap());

        f.run(|| {
            let states = get_active_quest_states(&f.env, &f.player);
            assert_eq!(states.len(), 1);
            assert_eq!(states.get(0).unwrap().quest_id, node1);
        });
    }

    #[test]
    fn queries_return_none_for_unknown_ids() {
        let f = fixture();
        f.run(|| {
            assert!(get_chain(&f.env, 1).is_none());
            assert!(get_quest_node(&f.env, 1).is_none());
            assert!(get_quest_state(&f.env, &f.player, 1).is_none());
            assert!(get_chain_progress(&f.env, &f.player, 1).is_none());
            assert_eq!(get_active_quests(&f.env, &f.player).len(), 0);
        });
    }
}
