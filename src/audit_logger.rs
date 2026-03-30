use soroban_sdk::{contracterror, contracttype, symbol_short, Address, BytesN, Env, Symbol, Vec};

/// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum AuditLoggerKey {
    Counter,
    Entry(u64),
}

/// ─── Audit Entry ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct AuditEntry {
    pub id: u64,
    pub timestamp: u64,
    pub actor: Option<Address>,
    pub action: Symbol,
    pub details: BytesN<128>,
}

/// ─── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AuditLoggerError {
    LogWriteFailed = 1,
    QueryLimitExceeded = 2,
    InvalidFilter = 3,
}

/// ─── Public API ─────────────────────────────────────────────────────────────

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

pub fn query_audit_logs(env: &Env, filter: Symbol, limit: u32) -> Vec<AuditEntry> {
    let mut results = Vec::new(env);
    let total: u64 = env.storage().instance().get(&AuditLoggerKey::Counter).unwrap_or(0);

    let max = if limit == 0 {
        total
    } else {
        core::cmp::min(total, limit as u64)
    };

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

    results
}

pub fn get_audit_count(env: &Env) -> u64 {
    env.storage().instance().get(&AuditLoggerKey::Counter).unwrap_or(0)
}
