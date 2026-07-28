use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Vec};

use crate::rate_limiter;
use crate::error_standard::{ErrorDescriptor, ErrorKind, StandardContractError};

/// Maximum number of operations per batch.
///
/// This is the documented **maximum safe batch size**: at
/// [`GAS_PER_BATCH_OP`] gas per operation, eight operations stay within the
/// [`DEFAULT_BATCH_GAS_BUDGET`]. Use [`max_ops_for_budget`] to derive a safe
/// size for a smaller, caller-supplied budget.
pub const MAX_BATCH_SIZE: u32 = 8;

/// Estimated gas (abstract units) consumed executing a single batch operation:
/// ship-membership lookup plus state mutation. Conservatively over-approximated.
pub const GAS_PER_BATCH_OP: u64 = 8_000;

/// Default gas budget for executing a batch. Sized so the maximum safe batch
/// ([`MAX_BATCH_SIZE`]) fits exactly: `MAX_BATCH_SIZE * GAS_PER_BATCH_OP`.
pub const DEFAULT_BATCH_GAS_BUDGET: u64 = 64_000;

// ─── Storage Keys ─────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum BatchKey {
    /// Per-player pending operation queue: `PlayerBatch(address)`.
    PlayerBatch(Address),
}

// ─── Errors ───────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum BatchError {
    /// Batch size exceeds the maximum of 8.
    BatchLimitExceeded = 1,
    /// No operations are queued for this player.
    EmptyBatch = 2,
    /// One or more operations failed; batch was rolled back.
    OperationFailed = 3,
    /// Gas limit enforcement: too many ops in-flight.
    GasLimitExceeded = 4,
    /// A referenced ship ID was not found in the provided list.
    ShipNotFound = 5,
}

impl StandardContractError for BatchError {
    fn descriptor(self) -> ErrorDescriptor {
        let (kind, retryable) = match self {
            Self::BatchLimitExceeded | Self::EmptyBatch => (ErrorKind::Validation, false),
            Self::ShipNotFound => (ErrorKind::NotFound, false),
            Self::GasLimitExceeded => (ErrorKind::ResourceLimit, true),
            Self::OperationFailed => (ErrorKind::Internal, true),
        };
        ErrorDescriptor {
            module: "batch",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

// ─── Data Types ───────────────────────────────────────────────────────────

/// Types of operations that can be batched.
#[derive(Clone, PartialEq)]
#[contracttype]
pub enum BatchOpType {
    /// Upgrade a ship's stats.
    Upgrade,
    /// Repair hull damage.
    Repair,
    /// Scan an area for resources.
    Scan,
    /// Harvest resources from a nebula.
    Harvest,
}

/// A single operation in a batch queue.
#[derive(Clone)]
#[contracttype]
pub struct BatchOp {
    /// The ship this operation targets.
    pub ship_id: u64,
    /// The type of operation to perform.
    pub op_type: BatchOpType,
    /// Generic operation parameter (e.g. upgrade level, scan seed, repair amount).
    pub params: u64,
}

/// Summary result returned after executing a batch.
#[derive(Clone)]
#[contracttype]
pub struct BatchResult {
    /// Total number of operations attempted.
    pub total_ops: u32,
    /// Number of operations that succeeded.
    pub succeeded: u32,
    /// Number of operations that failed (ship not found in provided list).
    pub failed: u32,
}

// ─── Gas Estimation & Budgeting ───────────────────────────────────────────

/// Estimate the gas required to execute `op_count` batch operations.
///
/// Uses saturating multiplication so an oversized `op_count` reports the
/// maximum gas rather than overflowing.
pub fn estimate_batch_gas(op_count: u32) -> u64 {
    (op_count as u64).saturating_mul(GAS_PER_BATCH_OP)
}

/// Largest number of operations that fit within `gas_budget`, capped at
/// [`MAX_BATCH_SIZE`]. Returns `0` when the budget cannot afford one op.
pub fn max_ops_for_budget(gas_budget: u64) -> u32 {
    // GAS_PER_BATCH_OP is a non-zero constant, so this never divides by zero.
    let affordable = gas_budget / GAS_PER_BATCH_OP;
    affordable.min(MAX_BATCH_SIZE as u64) as u32
}

/// Trim `operations` down to the largest prefix executable within `gas_budget`,
/// so a player can submit a large queue and process it in budget-sized chunks.
pub fn adjust_batch_to_budget(
    env: &Env,
    operations: &Vec<BatchOp>,
    gas_budget: u64,
) -> Vec<BatchOp> {
    let take = operations.len().min(max_ops_for_budget(gas_budget));
    let mut out = Vec::new(env);
    for i in 0..take {
        out.push_back(operations.get(i).unwrap());
    }
    out
}

// ─── Public API ──────────────────────────────────────────────────────────

/// Stage multiple ship operations into the player's batch queue.
///
/// Operations are stored in temporary storage (cleared at the end of the
/// ledger entry TTL). The batch is limited to `MAX_BATCH_SIZE` (8) ops
/// to enforce gas limits and prevent abuse. The player must authorize.
///
/// Returns the number of queued operations.
pub fn queue_batch_operation(
    env: &Env,
    player: &Address,
    operations: Vec<BatchOp>,
) -> Result<u32, BatchError> {
    player.require_auth();

    rate_limiter::check_rate_limit(env, player, rate_limiter::Operation::BatchOperation)
        .map_err(|_| BatchError::GasLimitExceeded)?;

    if operations.len() == 0 {
        return Err(BatchError::EmptyBatch);
    }

    if operations.len() > MAX_BATCH_SIZE {
        return Err(BatchError::BatchLimitExceeded);
    }

    // Reject queues whose estimated execution gas exceeds the default budget,
    // so an over-sized batch fails fast at queue time instead of mid-execution.
    if estimate_batch_gas(operations.len()) > DEFAULT_BATCH_GAS_BUDGET {
        return Err(BatchError::GasLimitExceeded);
    }

    let key = BatchKey::PlayerBatch(player.clone());
    env.storage().temporary().set(&key, &operations);

    Ok(operations.len())
}

/// Execute all queued operations for the given ship IDs atomically.
///
/// `ship_ids` is the set of valid ship IDs the player controls. Any queued
/// operation whose `ship_id` is not in this list is counted as failed and
/// logged. The batch uses atomic semantics: if any operation produces a
/// hard error the whole call panics; partial failures are logged but do
/// not abort the batch.
///
/// Clears the queue on completion. Emits a `BatchExecuted` event.
pub fn execute_batch(
    env: &Env,
    player: &Address,
    ship_ids: Vec<u64>,
) -> Result<BatchResult, BatchError> {
    player.require_auth();

    let key = BatchKey::PlayerBatch(player.clone());
    let operations: Vec<BatchOp> = env
        .storage()
        .temporary()
        .get(&key)
        .ok_or(BatchError::EmptyBatch)?;

    if operations.len() == 0 {
        return Err(BatchError::EmptyBatch);
    }

    if operations.len() > MAX_BATCH_SIZE {
        return Err(BatchError::GasLimitExceeded);
    }

    // Estimate gas before doing any work and bail out if the queued batch would
    // exceed the gas budget — prevents wasted fees on a doomed execution.
    if estimate_batch_gas(operations.len()) > DEFAULT_BATCH_GAS_BUDGET {
        return Err(BatchError::GasLimitExceeded);
    }

    let total_ops = operations.len();
    let mut succeeded: u32 = 0;
    let mut failed: u32 = 0;

    for i in 0..operations.len() {
        let op = operations.get(i).unwrap();

        // Check that the targeted ship is in the caller's fleet.
        let mut ship_valid = false;
        for j in 0..ship_ids.len() {
            if ship_ids.get(j).unwrap() == op.ship_id {
                ship_valid = true;
                break;
            }
        }

        if ship_valid {
            succeeded += 1;
        } else {
            failed += 1;
        }
    }

    // Clear the queue after execution (atomic: always clears, even on partial failure).
    env.storage().temporary().remove(&key);

    let result = BatchResult {
        total_ops,
        succeeded,
        failed,
    };

    env.events().publish(
        (symbol_short!("batch"), symbol_short!("executed")),
        (player.clone(), total_ops, succeeded, failed),
    );

    Ok(result)
}

/// Return the player's currently queued batch operations, if any.
pub fn get_player_batch(env: &Env, player: &Address) -> Option<Vec<BatchOp>> {
    let key = BatchKey::PlayerBatch(player.clone());
    env.storage().temporary().get(&key)
}

/// Clear the player's pending batch queue. Player must authorize.
pub fn clear_batch(env: &Env, player: &Address) {
    player.require_auth();
    let key = BatchKey::PlayerBatch(player.clone());
    env.storage().temporary().remove(&key);
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use soroban_sdk::{Env, Vec};

    proptest! {
        /// Gas estimation matches the per-operation cost.
        #[test]
        fn estimate_matches_per_op_cost(count in 0u32..=MAX_BATCH_SIZE) {
            prop_assert_eq!(estimate_batch_gas(count), count as u64 * GAS_PER_BATCH_OP);
        }

        /// The derived max op count never exceeds the cap and always fits the budget.
        #[test]
        fn max_ops_respects_cap_and_budget(gas_budget in 0u64..=1_000_000u64) {
            let n = max_ops_for_budget(gas_budget);
            prop_assert!(n <= MAX_BATCH_SIZE);
            prop_assert!(estimate_batch_gas(n) <= gas_budget);
        }
    }

    #[test]
    fn default_budget_affords_max_batch() {
        assert_eq!(max_ops_for_budget(DEFAULT_BATCH_GAS_BUDGET), MAX_BATCH_SIZE);
        assert_eq!(estimate_batch_gas(MAX_BATCH_SIZE), DEFAULT_BATCH_GAS_BUDGET);
    }

    #[test]
    fn adjust_batch_trims_to_budget() {
        let env = Env::default();
        let mut ops = Vec::new(&env);
        for i in 0..MAX_BATCH_SIZE as u64 {
            ops.push_back(BatchOp {
                ship_id: i,
                op_type: BatchOpType::Scan,
                params: 0,
            });
        }
        // Budget for only 2 ops.
        let trimmed = adjust_batch_to_budget(&env, &ops, GAS_PER_BATCH_OP * 2);
        assert_eq!(trimmed.len(), 2);
    }
}
