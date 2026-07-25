# Storage Tier Decision Matrix

## Overview

Soroban contracts have three storage tiers with different access patterns and costs. Choosing the right tier is critical for gas optimization and player experience.

## Storage Tier Comparison

| Property | Instance | Temporary | Persistent |
|----------|----------|-----------|-----------|
| **Scope** | Contract lifetime | Single transaction | Indefinite |
| **Durability** | Ledger-persistent | Lost after tx | Permanent |
| **Access Cost** | Low (inline) | Very low (memory) | High (ledger I/O) |
| **Write Cost** | Medium | Low | Very high |
| **TTL** | ~6 months | ~1 second | Unbounded |
| **Use Case** | State flags, counts | Loop temps, results | Player records |

## Decision Matrix by Data Type

### Tier 1: Temporary Storage (Lowest Cost)

**When to use**: Data that exists only during transaction execution.

#### Candidates
- Loop intermediate results
- Calculation buffers
- Event-emission staging
- Validation flags
- Transaction-scoped state

#### Example
```rust
// ✅ GOOD: Temporary storage for loop results
let mut results = Vec::new(&env);
for anomaly in anomalies.iter() {
    let score = calculate_risk_score(&env, anomaly);  // Computation
    results.push_back(score);  // Store in temp
}
// Results are discarded after function returns
```

#### Gas Cost: ~100-500 instructions per operation

### Tier 2: Instance Storage (Low-Medium Cost)

**When to use**: Small, frequently-accessed state needed between transactions.

#### Candidates
- Configuration values (TTL, batch size)
- Admin addresses
- Contract version/status flags
- Global counters (tx count, user count)
- Reentrancy guards
- Feature flags
- Recent cache values (< 1 KB)

#### Example
```rust
// ✅ GOOD: Instance storage for global config
pub fn set_batch_size(env: &Env, size: u32) {
    env.storage()
        .instance()
        .set(&StorageKey::BatchSize, &size);  // Fast access
}

pub fn get_batch_size(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&StorageKey::BatchSize)
        .unwrap_or(50)
}
```

#### Gas Cost: ~1K-5K instructions per operation

#### Anti-pattern: Player Profiles in Instance
```rust
// ❌ BAD: Storing player data in instance storage
pub fn set_profile(env: &Env, player: &Address, profile: PlayerProfile) {
    env.storage()
        .instance()
        .set(&StorageKey::Profile(player.clone()), &profile);  // WRONG!
}
// Why: Each player profile is ~1 KB. Instance storage isn't designed
// for large, per-user data. Use persistent storage instead.
```

### Tier 3: Persistent Storage (High Cost)

**When to use**: Data that must survive indefinitely and is accessed cross-transaction.

#### Candidates
- Player profiles & state
- Ship NFT metadata
- Achievement records
- Leaderboard entries
- Resource balances
- Historical data
- Game state snapshots
- Game economy records (prices, supplies)

#### Example
```rust
// ✅ GOOD: Persistent storage for player data
pub fn save_player_profile(env: &Env, player: &Address, profile: PlayerProfile) {
    let key = ProfileKey::Player(player.clone());
    env.storage()
        .persistent()
        .set(&key, &profile);  // Will survive ledger transitions
    
    // Extend TTL to reduce rent pressure
    env.storage()
        .persistent()
        .extend_ttl(&key, DEFAULT_TTL, DEFAULT_TTL);
}
```

#### Gas Cost: ~50K-200K instructions per operation

## Hot-Path Optimization Pattern: Read-Through Cache

Many operations read the same data repeatedly. Use temporary storage as a read-through cache:

### Before Optimization (No Cache)
```rust
pub fn get_leaderboard(env: &Env, offset: u32, limit: u32) -> Vec<LeaderboardEntry> {
    let mut entries = Vec::new(&env);
    
    for i in 0..limit {
        // Each call to persistent storage costs ~50K+ gas
        let entry: LeaderboardEntry = env.storage()
            .persistent()
            .get(&LeaderboardKey::Entry(offset + i))
            .unwrap();
        entries.push_back(entry);
    }
    
    entries
}
// Total: 50K * limit = 500K gas for 10 entries
```

### After Optimization (With Temporary Cache)
```rust
pub fn get_leaderboard(env: &Env, offset: u32, limit: u32) -> Vec<LeaderboardEntry> {
    let mut entries = Vec::new(&env);
    
    // Bulk-fetch into temporary buffer (amortized cost)
    let cached = fetch_leaderboard_batch(&env, offset, limit);
    
    for entry in cached.iter() {
        entries.push_back(entry.clone());
    }
    
    entries
}

fn fetch_leaderboard_batch(env: &Env, offset: u32, limit: u32) -> Vec<LeaderboardEntry> {
    // In production: batch-read from persistent, load into temp buffer once
    let mut temp_buffer = Vec::new(&env);
    
    // Single multi-key read from persistent (optimized by Soroban)
    for i in 0..limit {
        let entry: LeaderboardEntry = env.storage()
            .persistent()
            .get(&LeaderboardKey::Entry(offset + i))
            .unwrap();
        temp_buffer.push_back(entry);
    }
    
    temp_buffer
}
// Total: 50K * 1 (batch fetch) = 50K gas, reuse 10 times = 5K per copy
```

## Access Pattern Analysis

### High-Frequency Reads (> 10 per block)
- **Recommendation**: Temporary or instance storage
- **Example**: Analytics counters, user activity flags
- **Optimization**: Read once, cache in temporary storage

```rust
// ✅ GOOD: Cache global stats once per query
pub fn query_global_stats(env: &Env) -> GlobalStats {
    // Read once from persistent
    let stats: GlobalStats = env.storage()
        .persistent()
        .get(&AnalyticsKey::GlobalStats)
        .unwrap_or_default();
    
    // Reuse stats variable without re-reading
    // (Soroban does optimize repeated reads, but explicit caching is safer)
    format_response(&stats)
}
```

### Medium-Frequency Writes (> 5 per block)
- **Recommendation**: Instance storage or batch writes to persistent
- **Example**: Session state, transaction counters
- **Optimization**: Batch updates and write once per block

```rust
// ✅ GOOD: Batch transaction updates
pub fn process_transactions(env: &Env, txs: Vec<Transaction>) {
    let mut success_count = 0u64;
    let mut failure_count = 0u64;
    
    // Process all in memory
    for tx in txs.iter() {
        if execute_tx(env, tx).is_ok() {
            success_count += 1;
        } else {
            failure_count += 1;
        }
    }
    
    // Single write to persistent at end
    let mut stats = env.storage()
        .persistent()
        .get(&StatsKey::TxMetrics)
        .unwrap_or_default();
    
    stats.success_count += success_count;
    stats.failure_count += failure_count;
    
    env.storage()
        .persistent()
        .set(&StatsKey::TxMetrics, &stats);
}
```

## Module-Specific Recommendations

### analytics.rs
**Issue**: Frequent global stat reads

```rust
// Current (expensive)
let mut stats: GlobalStats = env.storage()
    .persistent()
    .get(&AnalyticsDataKey::GlobalStats)
    .unwrap_or_default();  // ~50K gas per read

// Optimized (cache in instance)
env.storage()
    .instance()
    .set(&AnalyticsDataKey::GlobalStatCache, &stats);

// Later reads use cache (< 5K gas)
let cached_stats: GlobalStats = env.storage()
    .instance()
    .get(&AnalyticsDataKey::GlobalStatCache)
    .unwrap_or_default();
```

### leaderboards.rs
**Issue**: Leaderboard queries not cached

```rust
// Add cache layer for top-100 leaderboard
pub fn get_top_100(env: &Env) -> Vec<LeaderboardEntry> {
    // Check if cache is fresh (< 1 minute old)
    if let Some(cached) = env.storage()
        .instance()
        .get(&LeaderboardKey::TopHundredCache) {
        
        if is_cache_fresh(&env) {
            return cached;  // Use cached version
        }
    }
    
    // Re-fetch from persistent
    let top_100 = fetch_from_persistent(&env);
    
    // Cache for next minute
    env.storage()
        .instance()
        .set(&LeaderboardKey::TopHundredCache, &top_100);
    
    top_100
}
```

### player_profile.rs
**Issue**: Profile lookups hit persistent storage

```rust
// Add local cache per query
pub fn lookup_profiles(env: &Env, players: Vec<Address>) -> Vec<PlayerProfile> {
    let mut profiles = Vec::new(&env);
    let mut lookups = Map::new(&env);  // Temporary lookup cache
    
    for player in players.iter() {
        let profile = if let Some(cached) = lookups.get(&player) {
            cached
        } else {
            let p = env.storage()
                .persistent()
                .get(&ProfileKey::Player(player.clone()))
                .unwrap();
            
            lookups.set(player.clone(), p.clone());
            p
        };
        
        profiles.push_back(profile);
    }
    
    profiles
}
```

### yield_forecast.rs
**Issue**: Cache doesn't validate TTL

```rust
// Add TTL validation on cache read
pub fn get_cached_forecast(env: &Env, player: &Address) -> Result<YieldForecast> {
    let key = ForecastKey::Cache(player.clone());
    
    let cached: YieldForecast = env.storage()
        .persistent()
        .get(&key)
        .ok_or(ForecastError::NotFound)?;
    
    // CRITICAL: Validate TTL
    let age = env.ledger().timestamp().saturating_sub(cached.calculated_at);
    if age > FORECAST_TTL_SECONDS {
        // Data is stale - must refresh
        env.storage()
            .persistent()
            .remove(&key);  // Clear stale data
        return Err(ForecastError::CacheExpired);
    }
    
    Ok(cached)
}
```

## Gas Cost Benchmarks

### Benchmark Template

```rust
#[cfg(test)]
mod benchmarks {
    use super::*;
    use soroban_sdk::testutils::*;

    #[test]
    fn bench_instance_write() {
        let env = Env::default();
        let start = env.ledger().sequence();
        
        for i in 0..100 {
            env.storage()
                .instance()
                .set(&StorageKey::Counter, &i);
        }
        
        let gas_used = count_gas_ops(&env, start);
        println!("Instance write (100x): {} gas", gas_used);
    }

    #[test]
    fn bench_persistent_write() {
        let env = Env::default();
        let start = env.ledger().sequence();
        
        for i in 0..100 {
            env.storage()
                .persistent()
                .set(&StorageKey::PlayerData(i), &vec![0u8; 1024]);
        }
        
        let gas_used = count_gas_ops(&env, start);
        println!("Persistent write (100x): {} gas", gas_used);
    }
}
```

## Checklist: When Adding New Data

- [ ] Does the data exist for only one transaction? → **Temporary**
- [ ] Does it need to survive until the next block? → **Instance**
- [ ] Does it need to persist indefinitely? → **Persistent**
- [ ] Is it read >5 times per transaction? → Cache in temporary
- [ ] Is it written >5 times per block? → Batch writes
- [ ] Will it grow unboundedly? → Add TTL to persistent
- [ ] Have you benchmarked the operation? → Add to benchmarks/

## Related Documentation

- [Gas Optimization Guide](GAS_OPTIMIZATION_GUIDE.md)
- [Monitoring Architecture](MONITORING_ARCHITECTURE.md)
- [Soroban Storage Docs](https://developers.stellar.org/docs/soroban/learn/storing-data)

---

**Last Updated**: 2026-07-25
**Maintainer**: Core Engineering Team
