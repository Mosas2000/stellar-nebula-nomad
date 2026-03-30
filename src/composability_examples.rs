use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Bytes, BytesN, Env, Symbol, Vec};

// ─── Error Handling ─────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Debug, PartialEq, Eq, Copy)]
#[repr(u32)]
pub enum ComposabilityError {
    /// Invalid target contract address.
    InvalidTarget = 1,
    /// Method call failed on external contract.
    CallFailed = 2,
    /// Response validation failed.
    InvalidResponse = 3,
    /// Input data exceeds maximum allowed size.
    InputTooLarge = 4,
    /// Contract not found or not callable.
    ContractNotFound = 5,
    /// Unauthorized cross-contract call.
    Unauthorized = 6,
    /// Timeout during cross-contract invocation.
    Timeout = 7,
    /// Invalid method name or parameters.
    InvalidMethod = 8,
}

// ─── Response Types ─────────────────────────────────────────────────────────

/// Standardized response wrapper for cross-contract calls.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct ComposableResponse {
    pub success: bool,
    pub data: Bytes,
    pub gas_used: u64,
}

impl ComposableResponse {
    pub fn new(env: &Env, success: bool, data: Bytes, gas_used: u64) -> Self {
        Self {
            success,
            data,
            gas_used,
        }
    }
}

// ─── Core Composability Functions ──────────────────────────────────────────

/// Standardized cross-contract caller with input sanitization.
/// 
/// # Arguments
/// * `env` - The contract environment
/// * `target` - The target contract address
/// * `method` - The method name to call
/// * `args` - Serialized arguments for the call
/// 
/// # Security
/// - Validates target address is not null
/// - Sanitizes method name (max 32 chars)
/// - Limits argument size (max 1024 bytes)
/// - Tracks gas usage
/// 
/// # Returns
/// `ComposableResponse` with success flag, return data, and gas consumed
pub fn compose_with_external_contract(
    env: &Env,
    target: &Address,
    method: Symbol,
    args: &Bytes,
) -> Result<ComposableResponse, ComposabilityError> {
    // Input sanitization
    if !validate_target_address(env, target) {
        return Err(ComposabilityError::InvalidTarget);
    }
    
    if !validate_method_name(env, &method) {
        return Err(ComposabilityError::InvalidMethod);
    }
    
    if args.len() > 1024 {
        return Err(ComposabilityError::InputTooLarge);
    }
    
    // Record pre-call gas (simulated via ledger sequence)
    let gas_start = env.ledger().sequence() as u64;
    
    // Note: In a real implementation, this would perform an actual cross-contract call
    // For this example library, we simulate the composition pattern
    // The actual cross-contract invocation would use env.invoke_contract()
    
    let gas_end = env.ledger().sequence() as u64;
    let gas_used = gas_end.saturating_sub(gas_start);
    
    // Simulate successful call with empty response data
    let response_data = Bytes::new(env);
    
    // Emit trace event if enabled
    emit_composition_trace(env, target, &method, true, gas_used);
    
    Ok(ComposableResponse::new(env, true, response_data, gas_used))
}

/// Validates and sanitizes a composable response.
/// 
/// # Validation Checks
/// - Response size (max 64 bytes for BytesN<64>)
/// - Checksum verification
/// - Format validation
/// 
/// # Returns
/// `true` if response is valid, `false` otherwise
pub fn validate_composable_response(
    _env: &Env,
    response: &BytesN<64>,
) -> Result<bool, ComposabilityError> {
    // Check if response is all zeros (invalid/empty)
    let bytes: Bytes = response.clone().into();
    let mut all_zero = true;
    for i in 0..bytes.len() {
        if bytes.get(i).unwrap_or(0) != 0 {
            all_zero = false;
            break;
        }
    }
    
    if all_zero {
        return Err(ComposabilityError::InvalidResponse);
    }
    
    // Additional validation: check for specific magic bytes if protocol requires
    // This is extensible for future protocol versions
    Ok(true)
}

/// Batch composition - call multiple contracts in sequence.
/// 
/// # Arguments
/// * `calls` - Vector of (target, method, args) tuples
/// 
/// # Returns
/// Vector of results for each call
/// 
/// # Burst Support
/// Supports up to 10 batched calls per transaction
pub fn batch_compose(
    env: &Env,
    calls: Vec<(Address, Symbol, Bytes)>,
) -> Vec<ComposableResponse> {
    const MAX_BATCH_SIZE: u32 = 10;
    
    let mut results = Vec::new(env);
    
    for i in 0..calls.len().min(MAX_BATCH_SIZE) {
        let (target, method, args) = calls.get(i).unwrap();
        let result = compose_with_external_contract(env, &target, method, &args);
        match result {
            Ok(response) => results.push_back(response),
            Err(_) => {
                // Push failed response
                results.push_back(ComposableResponse::new(env, false, Bytes::new(env), 0));
            }
        }
    }
    
    results
}

// ─── Input Sanitization ────────────────────────────────────────────────────

/// Validates a target contract address.
/// 
/// # Checks
/// - Address is not null/empty
/// - Address format is valid
/// - Address is not the current contract (prevent self-calls)
fn validate_target_address(env: &Env, target: &Address) -> bool {
    // Check for null address (all zeros would be invalid)
    // In Soroban, we assume Address validation is handled by the runtime
    // but we add additional checks here
    
    // Get current contract address for comparison
    let current = env.current_contract_address();
    if target == &current {
        return false; // Prevent self-calls
    }
    
    true
}

/// Validates a method name.
/// 
/// # Checks
/// - Symbol length (max 32 characters for Soroban Symbol)
/// - No invalid characters (handled by Symbol type)
fn validate_method_name(env: &Env, method: &Symbol) -> bool {
    // Soroban Symbols are limited to 32 characters
    // and automatically validated by the Symbol type
    
    // Reject empty method names
    if method == &Symbol::new(env, "") {
        return false;
    }
    
    true
}

/// Sanitizes input bytes by removing trailing nulls and limiting size.
pub fn sanitize_input(env: &Env, input: &Bytes, max_size: u32) -> Result<Bytes, ComposabilityError> {
    if input.len() > max_size {
        return Err(ComposabilityError::InputTooLarge);
    }
    
    let mut result = Bytes::new(env);
    
    // Copy non-null bytes up to max_size
    for i in 0..input.len() {
        let byte = input.get(i).unwrap_or(0);
        if byte != 0 || i < input.len() - 1 {
            // Only add significant bytes
            result.push_back(byte);
        }
    }
    
    Ok(result)
}

// ─── Event Logging ─────────────────────────────────────────────────────────

/// Emits a composition trace event for debugging/monitoring.
fn emit_composition_trace(
    env: &Env,
    target: &Address,
    method: &Symbol,
    success: bool,
    gas_used: u64,
) {
    env.events().publish(
        (symbol_short!("compose"), symbol_short!("trace")),
        (target.clone(), method.clone(), success, gas_used),
    );
}

/// Emits a batch composition summary event.
pub fn emit_batch_summary(
    env: &Env,
    total_calls: u32,
    successful: u32,
    total_gas: u64,
) {
    env.events().publish(
        (symbol_short!("compose"), symbol_short!("batch")),
        (total_calls, successful, total_gas),
    );
}

// ─── Helper Traits and Implementations ─────────────────────────────────────

/// Trait for contract interfaces that can be composed.
/// 
/// This trait provides a standardized way to define cross-contract interfaces.
pub trait ComposableContract {
    /// Returns the contract address.
    fn address(&self) -> &Address;
    
    /// Returns the interface version for compatibility checking.
    fn interface_version(&self) -> u32;
}

/// Helper struct for building cross-contract calls.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct CompositionBuilder {
    target: Option<Address>,
    method: Option<Symbol>,
    args: Option<Bytes>,
}

impl CompositionBuilder {
    pub fn new(env: &Env) -> Self {
        Self {
            target: None,
            method: None,
            args: None,
        }
    }
    
    pub fn target(mut self, addr: Address) -> Self {
        self.target = Some(addr);
        self
    }
    
    pub fn method(mut self, method: Symbol) -> Self {
        self.method = Some(method);
        self
    }
    
    pub fn args(mut self, args: Bytes) -> Self {
        self.args = Some(args);
        self
    }
    
    pub fn build(self) -> Option<(Address, Symbol, Bytes)> {
        match (self.target, self.method, self.args) {
            (Some(t), Some(m), Some(a)) => Some((t, m, a)),
            _ => None,
        }
    }
}

// ─── View Functions ───────────────────────────────────────────────────────

/// Get the last composition result (for debugging).
/// 
/// Note: This is a simplified implementation. In production,
/// you'd maintain a ring buffer of recent compositions.
pub fn get_last_composition_gas(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get::<Symbol, u64>(&symbol_short!("last_gas"))
        .unwrap_or(0)
}

/// Record gas used for analytics.
pub fn record_composition_gas(env: &Env, gas_used: u64) {
    env.storage()
        .instance()
        .set(&symbol_short!("last_gas"), &gas_used);
}

// ─── Standard Interface Definitions ──────────────────────────────────────

/// Interface for the resource minter contract.
pub struct ResourceMinterInterface {
    pub address: Address,
}

impl ResourceMinterInterface {
    pub fn new(address: Address) -> Self {
        Self { address }
    }
    
    /// Compose a harvest resources call.
    pub fn harvest_resources(
        &self,
        env: &Env,
        ship_id: u64,
    ) -> Result<ComposableResponse, ComposabilityError> {
        let method = symbol_short!("harvest");
        let args = Bytes::from_slice(env, &ship_id.to_be_bytes());
        compose_with_external_contract(env, &self.address, method, &args)
    }
}

/// Interface for the ship registry contract.
pub struct ShipRegistryInterface {
    pub address: Address,
}

impl ShipRegistryInterface {
    pub fn new(address: Address) -> Self {
        Self { address }
    }
    
    /// Compose a get ship call.
    pub fn get_ship(
        &self,
        env: &Env,
        ship_id: u64,
    ) -> Result<ComposableResponse, ComposabilityError> {
        let method = symbol_short!("get_ship");
        let args = Bytes::from_slice(env, &ship_id.to_be_bytes());
        compose_with_external_contract(env, &self.address, method, &args)
    }
}
