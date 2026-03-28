use soroban_sdk::{
    contracterror, contracttype, symbol_short, vec, Address, Env, Vec, Map, Symbol,
};

/// Maximum batch size for recycle/craft operations.
pub const RECYCLE_CRAFT_BATCH_SIZE: u32 = 8;

/// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum RecyclingKey {
    /// Recipe data keyed by recipe_id.
    Recipe(u64),
    /// Auto-incrementing recipe ID counter.
    RecipeCounter,
    /// Player-specific crafting stats (optional, for efficiency multipliers).
    PlayerStats(Address),
}

/// ─── Errors ─────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RecyclingError {
    /// Recipe does not exist.
    InvalidRecipe = 1,
    /// Inputs do not match recipe requirements.
    InvalidInputs = 2,
    /// Batch size exceeds limit.
    BatchTooLarge = 3,
    /// Insufficient resources to craft.
    InsufficientResources = 4,
    /// Crafted item already exists (idempotency guard).
    AlreadyCrafted = 5,
}

/// ─── Data Types ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct Recipe {
    pub id: u64,
    pub inputs: Vec<Symbol>,
    pub outputs: Vec<Symbol>,
    pub input_quantities: Vec<u32>,
    pub output_quantities: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct CraftingResult {
    pub recipe_id: u64,
    pub inputs_consumed: Vec<Symbol>,
    pub outputs_produced: Vec<Symbol>,
    pub crafted_at: u64,
}

/// ─── Public API ─────────────────────────────────────────────────────────────

/// Initialize the recipe library at deployment.
pub fn initialize_recycling(env: &Env) {
    if !env
        .storage()
        .instance()
        .has(&RecyclingKey::RecipeCounter)
    {
        env.storage()
            .instance()
            .set(&RecyclingKey::RecipeCounter, &0u64);
        // Load default recipes
        load_default_recipes(env);
    }
}

/// Load a small set of default recipes (placeholder).
fn load_default_recipes(env: &Env) {
    // Recipe 1: Recycle 2 ore -> 1 dust
    add_recipe(
        env,
        Vec::from_array(env, [symbol_short!("ore")]),
        Vec::from_array(env, [symbol_short!("dust")]),
        Vec::from_array(env, [2]),
        Vec::from_array(env, [1]),
    );
    // Recipe 2: Recycle 1 dust + 1 gas -> 1 exotic
    add_recipe(
        env,
        Vec::from_array(env, [symbol_short!("dust"), symbol_short!("gas")]),
        Vec::from_array(env, [symbol_short!("exotic")]),
        Vec::from_array(env, [1, 1]),
        Vec::from_array(env, [1]),
    );
}

/// Internal helper to add a recipe.
fn add_recipe(
    env: &Env,
    inputs: Vec<Symbol>,
    outputs: Vec<Symbol>,
    input_qts: Vec<u32>,
    output_qts: Vec<u32>,
) -> u64 {
    let counter: u64 = env
        .storage()
        .instance()
        .get(&RecyclingKey::RecipeCounter)
        .unwrap_or(0);
    let next_id = counter + 1;
    env.storage()
        .instance()
        .set(&RecyclingKey::RecipeCounter, &next_id);

    let recipe = Recipe {
        id: next_id,
        inputs,
        outputs,
        input_quantities: input_qts,
        output_quantities: output_qts,
    };

    env.storage()
        .instance()
        .set(&RecyclingKey::Recipe(next_id), &recipe);

    next_id
}

/// Recycle a resource into base materials (simple 1:many conversion).
/// For now, we treat any single resource as recyclable into dust with a fixed ratio.
pub fn recycle_resource(
    env: &Env,
    caller: &Address,
    resource: Symbol,
    amount: u32,
) -> Result<Vec<(Symbol, u32)>, RecyclingError> {
    caller.require_auth();

    // Simple placeholder: recycle any resource into dust at 50% rate.
    let dust_amount = amount / 2;
    if dust_amount == 0 {
        return Err(RecyclingError::InsufficientResources);
    }

    // Emit ResourceRecycled event
    env.events().publish(
        (symbol_short!("recycle"), symbol_short!("resource")),
        (
            caller.clone(),
            resource,
            amount,
            symbol_short!("dust"),
            dust_amount,
            env.ledger().timestamp(),
        ),
    );

    Ok(Vec::from_array(env, [(symbol_short!("dust"), dust_amount)]))
}

/// Craft a new item using a recipe, with optional efficiency multiplier based on ship stats (placeholder).
pub fn craft_new_item(
    env: &Env,
    caller: &Address,
    recipe_id: u64,
    inputs: Vec<Symbol>,
    quantities: Vec<u32>,
) -> Result<CraftingResult, RecyclingError> {
    caller.require_auth();

    if (inputs.len() as u32) > RECYCLE_CRAFT_BATCH_SIZE {
        return Err(RecyclingError::BatchTooLarge);
    }

    let recipe: Recipe = env
        .storage()
        .instance()
        .get(&RecyclingKey::Recipe(recipe_id))
        .ok_or(RecyclingError::InvalidRecipe)?;

    // Validate inputs match recipe
    if inputs.len() != recipe.inputs.len() || quantities.len() != recipe.input_quantities.len() {
        return Err(RecyclingError::InvalidInputs);
    }
    for i in 0..inputs.len() {
        if inputs.get(i).unwrap() != recipe.inputs.get(i).unwrap()
            || quantities.get(i).unwrap() != recipe.input_quantities.get(i).unwrap()
        {
            return Err(RecyclingError::InvalidInputs);
        }
    }

    // Efficiency multiplier: placeholder 1.0 (could be derived from ship stats)
    let efficiency_multiplier = 1.0;
    let mut final_outputs = Vec::new(env);
    let output_quantities = recipe.output_quantities;
    for i in 0..recipe.outputs.len() {
        let base_qty = output_quantities.get(i).unwrap();
        let boosted_qty = (base_qty as f32 * efficiency_multiplier) as u32;
        final_outputs.push_back(recipe.outputs.get(i).unwrap().clone());
    }

    let result = CraftingResult {
        recipe_id,
        inputs_consumed: inputs.clone(),
        outputs_produced: final_outputs.clone(),
        crafted_at: env.ledger().timestamp(),
    };

    // Emit ItemCrafted event
    env.events().publish(
        (symbol_short!("craft"), symbol_short!("item")),
        (
            caller.clone(),
            recipe_id,
            inputs,
            final_outputs,
            result.crafted_at,
        ),
    );

    Ok(result)
}

/// Get recipe by ID.
pub fn get_recipe(env: &Env, recipe_id: u64) -> Option<Recipe> {
    env.storage()
        .instance()
        .get(&RecyclingKey::Recipe(recipe_id))
}
