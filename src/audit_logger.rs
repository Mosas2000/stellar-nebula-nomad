//! Structured, append-only audit event recording.
//!
use soroban_sdk::{contracterror, contracttype, symbol_short, Address, BytesN, Env, Symbol, Vec};

pub const MAX_QUERY_LIMIT: u32 = 1000;

#[derive(Clone)]
#[contracttype]
pub enum AuditLoggerKey {
    Counter,
    Entry(u64),
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct AuditEntry {
    pub id: u64,
    pub timestamp: u64,
    pub actor: Option<Address>,
    pub action: Symbol,
    pub details: BytesN<128>,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AuditLoggerError {
    LogWriteFailed = 1,
    QueryLimitExceeded = 2,
    InvalidFilter = 3,
}

pub fn log_audit_event(
    env: &Env,
    actor: Option<&Address>,
    action: Symbol,
    details: BytesN<128>,
) -> Result<AuditEntry, AuditLoggerError> {
    let current_id: u64 = env
        .storage()
        .instance()
        .get(&AuditLoggerKey::Counter)
        .unwrap_or(0);

    let entry = AuditEntry {
        id: current_id,
        timestamp: env.ledger().timestamp(),
        actor: actor.cloned(),
        action: action.clone(),
        details: details.clone(),
    };

    env.storage()
        .instance()
        .set(&AuditLoggerKey::Entry(current_id), &entry);
    env.storage()
        .instance()
        .set(&AuditLoggerKey::Counter, &(current_id + 1));

    env.events().publish(
        (symbol_short!("audit"), symbol_short!("entry")),
        (entry.id, entry.timestamp, entry.actor.clone(), entry.action.clone(), entry.details.clone()),
    );

    Ok(entry)
}

pub fn query_audit_logs(env: &Env, filter: Symbol, limit: u32) -> Result<Vec<AuditEntry>, AuditLoggerError> {
    let capped_limit = core::cmp::min(limit, MAX_QUERY_LIMIT);
    let total: u64 = env.storage().instance().get(&AuditLoggerKey::Counter).unwrap_or(0);

    let max = if capped_limit == 0 {
        core::cmp::min(total, MAX_QUERY_LIMIT as u64)
    } else {
        core::cmp::min(total, capped_limit as u64)
    };

    let mut results = Vec::new(env);
    let mut i = 0u64;
    while i < max {
        if let Some(entry) = env
            .storage()
            .instance()
            .get::<AuditLoggerKey, AuditEntry>(&AuditLoggerKey::Entry(i))
        {
            if filter == entry.action.clone() {
                results.push_back(entry);
            }
        }
        i += 1;
    }

    Ok(results)
}

pub fn get_audit_count(env: &Env) -> u64 {
    env.storage().instance().get(&AuditLoggerKey::Counter).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, contract, contractimpl};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    fn make_env() -> (Env, soroban_sdk::Address) {
        let env = Env::default();
        let id = env.register(Stub, ());
        (env, id)
    }

    #[test]
    fn test_log_and_query_roundtrip() {
        let (env, id) = make_env();
        let player = Address::generate(&env);
        let action = symbol_short!("scan");
        let details = BytesN::from_array(&env, &[1u8; 128]);

        env.as_contract(&id, || {
            let entry = log_audit_event(&env, Some(&player), action.clone(), details.clone())
                .expect("log should succeed");
            assert_eq!(entry.actor, Some(player.clone()));
            assert_eq!(entry.action, action);

            let results = query_audit_logs(&env, action, 10)
                .expect("query should succeed");
            assert_eq!(results.len(), 1);
            assert_eq!(results.get(0).unwrap().id, entry.id);
        });
    }

    #[test]
    fn test_query_limit_capped() {
        let (env, id) = make_env();
        let player = Address::generate(&env);
        let action = symbol_short!("test");
        let details = BytesN::from_array(&env, &[0u8; 128]);

        env.as_contract(&id, || {
            for _ in 0..5 {
                let _ = log_audit_event(&env, Some(&player), action.clone(), details.clone());
            }

            let results = query_audit_logs(&env, action, 3)
                .expect("query should succeed");
            assert_eq!(results.len(), 3);
        });
    }

    #[test]
    fn test_query_limit_does_not_exceed_max() {
        let (env, id) = make_env();
        let player = Address::generate(&env);
        let action = symbol_short!("bulk");
        let details = BytesN::from_array(&env, &[0u8; 128]);

        env.as_contract(&id, || {
            for _ in 0..10 {
                let _ = log_audit_event(&env, Some(&player), action.clone(), details.clone());
            }

            let huge_limit = MAX_QUERY_LIMIT + 5000;
            let results = query_audit_logs(&env, action, huge_limit)
                .expect("query should succeed");
            assert!(results.len() as u32 <= MAX_QUERY_LIMIT);
        });
    }

    #[test]
    fn test_get_audit_count_returns_zero_when_empty() {
        let (env, id) = make_env();
        env.as_contract(&id, || {
            assert_eq!(get_audit_count(&env), 0);
        });
    }

    #[test]
    fn test_log_multiple_events_increments_counter() {
        let (env, id) = make_env();
        let player = Address::generate(&env);
        let details = BytesN::from_array(&env, &[0u8; 128]);

        env.as_contract(&id, || {
            let _ = log_audit_event(&env, Some(&player), symbol_short!("a"), details.clone());
            let _ = log_audit_event(&env, Some(&player), symbol_short!("b"), details.clone());
            assert_eq!(get_audit_count(&env), 2);
        });
    }
}
