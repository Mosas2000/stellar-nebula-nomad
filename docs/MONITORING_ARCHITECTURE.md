# Nebula Nomad Monitoring Architecture

## Overview

The monitoring infrastructure provides real-time observability into Soroban smart contract performance, health, and anomalies. The system is built on Prometheus + Grafana + AlertManager with Soroban-specific exporters.

## Key Components

### 1. Metrics Exporter Service (`src/metrics_exporter.rs`)

The on-chain metrics module tracks:

- **Transaction Metrics**
  - Success/failure counts
  - Total gas usage
  - Average gas per transaction
  
- **Error Tracking**
  - Error type frequency distribution
  - Last occurrence timestamps
  
- **Contract Health**
  - Error rate percentage
  - Uptime tracking
  
- **Performance Metrics**
  - Active user count
  - Daily transaction volume
  - Error spike detection
  - High gas usage detection

#### Usage

```rust
use metrics_exporter::{initialize_metrics, record_tx_success, get_contract_health};

// Initialize (admin only)
initialize_metrics(&env, &admin)?;

// Record successful transaction
record_tx_success(&env, 50_000); // gas_used

// Query health status
let health = get_contract_health(&env);
if !health.is_healthy {
    // Emit alert
}
```

### 2. Cache TTL Manager (`src/cache_ttl_manager.rs`)

Enforces strict TTL (Time-To-Live) on all cached financial data to prevent stale information exploits.

#### Cache Tiers

| Cache Type | TTL | Purpose |
|-----------|-----|---------|
| Analytics | 5 min | Hot transaction data |
| Player Profile | 15 min | User state |
| Yield Forecast | 15 min | Market predictions |
| Market Oracle | 10 min | Price data |
| Leaderboard | 1 hour | Rankings |
| State Snapshot | 30 min | System state |

#### Usage

```rust
use cache_ttl_manager::{cache_with_ttl, get_cached_with_ttl, YIELD_FORECAST_TTL};

// Store with TTL
cache_with_ttl(&env, symbol_short!("yield"), key, data, YIELD_FORECAST_TTL)?;

// Retrieve with automatic expiry validation
match get_cached_with_ttl(&env, symbol_short!("yield"), key) {
    Ok(data) => use_data(data),
    Err(CacheTTLError::CacheExpired) => refresh_from_source(),
    Err(e) => handle_error(e),
}

// Check if cache is still valid without retrieval
if is_cache_valid(&env, ns, key) {
    proceed_with_cached_version();
}
```

### 3. Migration Framework (`src/migration_framework.rs`)

Comprehensive system for safe contract upgrades and data schema evolution.

#### Key Features

- **Dry-run Validation**: Test migrations without state changes
- **Batch Processing**: Handle large datasets incrementally
- **Checkpoints**: Create rollback points before each batch
- **Backward Compatibility Checks**: Ensure schema compatibility
- **Migration History**: Audit trail of all upgrades

#### Workflow

```rust
use migration_framework::*;

// 1. Initialize migration framework
initialize_migrations(&env, &admin, 1)?;

// 2. Plan migration
let record = plan_migration(&env, &admin, 1, 2, symbol_short!("add_field"))?;

// 3. Dry-run on sample data
let report = dry_run_migration(&env, &admin, record.id, sample_records)?;
if !report.would_succeed {
    abort_migration();
}

// 4. Execute in batches
for batch_idx in 0..total_batches {
    execute_migration_batch(
        &env, &admin, record.id, batch_idx, total_batches, batch_data
    )?;
}

// 5. Record completion (or rollback if needed)
if error_occurred {
    rollback_migration(&env, &admin, record.id)?;
}
```

## Prometheus Configuration

### Scrape Targets

```yaml
# Soroban contract metrics (15s interval)
- job_name: 'soroban-metrics'
  targets: ['soroban-exporter:9201']

# Application health (20s interval)
- job_name: 'app-health'
  targets: ['app:8080']

# Stellar Horizon RPC (30s interval)
- job_name: 'stellar-horizon'
  targets: ['horizon:8000']
```

### Alert Rules

Critical alerts are defined in `monitoring/prometheus/alert-rules.yml`:

- **SorobanTxSuccessRateLow**: Triggers when success rate < 95%
- **SorobanGasUsageSpike**: Detects 50%+ increase in gas usage
- **CacheTTLExpirationsHigh**: Alerts on >100 cache expirations/min
- **StaleYieldForecastData**: Detects expired financial data
- **MigrationTimeout**: Flags migrations running >1 hour
- **ActiveUserCountAnomalous**: Detects unusual user activity patterns

## Grafana Dashboards

### Main Dashboard: `nebula-overview.json`

Displays:
- Transaction success rate (gauge)
- Gas usage trends (time-series)
- Error rate by type (bar chart)
- Active users (gauge)
- Contract health status (status indicator)

### Detailed Panels

#### Contract Health
```json
{
  "title": "Contract Health Status",
  "targets": [
    {
      "expr": "soroban_contract_health_status",
      "legendFormat": "Health: {{ is_healthy }}"
    }
  ]
}
```

#### Cache Performance
```json
{
  "title": "Cache Hit Rate",
  "targets": [
    {
      "expr": "rate(cache_hits_total[5m]) / (rate(cache_hits_total[5m]) + rate(cache_misses_total[5m]))",
      "legendFormat": "Hit Rate"
    }
  ]
}
```

#### Migration Status
```json
{
  "title": "Active Migrations",
  "targets": [
    {
      "expr": "count(migration_status == 'in_progress')",
      "legendFormat": "Running Migrations"
    }
  ]
}
```

## Log Aggregation (Loki Integration)

### Configuration

Add to `docker-compose.yml`:

```yaml
loki:
  image: grafana/loki:latest
  ports:
    - "3100:3100"
  volumes:
    - ./monitoring/loki/loki-config.yml:/etc/loki/local-config.yml
  command: -config.file=/etc/loki/local-config.yml

promtail:
  image: grafana/promtail:latest
  volumes:
    - /var/log:/var/log
    - ./monitoring/promtail/promtail-config.yml:/etc/promtail/config.yml
  command: -config.file=/etc/promtail/config.yml
```

### Log Queries

Query contract errors in Grafana:

```logql
{service="soroban"} |= "error"
```

Query migration logs:

```logql
{service="soroban"} |= "migration"
```

## Best Practices

### 1. Cache Invalidation

Always invalidate cache when data changes:

```rust
// After updating yield data
invalidate_cache_entry(&env, symbol_short!("yield"), player_key, symbol_short!("updated"));
```

### 2. Metrics Recording

Record metrics for audit trails:

```rust
// On transaction failure
record_tx_failure(&env, error_type);

// On user activity
record_active_user(&env, &user_address);
```

### 3. Health Checks

Implement regular health checks:

```rust
let health = get_contract_health(&env);
emit_health_event(&env, health);

let anomalies = check_anomalies(&env);
if !anomalies.is_empty() {
    trigger_alerts(&env, anomalies);
}
```

### 4. Migration Safety

Always follow the migration checklist:

- [ ] Run dry-run on production data sample
- [ ] Verify backward compatibility
- [ ] Create rollback checkpoint
- [ ] Execute in batches
- [ ] Monitor error rate during migration
- [ ] Validate data integrity post-migration

## Troubleshooting

### High Cache Expiration Rate

**Symptom**: CacheTTLExpirationsHigh alert firing

**Solutions**:
1. Increase TTL for frequently-accessed data
2. Implement cache refresh strategy
3. Check if data source is stale

### Gas Usage Spike

**Symptom**: SorobanGasUsageSpike alert

**Investigation**:
```yaml
# Query peak gas usage periods
{
  "expr": "rate(soroban_gas_used_total[5m])",
  "range": "last 24h"
}
```

**Common causes**:
- Migration batches processing large datasets
- New user onboarding surge
- Contract upgrade side effects

### Stale Data Detection

**Symptom**: StaleYieldForecastData alert

**Recovery steps**:
1. Pause yield forecast queries
2. Invalidate forecast cache
3. Refresh from market oracle
4. Resume normal operations

## Monitoring Commands

### Query contract health via CLI

```bash
curl http://soroban-exporter:9201/api/contract/health
```

### Export current metrics snapshot

```bash
curl http://soroban-exporter:9201/metrics | grep soroban_
```

### Check cache statistics

```bash
curl http://soroban-exporter:9201/api/cache/stats
```

## Related Documentation

- [Gas Optimization Guide](GAS_OPTIMIZATION_GUIDE.md)
- [Upgrade Guide](UPGRADE_GUIDE.md)
- [Cache TTL Strategy](#cache-ttl-strategy)

---

**Last Updated**: 2026-07-25
**Maintainer**: Core Engineering Team
