use soroban_sdk::{contracterror, contracttype, symbol_short, Address, BytesN, Env, Symbol, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum IndexerError {
    InvalidCallback = 1,
    Unauthorized = 2,
    RateLimitExceeded = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexerCallback {
    pub id: Symbol,
    pub status: Symbol, // Active / Inactive
}

pub fn register_indexer_callback(env: Env, caller: Address, callback_id: Symbol) -> Result<(), IndexerError> {
    caller.require_auth();

    let callback = IndexerCallback {
        id: callback_id.clone(),
        status: symbol_short!("Active"),
    };

    env.storage().persistent().set(&(symbol_short!("idx_cb"), callback_id.clone()), &callback);

    env.events().publish(
        (symbol_short!("indexer"), symbol_short!("registrd")),
        (callback_id,),
    );

    Ok(())
}

pub fn trigger_indexer_event(env: Env, event_type: Symbol, payload: BytesN<256>) -> Result<(), IndexerError> {
    // This function serves as a standardized hook for the indexer.
    // Every call triggers an event that horizon indexers can filter and aggregate.

    env.events().publish(
        (symbol_short!("idxr_ev"), event_type),
        (payload,),
    );

    Ok(())
}

pub fn get_callback_status(env: Env, callback_id: Symbol) -> Option<IndexerCallback> {
    env.storage().persistent().get(&(symbol_short!("idx_cb"), callback_id))
}
