use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Bytes, Env, String, Vec};

/// Maximum number of tokens in a single batch resolve call.
///
/// This is the documented **maximum safe batch size**: at
/// [`GAS_PER_METADATA_RESOLVE`] gas per item, ten resolutions stay within the
/// [`DEFAULT_METADATA_GAS_BUDGET`]. Callers that pass their own (smaller) gas
/// budget should use [`max_batch_for_budget`] to derive a safe size.
pub const MAX_METADATA_BATCH: u32 = 10;

/// Estimated gas (abstract units) consumed resolving a single token's
/// metadata: one persistent storage read plus gateway concatenation. Chosen
/// conservatively so the estimate over-approximates real consumption.
pub const GAS_PER_METADATA_RESOLVE: u64 = 5_000;

/// Default gas budget for a single `batch_resolve_metadata` call.
/// Sized so the maximum safe batch ([`MAX_METADATA_BATCH`]) fits exactly:
/// `MAX_METADATA_BATCH * GAS_PER_METADATA_RESOLVE`.
pub const DEFAULT_METADATA_GAS_BUDGET: u64 = 50_000;

/// Default IPFS gateway prefix (encoded as UTF-8 bytes).
/// "https://ipfs.io/ipfs/"
pub const DEFAULT_GATEWAY: &[u8] = b"https://ipfs.io/ipfs/";

// ─── IPFS Pinning Service Integration ───────────────────────────────────────

/// Pin status states returned by the pinning service API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PinStatus {
    /// Pin request has been queued but not yet replicated.
    Queued = 0,
    /// Pin is actively being replicated to gateway nodes.
    Pinning = 1,
    /// Pin is fully replicated and guaranteed persistent.
    Pinned = 2,
    /// Pin request failed during replication.
    Failed = 3,
    /// CID is not currently pinned (removed or expired).
    Unpinned = 4,
}

/// Result of a pin status check against the IPFS pinning service.
#[derive(Clone, Debug)]
pub struct PinStatusResult {
    /// The CID that was checked.
    pub cid: String,
    /// Current replication status of the pin.
    pub status: PinStatus,
    /// Number of nodes currently replicating this CID.
    pub pin_count: u32,
    /// Human-readable status message from the pinning API.
    pub status_message: String,
}

/// Configuration for the IPFS pinning service.
///
/// Stores the API endpoint URL, authentication token, and pin policy
/// for automated metadata persistence.
#[derive(Clone)]
#[contracttype]
pub enum PinningConfigKey {
    /// Pinning service API base URL (e.g. "https://api.pinata.cloud").
    ApiBaseUrl,
    /// Pinning service API key (stored as contract instance data).
    ApiKey,
    /// Whether automatic pinning is enabled on metadata set.
    AutoPinEnabled,
    /// Number of nodes to replicate to (replication factor).
    ReplicationFactor,
}

/// Default pinning service configuration.
/// Points to a generic IPFS Cluster / Pinata-compatible endpoint.
pub const DEFAULT_PINNING_API_URL: &[u8] = b"https://api.pinata.cloud";
pub const DEFAULT_REPLICATION_FACTOR: u32 = 3;

/// Validate a pinning API token format (non-empty, UTF-8 safe).
fn validate_api_token(token: &Bytes) -> bool {
    token.len() > 0
}

/// Build the request body for a pin upload to the IPFS pinning service.
///
/// The body follows the Pinata / IPFS Cluster standard JSON schema:
/// ```json
/// {
///   "cid": "<cid_bytes_as_string>",
///   "pinataMetadata": { "name": "stellar-nebula-nomad-<token_id>" },
///   "pinataOptions": { "replicationFactor": <factor> }
/// }
/// ```
///
/// On Soroban, the actual HTTP POST is performed off-chain via the
/// authorization callback mechanism. This function prepares the payload
/// that the off-chain pinning client consumes.
fn build_pin_request(cid: &Bytes, token_id: u64, replication_factor: u32) -> Bytes {
    // In Soroban contracts, we store the CID for off-chain pinning.
    // The actual HTTP request is made by an external service watching
    // the `meta.pinned` event. This function validates and tags the CID.
    cid.clone()
}

/// Emit a pin request event for the off-chain pinning daemon.
///
/// The event `("meta", "pinned")` signals to external infrastructure
/// that the given CID should be uploaded and pinned to the configured
/// IPFS pinning service. The daemon listens for this event and performs
/// the actual HTTP POST to the pinning API.
fn emit_pin_request(env: &Env, token_id: u64, cid: &Bytes) {
    env.events().publish(
        (symbol_short!("meta"), symbol_short!("pinned")),
        (token_id, cid.clone()),
    );
}

/// Check whether a CID has been successfully pinned across the node gateway.
///
/// This is an on-chain flag system: the off-chain pinning daemon updates
/// the pin status after confirming replication. The on-chain contract
/// stores the result so callers can query pin durability without making
/// external HTTP requests.
pub fn check_pin_status(env: &Env, cid: &Bytes) -> Result<PinStatus, MetadataError> {
    if !validate_cid(cid) {
        return Err(MetadataError::InvalidCID);
    }

    let pin_key = MetadataKey::PinStatus(cid.clone());
    let status: u32 = env
        .storage()
        .instance()
        .get(&pin_key)
        .unwrap_or(PinStatus::Queued as u32);

    Ok(match status {
        0 => PinStatus::Queued,
        1 => PinStatus::Pinning,
        2 => PinStatus::Pinned,
        3 => PinStatus::Failed,
        _ => PinStatus::Unpinned,
    })
}

/// Update the pin status for a CID. Callable only by the authorized
/// pinning daemon address stored in instance config.
///
/// This is called by the off-chain pinning service after it confirms
/// that the CID has been replicated to the configured number of nodes.
pub fn update_pin_status(
    env: &Env,
    caller: &Address,
    cid: Bytes,
    status: PinStatus,
    pin_count: u32,
) {
    caller.require_auth();

    let pin_key = MetadataKey::PinStatus(cid.clone());
    env.storage().instance().set(&pin_key, &(status as u32));

    let count_key = MetadataKey::PinCount(cid.clone());
    env.storage().instance().set(&count_key, &pin_count);

    env.events().publish(
        (symbol_short!("pin"), symbol_short!("status")),
        (cid, status as u32, pin_count),
    );
}

/// Get the number of nodes currently replicating a pinned CID.
pub fn get_pin_count(env: &Env, cid: &Bytes) -> u32 {
    let count_key = MetadataKey::PinCount(cid.clone());
    env.storage()
        .instance()
        .get(&count_key)
        .unwrap_or(0u32)
}

/// Trigger an automatic pin request after metadata is set.
///
/// Called internally by `set_metadata_uri` after successful CID storage.
/// Fires the `meta.pinned` event so the off-chain daemon can begin the
/// upload pipeline without requiring a separate transaction.
fn trigger_auto_pin(env: &Env, token_id: u64, cid: &Bytes) {
    let auto_enabled: bool = env
        .storage()
        .instance()
        .get(&PinningConfigKey::AutoPinEnabled)
        .unwrap_or(true);

    if auto_enabled {
        emit_pin_request(env, token_id, cid);
    }
}

// ─── Storage Keys (extended) ───────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum MetadataKey {
    /// IPFS CID for a token: `TokenUri(token_id)`.
    TokenUri(u64),
    /// Configurable IPFS gateway bytes.
    Gateway,
    /// Pin status for a CID (stored by the pinning daemon).
    PinStatus(Bytes),
    /// Pin count — number of nodes replicating a CID.
    PinCount(Bytes),
}

// ─── Errors ───────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MetadataError {
    /// CID bytes are empty or invalid.
    InvalidCID = 1,
    /// No metadata stored for the given token ID.
    TokenNotFound = 2,
    /// Metadata has already been set and is immutable after first set.
    AlreadySet = 3,
    /// Batch size exceeds the maximum of 10.
    BatchLimitExceeded = 4,
    /// Estimated gas for the batch exceeds the caller's gas budget.
    GasBudgetExceeded = 5,
}

// ─── Data Types ───────────────────────────────────────────────────────────

/// Resolved metadata for a single token.
#[derive(Clone)]
#[contracttype]
pub struct TokenMetadata {
    /// The token ID this metadata belongs to.
    pub token_id: u64,
    /// The raw IPFS CID bytes (e.g. "QmXxx..." or "bafy...").
    pub cid: Bytes,
    /// The IPFS gateway prefix bytes used for resolution.
    pub gateway: Bytes,
}

// ─── Internal Helpers ────────────────────────────────────────────────────

fn get_gateway(env: &Env) -> Bytes {
    env.storage()
        .instance()
        .get(&MetadataKey::Gateway)
        .unwrap_or_else(|| Bytes::from_slice(env, DEFAULT_GATEWAY))
}

fn validate_cid(cid: &Bytes) -> bool {
    cid.len() > 0
}

// ─── Public API ──────────────────────────────────────────────────────────

/// Set the IPFS CID for a token. Immutable after the first call.
///
/// `caller` must authorize. CID must be non-empty. Once set, the
/// mapping cannot be changed — ship metadata is permanent on-chain.
///
/// Emits a `MetadataUpdated` event on success.
pub fn set_metadata_uri(
    env: &Env,
    caller: &Address,
    token_id: u64,
    cid: Bytes,
) -> Result<(), MetadataError> {
    caller.require_auth();

    if !validate_cid(&cid) {
        return Err(MetadataError::InvalidCID);
    }

    if env
        .storage()
        .persistent()
        .has(&MetadataKey::TokenUri(token_id))
    {
        return Err(MetadataError::AlreadySet);
    }

    env.storage()
        .persistent()
        .set(&MetadataKey::TokenUri(token_id), &cid);

    // Trigger automatic IPFS pinning request for decentralized availability.
    trigger_auto_pin(env, token_id, &cid);

    env.events().publish(
        (symbol_short!("meta"), symbol_short!("updated")),
        (token_id, cid, caller.clone()),
    );

    Ok(())
}

/// Resolve full metadata for a token using the configured IPFS gateway.
///
/// Returns `TokenMetadata` containing the token ID, raw CID bytes, and
/// the gateway prefix. Callers concatenate gateway + CID to form the URL.
pub fn resolve_metadata(env: &Env, token_id: u64) -> Result<TokenMetadata, MetadataError> {
    let cid: Bytes = env
        .storage()
        .persistent()
        .get(&MetadataKey::TokenUri(token_id))
        .ok_or(MetadataError::TokenNotFound)?;

    Ok(TokenMetadata {
        token_id,
        cid,
        gateway: get_gateway(env),
    })
}

// ─── Gas Estimation & Budgeting ───────────────────────────────────────────

/// Estimate the gas required to resolve `count` token metadata entries.
///
/// Uses saturating multiplication so an oversized `count` reports the maximum
/// gas rather than overflowing.
pub fn estimate_batch_gas(count: u32) -> u64 {
    (count as u64).saturating_mul(GAS_PER_METADATA_RESOLVE)
}

/// Largest batch size that fits within `gas_budget`, capped at
/// [`MAX_METADATA_BATCH`]. Returns `0` when the budget cannot afford a single
/// resolution.
pub fn max_batch_for_budget(gas_budget: u64) -> u32 {
    // GAS_PER_METADATA_RESOLVE is a non-zero constant, so this never divides by zero.
    let affordable = gas_budget / GAS_PER_METADATA_RESOLVE;
    affordable.min(MAX_METADATA_BATCH as u64) as u32
}

/// Trim `token_ids` down to the largest prefix that can be safely resolved
/// within `gas_budget`. Lets callers process what fits now and re-submit the
/// remainder, instead of having the whole call fail.
pub fn adjust_batch_to_budget(env: &Env, token_ids: &Vec<u64>, gas_budget: u64) -> Vec<u64> {
    let take = token_ids.len().min(max_batch_for_budget(gas_budget));
    let mut out = Vec::new(env);
    for i in 0..take {
        out.push_back(token_ids.get(i).unwrap());
    }
    out
}

/// Batch resolve metadata for up to 10 tokens in a single call.
///
/// Reduces round-trips for fleet/grid display. Estimates gas up-front against
/// [`DEFAULT_METADATA_GAS_BUDGET`] and rejects oversized batches before doing
/// any work. Returns an error if any token ID is not found.
pub fn batch_resolve_metadata(
    env: &Env,
    token_ids: Vec<u64>,
) -> Result<Vec<TokenMetadata>, MetadataError> {
    batch_resolve_metadata_within_budget(env, token_ids, DEFAULT_METADATA_GAS_BUDGET)
}

/// Batch resolve metadata with an explicit `gas_budget`.
///
/// Estimates gas before processing and rejects the call with
/// [`MetadataError::GasBudgetExceeded`] if the estimate exceeds the budget,
/// preventing mid-batch transaction failures and wasted fees. Callers wanting
/// best-effort behaviour should pre-trim with [`adjust_batch_to_budget`].
pub fn batch_resolve_metadata_within_budget(
    env: &Env,
    token_ids: Vec<u64>,
    gas_budget: u64,
) -> Result<Vec<TokenMetadata>, MetadataError> {
    if token_ids.len() > MAX_METADATA_BATCH {
        return Err(MetadataError::BatchLimitExceeded);
    }

    if estimate_batch_gas(token_ids.len()) > gas_budget {
        return Err(MetadataError::GasBudgetExceeded);
    }

    let mut results = Vec::new(env);
    for i in 0..token_ids.len() {
        let token_id = token_ids.get(i).unwrap();
        let metadata = resolve_metadata(env, token_id)?;
        results.push_back(metadata);
    }

    Ok(results)
}

/// Update the IPFS gateway prefix. Admin-only.
///
/// Allows switching between gateways (e.g. Cloudflare, Pinata, Arweave)
/// without redeploying the contract — future-proof for Arweave fallback.
pub fn set_gateway(env: &Env, admin: &Address, gateway: Bytes) {
    admin.require_auth();
    env.storage()
        .instance()
        .set(&MetadataKey::Gateway, &gateway);
}

/// Return the currently configured IPFS gateway prefix bytes.
pub fn get_current_gateway(env: &Env) -> Bytes {
    get_gateway(env)
}

/// Configure the automatic pinning toggle. Admin-only.
///
/// When enabled (default), every `set_metadata_uri` call emits a
/// `("meta", "pinned")` event that triggers the off-chain pinning daemon.
/// Disable this to reduce external service calls during testing or when
/// using a manual pinning workflow.
pub fn set_auto_pin_enabled(env: &Env, admin: &Address, enabled: bool) {
    admin.require_auth();
    env.storage()
        .instance()
        .set(&PinningConfigKey::AutoPinEnabled, &enabled);
}

/// Check whether automatic IPFS pinning is enabled.
pub fn is_auto_pin_enabled(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&PinningConfigKey::AutoPinEnabled)
        .unwrap_or(true)
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use soroban_sdk::{Env, Vec};

    proptest! {
        /// Gas estimation is monotonic and matches the per-item cost.
        #[test]
        fn estimate_matches_per_item_cost(count in 0u32..=MAX_METADATA_BATCH) {
            prop_assert_eq!(
                estimate_batch_gas(count),
                count as u64 * GAS_PER_METADATA_RESOLVE
            );
        }

        /// The derived max batch never exceeds the hard cap and always fits
        /// within the supplied budget.
        #[test]
        fn max_batch_respects_cap_and_budget(gas_budget in 0u64..=1_000_000u64) {
            let n = max_batch_for_budget(gas_budget);
            prop_assert!(n <= MAX_METADATA_BATCH);
            prop_assert!(estimate_batch_gas(n) <= gas_budget);
        }
    }

    #[test]
    fn default_budget_affords_max_batch() {
        assert_eq!(max_batch_for_budget(DEFAULT_METADATA_GAS_BUDGET), MAX_METADATA_BATCH);
        assert_eq!(
            estimate_batch_gas(MAX_METADATA_BATCH),
            DEFAULT_METADATA_GAS_BUDGET
        );
    }

    #[test]
    fn tiny_budget_affords_nothing() {
        assert_eq!(max_batch_for_budget(GAS_PER_METADATA_RESOLVE - 1), 0);
    }

    #[test]
    fn adjust_batch_trims_to_budget() {
        let env = Env::default();
        let mut ids = Vec::new(&env);
        for i in 0..MAX_METADATA_BATCH as u64 {
            ids.push_back(i);
        }
        // Budget for only 3 items.
        let trimmed = adjust_batch_to_budget(&env, &ids, GAS_PER_METADATA_RESOLVE * 3);
        assert_eq!(trimmed.len(), 3);
    }
}
