//! Loot box system (Issue #287): provably-fair RNG, transparent odds,
//! no-real-money regulatory compliance.
//!
//! ## Provable fairness
//! `randomness_oracle` already provides a manipulation-resistant on-chain
//! seed (ledger sequence + timestamp + network id + rolling entropy pool),
//! but that alone only proves the *contract* didn't cherry-pick a seed —
//! it can't prove the *player* wasn't shown a pre-computed favorable
//! outcome. This module adds a commit-reveal layer on top of it: the player
//! commits to a secret seed's hash before the box is opened, the contract
//! locks in the oracle's server seed at that same moment, and only later
//! does the player reveal their secret — at which point the final outcome
//! is `sha256(player_seed || server_seed)`. Because both seeds were fixed
//! *before* the reveal, and the mapping from that hash to a reward is a
//! public, fixed odds table, anyone can independently recompute the result
//! from the emitted event and confirm it matches — that's what "provably
//! fair" actually means, not just "hard to predict."
//!
//! ## No real money (Issue #287: "NonGambling")
//! Loot boxes are opened using `LootToken`, a currency tracked entirely
//! within this module that can only ever be *earned* (via
//! `grant_loot_tokens`, gated to the admin/game-logic caller for completing
//! achievements, tournaments, etc.) and can never be transferred or
//! purchased. There is no path from real money to a loot box outcome.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Bytes, BytesN, Env, Vec};

use crate::randomness_oracle;
use crate::resource_minter::{self, ResourceType};

// ── Error ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum LootError {
    /// Admin already set — `set_loot_admin` is a one-time initializer.
    AlreadyInitialized = 1,
    /// Caller is not the loot admin.
    Unauthorized = 2,
    /// Loot box type not found.
    BoxTypeNotFound = 3,
    /// Odds table is empty, or its weights don't sum to exactly 10_000 bps.
    InvalidOddsTable = 4,
    /// Player doesn't have enough `LootToken` to open this box.
    InsufficientLootTokens = 5,
    /// Open request not found.
    RequestNotFound = 6,
    /// This request was already revealed.
    AlreadyRevealed = 7,
    /// The revealed seed doesn't hash to the value committed at open time.
    SeedMismatch = 8,
    /// Caller doesn't own this open request.
    NotRequestOwner = 9,
}

// ── Storage Keys ────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum LootDataKey {
    Admin,
    BoxType(u64),
    BoxTypeCounter,
    /// Non-transferable, earn-only currency balance.
    LootTokenBalance(Address),
    OpenRequest(u64),
    OpenRequestCounter,
}

// ── Data Types ────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct LootEntry {
    pub resource_type: ResourceType,
    pub amount: u64,
    /// Basis-point weight of this entry — the transparent odds table.
    pub weight_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct LootBoxType {
    pub box_type_id: u64,
    pub name: soroban_sdk::String,
    pub cost_loot_tokens: u64,
    /// Weights must sum to exactly 10_000 — this *is* the published odds
    /// table; it's stored on-chain and readable by anyone via
    /// `get_box_type`, not hidden server-side.
    pub entries: Vec<LootEntry>,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct OpenRequest {
    pub request_id: u64,
    pub player: Address,
    pub box_type_id: u64,
    /// sha256(player_seed) — committed before the server seed is known to
    /// have been "chosen" for this specific request.
    pub committed_hash: BytesN<32>,
    /// The oracle's seed, locked in at commit time.
    pub server_seed: BytesN<32>,
    pub revealed: bool,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct LootResult {
    pub request_id: u64,
    pub player: Address,
    pub box_type_id: u64,
    pub resource_type: ResourceType,
    pub amount: u64,
    pub player_seed: BytesN<32>,
    pub server_seed: BytesN<32>,
}

// ── Admin ───────────────────────────────────────────────────────────────

pub fn set_loot_admin(env: &Env, admin: &Address) -> Result<(), LootError> {
    admin.require_auth();
    if env
        .storage()
        .instance()
        .get::<_, Address>(&LootDataKey::Admin)
        .is_some()
    {
        return Err(LootError::AlreadyInitialized);
    }
    env.storage().instance().set(&LootDataKey::Admin, admin);
    Ok(())
}

fn require_admin(env: &Env, caller: &Address) -> Result<(), LootError> {
    caller.require_auth();
    let admin: Address = env
        .storage()
        .instance()
        .get(&LootDataKey::Admin)
        .ok_or(LootError::Unauthorized)?;
    if admin != *caller {
        return Err(LootError::Unauthorized);
    }
    Ok(())
}

/// Define a new loot box type with its (fully public) odds table. Weights
/// must sum to exactly 10_000 basis points.
pub fn create_box_type(
    env: &Env,
    admin: &Address,
    name: soroban_sdk::String,
    cost_loot_tokens: u64,
    entries: Vec<LootEntry>,
) -> Result<u64, LootError> {
    require_admin(env, admin)?;

    if entries.is_empty() {
        return Err(LootError::InvalidOddsTable);
    }
    let mut sum: u32 = 0;
    for i in 0..entries.len() {
        let e = entries.get(i).unwrap();
        sum = sum.checked_add(e.weight_bps).ok_or(LootError::InvalidOddsTable)?;
    }
    if sum != 10_000 {
        return Err(LootError::InvalidOddsTable);
    }

    let counter: u64 = env
        .storage()
        .persistent()
        .get(&LootDataKey::BoxTypeCounter)
        .unwrap_or(0);
    let box_type_id = counter + 1;
    env.storage()
        .persistent()
        .set(&LootDataKey::BoxTypeCounter, &box_type_id);

    let box_type = LootBoxType {
        box_type_id,
        name,
        cost_loot_tokens,
        entries,
    };
    env.storage()
        .persistent()
        .set(&LootDataKey::BoxType(box_type_id), &box_type);

    env.events().publish(
        (symbol_short!("loot"), symbol_short!("boxtype")),
        box_type_id,
    );

    Ok(box_type_id)
}

pub fn get_box_type(env: &Env, box_type_id: u64) -> Result<LootBoxType, LootError> {
    env.storage()
        .persistent()
        .get(&LootDataKey::BoxType(box_type_id))
        .ok_or(LootError::BoxTypeNotFound)
}

// ── LootToken (non-transferable, earn-only currency) ─────────────────────

pub fn get_loot_token_balance(env: &Env, player: &Address) -> u64 {
    env.storage()
        .persistent()
        .get(&LootDataKey::LootTokenBalance(player.clone()))
        .unwrap_or(0)
}

/// Grant `LootToken` to a player — the *only* way this currency ever
/// increases. Admin-gated so it's only called from trusted game-logic paths
/// (achievement completion, tournament participation, etc.), never in
/// exchange for a token/asset a player could have bought.
pub fn grant_loot_tokens(
    env: &Env,
    admin: &Address,
    player: &Address,
    amount: u64,
) -> Result<u64, LootError> {
    require_admin(env, admin)?;
    let key = LootDataKey::LootTokenBalance(player.clone());
    let balance: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    let new_balance = balance.saturating_add(amount);
    env.storage().persistent().set(&key, &new_balance);

    env.events().publish(
        (symbol_short!("loot"), symbol_short!("grant")),
        (player.clone(), amount),
    );

    Ok(new_balance)
}

fn debit_loot_tokens(env: &Env, player: &Address, amount: u64) -> Result<u64, LootError> {
    let key = LootDataKey::LootTokenBalance(player.clone());
    let balance: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    let new_balance = balance
        .checked_sub(amount)
        .ok_or(LootError::InsufficientLootTokens)?;
    env.storage().persistent().set(&key, &new_balance);
    Ok(new_balance)
}

// ── Commit-Reveal Box Opening ─────────────────────────────────────────────

/// Commit to opening a box. `player_seed_hash` must be `sha256(secret)` for
/// a secret only the player knows — they reveal it later in
/// `reveal_loot_open`. The oracle's server seed is captured *now*, before
/// the player has any chance to see it and choose whether to reveal.
pub fn commit_loot_open(
    env: &Env,
    player: &Address,
    box_type_id: u64,
    player_seed_hash: BytesN<32>,
) -> Result<u64, LootError> {
    player.require_auth();

    let box_type = get_box_type(env, box_type_id)?;
    debit_loot_tokens(env, player, box_type.cost_loot_tokens)?;

    let server_seed = randomness_oracle::request_random_seed(env);

    let counter: u64 = env
        .storage()
        .persistent()
        .get(&LootDataKey::OpenRequestCounter)
        .unwrap_or(0);
    let request_id = counter + 1;
    env.storage()
        .persistent()
        .set(&LootDataKey::OpenRequestCounter, &request_id);

    let request = OpenRequest {
        request_id,
        player: player.clone(),
        box_type_id,
        committed_hash: player_seed_hash,
        server_seed: server_seed.clone(),
        revealed: false,
    };
    env.storage()
        .persistent()
        .set(&LootDataKey::OpenRequest(request_id), &request);

    env.events().publish(
        (symbol_short!("loot"), symbol_short!("commit")),
        (request_id, player.clone(), box_type_id, server_seed),
    );

    Ok(request_id)
}

/// Reveal the committed seed and resolve the loot box outcome. Anyone can
/// independently verify the result afterward: recompute
/// `sha256(player_seed) == committed_hash` (proves the player didn't change
/// their seed after seeing the server seed) and
/// `sha256(player_seed || server_seed)` against the published odds table in
/// `get_box_type`.
pub fn reveal_loot_open(
    env: &Env,
    player: &Address,
    request_id: u64,
    player_seed: BytesN<32>,
) -> Result<LootResult, LootError> {
    player.require_auth();

    let mut request: OpenRequest = env
        .storage()
        .persistent()
        .get(&LootDataKey::OpenRequest(request_id))
        .ok_or(LootError::RequestNotFound)?;

    if request.player != *player {
        return Err(LootError::NotRequestOwner);
    }
    if request.revealed {
        return Err(LootError::AlreadyRevealed);
    }

    let seed_bytes: Bytes = player_seed.clone().into();
    let computed_hash: BytesN<32> = env.crypto().sha256(&seed_bytes).into();
    if computed_hash != request.committed_hash {
        return Err(LootError::SeedMismatch);
    }

    let box_type = get_box_type(env, request.box_type_id)?;

    let mut combined = Bytes::new(env);
    combined.append(&player_seed.clone().into());
    combined.append(&request.server_seed.clone().into());
    let outcome_hash: BytesN<32> = env.crypto().sha256(&combined).into();
    let entry = pick_weighted_entry(env, &box_type.entries, &outcome_hash);

    resource_minter::credit_balance(env, player, &entry.resource_type, entry.amount)
        .map_err(|_| LootError::InvalidOddsTable)?;

    request.revealed = true;
    env.storage()
        .persistent()
        .set(&LootDataKey::OpenRequest(request_id), &request);

    let result = LootResult {
        request_id,
        player: player.clone(),
        box_type_id: request.box_type_id,
        resource_type: entry.resource_type,
        amount: entry.amount,
        player_seed: player_seed.clone(),
        server_seed: request.server_seed.clone(),
    };

    env.events().publish(
        (symbol_short!("loot"), symbol_short!("reveal")),
        (
            request_id,
            player.clone(),
            result.resource_type.clone(),
            result.amount,
            player_seed,
        ),
    );

    Ok(result)
}

pub fn get_open_request(env: &Env, request_id: u64) -> Result<OpenRequest, LootError> {
    env.storage()
        .persistent()
        .get(&LootDataKey::OpenRequest(request_id))
        .ok_or(LootError::RequestNotFound)
}

/// Map a 32-byte outcome hash onto the weighted odds table: take its first
/// 4 bytes as a big-endian u32, reduce modulo 10_000, and walk the
/// cumulative weights. Deterministic and fully reproducible by anyone given
/// the same hash — that's what makes the odds table "transparent" rather
/// than just "trust us."
fn pick_weighted_entry(_env: &Env, entries: &Vec<LootEntry>, outcome_hash: &BytesN<32>) -> LootEntry {
    let bytes: Bytes = outcome_hash.clone().into();
    let mut roll_source: u32 = 0;
    for i in 0..4u32 {
        roll_source = (roll_source << 8) | (bytes.get(i).unwrap_or(0) as u32);
    }
    let roll = roll_source % 10_000;

    let mut cumulative: u32 = 0;
    for i in 0..entries.len() {
        let e = entries.get(i).unwrap();
        cumulative += e.weight_bps;
        if roll < cumulative {
            return e;
        }
    }
    // Weights are validated to sum to exactly 10_000 at box-type creation,
    // so `roll` (< 10_000) always falls inside the loop above. This is an
    // unreachable safety net, not a real fallback path.
    entries.get(entries.len() - 1).unwrap()
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{String as SorobanString, Env};

    fn make_odds_table(env: &Env) -> Vec<LootEntry> {
        let mut entries = Vec::new(env);
        entries.push_back(LootEntry {
            resource_type: ResourceType::StellarDust,
            amount: 10,
            weight_bps: 7_000, // common: 70%
        });
        entries.push_back(LootEntry {
            resource_type: ResourceType::DarkMatter,
            amount: 5,
            weight_bps: 2_500, // uncommon: 25%
        });
        entries.push_back(LootEntry {
            resource_type: ResourceType::ExoticMatter,
            amount: 1,
            weight_bps: 500, // rare: 5%
        });
        entries
    }

    #[test]
    fn create_box_type_rejects_odds_not_summing_to_10000() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        set_loot_admin(&env, &admin).unwrap();

        let mut bad_entries = Vec::new(&env);
        bad_entries.push_back(LootEntry {
            resource_type: ResourceType::StellarDust,
            amount: 10,
            weight_bps: 9_000, // sums to 9000, not 10000
        });

        let err = create_box_type(
            &env,
            &admin,
            SorobanString::from_str(&env, "Broken Box"),
            0,
            bad_entries,
        )
        .unwrap_err();
        assert_eq!(err, LootError::InvalidOddsTable);
    }

    #[test]
    fn set_loot_admin_is_one_time_only() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let other = Address::generate(&env);

        set_loot_admin(&env, &admin).unwrap();
        let err = set_loot_admin(&env, &other).unwrap_err();
        assert_eq!(err, LootError::AlreadyInitialized);
    }

    #[test]
    fn opening_a_box_requires_loot_tokens_not_real_money() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let player = Address::generate(&env);
        set_loot_admin(&env, &admin).unwrap();

        let box_type_id =
            create_box_type(&env, &admin, SorobanString::from_str(&env, "Starter Box"), 100, make_odds_table(&env))
                .unwrap();

        // No LootTokens granted yet — opening must fail, there is no way to
        // pay with anything else (no real-money path exists at all).
        let seed = BytesN::from_array(&env, &[7u8; 32]);
        let seed_bytes: Bytes = seed.clone().into();
        let hash: BytesN<32> = env.crypto().sha256(&seed_bytes).into();
        let err = commit_loot_open(&env, &player, box_type_id, hash).unwrap_err();
        assert_eq!(err, LootError::InsufficientLootTokens);

        // Only path to afford it is admin-granted tokens (earned, not bought).
        grant_loot_tokens(&env, &admin, &player, 100).unwrap();
        let request_id = commit_loot_open(&env, &player, box_type_id, hash).unwrap();
        assert_eq!(get_loot_token_balance(&env, &player), 0);

        let result = reveal_loot_open(&env, &player, request_id, seed).unwrap();
        assert!(result.amount > 0);
    }

    #[test]
    fn reveal_rejects_a_seed_that_does_not_match_the_commitment() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let player = Address::generate(&env);
        set_loot_admin(&env, &admin).unwrap();
        grant_loot_tokens(&env, &admin, &player, 100).unwrap();

        let box_type_id =
            create_box_type(&env, &admin, SorobanString::from_str(&env, "Box"), 100, make_odds_table(&env))
                .unwrap();

        let real_seed = BytesN::from_array(&env, &[1u8; 32]);
        let real_seed_bytes: Bytes = real_seed.clone().into();
        let hash: BytesN<32> = env.crypto().sha256(&real_seed_bytes).into();
        let request_id = commit_loot_open(&env, &player, box_type_id, hash).unwrap();

        // Trying to reveal a *different* seed than what was committed to
        // must fail — this is exactly what prevents a player (or the house)
        // from picking a more favorable seed after the fact.
        let wrong_seed = BytesN::from_array(&env, &[2u8; 32]);
        let err = reveal_loot_open(&env, &player, request_id, wrong_seed).unwrap_err();
        assert_eq!(err, LootError::SeedMismatch);
    }

    #[test]
    fn reveal_is_one_time_only() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let player = Address::generate(&env);
        set_loot_admin(&env, &admin).unwrap();
        grant_loot_tokens(&env, &admin, &player, 200).unwrap();

        let box_type_id =
            create_box_type(&env, &admin, SorobanString::from_str(&env, "Box"), 100, make_odds_table(&env))
                .unwrap();

        let seed = BytesN::from_array(&env, &[9u8; 32]);
        let seed_bytes: Bytes = seed.clone().into();
        let hash: BytesN<32> = env.crypto().sha256(&seed_bytes).into();
        let request_id = commit_loot_open(&env, &player, box_type_id, hash).unwrap();

        reveal_loot_open(&env, &player, request_id, seed.clone()).unwrap();
        let err = reveal_loot_open(&env, &player, request_id, seed).unwrap_err();
        assert_eq!(err, LootError::AlreadyRevealed);
    }

    #[test]
    fn weighted_pick_respects_cumulative_bucket_boundaries() {
        let env = Env::default();
        let entries = make_odds_table(&env);

        // roll = 0 -> falls in the first bucket [0, 7000).
        let low_hash = BytesN::from_array(&env, &[0u8; 32]);
        let picked = pick_weighted_entry(&env, &entries, &low_hash);
        assert_eq!(picked.resource_type, ResourceType::StellarDust);

        // Force a specific roll value via the first 4 bytes: 9999 falls in
        // the last bucket [9500, 10000).
        let mut bytes = [0u8; 32];
        let roll: u32 = 9_999;
        bytes[0..4].copy_from_slice(&roll.to_be_bytes());
        let high_hash = BytesN::from_array(&env, &bytes);
        let picked = pick_weighted_entry(&env, &entries, &high_hash);
        assert_eq!(picked.resource_type, ResourceType::ExoticMatter);
    }
}
