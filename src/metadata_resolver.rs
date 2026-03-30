use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Bytes, Env, Vec};

/// Maximum number of tokens in a single batch resolve call.
pub const MAX_METADATA_BATCH: u32 = 10;

/// Default IPFS gateway prefix (encoded as UTF-8 bytes).
/// "https://ipfs.io/ipfs/"
pub const DEFAULT_GATEWAY: &[u8] = b"https://ipfs.io/ipfs/";

// ─── Storage Keys ─────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum MetadataKey {
    /// IPFS CID for a token: `TokenUri(token_id)`.
    TokenUri(u64),
    /// Configurable IPFS gateway bytes.
    Gateway,
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

/// Batch resolve metadata for up to 10 tokens in a single call.
///
/// Reduces round-trips for fleet/grid display. Returns an error if any
/// token ID is not found or if the batch exceeds `MAX_METADATA_BATCH`.
pub fn batch_resolve_metadata(
    env: &Env,
    token_ids: Vec<u64>,
) -> Result<Vec<TokenMetadata>, MetadataError> {
    if token_ids.len() > MAX_METADATA_BATCH {
        return Err(MetadataError::BatchLimitExceeded);
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
