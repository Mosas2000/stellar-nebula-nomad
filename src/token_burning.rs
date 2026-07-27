//! Asset burning for deflationary mechanics — Issue #281
//!
//! Every resource sink in the game routes through this module so that supply
//! removal is accounted for in one place. Three burn mechanisms are provided:
//!
//!   1. **Voluntary** — [`burn`], a holder destroys their own resources.
//!   2. **Sink** — [`burn_for_sink`], a subsystem (crafting, upgrades,
//!      recycling) destroys resources it has already authorized as the cost of
//!      an action.
//!   3. **Fee** — [`apply_deflationary_fee`], a configurable slice of a gross
//!      amount is burned and the net returned to the caller, making ordinary
//!      throughput deflationary without a separate player action.
//!
//! Burns debit the holder *and* the circulating supply via
//! [`crate::resource_minter`], which keeps the monotonic `TotalMinted` counter
//! intact. The deflation rate is therefore `burned / ever_minted` and is exact
//! rather than estimated — see [`deflation_rate_bps`].

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env};

use crate::resource_minter::{self, MinterError, ResourceType};

// ─── Constants ────────────────────────────────────────────────────────────────

/// Basis-point denominator.
pub const BPS_DENOMINATOR: u64 = 10_000;

/// Hard ceiling on the configurable burn fee (10 %). Guards against an admin
/// key setting a confiscatory rate.
pub const MAX_BURN_FEE_BPS: u32 = 1_000;

/// Burn fee applied by [`apply_deflationary_fee`] until an admin overrides it
/// (2 %).
pub const DEFAULT_BURN_FEE_BPS: u32 = 200;

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum BurnKey {
    /// Cumulative burned per resource type.
    TotalBurned(ResourceType),
    /// Cumulative burned by a player, across all resource types.
    PlayerBurned(Address),
    /// Cumulative burned by a player for one resource type.
    PlayerBurnedType(Address, ResourceType),
    /// Number of burn events for a player.
    PlayerBurnCount(Address),
    /// Global burn-event counter, also the next burn record ID.
    BurnCount,
    /// Individual burn receipt keyed by burn ID.
    Record(u64),
    /// Number of distinct addresses that have burned at least once.
    UniqueBurners,
    /// Timestamp of the most recent burn.
    LastBurnAt,
    /// Configurable fee rate in basis points.
    FeeBps,
    /// Admin authorized to change the fee rate.
    FeeAdmin,
}

// ─── Data Types ───────────────────────────────────────────────────────────────

/// Why a burn happened. Recorded on every receipt so the economy dashboard can
/// attribute deflation to the mechanic that caused it.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BurnReason {
    /// Holder chose to destroy their own resources.
    Voluntary,
    /// Consumed as the cost of crafting.
    Crafting,
    /// Consumed as the cost of a ship upgrade.
    Upgrade,
    /// Slice of a transfer or trade taken as a deflationary fee.
    TransactionFee,
    /// Slice of a marketplace sale.
    MarketplaceFee,
    /// Removed by governance action (e.g. treasury buy-back).
    Governance,
}

/// Immutable receipt for a single burn.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BurnRecord {
    pub burn_id: u64,
    pub burner: Address,
    pub resource_type: ResourceType,
    pub amount: u64,
    pub reason: BurnReason,
    pub burned_at: u64,
    /// Circulating supply of `resource_type` immediately after this burn.
    pub supply_after: u64,
}

/// Global burn statistics for one resource type.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BurnStats {
    pub resource_type: ResourceType,
    /// Cumulative amount burned.
    pub total_burned: u64,
    /// Cumulative amount ever minted (the deflation denominator).
    pub total_minted: u64,
    /// Current circulating supply.
    pub circulating_supply: u64,
    /// `total_burned / total_minted` in basis points.
    pub deflation_rate_bps: u32,
    /// Global number of burn events (all resource types).
    pub burn_events: u64,
    /// Distinct addresses that have burned at least once.
    pub unique_burners: u64,
    pub last_burn_at: u64,
}

/// Per-player burn contribution.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerBurnStats {
    pub player: Address,
    /// Total burned across all resource types.
    pub total_burned: u64,
    /// Number of burn events attributed to this player.
    pub burn_count: u64,
    /// Share of all burned supply contributed by this player, in basis points.
    pub share_bps: u32,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum BurningError {
    /// Burn amount must be greater than zero.
    InvalidAmount = 1,
    /// Holder does not have enough of the resource to burn.
    InsufficientBalance = 2,
    /// Burn accounting overflowed.
    ArithmeticOverflow = 3,
    /// Requested fee rate exceeds [`MAX_BURN_FEE_BPS`].
    FeeRateTooHigh = 4,
    /// Caller is not the configured fee admin.
    Unauthorized = 5,
    /// The fee admin has already been set and cannot be re-initialized.
    AlreadyInitialized = 6,
}

impl From<MinterError> for BurningError {
    fn from(e: MinterError) -> Self {
        match e {
            MinterError::InsufficientBalance => BurningError::InsufficientBalance,
            MinterError::InvalidAmount => BurningError::InvalidAmount,
            _ => BurningError::ArithmeticOverflow,
        }
    }
}

// ─── Fee configuration ────────────────────────────────────────────────────────

/// Set the admin permitted to change the burn fee rate. One-shot.
pub fn initialize_burn_admin(env: &Env, admin: Address) -> Result<(), BurningError> {
    admin.require_auth();

    if env.storage().instance().has(&BurnKey::FeeAdmin) {
        return Err(BurningError::AlreadyInitialized);
    }
    env.storage().instance().set(&BurnKey::FeeAdmin, &admin);

    env.events().publish(
        (symbol_short!("burn"), symbol_short!("admin")),
        admin.clone(),
    );

    Ok(())
}

/// The currently configured deflationary fee rate, in basis points.
pub fn get_burn_fee_bps(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&BurnKey::FeeBps)
        .unwrap_or(DEFAULT_BURN_FEE_BPS)
}

/// Update the deflationary fee rate (admin only, capped at
/// [`MAX_BURN_FEE_BPS`]).
pub fn set_burn_fee_bps(env: &Env, caller: Address, bps: u32) -> Result<(), BurningError> {
    caller.require_auth();

    if bps > MAX_BURN_FEE_BPS {
        return Err(BurningError::FeeRateTooHigh);
    }

    // Before initialization the first caller to set a rate is not privileged;
    // afterwards only the admin may change it.
    if let Some(admin) = env
        .storage()
        .instance()
        .get::<BurnKey, Address>(&BurnKey::FeeAdmin)
    {
        if admin != caller {
            return Err(BurningError::Unauthorized);
        }
    } else {
        return Err(BurningError::Unauthorized);
    }

    let previous = get_burn_fee_bps(env);
    env.storage().instance().set(&BurnKey::FeeBps, &bps);

    env.events().publish(
        (symbol_short!("burn"), symbol_short!("feerate")),
        (caller, previous, bps),
    );

    Ok(())
}

// ─── Burn mechanisms ──────────────────────────────────────────────────────────

/// Record a burn against every counter and emit the burn event.
///
/// Assumes the holder's balance has already been debited by the caller.
fn record_burn(
    env: &Env,
    burner: &Address,
    resource_type: &ResourceType,
    amount: u64,
    reason: BurnReason,
    supply_after: u64,
) -> Result<BurnRecord, BurningError> {
    // ── Per-type total ────────────────────────────────────────────────────
    let total_key = BurnKey::TotalBurned(resource_type.clone());
    let total: u64 = env.storage().persistent().get(&total_key).unwrap_or(0);
    let new_total = total
        .checked_add(amount)
        .ok_or(BurningError::ArithmeticOverflow)?;
    env.storage().persistent().set(&total_key, &new_total);

    // ── Per-player totals ─────────────────────────────────────────────────
    let player_key = BurnKey::PlayerBurned(burner.clone());
    let player_total: u64 = env.storage().persistent().get(&player_key).unwrap_or(0);
    // Presence of the burn counter — not a non-zero total — is what makes an
    // address a known burner, so a zero-amount history can never double-count.
    let is_first_burn = !env
        .storage()
        .persistent()
        .has(&BurnKey::PlayerBurnCount(burner.clone()));
    let new_player_total = player_total
        .checked_add(amount)
        .ok_or(BurningError::ArithmeticOverflow)?;
    env.storage()
        .persistent()
        .set(&player_key, &new_player_total);

    let player_type_key = BurnKey::PlayerBurnedType(burner.clone(), resource_type.clone());
    let player_type_total: u64 = env
        .storage()
        .persistent()
        .get(&player_type_key)
        .unwrap_or(0);
    env.storage().persistent().set(
        &player_type_key,
        &player_type_total
            .checked_add(amount)
            .ok_or(BurningError::ArithmeticOverflow)?,
    );

    let count_key = BurnKey::PlayerBurnCount(burner.clone());
    let player_count: u64 = env.storage().persistent().get(&count_key).unwrap_or(0);
    env.storage()
        .persistent()
        .set(&count_key, &player_count.saturating_add(1));

    if is_first_burn {
        let burners: u64 = env
            .storage()
            .persistent()
            .get(&BurnKey::UniqueBurners)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&BurnKey::UniqueBurners, &burners.saturating_add(1));
    }

    // ── Receipt ───────────────────────────────────────────────────────────
    let burn_id: u64 = env
        .storage()
        .persistent()
        .get::<BurnKey, u64>(&BurnKey::BurnCount)
        .unwrap_or(0)
        .saturating_add(1);
    env.storage()
        .persistent()
        .set(&BurnKey::BurnCount, &burn_id);

    let burned_at = env.ledger().timestamp();
    let record = BurnRecord {
        burn_id,
        burner: burner.clone(),
        resource_type: resource_type.clone(),
        amount,
        reason,
        burned_at,
        supply_after,
    };
    env.storage()
        .persistent()
        .set(&BurnKey::Record(burn_id), &record);
    env.storage()
        .persistent()
        .set(&BurnKey::LastBurnAt, &burned_at);

    env.events().publish(
        (symbol_short!("burn"), symbol_short!("burned")),
        (
            burner.clone(),
            resource_type.clone(),
            amount,
            reason,
            supply_after,
        ),
    );

    // A burn that empties the circulating supply is worth surfacing on its own
    // topic — it means every minted unit of the resource is now destroyed.
    if supply_after == 0 {
        env.events().publish(
            (symbol_short!("burn"), symbol_short!("supply0")),
            (resource_type.clone(), new_total),
        );
    }

    Ok(record)
}

/// Destroy `amount` of the caller's own resources.
///
/// Requires the burner's authorization. Debits the balance, reduces the
/// circulating supply and emits `burn/burned`.
pub fn burn(
    env: &Env,
    burner: Address,
    resource_type: ResourceType,
    amount: u64,
    reason: BurnReason,
) -> Result<BurnRecord, BurningError> {
    burner.require_auth();
    burn_for_sink(env, &burner, &resource_type, amount, reason)
}

/// Destroy `amount` of `holder`'s resources without requiring their auth.
///
/// For subsystem sinks (crafting, upgrades, fees) that have already established
/// the holder's consent to spend. Callers that have *not* must use [`burn`].
pub fn burn_for_sink(
    env: &Env,
    holder: &Address,
    resource_type: &ResourceType,
    amount: u64,
    reason: BurnReason,
) -> Result<BurnRecord, BurningError> {
    if amount == 0 {
        return Err(BurningError::InvalidAmount);
    }

    // Debit the holder first so an insufficient balance aborts before any
    // supply or statistics state is touched.
    resource_minter::debit_balance(env, holder, resource_type, amount)?;
    let supply_after = resource_minter::reduce_supply(env, resource_type, amount)?;

    record_burn(env, holder, resource_type, amount, reason, supply_after)
}

/// Burn the configured fee slice of `gross` and return the net amount.
///
/// This is the mechanism that makes ordinary throughput deflationary: callers
/// that move resources around route the gross amount through here and forward
/// the returned net. A `gross` too small to produce a non-zero fee is passed
/// through untouched rather than rounding the fee up.
pub fn apply_deflationary_fee(
    env: &Env,
    holder: &Address,
    resource_type: &ResourceType,
    gross: u64,
    reason: BurnReason,
) -> Result<u64, BurningError> {
    if gross == 0 {
        return Err(BurningError::InvalidAmount);
    }

    let fee_bps = get_burn_fee_bps(env) as u64;
    let fee = gross
        .checked_mul(fee_bps)
        .ok_or(BurningError::ArithmeticOverflow)?
        / BPS_DENOMINATOR;

    if fee == 0 {
        return Ok(gross);
    }

    burn_for_sink(env, holder, resource_type, fee, reason)?;

    // `fee <= gross` because fee_bps <= MAX_BURN_FEE_BPS < BPS_DENOMINATOR.
    Ok(gross - fee)
}

/// Transfer resources from `from` to `to`, burning the deflationary fee.
///
/// The primary mechanism by which ordinary throughput reduces supply: the
/// sender is charged `amount`, the fee slice is destroyed, and the recipient
/// receives the net. Returns the net amount credited to `to`.
pub fn transfer_with_burn(
    env: &Env,
    from: Address,
    to: Address,
    resource_type: ResourceType,
    amount: u64,
) -> Result<u64, BurningError> {
    from.require_auth();

    if amount == 0 {
        return Err(BurningError::InvalidAmount);
    }
    // Check up front so the sender is never charged the fee on a transfer that
    // cannot complete.
    if resource_minter::balance_of(env, &from, &resource_type) < amount {
        return Err(BurningError::InsufficientBalance);
    }

    let net = apply_deflationary_fee(
        env,
        &from,
        &resource_type,
        amount,
        BurnReason::TransactionFee,
    )?;

    resource_minter::move_balance(env, &from, &to, &resource_type, net)?;

    env.events().publish(
        (symbol_short!("burn"), symbol_short!("xferburn")),
        (from, to, resource_type, amount, net),
    );

    Ok(net)
}

// ─── Statistics ───────────────────────────────────────────────────────────────

/// Cumulative amount burned for one resource type.
pub fn total_burned(env: &Env, resource_type: &ResourceType) -> u64 {
    env.storage()
        .persistent()
        .get(&BurnKey::TotalBurned(resource_type.clone()))
        .unwrap_or(0)
}

/// Fraction of all ever-minted supply that has been burned, in basis points.
///
/// Returns 0 when nothing has been minted yet.
pub fn deflation_rate_bps(env: &Env, resource_type: &ResourceType) -> u32 {
    let minted = resource_minter::total_minted(env, resource_type);
    if minted == 0 {
        return 0;
    }
    let burned = total_burned(env, resource_type);

    // burned <= minted always, so the quotient fits comfortably in u32.
    let rate = (burned as u128 * BPS_DENOMINATOR as u128) / minted as u128;
    rate as u32
}

/// Full burn statistics for one resource type.
pub fn get_burn_stats(env: &Env, resource_type: ResourceType) -> BurnStats {
    BurnStats {
        total_burned: total_burned(env, &resource_type),
        total_minted: resource_minter::total_minted(env, &resource_type),
        circulating_supply: resource_minter::circulating_supply(env, &resource_type),
        deflation_rate_bps: deflation_rate_bps(env, &resource_type),
        burn_events: env
            .storage()
            .persistent()
            .get(&BurnKey::BurnCount)
            .unwrap_or(0),
        unique_burners: env
            .storage()
            .persistent()
            .get(&BurnKey::UniqueBurners)
            .unwrap_or(0),
        last_burn_at: env
            .storage()
            .persistent()
            .get(&BurnKey::LastBurnAt)
            .unwrap_or(0),
        resource_type,
    }
}

/// A single player's burn contribution across all resource types.
pub fn get_player_burn_stats(env: &Env, player: Address) -> PlayerBurnStats {
    let total_burned: u64 = env
        .storage()
        .persistent()
        .get(&BurnKey::PlayerBurned(player.clone()))
        .unwrap_or(0);
    let burn_count: u64 = env
        .storage()
        .persistent()
        .get(&BurnKey::PlayerBurnCount(player.clone()))
        .unwrap_or(0);

    // Share is measured against the sum of every resource type's burn total.
    let global_burned = total_burned_all(env);
    let share_bps = if global_burned == 0 {
        0
    } else {
        ((total_burned as u128 * BPS_DENOMINATOR as u128) / global_burned as u128) as u32
    };

    PlayerBurnStats {
        player,
        total_burned,
        burn_count,
        share_bps,
    }
}

/// Sum of burned amounts across every resource type.
pub fn total_burned_all(env: &Env) -> u64 {
    total_burned(env, &ResourceType::StellarDust)
        .saturating_add(total_burned(env, &ResourceType::DarkMatter))
        .saturating_add(total_burned(env, &ResourceType::ExoticMatter))
}

/// Amount a player has burned of one specific resource type.
pub fn player_burned_of(env: &Env, player: &Address, resource_type: &ResourceType) -> u64 {
    env.storage()
        .persistent()
        .get(&BurnKey::PlayerBurnedType(
            player.clone(),
            resource_type.clone(),
        ))
        .unwrap_or(0)
}

/// Retrieve a burn receipt by ID.
pub fn get_burn_record(env: &Env, burn_id: u64) -> Option<BurnRecord> {
    env.storage().persistent().get(&BurnKey::Record(burn_id))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{contract, contractimpl};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    /// Burning touches the balance, supply and statistics ledgers, so tests run
    /// inside a contract invocation. Each `run` is a fresh frame — necessary
    /// because `require_auth` may only be satisfied once per frame, so two
    /// authorized burns cannot share one.
    struct Harness {
        env: Env,
        contract: Address,
        holder: Address,
        rt: ResourceType,
    }

    fn setup(seed_amount: u64) -> Harness {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register(Stub, ());
        let holder = Address::generate(&env);
        let rt = ResourceType::StellarDust;

        let h = Harness {
            env,
            contract,
            holder,
            rt,
        };
        if seed_amount > 0 {
            let holder = h.holder.clone();
            let rt = h.rt.clone();
            h.run(|| {
                resource_minter::credit_balance(&h.env, &holder, &rt, seed_amount).unwrap();
            });
        }
        h
    }

    impl Harness {
        fn run<T>(&self, f: impl FnOnce() -> T) -> T {
            self.env.as_contract(&self.contract, f)
        }
    }

    // ── Burn mechanisms ───────────────────────────────────────────────────

    #[test]
    fn voluntary_burn_reduces_balance_and_supply() {
        let h = setup(1_000);

        let record = h.run(|| {
            burn(
                &h.env,
                h.holder.clone(),
                h.rt.clone(),
                250,
                BurnReason::Voluntary,
            )
            .unwrap()
        });

        assert_eq!(record.amount, 250);
        assert_eq!(record.reason, BurnReason::Voluntary);
        assert_eq!(record.supply_after, 750);
        h.run(|| {
            assert_eq!(resource_minter::balance_of(&h.env, &h.holder, &h.rt), 750);
            assert_eq!(resource_minter::circulating_supply(&h.env, &h.rt), 750);
        });
    }

    #[test]
    fn burning_never_reduces_the_historical_mint_total() {
        let h = setup(1_000);
        h.run(|| {
            burn(
                &h.env,
                h.holder.clone(),
                h.rt.clone(),
                400,
                BurnReason::Voluntary,
            )
            .unwrap()
        });

        h.run(|| {
            assert_eq!(resource_minter::total_minted(&h.env, &h.rt), 1_000);
            assert_eq!(resource_minter::circulating_supply(&h.env, &h.rt), 600);
        });
    }

    #[test]
    fn burn_beyond_balance_is_rejected_and_changes_nothing() {
        let h = setup(100);

        h.run(|| {
            assert_eq!(
                burn(
                    &h.env,
                    h.holder.clone(),
                    h.rt.clone(),
                    101,
                    BurnReason::Voluntary
                ),
                Err(BurningError::InsufficientBalance)
            );
        });

        h.run(|| {
            assert_eq!(resource_minter::balance_of(&h.env, &h.holder, &h.rt), 100);
            assert_eq!(resource_minter::circulating_supply(&h.env, &h.rt), 100);
            assert_eq!(total_burned(&h.env, &h.rt), 0);
        });
    }

    #[test]
    fn zero_burn_is_rejected() {
        let h = setup(100);
        h.run(|| {
            assert_eq!(
                burn(
                    &h.env,
                    h.holder.clone(),
                    h.rt.clone(),
                    0,
                    BurnReason::Voluntary
                ),
                Err(BurningError::InvalidAmount)
            );
        });
    }

    #[test]
    fn sink_burn_attributes_the_reason() {
        let h = setup(500);

        let record =
            h.run(|| burn_for_sink(&h.env, &h.holder, &h.rt, 120, BurnReason::Crafting).unwrap());
        assert_eq!(record.reason, BurnReason::Crafting);
        assert_eq!(record.burner, h.holder);
    }

    // ── Deflationary fee ──────────────────────────────────────────────────

    #[test]
    fn deflationary_fee_burns_a_slice_and_returns_the_net() {
        let h = setup(10_000);

        // Default 2 % of 1_000 = 20.
        let net = h.run(|| {
            apply_deflationary_fee(&h.env, &h.holder, &h.rt, 1_000, BurnReason::TransactionFee)
                .unwrap()
        });

        assert_eq!(net, 980);
        h.run(|| {
            assert_eq!(total_burned(&h.env, &h.rt), 20);
            assert_eq!(resource_minter::balance_of(&h.env, &h.holder, &h.rt), 9_980);
        });
    }

    #[test]
    fn deflationary_fee_passes_dust_amounts_through_untouched() {
        let h = setup(1_000);

        // 2 % of 10 rounds down to 0 — no burn rather than a rounded-up fee.
        let net = h.run(|| {
            apply_deflationary_fee(&h.env, &h.holder, &h.rt, 10, BurnReason::TransactionFee)
                .unwrap()
        });

        assert_eq!(net, 10);
        h.run(|| {
            assert_eq!(total_burned(&h.env, &h.rt), 0);
            assert_eq!(resource_minter::balance_of(&h.env, &h.holder, &h.rt), 1_000);
        });
    }

    #[test]
    fn deflationary_fee_rejects_zero_gross() {
        let h = setup(1_000);
        h.run(|| {
            assert_eq!(
                apply_deflationary_fee(&h.env, &h.holder, &h.rt, 0, BurnReason::TransactionFee),
                Err(BurningError::InvalidAmount)
            );
        });
    }

    // ── Transfer with burn ────────────────────────────────────────────────

    #[test]
    fn transfer_with_burn_charges_the_sender_and_shrinks_supply() {
        let h = setup(10_000);
        let recipient = Address::generate(&h.env);

        // 2 % of 1_000 = 20 burned; the recipient gets 980.
        let net = h.run(|| {
            transfer_with_burn(
                &h.env,
                h.holder.clone(),
                recipient.clone(),
                h.rt.clone(),
                1_000,
            )
            .unwrap()
        });

        assert_eq!(net, 980);
        h.run(|| {
            assert_eq!(resource_minter::balance_of(&h.env, &h.holder, &h.rt), 9_000);
            assert_eq!(resource_minter::balance_of(&h.env, &recipient, &h.rt), 980);
            assert_eq!(resource_minter::circulating_supply(&h.env, &h.rt), 9_980);
            assert_eq!(total_burned(&h.env, &h.rt), 20);
            assert_eq!(
                resource_minter::total_minted(&h.env, &h.rt),
                10_000,
                "a transfer must not inflate the historical mint total"
            );
        });
    }

    #[test]
    fn transfer_with_burn_rejects_an_underfunded_sender_without_charging_a_fee() {
        let h = setup(100);
        let recipient = Address::generate(&h.env);

        h.run(|| {
            assert_eq!(
                transfer_with_burn(
                    &h.env,
                    h.holder.clone(),
                    recipient.clone(),
                    h.rt.clone(),
                    101
                ),
                Err(BurningError::InsufficientBalance)
            );
        });

        h.run(|| {
            assert_eq!(resource_minter::balance_of(&h.env, &h.holder, &h.rt), 100);
            assert_eq!(total_burned(&h.env, &h.rt), 0);
        });
    }

    #[test]
    fn transfer_with_burn_rejects_zero() {
        let h = setup(100);
        let recipient = Address::generate(&h.env);
        h.run(|| {
            assert_eq!(
                transfer_with_burn(&h.env, h.holder.clone(), recipient, h.rt.clone(), 0),
                Err(BurningError::InvalidAmount)
            );
        });
    }

    // ── Fee configuration ─────────────────────────────────────────────────

    #[test]
    fn fee_rate_is_admin_gated_and_capped() {
        let h = setup(0);
        let admin = Address::generate(&h.env);
        let stranger = h.holder.clone();

        h.run(|| initialize_burn_admin(&h.env, admin.clone()).unwrap());
        h.run(|| assert_eq!(get_burn_fee_bps(&h.env), DEFAULT_BURN_FEE_BPS));

        h.run(|| set_burn_fee_bps(&h.env, admin.clone(), 500).unwrap());
        h.run(|| assert_eq!(get_burn_fee_bps(&h.env), 500));

        h.run(|| {
            assert_eq!(
                set_burn_fee_bps(&h.env, admin.clone(), MAX_BURN_FEE_BPS + 1),
                Err(BurningError::FeeRateTooHigh)
            );
        });
        h.run(|| {
            assert_eq!(
                set_burn_fee_bps(&h.env, stranger.clone(), 100),
                Err(BurningError::Unauthorized)
            );
        });
        h.run(|| {
            assert_eq!(
                get_burn_fee_bps(&h.env),
                500,
                "rejected writes change nothing"
            )
        });
    }

    #[test]
    fn burn_admin_cannot_be_reinitialized() {
        let h = setup(0);
        let admin = Address::generate(&h.env);
        let usurper = Address::generate(&h.env);

        h.run(|| initialize_burn_admin(&h.env, admin.clone()).unwrap());
        h.run(|| {
            assert_eq!(
                initialize_burn_admin(&h.env, usurper.clone()),
                Err(BurningError::AlreadyInitialized)
            );
        });
    }

    #[test]
    fn fee_rate_cannot_be_set_before_an_admin_exists() {
        let h = setup(0);
        h.run(|| {
            assert_eq!(
                set_burn_fee_bps(&h.env, h.holder.clone(), 100),
                Err(BurningError::Unauthorized)
            );
        });
    }

    #[test]
    fn configured_fee_rate_is_honoured_by_the_fee_path() {
        let h = setup(10_000);
        let admin = Address::generate(&h.env);
        h.run(|| initialize_burn_admin(&h.env, admin.clone()).unwrap());
        h.run(|| set_burn_fee_bps(&h.env, admin.clone(), MAX_BURN_FEE_BPS).unwrap());

        // 10 % of 1_000 = 100.
        let net = h.run(|| {
            apply_deflationary_fee(&h.env, &h.holder, &h.rt, 1_000, BurnReason::MarketplaceFee)
                .unwrap()
        });
        assert_eq!(net, 900);
        h.run(|| assert_eq!(total_burned(&h.env, &h.rt), 100));
    }

    // ── Statistics ────────────────────────────────────────────────────────

    #[test]
    fn deflation_rate_reflects_burned_over_minted() {
        let h = setup(1_000);
        h.run(|| assert_eq!(deflation_rate_bps(&h.env, &h.rt), 0));

        h.run(|| {
            burn(
                &h.env,
                h.holder.clone(),
                h.rt.clone(),
                250,
                BurnReason::Voluntary,
            )
            .unwrap()
        });

        // 250 / 1000 = 25 % = 2_500 bps.
        h.run(|| assert_eq!(deflation_rate_bps(&h.env, &h.rt), 2_500));
    }

    #[test]
    fn deflation_rate_is_zero_before_anything_is_minted() {
        let h = setup(0);
        h.run(|| assert_eq!(deflation_rate_bps(&h.env, &ResourceType::DarkMatter), 0));
    }

    #[test]
    fn burn_stats_aggregate_the_whole_picture() {
        let h = setup(2_000);
        h.run(|| {
            burn(
                &h.env,
                h.holder.clone(),
                h.rt.clone(),
                300,
                BurnReason::Voluntary,
            )
            .unwrap()
        });
        h.run(|| {
            burn(
                &h.env,
                h.holder.clone(),
                h.rt.clone(),
                200,
                BurnReason::Crafting,
            )
            .unwrap()
        });

        h.run(|| {
            let stats = get_burn_stats(&h.env, h.rt.clone());
            assert_eq!(stats.resource_type, h.rt);
            assert_eq!(stats.total_burned, 500);
            assert_eq!(stats.total_minted, 2_000);
            assert_eq!(stats.circulating_supply, 1_500);
            assert_eq!(stats.deflation_rate_bps, 2_500);
            assert_eq!(stats.burn_events, 2);
            assert_eq!(stats.unique_burners, 1);
            assert_eq!(stats.last_burn_at, h.env.ledger().timestamp());
        });
    }

    #[test]
    fn per_player_stats_track_share_of_total_burned() {
        let h = setup(1_000);
        let alice = h.holder.clone();
        let bob = Address::generate(&h.env);

        h.run(|| resource_minter::credit_balance(&h.env, &bob, &h.rt, 1_000).unwrap());
        h.run(|| {
            burn(
                &h.env,
                alice.clone(),
                h.rt.clone(),
                300,
                BurnReason::Voluntary,
            )
            .unwrap()
        });
        h.run(|| {
            burn(
                &h.env,
                bob.clone(),
                h.rt.clone(),
                100,
                BurnReason::Voluntary,
            )
            .unwrap()
        });

        h.run(|| {
            let alice_stats = get_player_burn_stats(&h.env, alice.clone());
            let bob_stats = get_player_burn_stats(&h.env, bob.clone());

            assert_eq!(alice_stats.total_burned, 300);
            assert_eq!(alice_stats.burn_count, 1);
            assert_eq!(alice_stats.share_bps, 7_500);
            assert_eq!(bob_stats.total_burned, 100);
            assert_eq!(bob_stats.share_bps, 2_500);
            assert_eq!(get_burn_stats(&h.env, h.rt.clone()).unique_burners, 2);
        });
    }

    #[test]
    fn per_player_stats_are_zero_for_a_non_burner() {
        let h = setup(0);
        let stranger = Address::generate(&h.env);

        h.run(|| {
            let stats = get_player_burn_stats(&h.env, stranger.clone());
            assert_eq!(stats.total_burned, 0);
            assert_eq!(stats.burn_count, 0);
            assert_eq!(stats.share_bps, 0);
        });
    }

    #[test]
    fn burn_totals_are_tracked_per_resource_type() {
        let h = setup(0);
        let holder = h.holder.clone();

        h.run(|| {
            resource_minter::credit_balance(&h.env, &holder, &ResourceType::StellarDust, 500)
                .unwrap();
            resource_minter::credit_balance(&h.env, &holder, &ResourceType::DarkMatter, 500)
                .unwrap();
        });

        h.run(|| {
            burn(
                &h.env,
                holder.clone(),
                ResourceType::StellarDust,
                100,
                BurnReason::Voluntary,
            )
            .unwrap()
        });
        h.run(|| {
            burn(
                &h.env,
                holder.clone(),
                ResourceType::DarkMatter,
                50,
                BurnReason::Upgrade,
            )
            .unwrap()
        });

        h.run(|| {
            assert_eq!(total_burned(&h.env, &ResourceType::StellarDust), 100);
            assert_eq!(total_burned(&h.env, &ResourceType::DarkMatter), 50);
            assert_eq!(total_burned(&h.env, &ResourceType::ExoticMatter), 0);
            assert_eq!(total_burned_all(&h.env), 150);
            assert_eq!(
                player_burned_of(&h.env, &holder, &ResourceType::StellarDust),
                100
            );
        });
    }

    #[test]
    fn burn_receipts_are_retrievable_and_sequential() {
        let h = setup(1_000);

        let first = h.run(|| {
            burn(
                &h.env,
                h.holder.clone(),
                h.rt.clone(),
                10,
                BurnReason::Voluntary,
            )
            .unwrap()
        });
        let second = h.run(|| {
            burn(
                &h.env,
                h.holder.clone(),
                h.rt.clone(),
                20,
                BurnReason::Governance,
            )
            .unwrap()
        });

        assert_eq!(first.burn_id, 1);
        assert_eq!(second.burn_id, 2);
        h.run(|| {
            assert_eq!(get_burn_record(&h.env, 1), Some(first.clone()));
            assert_eq!(get_burn_record(&h.env, 2), Some(second.clone()));
            assert_eq!(get_burn_record(&h.env, 3), None);
        });
    }

    #[test]
    fn burning_the_entire_supply_is_permitted() {
        let h = setup(750);

        let record = h.run(|| {
            burn(
                &h.env,
                h.holder.clone(),
                h.rt.clone(),
                750,
                BurnReason::Governance,
            )
            .unwrap()
        });

        assert_eq!(record.supply_after, 0);
        h.run(|| {
            assert_eq!(resource_minter::balance_of(&h.env, &h.holder, &h.rt), 0);
            assert_eq!(deflation_rate_bps(&h.env, &h.rt), BPS_DENOMINATOR as u32);
        });
    }
}
