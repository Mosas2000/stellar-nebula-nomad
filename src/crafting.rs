//! Recipe-based resource crafting.
//!
use crate::recipes::{
    get_recipe, get_recipe_specialization, is_rare, is_unlocked, unlock_rare_recipe, Specialization,
};
use soroban_sdk::{contracterror, symbol_short, Address, Env, Symbol, Vec};

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CraftingError {
    RecipeLocked = 1,
    InsufficientLevel = 2,
    InsufficientResources = 3,
    RecipeNotFound = 4,
    /// Player already chose a specialization; it cannot be changed (Issue #266).
    SpecializationAlreadyChosen = 5,
    /// The recipe requires a specialization the player has not chosen.
    WrongSpecialization = 6,
    /// The requested skill node does not exist.
    NodeNotFound = 7,
    /// The skill node was already unlocked.
    NodeAlreadyUnlocked = 8,
    /// Not enough skill points to unlock this node.
    InsufficientSkillPoints = 9,
}

#[soroban_sdk::contracttype]
pub enum CraftingDataKey {
    PlayerLevel(Address),
    PlayerXP(Address),
    /// Player's chosen crafting specialization, if any (Issue #266).
    PlayerSpecialization(Address),
    /// Unspent skill points, earned on level-up.
    SkillPoints(Address),
    /// Whether (player, node_id) has been unlocked.
    UnlockedNode(Address, u32),
    /// Number of times (player, recipe_id) has been successfully crafted —
    /// drives the mastery output bonus.
    MasteryCount(Address, u32),
}

// ── Skill Tree (Issue #266) ─────────────────────────────────────────────────
//
// A small, fixed skill tree per specialization: (node_id, specialization, cost).
// The first node in each branch ("keen eye") boosts rare-recipe discovery odds.

/// (node_id, specialization, skill point cost).
const SKILL_TREE: &[(u32, Specialization, u32)] = &[
    (1, Specialization::Metallurgy, 1),
    (2, Specialization::Metallurgy, 2),
    (3, Specialization::Alchemy, 1),
    (4, Specialization::Alchemy, 2),
    (5, Specialization::Engineering, 1),
    (6, Specialization::Engineering, 2),
];

/// Nodes that boost rare-recipe discovery odds when unlocked (the "keen eye"
/// node in each branch).
const DISCOVERY_NODES: &[u32] = &[1, 3, 5];

/// Crafts of the same recipe between each +1 mastery output bonus.
const MASTERY_INTERVAL: u32 = 10;

/// Base rare-recipe discovery chance out of 100.
const BASE_DISCOVERY_PCT: u64 = 5;
/// Boosted discovery chance (with a "keen eye" node unlocked) out of 100.
const BOOSTED_DISCOVERY_PCT: u64 = 10;

pub fn craft(env: Env, player: Address, recipe_id: u32) -> Result<(), CraftingError> {
    player.require_auth();
    let recipe = get_recipe(&env, recipe_id).map_err(|_| CraftingError::RecipeNotFound)?;

    if is_rare(&recipe) && !is_unlocked(&env, &player, recipe_id) {
        return Err(CraftingError::RecipeLocked);
    }

    // Specialization gate: recipes tagged via set_recipe_specialization
    // require the player to have chosen the matching tree (Issue #266).
    if let Some(required_spec) = get_recipe_specialization(&env, recipe_id) {
        if get_specialization(&env, &player) != Some(required_spec) {
            return Err(CraftingError::WrongSpecialization);
        }
    }

    let level = get_level(&env, player.clone());
    if level < recipe.required_level {
        return Err(CraftingError::InsufficientLevel);
    }

    require_resources(&env, &player, &recipe.inputs)?;
    consume_resources(&env, &player, &recipe.inputs);

    // Mastery bonus: every MASTERY_INTERVAL crafts of the same recipe grants
    // +1 extra output (Issue #266).
    let mastery_count = record_craft(&env, &player, recipe_id);
    let bonus_output = mastery_count / MASTERY_INTERVAL;
    let (output_symbol, base_amount) = recipe.output.clone();
    mint_resource(
        &env,
        &player,
        (output_symbol, base_amount.saturating_add(bonus_output)),
    );

    let xp_gain = 10 + (recipe.rarity * 5);
    add_xp(&env, player.clone(), xp_gain);

    // Recipe discovery: base 5% chance, boosted to 10% with a "keen eye"
    // skill node unlocked (Issue #266).
    let discovery_pct = if has_discovery_boost(&env, &player) {
        BOOSTED_DISCOVERY_PCT
    } else {
        BASE_DISCOVERY_PCT
    };
    let random: u64 = env.prng().gen();
    if random % 100 < discovery_pct {
        unlock_rare_recipe(&env, player.clone(), 999);
        env.events().publish(
            (symbol_short!("rare_dis"), player.clone()),
            symbol_short!("unlocked"),
        );
    }

    Ok(())
}

pub fn add_xp(env: &Env, player: Address, xp: u32) {
    let current_xp = get_xp(env, player.clone());
    let new_xp = current_xp + xp;

    let old_level = get_level(env, player.clone());
    let new_level = 1 + (new_xp / 100);

    env.storage()
        .persistent()
        .set(&CraftingDataKey::PlayerXP(player.clone()), &new_xp);

    if new_level > old_level {
        env.storage()
            .persistent()
            .set(&CraftingDataKey::PlayerLevel(player.clone()), &new_level);

        // Award 1 skill point per level gained (Issue #266).
        let levels_gained = new_level - old_level;
        let points = get_skill_points(env, player.clone()).saturating_add(levels_gained);
        env.storage()
            .persistent()
            .set(&CraftingDataKey::SkillPoints(player.clone()), &points);

        env.events()
            .publish((symbol_short!("levelup"), player), new_level);
    }
}

// ── Skill Tree API (Issue #266) ─────────────────────────────────────────────

pub fn get_skill_points(env: &Env, player: Address) -> u32 {
    env.storage()
        .persistent()
        .get(&CraftingDataKey::SkillPoints(player))
        .unwrap_or(0)
}

pub fn get_specialization(env: &Env, player: &Address) -> Option<Specialization> {
    env.storage()
        .persistent()
        .get(&CraftingDataKey::PlayerSpecialization(player.clone()))
}

/// Choose a crafting specialization. One-time — cannot be changed afterward.
pub fn choose_specialization(
    env: &Env,
    player: Address,
    specialization: Specialization,
) -> Result<(), CraftingError> {
    player.require_auth();

    if get_specialization(env, &player).is_some() {
        return Err(CraftingError::SpecializationAlreadyChosen);
    }

    env.storage().persistent().set(
        &CraftingDataKey::PlayerSpecialization(player.clone()),
        &specialization,
    );

    env.events()
        .publish((symbol_short!("spec"), player), symbol_short!("chosen"));

    Ok(())
}

pub fn is_node_unlocked(env: &Env, player: &Address, node_id: u32) -> bool {
    env.storage()
        .persistent()
        .get(&CraftingDataKey::UnlockedNode(player.clone(), node_id))
        .unwrap_or(false)
}

/// Spend skill points to unlock a node in the player's chosen specialization
/// tree. The node must belong to the player's specialization, must not
/// already be unlocked, and the player must have enough skill points.
pub fn unlock_skill_node(env: &Env, player: Address, node_id: u32) -> Result<(), CraftingError> {
    player.require_auth();

    let (_, node_spec, cost) = SKILL_TREE
        .iter()
        .find(|(id, _, _)| *id == node_id)
        .ok_or(CraftingError::NodeNotFound)?;

    let player_spec = get_specialization(env, &player).ok_or(CraftingError::WrongSpecialization)?;
    if player_spec != *node_spec {
        return Err(CraftingError::WrongSpecialization);
    }

    if is_node_unlocked(env, &player, node_id) {
        return Err(CraftingError::NodeAlreadyUnlocked);
    }

    let points = get_skill_points(env, player.clone());
    if points < *cost {
        return Err(CraftingError::InsufficientSkillPoints);
    }

    env.storage().persistent().set(
        &CraftingDataKey::SkillPoints(player.clone()),
        &(points - *cost),
    );
    env.storage().persistent().set(
        &CraftingDataKey::UnlockedNode(player.clone(), node_id),
        &true,
    );

    env.events()
        .publish((symbol_short!("skill"), player), node_id);

    Ok(())
}

fn has_discovery_boost(env: &Env, player: &Address) -> bool {
    DISCOVERY_NODES
        .iter()
        .any(|id| is_node_unlocked(env, player, *id))
}

pub fn get_mastery_count(env: &Env, player: &Address, recipe_id: u32) -> u32 {
    env.storage()
        .persistent()
        .get(&CraftingDataKey::MasteryCount(player.clone(), recipe_id))
        .unwrap_or(0)
}

fn record_craft(env: &Env, player: &Address, recipe_id: u32) -> u32 {
    let count = get_mastery_count(env, player, recipe_id).saturating_add(1);
    env.storage().persistent().set(
        &CraftingDataKey::MasteryCount(player.clone(), recipe_id),
        &count,
    );
    count
}

pub fn get_level(env: &Env, player: Address) -> u32 {
    env.storage()
        .persistent()
        .get(&CraftingDataKey::PlayerLevel(player))
        .unwrap_or(1)
}

pub fn get_xp(env: &Env, player: Address) -> u32 {
    env.storage()
        .persistent()
        .get(&CraftingDataKey::PlayerXP(player))
        .unwrap_or(0)
}

fn require_resources(
    env: &Env,
    player: &Address,
    inputs: &Vec<(Symbol, u32)>,
) -> Result<(), CraftingError> {
    for input in inputs.iter() {
        let (symbol, required) = input;
        let balance = get_resource_balance(env, player, symbol);
        if balance < required {
            return Err(CraftingError::InsufficientResources);
        }
    }
    Ok(())
}

fn consume_resources(env: &Env, player: &Address, inputs: &Vec<(Symbol, u32)>) {
    for input in inputs.iter() {
        let (symbol, amount) = input;
        let balance = get_resource_balance(env, player, symbol.clone());
        set_resource_balance(env, player, symbol, balance - amount);
    }
}

fn mint_resource(env: &Env, player: &Address, output: (Symbol, u32)) {
    let (symbol, amount) = output;
    let balance = get_resource_balance(env, player, symbol.clone());
    set_resource_balance(env, player, symbol, balance + amount);

    env.events()
        .publish((symbol_short!("crafted"), player.clone()), amount);
}

fn get_resource_balance(env: &Env, player: &Address, symbol: Symbol) -> u32 {
    let key = (symbol_short!("res_bal"), player.clone(), symbol);
    env.storage().instance().get(&key).unwrap_or(0)
}

fn set_resource_balance(env: &Env, player: &Address, symbol: Symbol, amount: u32) {
    let key = (symbol_short!("res_bal"), player.clone(), symbol);
    env.storage().instance().set(&key, &amount);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipes::{self, Recipe};
    use soroban_sdk::{testutils::Address as _, Env, Symbol, Vec};

    use soroban_sdk::{contract, contractimpl};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    fn make_env() -> (Env, soroban_sdk::Address) {
        let env = Env::default();
        let id = env.register(Stub, ());
        (env, id)
    }

    fn seed_resource(env: &Env, player: &Address, sym: Symbol, amount: u32) {
        let key = (symbol_short!("res_bal"), player.clone(), sym);
        env.storage().instance().set(&key, &amount);
    }

    fn make_recipe(env: &Env, id: u32, rarity: u32, input: Symbol, output: Symbol) -> Recipe {
        let mut inputs = Vec::new(env);
        inputs.push_back((input, 5u32));
        Recipe {
            id,
            inputs,
            output: (output, 1u32),
            rarity,
            required_level: 1,
        }
    }

    #[test]
    fn test_craft_common_recipe_succeeds() {
        let (env, id) = make_env();
        let player = Address::generate(&env);
        let iron = Symbol::new(&env, "iron");
        let steel = Symbol::new(&env, "steel");

        env.mock_all_auths();
        env.as_contract(&id, || {
            let recipe = make_recipe(&env, 1, 1, iron.clone(), steel.clone());
            recipes::set_recipe(&env, &recipe);

            seed_resource(&env, &player, iron.clone(), 10);

            assert!(craft(env.clone(), player.clone(), 1).is_ok());

            let key = (symbol_short!("res_bal"), player.clone(), steel.clone());
            let out_bal: u32 = env.storage().instance().get(&key).unwrap_or(0);
            assert_eq!(out_bal, 1);
        });
    }

    #[test]
    fn test_craft_locked_rare_returns_error() {
        let (env, id) = make_env();
        let player = Address::generate(&env);
        let crystal = Symbol::new(&env, "crystal");
        let gem = Symbol::new(&env, "gem");

        env.mock_all_auths();
        env.as_contract(&id, || {
            let recipe = make_recipe(&env, 10, 3, crystal.clone(), gem.clone());
            recipes::set_recipe(&env, &recipe);

            seed_resource(&env, &player, crystal.clone(), 10);

            let result = craft(env.clone(), player.clone(), 10);
            assert_eq!(result, Err(CraftingError::RecipeLocked));
        });
    }

    #[test]
    fn test_craft_after_unlock_succeeds() {
        let (env, id) = make_env();
        let player = Address::generate(&env);
        let crystal = Symbol::new(&env, "crystal");
        let gem = Symbol::new(&env, "gem");

        env.mock_all_auths();
        env.as_contract(&id, || {
            let recipe = make_recipe(&env, 10, 3, crystal.clone(), gem.clone());
            recipes::set_recipe(&env, &recipe);

            recipes::unlock_rare_recipe(&env, player.clone(), 10);

            seed_resource(&env, &player, crystal.clone(), 10);

            assert!(craft(env.clone(), player.clone(), 10).is_ok());

            let key = (symbol_short!("res_bal"), player.clone(), gem.clone());
            let out_bal: u32 = env.storage().instance().get(&key).unwrap_or(0);
            assert_eq!(out_bal, 1);
        });
    }

    #[test]
    fn test_craft_insufficient_resources_returns_error() {
        let (env, id) = make_env();
        let player = Address::generate(&env);
        let iron = Symbol::new(&env, "iron");
        let steel = Symbol::new(&env, "steel");

        env.mock_all_auths();
        env.as_contract(&id, || {
            let recipe = make_recipe(&env, 1, 1, iron.clone(), steel.clone());
            recipes::set_recipe(&env, &recipe);

            let result = craft(env.clone(), player.clone(), 1);
            assert_eq!(result, Err(CraftingError::InsufficientResources));
        });
    }

    #[test]
    fn test_craft_recipe_not_found_returns_error() {
        let (env, id) = make_env();
        let player = Address::generate(&env);

        env.mock_all_auths();
        env.as_contract(&id, || {
            let result = craft(env.clone(), player.clone(), 999);
            assert_eq!(result, Err(CraftingError::RecipeNotFound));
        });
    }

    // ── Skill Trees (Issue #266) ────────────────────────────────────────────

    #[test]
    fn test_choose_specialization_once_only() {
        let (env, id) = make_env();
        let player = Address::generate(&env);

        env.mock_all_auths();
        env.as_contract(&id, || {
            choose_specialization(&env, player.clone(), Specialization::Metallurgy).unwrap();
            assert_eq!(
                get_specialization(&env, &player),
                Some(Specialization::Metallurgy)
            );

            let result = choose_specialization(&env, player.clone(), Specialization::Alchemy);
            assert_eq!(result, Err(CraftingError::SpecializationAlreadyChosen));
        });
    }

    #[test]
    fn test_craft_gated_by_specialization() {
        let (env, id) = make_env();
        let player = Address::generate(&env);
        let ore = Symbol::new(&env, "ore");
        let ingot = Symbol::new(&env, "ingot");

        env.mock_all_auths();
        env.as_contract(&id, || {
            let recipe = make_recipe(&env, 1, 1, ore.clone(), ingot.clone());
            recipes::set_recipe(&env, &recipe);
            recipes::set_recipe_specialization(&env, 1, Specialization::Metallurgy);

            seed_resource(&env, &player, ore.clone(), 10);

            // No specialization chosen yet.
            let result = craft(env.clone(), player.clone(), 1);
            assert_eq!(result, Err(CraftingError::WrongSpecialization));

            // Wrong specialization chosen.
            choose_specialization(&env, player.clone(), Specialization::Alchemy).unwrap();
            let result = craft(env.clone(), player.clone(), 1);
            assert_eq!(result, Err(CraftingError::WrongSpecialization));
        });
    }

    #[test]
    fn test_craft_succeeds_with_matching_specialization() {
        let (env, id) = make_env();
        let player = Address::generate(&env);
        let ore = Symbol::new(&env, "ore");
        let ingot = Symbol::new(&env, "ingot");

        env.mock_all_auths();
        env.as_contract(&id, || {
            let recipe = make_recipe(&env, 1, 1, ore.clone(), ingot.clone());
            recipes::set_recipe(&env, &recipe);
            recipes::set_recipe_specialization(&env, 1, Specialization::Metallurgy);

            seed_resource(&env, &player, ore.clone(), 10);
            choose_specialization(&env, player.clone(), Specialization::Metallurgy).unwrap();

            assert!(craft(env.clone(), player.clone(), 1).is_ok());
        });
    }

    #[test]
    fn test_unlock_skill_node_flow() {
        let (env, id) = make_env();
        let player = Address::generate(&env);

        env.mock_all_auths();
        env.as_contract(&id, || {
            choose_specialization(&env, player.clone(), Specialization::Metallurgy).unwrap();

            // No skill points yet.
            let result = unlock_skill_node(&env, player.clone(), 1);
            assert_eq!(result, Err(CraftingError::InsufficientSkillPoints));

            // Level the player up to 3 (100 xp per level) => 2 skill points.
            add_xp(&env, player.clone(), 250);
            assert_eq!(get_skill_points(&env, player.clone()), 2);

            // Node 1 is Metallurgy, cost 1.
            unlock_skill_node(&env, player.clone(), 1).unwrap();
            assert!(is_node_unlocked(&env, &player, 1));
            assert_eq!(get_skill_points(&env, player.clone()), 1);

            // Already unlocked.
            let result = unlock_skill_node(&env, player.clone(), 1);
            assert_eq!(result, Err(CraftingError::NodeAlreadyUnlocked));

            // Node 3 belongs to Alchemy, not the player's Metallurgy tree.
            let result = unlock_skill_node(&env, player.clone(), 3);
            assert_eq!(result, Err(CraftingError::WrongSpecialization));

            // Unknown node.
            let result = unlock_skill_node(&env, player.clone(), 999);
            assert_eq!(result, Err(CraftingError::NodeNotFound));
        });
    }

    #[test]
    fn test_discovery_boost_active_with_keen_eye_node() {
        let (env, id) = make_env();
        let player = Address::generate(&env);

        env.mock_all_auths();
        env.as_contract(&id, || {
            assert!(!has_discovery_boost(&env, &player));

            choose_specialization(&env, player.clone(), Specialization::Engineering).unwrap();
            add_xp(&env, player.clone(), 100); // level 2 => 1 skill point
            unlock_skill_node(&env, player.clone(), 5).unwrap(); // Engineering keen-eye node

            assert!(has_discovery_boost(&env, &player));
        });
    }

    #[test]
    fn test_mastery_bonus_grants_extra_output_after_interval() {
        let (env, id) = make_env();
        let player = Address::generate(&env);
        let iron = Symbol::new(&env, "iron");
        let steel = Symbol::new(&env, "steel");

        env.mock_all_auths();
        env.as_contract(&id, || {
            let recipe = make_recipe(&env, 1, 1, iron.clone(), steel.clone());
            recipes::set_recipe(&env, &recipe);
            seed_resource(&env, &player, iron.clone(), 5 * MASTERY_INTERVAL);

            for _ in 0..MASTERY_INTERVAL {
                craft(env.clone(), player.clone(), 1).unwrap();
            }

            assert_eq!(get_mastery_count(&env, &player, 1), MASTERY_INTERVAL);

            let key = (symbol_short!("res_bal"), player.clone(), steel.clone());
            let out_bal: u32 = env.storage().instance().get(&key).unwrap_or(0);
            // 9 crafts at base output 1, plus the 10th craft's +1 mastery bonus.
            assert_eq!(out_bal, MASTERY_INTERVAL + 1);
        });
    }
}
