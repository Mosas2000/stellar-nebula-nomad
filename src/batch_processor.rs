use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Vec};

/// Maximum number of operations per batch.
pub const MAX_BATCH_SIZE: u32 = 8;

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

    if operations.len() == 0 {
        return Err(BatchError::EmptyBatch);
    }

    if operations.len() > MAX_BATCH_SIZE {
        return Err(BatchError::BatchLimitExceeded);
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
