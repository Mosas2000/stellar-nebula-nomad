//! Horizon API client for querying Stellar network state

use soroban_sdk::{contracttype, Address, Env, String, Vec};

// ── Data types ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct HorizonQuery {
    pub endpoint: String,
    pub params: Vec<String>,
}

#[contracttype]
#[derive(Clone)]
pub struct AccountInfo {
    pub address: Address,
    pub balance: i128,
    pub sequence: u64,
}

/// Result returned after submitting a transaction for Horizon indexing.
#[contracttype]
#[derive(Clone)]
pub struct TransactionResult {
    pub tx_hash: String,
    pub operation: String,
    pub accepted: bool,
}

/// Filter used to narrow event-stream subscriptions.
#[contracttype]
#[derive(Clone)]
pub struct EventFilter {
    /// Topic prefix to match (e.g. `"horizon"` matches all Horizon events).
    pub topic: String,
    /// Optional sub-topic (empty string = wildcard).
    pub sub_topic: String,
}

// ── Account queries ───────────────────────────────────────────────────────────

/// Query account information via Horizon (off-chain indexer integration).
/// Emits an event so an off-chain indexer can respond with live data.
pub fn query_account_info(env: &Env, address: &Address) -> AccountInfo {
    env.events().publish(
        (soroban_sdk::symbol_short!("horizon"), soroban_sdk::symbol_short!("query")),
        address.clone(),
    );

    AccountInfo {
        address: address.clone(),
        balance: 0,
        sequence: 0,
    }
}

// ── Transaction submission ────────────────────────────────────────────────────

/// Emit a transaction for Horizon indexing.
/// Returns a `TransactionResult` confirming the submission was recorded.
pub fn submit_transaction(
    env: &Env,
    tx_hash: String,
    operation: String,
) -> TransactionResult {
    env.events().publish(
        (soroban_sdk::symbol_short!("horizon"), soroban_sdk::symbol_short!("tx")),
        (tx_hash.clone(), operation.clone()),
    );

    TransactionResult {
        tx_hash,
        operation,
        accepted: true,
    }
}

/// Legacy helper — delegates to `submit_transaction` and discards the result.
pub fn emit_tx_for_indexing(env: &Env, tx_hash: String, operation: String) {
    submit_transaction(env, tx_hash, operation);
}

// ── Event streaming ───────────────────────────────────────────────────────────

/// Register an event-stream subscription for `subscriber`.
///
/// Emits a `(horizon, stream)` event carrying the filter so off-chain
/// indexers know to forward matching events to this subscriber.
pub fn stream_events(env: &Env, subscriber: &Address, filter: EventFilter) {
    env.events().publish(
        (soroban_sdk::symbol_short!("horizon"), soroban_sdk::symbol_short!("stream")),
        (subscriber.clone(), filter.topic.clone(), filter.sub_topic.clone()),
    );
}

/// Cancel an active event-stream subscription for `subscriber`.
pub fn unsubscribe_stream(env: &Env, subscriber: &Address) {
    env.events().publish(
        (soroban_sdk::symbol_short!("horizon"), soroban_sdk::symbol_short!("unsub")),
        subscriber.clone(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Events};
    use soroban_sdk::{vec, IntoVal, TryFromVal};

    // ── Account info ──────────────────────────────────────────────────────────

    #[test]
    fn test_query_account_returns_address() {
        let env = Env::default();
        let address = Address::generate(&env);
        let info = query_account_info(&env, &address);
        assert_eq!(info.address, address);
        assert_eq!(info.balance, 0);
        assert_eq!(info.sequence, 0);
    }

    #[test]
    fn test_query_account_emits_event() {
        let env = Env::default();
        let address = Address::generate(&env);
        query_account_info(&env, &address);

        let events = env.events().all();
        assert!(!events.is_empty(), "expected at least one event");
        let (_contract_id, topics, _data) = events.get(0).unwrap();
        assert_eq!(
            soroban_sdk::Symbol::try_from_val(&env, &topics.get(0).unwrap()).unwrap(),
            soroban_sdk::symbol_short!("horizon")
        );
        assert_eq!(
            soroban_sdk::Symbol::try_from_val(&env, &topics.get(1).unwrap()).unwrap(),
            soroban_sdk::symbol_short!("query")
        );
    }

    // ── Transaction submission ────────────────────────────────────────────────

    #[test]
    fn test_submit_transaction_accepted() {
        let env = Env::default();
        let tx_hash = String::from_str(&env, "abc123");
        let operation = String::from_str(&env, "scan_nebula");
        let result = submit_transaction(&env, tx_hash.clone(), operation.clone());
        assert!(result.accepted);
        assert_eq!(result.tx_hash, tx_hash);
        assert_eq!(result.operation, operation);
    }

    #[test]
    fn test_submit_transaction_emits_event() {
        let env = Env::default();
        let tx_hash = String::from_str(&env, "deadbeef");
        let operation = String::from_str(&env, "mint_ship");
        submit_transaction(&env, tx_hash, operation);

        let events = env.events().all();
        assert!(!events.is_empty());
        let (_contract_id, topics, _data) = events.get(0).unwrap();
        assert_eq!(
            soroban_sdk::Symbol::try_from_val(&env, &topics.get(1).unwrap()).unwrap(),
            soroban_sdk::symbol_short!("tx")
        );
    }

    #[test]
    fn test_emit_tx_for_indexing_compat() {
        let env = Env::default();
        let tx_hash = String::from_str(&env, "abc123");
        let operation = String::from_str(&env, "scan_nebula");
        // Should not panic — legacy wrapper.
        emit_tx_for_indexing(&env, tx_hash, operation);
        assert!(!env.events().all().is_empty());
    }

    #[test]
    fn test_multiple_transactions_each_emit_event() {
        let env = Env::default();
        submit_transaction(
            &env,
            String::from_str(&env, "tx1"),
            String::from_str(&env, "op1"),
        );
        submit_transaction(
            &env,
            String::from_str(&env, "tx2"),
            String::from_str(&env, "op2"),
        );
        assert_eq!(env.events().all().len(), 2);
    }

    // ── Event streaming ───────────────────────────────────────────────────────

    #[test]
    fn test_stream_events_emits_subscription() {
        let env = Env::default();
        let subscriber = Address::generate(&env);
        let filter = EventFilter {
            topic: String::from_str(&env, "horizon"),
            sub_topic: String::from_str(&env, "tx"),
        };
        stream_events(&env, &subscriber, filter);

        let events = env.events().all();
        assert!(!events.is_empty());
        let (_contract_id, topics, _data) = events.get(0).unwrap();
        assert_eq!(
            soroban_sdk::Symbol::try_from_val(&env, &topics.get(1).unwrap()).unwrap(),
            soroban_sdk::symbol_short!("stream")
        );
    }

    #[test]
    fn test_stream_wildcard_sub_topic() {
        let env = Env::default();
        let subscriber = Address::generate(&env);
        let filter = EventFilter {
            topic: String::from_str(&env, "horizon"),
            sub_topic: String::from_str(&env, ""),
        };
        stream_events(&env, &subscriber, filter);
        assert!(!env.events().all().is_empty());
    }

    #[test]
    fn test_unsubscribe_stream_emits_event() {
        let env = Env::default();
        let subscriber = Address::generate(&env);
        unsubscribe_stream(&env, &subscriber);

        let events = env.events().all();
        assert!(!events.is_empty());
        let (_contract_id, topics, _data) = events.get(0).unwrap();
        assert_eq!(
            soroban_sdk::Symbol::try_from_val(&env, &topics.get(1).unwrap()).unwrap(),
            soroban_sdk::symbol_short!("unsub")
        );
    }

    #[test]
    fn test_subscribe_then_unsubscribe_sequence() {
        let env = Env::default();
        let subscriber = Address::generate(&env);
        let filter = EventFilter {
            topic: String::from_str(&env, "horizon"),
            sub_topic: String::from_str(&env, "query"),
        };
        stream_events(&env, &subscriber, filter);
        unsubscribe_stream(&env, &subscriber);
        assert_eq!(env.events().all().len(), 2);
    }

    // ── Combined workflow ─────────────────────────────────────────────────────

    #[test]
    fn test_full_horizon_workflow() {
        let env = Env::default();
        let address = Address::generate(&env);

        // Query account.
        let info = query_account_info(&env, &address);
        assert_eq!(info.address, address);

        // Submit a transaction.
        let result = submit_transaction(
            &env,
            String::from_str(&env, "cafebabe"),
            String::from_str(&env, "harvest_resources"),
        );
        assert!(result.accepted);

        // Subscribe to events.
        let filter = EventFilter {
            topic: String::from_str(&env, "horizon"),
            sub_topic: String::from_str(&env, "tx"),
        };
        stream_events(&env, &address, filter);

        // All three operations each emit one event.
        assert_eq!(env.events().all().len(), 3);
    }
}
