use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec, Map, i128,
};

// ─── Prometheus Metrics Exporter for Soroban Contracts ─────────────────────
//
// This module provides infrastructure for exporting contract metrics to
// Prometheus/Grafana. Metrics are stored on-chain and queried by the external
// metrics exporter service.

// ─── Configuration ───────────────────────────────────────────────────────

/// Maximum number of metrics retained per type.
pub const MAX_METRICS_RETENTION: u32 = 1000;

/// Metrics aggregation window (seconds).
pub const METRICS_WINDOW: u64 = 60;

// ─── Storage Keys ────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum MetricsKey {
    /// Transaction success counter.
    TxSuccessCount,
    /// Transaction failure counter.
    TxFailureCount,
    /// Total gas used (cumulative).
    TotalGasUsed,
    /// Error type frequency map.
    ErrorTypeFrequency,
    /// Active user count.
    ActiveUserCount,
    /// Contract health status.
    ContractHealth,
    /// Last metrics update timestamp.
    LastUpdateTime,
    /// Metrics window buffer (rolling).
    MetricsWindow(Symbol),
}

// ─── Error Types ─────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MetricsError {
    /// Unauthorized metrics access.
    Unauthorized = 1,
    /// Metrics not initialized.
    NotInitialized = 2,
    /// Invalid metric value.
    InvalidMetric = 3,
}

// ─── Data Structures ─────────────────────────────────────────────────────

/// Transaction metrics snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct TransactionMetrics {
    pub success_count: u64,
    pub failure_count: u64,
    pub total_gas_used: u128,
    pub avg_gas_per_tx: u128,
    pub timestamp: u64,
}

/// Error rate breakdown.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct ErrorMetrics {
    pub error_type: Symbol,
    pub frequency: u64,
    pub last_occurred: u64,
}

/// Contract health status indicator.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct ContractHealthStatus {
    pub is_healthy: bool,
    pub error_rate_percent: u32, // 0-100
    pub last_check: u64,
    pub uptime_seconds: u64,
}

/// Aggregated performance metrics.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct PerformanceMetrics {
    pub active_users: u32,
    pub daily_transactions: u64,
    pub error_spike_detected: bool,
    pub high_gas_usage_detected: bool,
}

// ─── Initialization ─────────────────────────────────────────────────────

/// Initialize the metrics system.
pub fn initialize_metrics(env: &Env, admin: &Address) -> Result<(), MetricsError> {
    admin.require_auth();

    if env.storage().instance().has(&MetricsKey::TxSuccessCount) {
        return Ok(());
    }

    env.storage()
        .instance()
        .set(&MetricsKey::TxSuccessCount, &0u64);
    env.storage()
        .instance()
        .set(&MetricsKey::TxFailureCount, &0u64);
    env.storage()
        .instance()
        .set(&MetricsKey::TotalGasUsed, &0u128);
    env.storage()
        .instance()
        .set(&MetricsKey::ActiveUserCount, &0u32);
    env.storage()
        .instance()
        .set(&MetricsKey::LastUpdateTime, &env.ledger().timestamp());

    env.events().publish(
        (symbol_short!("metrics"), symbol_short!("init")),
        (&admin, env.ledger().timestamp()),
    );

    Ok(())
}

// ─── Metrics Recording ──────────────────────────────────────────────────

/// Record a successful transaction and its gas usage.
pub fn record_tx_success(env: &Env, gas_used: u128) {
    let mut success: u64 = env
        .storage()
        .instance()
        .get(&MetricsKey::TxSuccessCount)
        .unwrap_or(0);
    success += 1;

    let mut total_gas: u128 = env
        .storage()
        .instance()
        .get(&MetricsKey::TotalGasUsed)
        .unwrap_or(0);
    total_gas += gas_used;

    env.storage()
        .instance()
        .set(&MetricsKey::TxSuccessCount, &success);
    env.storage()
        .instance()
        .set(&MetricsKey::TotalGasUsed, &total_gas);
    env.storage()
        .instance()
        .set(&MetricsKey::LastUpdateTime, &env.ledger().timestamp());

    env.events().publish(
        (symbol_short!("metrics"), symbol_short!("tx_success")),
        (success, gas_used, env.ledger().timestamp()),
    );
}

/// Record a failed transaction with error type.
pub fn record_tx_failure(env: &Env, error_type: Symbol) {
    let mut failures: u64 = env
        .storage()
        .instance()
        .get(&MetricsKey::TxFailureCount)
        .unwrap_or(0);
    failures += 1;

    env.storage()
        .instance()
        .set(&MetricsKey::TxFailureCount, &failures);
    env.storage()
        .instance()
        .set(&MetricsKey::LastUpdateTime, &env.ledger().timestamp());

    // Track error frequency.
    let mut error_freq: u64 = env
        .storage()
        .persistent()
        .get(&MetricsKey::ErrorTypeFrequency)
        .and_then(|m: Map<Symbol, u64>| m.get(&error_type))
        .unwrap_or(0);
    error_freq += 1;

    env.events().publish(
        (symbol_short!("metrics"), symbol_short!("tx_failure")),
        (failures, error_type, env.ledger().timestamp()),
    );
}

/// Record an active user (typically called on auth).
pub fn record_active_user(env: &Env, _user: &Address) {
    let mut active: u32 = env
        .storage()
        .instance()
        .get(&MetricsKey::ActiveUserCount)
        .unwrap_or(0);

    // Increment with a max cap to prevent unbounded growth.
    if active < 10_000 {
        active += 1;
    }

    env.storage()
        .instance()
        .set(&MetricsKey::ActiveUserCount, &active);
}

// ─── Metrics Queries ────────────────────────────────────────────────────

/// Get current transaction metrics.
pub fn get_transaction_metrics(env: &Env) -> TransactionMetrics {
    let success: u64 = env
        .storage()
        .instance()
        .get(&MetricsKey::TxSuccessCount)
        .unwrap_or(0);
    let failures: u64 = env
        .storage()
        .instance()
        .get(&MetricsKey::TxFailureCount)
        .unwrap_or(0);
    let total_gas: u128 = env
        .storage()
        .instance()
        .get(&MetricsKey::TotalGasUsed)
        .unwrap_or(0);

    let total_tx = success + failures;
    let avg_gas = if total_tx > 0 {
        total_gas / (total_tx as u128)
    } else {
        0
    };

    TransactionMetrics {
        success_count: success,
        failure_count: failures,
        total_gas_used: total_gas,
        avg_gas_per_tx: avg_gas,
        timestamp: env.ledger().timestamp(),
    }
}

/// Get contract health status.
pub fn get_contract_health(env: &Env) -> ContractHealthStatus {
    let success: u64 = env
        .storage()
        .instance()
        .get(&MetricsKey::TxSuccessCount)
        .unwrap_or(0);
    let failures: u64 = env
        .storage()
        .instance()
        .get(&MetricsKey::TxFailureCount)
        .unwrap_or(0);

    let total_tx = success + failures;
    let error_rate = if total_tx > 0 {
        ((failures * 100) / total_tx) as u32
    } else {
        0
    };

    let is_healthy = error_rate < 5; // Alert if error rate > 5%.

    let last_update: u64 = env
        .storage()
        .instance()
        .get(&MetricsKey::LastUpdateTime)
        .unwrap_or(0);
    let current_time = env.ledger().timestamp();
    let uptime = if current_time > last_update {
        current_time - last_update
    } else {
        0
    };

    ContractHealthStatus {
        is_healthy,
        error_rate_percent: error_rate,
        last_check: current_time,
        uptime_seconds: uptime,
    }
}

/// Get performance metrics.
pub fn get_performance_metrics(env: &Env) -> PerformanceMetrics {
    let active: u32 = env
        .storage()
        .instance()
        .get(&MetricsKey::ActiveUserCount)
        .unwrap_or(0);
    let success: u64 = env
        .storage()
        .instance()
        .get(&MetricsKey::TxSuccessCount)
        .unwrap_or(0);
    let total_gas: u128 = env
        .storage()
        .instance()
        .get(&MetricsKey::TotalGasUsed)
        .unwrap_or(0);

    // Alert on error spikes (>10% error rate).
    let failures: u64 = env
        .storage()
        .instance()
        .get(&MetricsKey::TxFailureCount)
        .unwrap_or(0);
    let total_tx = success + failures;
    let error_spike = if total_tx > 0 {
        (failures * 100) / total_tx > 10
    } else {
        false
    };

    // Alert on high gas usage (avg > 1M gas per tx).
    let avg_gas = if total_tx > 0 {
        total_gas / (total_tx as u128)
    } else {
        0
    };
    let high_gas = avg_gas > 1_000_000;

    PerformanceMetrics {
        active_users: active,
        daily_transactions: success,
        error_spike_detected: error_spike,
        high_gas_usage_detected: high_gas,
    }
}

// ─── Alerting Logic ─────────────────────────────────────────────────────

/// Check for alerting conditions (anomalies).
pub fn check_anomalies(env: &Env) -> Vec<Symbol> {
    let mut anomalies = Vec::new(&env);

    let health = get_contract_health(env);
    if !health.is_healthy {
        anomalies.push_back(symbol_short!("unhealthy"));
    }

    let perf = get_performance_metrics(env);
    if perf.error_spike_detected {
        anomalies.push_back(symbol_short!("error_spike"));
    }
    if perf.high_gas_usage_detected {
        anomalies.push_back(symbol_short!("high_gas"));
    }

    anomalies
}

/// Reset metrics (admin only).
pub fn reset_metrics(env: &Env, admin: &Address) -> Result<(), MetricsError> {
    admin.require_auth();

    env.storage()
        .instance()
        .set(&MetricsKey::TxSuccessCount, &0u64);
    env.storage()
        .instance()
        .set(&MetricsKey::TxFailureCount, &0u64);
    env.storage()
        .instance()
        .set(&MetricsKey::TotalGasUsed, &0u128);
    env.storage()
        .instance()
        .set(&MetricsKey::ActiveUserCount, &0u32);

    env.events().publish(
        (symbol_short!("metrics"), symbol_short!("reset")),
        (&admin, env.ledger().timestamp()),
    );

    Ok(())
}
