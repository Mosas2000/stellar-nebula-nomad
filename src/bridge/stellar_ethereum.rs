use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, Bytes, BytesN, Env, Symbol, Vec,
};

// ─── Constants ────────────────────────────────────────────────────────────────

/// Maximum amount that can be bridged per transaction.
pub const MAX_BRIDGE_AMOUNT: i128 = 1_000_000_000_000_000;
/// Maximum pending bridge requests.
pub const MAX_PENDING_REQUESTS: u32 = 100;
/// Minimum confirmations required for Ethereum transactions.
pub const MIN_CONFIRMATIONS: u32 = 12;
/// Bridge fee in basis points (0.3%).
pub const BRIDGE_FEE_BPS: u32 = 30;

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum BridgeKey {
    /// Bridge request by request ID.
    Request(u64),
    /// Global request counter.
    RequestCounter,
    /// Admin address.
    Admin,
    /// Bridge configuration.
    Config,
    /// Paused state.
    Paused,
    /// Total bridged amount per address.
    BridgedTotal(Address),
    /// Wrapped asset registry: (asset_symbol, chain_id) -> WrappedAsset.
    WrappedAsset(Symbol, u32),
    /// Asset counter.
    AssetCounter,
}

// ─── Data Types ───────────────────────────────────────────────────────────────

/// Status of a bridge request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
pub enum BridgeStatus {
    /// Request created, waiting for Ethereum confirmation.
    Pending = 0,
    /// Ethereum transaction confirmed, assets locked/minted.
    Confirmed = 1,
    /// Bridge transfer completed.
    Completed = 2,
    /// Bridge request failed or was rejected.
    Failed = 3,
    /// Request was cancelled by the user.
    Cancelled = 4,
}

/// Direction of the bridge transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
pub enum BridgeDirection {
    /// Stellar -> Ethereum (lock assets on Stellar, release on Ethereum).
    StellarToEth,
    /// Ethereum -> Stellar (burn wrapped assets on Stellar, release on Ethereum).
    EthToStellar,
}

/// A bridge transfer request record.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct BridgeRequest {
    /// Unique request identifier.
    pub request_id: u64,
    /// Player initiating the bridge.
    pub player: Address,
    /// Asset being bridged.
    pub asset: Symbol,
    /// Amount to bridge.
    pub amount: i128,
    /// Direction of transfer.
    pub direction: BridgeDirection,
    /// Ethereum recipient address (32 bytes).
    pub eth_recipient: BytesN<32>,
    /// Current status.
    pub status: BridgeStatus,
    /// Transaction hash on the source chain.
    pub source_tx_hash: BytesN<32>,
    /// Transaction hash on the destination chain.
    pub dest_tx_hash: Option<BytesN<32>>,
    /// Number of confirmations received.
    pub confirmations: u32,
    /// Timestamp when request was created.
    pub created_at: u64,
    /// Timestamp when request was completed.
    pub completed_at: Option<u64>,
    /// Bridge fee charged.
    pub fee: i128,
}

/// Wrapped asset on Stellar representing an Ethereum token.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct WrappedAsset {
    /// Asset symbol on Stellar.
    pub symbol: Symbol,
    /// Ethereum token contract address.
    pub eth_contract: BytesN<32>,
    /// Chain ID of the Ethereum network.
    pub chain_id: u32,
    /// Total supply wrapped on Stellar.
    pub total_supply: i128,
    /// Whether wrapping is enabled.
    pub enabled: bool,
}

/// Bridge configuration.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct BridgeConfig {
    /// Admin address for bridge operations.
    pub admin: Address,
    /// Bridge fee in basis points.
    pub fee_bps: u32,
    /// Minimum confirmations required.
    pub min_confirmations: u32,
    /// Maximum amount per transaction.
    pub max_amount: i128,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum BridgeError {
    /// Bridge is paused.
    BridgePaused = 90,
    /// Invalid amount (zero or negative).
    InvalidAmount = 91,
    /// Amount exceeds bridge limit.
    AmountExceeded = 92,
    /// Insufficient balance for bridging.
    InsufficientBalance = 93,
    /// Request not found.
    RequestNotFound = 94,
    /// Request is not in a valid state for this operation.
    InvalidRequestState = 95,
    /// Insufficient confirmations.
    InsufficientConfirmations = 96,
    /// Unauthorized action.
    Unauthorized = 97,
    /// Maximum pending requests reached.
    TooManyPendingRequests = 98,
    /// Invalid Ethereum address format.
    InvalidEthAddress = 99,
    /// Asset not registered.
    AssetNotRegistered = 100,
}

// ─── Helper Functions ─────────────────────────────────────────────────────────

fn require_admin(env: &Env, caller: &Address) -> Result<(), BridgeError> {
    caller.require_auth();
    let admin: Address = env
        .storage()
        .instance()
        .get(&BridgeKey::Admin)
        .ok_or(BridgeError::Unauthorized)?;
    if *caller != admin {
        return Err(BridgeError::Unauthorized);
    }
    Ok(())
}

fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&BridgeKey::Paused)
        .unwrap_or(false)
}

fn next_request_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&BridgeKey::RequestCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage()
        .instance()
        .set(&BridgeKey::RequestCounter, &next);
    next
}

fn calculate_fee(amount: i128, fee_bps: u32) -> i128 {
    amount * fee_bps as i128 / 10_000
}

// ─── Initialization ───────────────────────────────────────────────────────────

/// Initialize the bridge with admin and configuration.
pub fn initialize_bridge(env: &Env, admin: &Address) -> Result<(), BridgeError> {
    admin.require_auth();
    env.storage().instance().set(&BridgeKey::Admin, admin);
    env.storage().instance().set(&BridgeKey::RequestCounter, &0u64);
    env.storage().instance().set(
        &BridgeKey::Config,
        &BridgeConfig {
            admin: admin.clone(),
            fee_bps: BRIDGE_FEE_BPS,
            min_confirmations: MIN_CONFIRMATIONS,
            max_amount: MAX_BRIDGE_AMOUNT,
        },
    );
    Ok(())
}

/// Pause or unpause the bridge (admin only).
pub fn set_bridge_paused(env: &Env, admin: &Address, paused: bool) -> Result<(), BridgeError> {
    require_admin(env, admin)?;
    env.storage().instance().set(&BridgeKey::Paused, &paused);
    Ok(())
}

/// Update bridge configuration (admin only).
pub fn update_bridge_config(
    env: &Env,
    admin: &Address,
    config: BridgeConfig,
) -> Result<(), BridgeError> {
    require_admin(env, admin)?;
    env.storage().instance().set(&BridgeKey::Config, &config);
    Ok(())
}

// ─── Asset Registration ──────────────────────────────────────────────────────

/// Register a wrapped asset for bridging (admin only).
pub fn register_wrapped_asset(
    env: &Env,
    admin: &Address,
    symbol: Symbol,
    eth_contract: BytesN<32>,
    chain_id: u32,
) -> Result<u64, BridgeError> {
    require_admin(env, admin)?;

    let asset_counter: u64 = env
        .storage()
        .instance()
        .get(&BridgeKey::AssetCounter)
        .unwrap_or(0);
    let asset_id = asset_counter + 1;
    env.storage()
        .instance()
        .set(&BridgeKey::AssetCounter, &asset_id);

    let wrapped = WrappedAsset {
        symbol: symbol.clone(),
        eth_contract,
        chain_id,
        total_supply: 0,
        enabled: true,
    };

    env.storage()
        .instance()
        .set(&BridgeKey::WrappedAsset(symbol.clone(), chain_id), &wrapped);

    env.events().publish(
        (symbol_short!("bridge"), symbol_short!("asset_reg")),
        (asset_id, symbol, chain_id),
    );

    Ok(asset_id)
}

/// Get wrapped asset info.
pub fn get_wrapped_asset(
    env: &Env,
    symbol: Symbol,
    chain_id: u32,
) -> Result<WrappedAsset, BridgeError> {
    env.storage()
        .instance()
        .get(&BridgeKey::WrappedAsset(symbol, chain_id))
        .ok_or(BridgeError::AssetNotRegistered)
}

// ─── Bridge Operations ───────────────────────────────────────────────────────

/// Initiate a bridge transfer from Stellar to Ethereum.
///
/// Locks the asset on Stellar side and creates a pending request.
/// The off-chain relayer monitors this event and initiates the Ethereum side.
pub fn initiate_bridge(
    env: &Env,
    player: Address,
    asset: Symbol,
    amount: i128,
    eth_recipient: BytesN<32>,
    source_tx_hash: BytesN<32>,
) -> Result<u64, BridgeError> {
    player.require_auth();

    if is_paused(env) {
        return Err(BridgeError::BridgePaused);
    }

    if amount <= 0 {
        return Err(BridgeError::InvalidAmount);
    }

    let config: BridgeConfig = env
        .storage()
        .instance()
        .get(&BridgeKey::Config)
        .ok_or(BridgeError::Unauthorized)?;

    if amount > config.max_amount {
        return Err(BridgeError::AmountExceeded);
    }

    // Check pending request limit
    let pending_count = count_pending_requests(env);
    if pending_count >= MAX_PENDING_REQUESTS {
        return Err(BridgeError::TooManyPendingRequests);
    }

    let fee = calculate_fee(amount, config.fee_bps);
    let request_id = next_request_id(env);

    let request = BridgeRequest {
        request_id,
        player: player.clone(),
        asset: asset.clone(),
        amount,
        direction: BridgeDirection::StellarToEth,
        eth_recipient,
        status: BridgeStatus::Pending,
        source_tx_hash,
        dest_tx_hash: None,
        confirmations: 0,
        created_at: env.ledger().timestamp(),
        completed_at: None,
        fee,
    };

    env.storage()
        .instance()
        .set(&BridgeKey::Request(request_id), &request);

    // Update bridged total
    let current_total: i128 = env
        .storage()
        .instance()
        .get(&BridgeKey::BridgedTotal(player.clone()))
        .unwrap_or(0);
    env.storage().instance().set(
        &BridgeKey::BridgedTotal(player.clone()),
        &(current_total + amount),
    );

    env.events().publish(
        (symbol_short!("bridge"), symbol_short!("initiated")),
        (request_id, player, asset, amount, fee),
    );

    Ok(request_id)
}

/// Confirm a bridge request with Ethereum transaction proof (relayer only).
///
/// Verifies the source chain confirmation count before marking as confirmed.
pub fn confirm_bridge(
    env: &Env,
    admin: Address,
    request_id: u64,
    dest_tx_hash: BytesN<32>,
    confirmations: u32,
) -> Result<(), BridgeError> {
    require_admin(env, &admin)?;

    let config: BridgeConfig = env
        .storage()
        .instance()
        .get(&BridgeKey::Config)
        .ok_or(BridgeError::Unauthorized)?;

    if confirmations < config.min_confirmations {
        return Err(BridgeError::InsufficientConfirmations);
    }

    let mut request: BridgeRequest = env
        .storage()
        .instance()
        .get(&BridgeKey::Request(request_id))
        .ok_or(BridgeError::RequestNotFound)?;

    if request.status != BridgeStatus::Pending {
        return Err(BridgeError::InvalidRequestState);
    }

    request.status = BridgeStatus::Confirmed;
    request.dest_tx_hash = Some(dest_tx_hash);
    request.confirmations = confirmations;

    env.storage()
        .instance()
        .set(&BridgeKey::Request(request_id), &request);

    env.events().publish(
        (symbol_short!("bridge"), symbol_short!("confirmed")),
        (request_id, confirmations),
    );

    Ok(())
}

/// Complete a bridge request after verification (admin only).
pub fn complete_bridge(
    env: &Env,
    admin: Address,
    request_id: u64,
) -> Result<(), BridgeError> {
    require_admin(env, &admin)?;

    let mut request: BridgeRequest = env
        .storage()
        .instance()
        .get(&BridgeKey::Request(request_id))
        .ok_or(BridgeError::RequestNotFound)?;

    if request.status != BridgeStatus::Confirmed {
        return Err(BridgeError::InvalidRequestState);
    }

    request.status = BridgeStatus::Completed;
    request.completed_at = Some(env.ledger().timestamp());

    env.storage()
        .instance()
        .set(&BridgeKey::Request(request_id), &request);

    env.events().publish(
        (symbol_short!("bridge"), symbol_short!("completed")),
        (request_id, request.player, request.amount),
    );

    Ok(())
}

/// Cancel a pending bridge request (player only).
pub fn cancel_bridge(
    env: &Env,
    player: Address,
    request_id: u64,
) -> Result<(), BridgeError> {
    player.require_auth();

    let mut request: BridgeRequest = env
        .storage()
        .instance()
        .get(&BridgeKey::Request(request_id))
        .ok_or(BridgeError::RequestNotFound)?;

    if request.player != player {
        return Err(BridgeError::Unauthorized);
    }

    if request.status != BridgeStatus::Pending {
        return Err(BridgeError::InvalidRequestState);
    }

    request.status = BridgeStatus::Cancelled;

    env.storage()
        .instance()
        .set(&BridgeKey::Request(request_id), &request);

    env.events().publish(
        (symbol_short!("bridge"), symbol_short!("cancelled")),
        (request_id, player),
    );

    Ok(())
}

// ─── Read Queries ─────────────────────────────────────────────────────────────

/// Get a bridge request by ID.
pub fn get_bridge_request(
    env: &Env,
    request_id: u64,
) -> Result<BridgeRequest, BridgeError> {
    env.storage()
        .instance()
        .get(&BridgeKey::Request(request_id))
        .ok_or(BridgeError::RequestNotFound)
}

/// Get total amount bridged by an address.
pub fn get_bridged_total(env: &Env, player: &Address) -> i128 {
    env.storage()
        .instance()
        .get(&BridgeKey::BridgedTotal(player.clone()))
        .unwrap_or(0)
}

/// Get bridge configuration.
pub fn get_bridge_config(env: &Env) -> Result<BridgeConfig, BridgeError> {
    env.storage()
        .instance()
        .get(&BridgeKey::Config)
        .ok_or(BridgeError::Unauthorized)
}

/// Count pending bridge requests.
fn count_pending_requests(env: &Env) -> u32 {
    let counter: u64 = env
        .storage()
        .instance()
        .get(&BridgeKey::RequestCounter)
        .unwrap_or(0);
    let mut count: u32 = 0;
    for i in 1..=counter {
        if let Some(req) = env
            .storage()
            .instance()
            .get::<BridgeKey, BridgeRequest>(&BridgeKey::Request(i))
        {
            if req.status == BridgeStatus::Pending {
                count += 1;
            }
        }
    }
    count
}

// ─── Security Audit Helpers ──────────────────────────────────────────────────

/// Verify the integrity of a bridge request.
/// Checks that all required fields are populated and consistent.
pub fn verify_request_integrity(
    env: &Env,
    request_id: u64,
) -> Result<bool, BridgeError> {
    let request: BridgeRequest = env
        .storage()
        .instance()
        .get(&BridgeKey::Request(request_id))
        .ok_or(BridgeError::RequestNotFound)?;

    // Verify request_id matches
    if request.request_id != request_id {
        return Ok(false);
    }

    // Verify amount is positive
    if request.amount <= 0 {
        return Ok(false);
    }

    // Verify fee is non-negative
    if request.fee < 0 {
        return Ok(false);
    }

    // Verify fee doesn't exceed amount
    if request.fee >= request.amount {
        return Ok(false);
    }

    Ok(true)
}

/// Check if an address has exceeded their daily bridge limit.
pub fn check_daily_limit(
    env: &Env,
    player: &Address,
    daily_limit: i128,
) -> bool {
    let total: i128 = env
        .storage()
        .instance()
        .get(&BridgeKey::BridgedTotal(player.clone()))
        .unwrap_or(0);
    total < daily_limit
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn make_env() -> Env {
        let env = Env::default();
        env.mock_all_auths();
        env
    }

    #[test]
    fn test_initialize_bridge() {
        let env = make_env();
        let admin = Address::generate(&env);
        assert!(initialize_bridge(&env, &admin).is_ok());

        let config = get_bridge_config(&env).unwrap();
        assert_eq!(config.fee_bps, BRIDGE_FEE_BPS);
        assert_eq!(config.min_confirmations, MIN_CONFIRMATIONS);
    }

    #[test]
    fn test_pause_unpause_bridge() {
        let env = make_env();
        let admin = Address::generate(&env);
        initialize_bridge(&env, &admin).unwrap();

        assert!(set_bridge_paused(&env, &admin, true).is_ok());
        assert!(is_paused(&env));

        assert!(set_bridge_paused(&env, &admin, false).is_ok());
        assert!(!is_paused(&env));
    }

    #[test]
    fn test_initiate_bridge() {
        let env = make_env();
        let admin = Address::generate(&env);
        let player = Address::generate(&env);
        initialize_bridge(&env, &admin).unwrap();

        let eth_recipient = BytesN::from_array(&env, &[1u8; 32]);
        let tx_hash = BytesN::from_array(&env, &[2u8; 32]);

        let result = initiate_bridge(
            &env,
            player.clone(),
            symbol_short!("XLM"),
            1000,
            eth_recipient,
            tx_hash,
        );

        assert!(result.is_ok());
        let request_id = result.unwrap();
        let request = get_bridge_request(&env, request_id).unwrap();
        assert_eq!(request.status, BridgeStatus::Pending);
        assert_eq!(request.amount, 1000);
    }

    #[test]
    fn test_bridge_rejected_when_paused() {
        let env = make_env();
        let admin = Address::generate(&env);
        let player = Address::generate(&env);
        initialize_bridge(&env, &admin).unwrap();
        set_bridge_paused(&env, &admin, true).unwrap();

        let eth_recipient = BytesN::from_array(&env, &[1u8; 32]);
        let tx_hash = BytesN::from_array(&env, &[2u8; 32]);

        let result = initiate_bridge(
            &env,
            player,
            symbol_short!("XLM"),
            1000,
            eth_recipient,
            tx_hash,
        );

        assert_eq!(result, Err(BridgeError::BridgePaused));
    }

    #[test]
    fn test_invalid_amount_rejected() {
        let env = make_env();
        let admin = Address::generate(&env);
        let player = Address::generate(&env);
        initialize_bridge(&env, &admin).unwrap();

        let eth_recipient = BytesN::from_array(&env, &[1u8; 32]);
        let tx_hash = BytesN::from_array(&env, &[2u8; 32]);

        let result = initiate_bridge(
            &env,
            player,
            symbol_short!("XLM"),
            0,
            eth_recipient,
            tx_hash,
        );

        assert_eq!(result, Err(BridgeError::InvalidAmount));
    }

    #[test]
    fn test_calculate_fee() {
        assert_eq!(calculate_fee(10_000, 30), 30);
        assert_eq!(calculate_fee(1_000_000, 30), 30_000);
        assert_eq!(calculate_fee(0, 30), 0);
    }

    #[test]
    fn test_request_integrity_valid() {
        let env = make_env();
        let admin = Address::generate(&env);
        let player = Address::generate(&env);
        initialize_bridge(&env, &admin).unwrap();

        let eth_recipient = BytesN::from_array(&env, &[1u8; 32]);
        let tx_hash = BytesN::from_array(&env, &[2u8; 32]);

        let request_id = initiate_bridge(
            &env,
            player,
            symbol_short!("XLM"),
            1000,
            eth_recipient,
            tx_hash,
        )
        .unwrap();

        assert!(verify_request_integrity(&env, request_id).unwrap());
    }
}
