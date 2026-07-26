use soroban_sdk::{contracterror, contracttype, Address, Env, Symbol, Vec};

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RecipeError {
    RecipeNotFound = 1,
}

// ── Rare rarity threshold ─────────────────────────────────────────────────────

/// Recipes with rarity >= this value are considered rare and require an unlock.
pub const RARE_RARITY_THRESHOLD: u32 = 3;

// ── Data Types ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    pub id: u32,
    pub inputs: Vec<(Symbol, u32)>,
    pub output: (Symbol, u32),
    /// Rarity tier (1 = common, 2 = uncommon, 3+ = rare). u32 for contracttype compat.
    pub rarity: u32,
    pub required_level: u32,
}

/// Crafting skill specialization a recipe can belong to (Issue #266).
///
/// Recipes are not required to belong to a specialization — only recipes
/// tagged via [`set_recipe_specialization`] gate on the player's chosen tree.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Specialization {
    Metallurgy,
    Alchemy,
    Engineering,
}

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum RecipeKey {
    Recipe(u32),
    PlayerRareUnlocked(Address, u32),
    /// Specialization a recipe is tagged with, if any (Issue #266).
    RecipeSpecialization(u32),
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns true if the recipe's rarity meets or exceeds RARE_RARITY_THRESHOLD.
pub fn is_rare(recipe: &Recipe) -> bool {
    recipe.rarity >= RARE_RARITY_THRESHOLD
}

/// Returns true if the player has unlocked the given rare recipe.
pub fn is_unlocked(env: &Env, player: &Address, recipe_id: u32) -> bool {
    env.storage()
        .instance()
        .get(&RecipeKey::PlayerRareUnlocked(player.clone(), recipe_id))
        .unwrap_or(false)
}

// ── CRUD ──────────────────────────────────────────────────────────────────────

pub fn get_recipe(env: &Env, id: u32) -> Result<Recipe, RecipeError> {
    env.storage()
        .instance()
        .get(&RecipeKey::Recipe(id))
        .ok_or(RecipeError::RecipeNotFound)
}

pub fn set_recipe(env: &Env, recipe: &Recipe) {
    env.storage()
        .instance()
        .set(&RecipeKey::Recipe(recipe.id), recipe);
}

pub fn unlock_rare_recipe(env: &Env, player: Address, recipe_id: u32) {
    env.storage()
        .instance()
        .set(&RecipeKey::PlayerRareUnlocked(player, recipe_id), &true);
}

/// Tag a recipe as belonging to a crafting specialization tree (Issue #266).
///
/// Untagged recipes remain craftable by anyone regardless of specialization.
pub fn set_recipe_specialization(env: &Env, recipe_id: u32, specialization: Specialization) {
    env.storage()
        .instance()
        .set(&RecipeKey::RecipeSpecialization(recipe_id), &specialization);
}

/// The specialization a recipe is tagged with, if any.
pub fn get_recipe_specialization(env: &Env, recipe_id: u32) -> Option<Specialization> {
    env.storage()
        .instance()
        .get(&RecipeKey::RecipeSpecialization(recipe_id))
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn test_recipe_specialization_defaults_to_none() {
        let (env, id) = make_env();
        env.as_contract(&id, || {
            assert_eq!(get_recipe_specialization(&env, 1), None);
        });
    }

    #[test]
    fn test_set_and_get_recipe_specialization() {
        let (env, id) = make_env();
        env.as_contract(&id, || {
            set_recipe_specialization(&env, 1, Specialization::Alchemy);
            assert_eq!(
                get_recipe_specialization(&env, 1),
                Some(Specialization::Alchemy)
            );
            // Untagged recipes are unaffected.
            assert_eq!(get_recipe_specialization(&env, 2), None);
        });
    }
}
