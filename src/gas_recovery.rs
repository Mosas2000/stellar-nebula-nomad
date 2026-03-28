use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, BytesN, Env, Vec, Map,
};

/// Default refund percentage in basis points (100 = 1%).
pub const DEFAULT_REFUND_BPS: u32 = 500; // 5%

/// Maximum refunds to process in a single batch.
pub const REFUND_BATCH_SIZE: u32 = 10;

/// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum RefundKey {
    /// Refund configuration: percentage in basis points.
    Config,
    /// Refund request status keyed by transaction hash.
    Refund(BytesN<32>),
    /// Admin address authorized to process refunds.
    Admin,
    /// Counter for total refund amount processed.
    TotalRefunded,
}

/// ─── Errors ─────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RefundError {
    /// Transaction is not eligible for a refund.
    NotEligibleForRefund = 1,
    /// Refund already processed for this transaction.
    AlreadyRefunded = 2,
    /// Caller is not authorized to process refunds.
    NotAuthorized = 3,
    /// Batch size exceeds limit.
    BatchTooLarge = 4,
    /// Invalid refund percentage.
    InvalidPercentage = 5,
}

/// ─── Data Types ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct RefundRequest {
    pub tx_hash: BytesN<32>,
    pub caller: Address,
    pub gas_used: u64,
    pub refund_amount: u64,
    pub requested_at: u64,
    pub processed: bool,
}

/// ─── Public API ─────────────────────────────────────────────────────────────

/// Initialize refund module with default config and set admin.
pub fn initialize_refund(env: &Env, admin: &Address) {
    admin.require_auth();
    env.storage().instance().set(&RefundKey::Admin, admin);
    env.storage()
        .instance()
        .set(&RefundKey::Config, &DEFAULT_REFUND_BPS);
    env.storage()
        .instance()
        .set(&RefundKey::TotalRefunded, &0u64);
}

/// Set refund percentage (basis points). Admin-only.
pub fn set_refund_percentage(env: &Env, admin: &Address, bps: u32) -> Result<(), RefundError> {
    admin.require_auth();
    if env.storage().instance().get::<RefundKey, Address>(&RefundKey::Admin) != Some(admin.clone()) {
        return Err(RefundError::NotAuthorized);
    }
    if bps > 10_000 {
        return Err(RefundError::InvalidPercentage);
    }
    env.storage().instance().set(&RefundKey::Config, &bps);
    Ok(())
}

/// Request a refund for a failed transaction hash.
pub fn request_refund(
    env: &Env,
    caller: &Address,
    tx_hash: BytesN<32>,
    gas_used: u64,
) -> Result<RefundRequest, RefundError> {
    caller.require_auth();

    if env
        .storage()
        .instance()
        .get::<RefundKey, RefundRequest>(&RefundKey::Refund(tx_hash.clone()))
        .is_some()
    {
        return Err(RefundError::AlreadyRefunded);
    }

    if !verify_refund_eligibility(env, &tx_hash) {
        return Err(RefundError::NotEligibleForRefund);
    }

    let bps: u32 = env
        .storage()
        .instance()
        .get(&RefundKey::Config)
        .unwrap_or(DEFAULT_REFUND_BPS);
    let refund_amount = (gas_used * bps as u64) / 10_000;

    let request = RefundRequest {
        tx_hash: tx_hash.clone(),
        caller: caller.clone(),
        gas_used,
        refund_amount,
        requested_at: env.ledger().timestamp(),
        processed: false,
    };

    env.storage()
        .instance()
        .set(&RefundKey::Refund(tx_hash.clone()), &request);

    env.events().publish(
        (symbol_short!("refund"), symbol_short!("requested")),
        (
            caller.clone(),
            tx_hash,
            gas_used,
            refund_amount,
            request.requested_at,
        ),
    );

    Ok(request)
}

/// Verify if a transaction hash is eligible for refund.
/// Placeholder: real implementation would check failure logs, status, etc.
pub fn verify_refund_eligibility(env: &Env, tx_hash: &BytesN<32>) -> bool {
    // Simplistic eligibility: no existing refund record.
    env.storage()
        .instance()
        .get::<RefundKey, RefundRequest>(&RefundKey::Refund(tx_hash.clone()))
        .is_none()
}

/// Process a batch of refunds up to REFUND_BATCH_SIZE.
pub fn process_refund_batch(
    env: &Env,
    admin: &Address,
    tx_hashes: Vec<BytesN<32>>,
) -> Result<u64, RefundError> {
    admin.require_auth();
    if env.storage().instance().get::<RefundKey, Address>(&RefundKey::Admin) != Some(admin.clone()) {
        return Err(RefundError::NotAuthorized);
    }
    if (tx_hashes.len() as u32) > REFUND_BATCH_SIZE {
        return Err(RefundError::BatchTooLarge);
    }

    let mut total_processed = 0u64;
    for i in 0..tx_hashes.len() {
        let tx_hash = tx_hashes.get(i).unwrap();
        if let Some(mut req) = env
            .storage()
            .instance()
            .get::<RefundKey, RefundRequest>(&RefundKey::Refund(tx_hash.clone()))
        {
            if !req.processed {
                req.processed = true;
                env.storage()
                    .instance()
                    .set(&RefundKey::Refund(tx_hash.clone()), &req);

                // Update total refunded counter
                let mut total = env
                    .storage()
                    .instance()
                    .get(&RefundKey::TotalRefunded)
                    .unwrap_or(0u64);
                total += req.refund_amount;
                env.storage()
                    .instance()
                    .set(&RefundKey::TotalRefunded, &total);

                env.events().publish(
                    (symbol_short!("refund"), symbol_short!("processed")),
                    (
                        req.caller.clone(),
                        tx_hash,
                        req.refund_amount,
                        env.ledger().timestamp(),
                    ),
                );

                total_processed += req.refund_amount;
            }
        }
    }

    Ok(total_processed)
}

/// Get refund request by transaction hash.
pub fn get_refund_request(env: &Env, tx_hash: BytesN<32>) -> Option<RefundRequest> {
    env.storage()
        .instance()
        .get(&RefundKey::Refund(tx_hash))
}
