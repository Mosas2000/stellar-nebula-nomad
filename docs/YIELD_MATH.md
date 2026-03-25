# Yield Math: Resource Harvesting & Passive Staking

This document explains the economic formulae used in `src/resource_minter.rs` for
harvesting stardust and calculating cosmic essence (Plasma) yields.

---

## 1. Stardust Harvest

### Formula

```
minted = min(stardust_per_anomaly + anomaly_index × 10,  daily_cap - harvested_today)
```

### Parameters

| Parameter             | Default | Notes                                    |
|-----------------------|---------|------------------------------------------|
| `stardust_per_anomaly`| 100     | Base mint per harvest call               |
| `anomaly_index`       | 0–N     | Rarity score returned by Nebula Explorer |
| `daily_cap`           | 1 000   | Max stardust per ship per ~24 h window   |

### Example

| anomaly_index | raw amount | cap remaining | minted |
|---------------|-----------|---------------|--------|
| 0             | 100       | 1 000         | 100    |
| 5             | 150       | 1 000         | 150    |
| 9             | 190       | 200           | 190    |
| 9             | 190       | 100           | 100    |  ← capped

### Daily window reset

The daily window resets when:

```
current_ledger − last_reset_ledger ≥ LEDGERS_PER_DAY
```

`LEDGERS_PER_DAY = 17 280`  (5 s/ledger × 86 400 s/day ÷ 5 = 17 280)

---

## 2. Staking Yield (Cosmic Essence / Plasma)

### APY Model

The contract uses a **continuous pro-rated APY** model backed by ledger sequence
numbers.  There are no discrete compounding periods — yield accrues every ledger
and can be claimed at any time.

### Formula

```
yield = principal × apy_bps / 10_000 × Δledgers / LEDGERS_PER_YEAR
```

Where:

| Variable           | Description                                          |
|--------------------|------------------------------------------------------|
| `principal`        | Staked resource amount                               |
| `apy_bps`          | APY in basis points (500 = 5 %)                      |
| `Δledgers`         | `current_ledger − last_claim_ledger`                 |
| `LEDGERS_PER_YEAR` | `17 280 × 365 = 6 307 200`                           |

### Worked examples

#### 5 % APY, 100 stardust staked

| Period             | Δledgers        | Yield (integer)  |
|--------------------|-----------------|------------------|
| 1 day              | 17 280          | 0 (< 1 unit)     |
| 30 days            | 518 400         | 0 (< 1 unit)     |
| 73 days (~20 %)    | 1 261 440       | 1 plasma         |
| 146 days (~40 %)   | 2 522 880       | 2 plasma         |
| 1 year (365 days)  | 6 307 200       | **5 plasma**     |
| 2 years            | 12 614 400      | **10 plasma**    |

#### 10 % APY, 1 000 stardust staked

| Period   | Δledgers    | Yield (integer) |
|----------|-------------|-----------------|
| 30 days  | 518 400     | 8 plasma        |
| 90 days  | 1 555 200   | 24 plasma       |
| 1 year   | 6 307 200   | **100 plasma**  |

### Integer truncation

All arithmetic uses `i128` integer division.  Sub-unit yields are truncated (not
rounded) per ledger.  This is intentional:

- Makes contract execution **deterministic** across all nodes.
- Prevents micro-rounding exploits (an attacker cannot profit by claiming every
  single ledger because fractional cosmic essence is discarded).
- Long-term stakers receive the full expected yield because truncation is applied
  per claim interval, not per ledger.

**Best practice:** claim yield infrequently (weekly / monthly) to minimise
truncation loss.

---

## 3. Time-Lock Security

### Why time-locks exist

Without a minimum stake duration an attacker could:

1. Stake a large amount at ledger N.
2. Advance one ledger (Δ = 1).
3. Unstake, collect yield = principal × apy_bps / (10_000 × 6_307_200) per ledger.
4. Repeat thousands of times in a single block batch.

The result is near-zero yield per iteration but infinite repetition within a
block — a flash-stake attack draining the yield pool.

### Protection

```
unstake is blocked while  current_ledger < start_ledger + min_stake_duration
```

`min_stake_duration` defaults to `LEDGERS_PER_DAY = 17 280` (≈ 1 day).
Administrators can increase this value for stronger protection.

---

## 4. Multi-Resource Economics

| Resource   | Source                        | Use                                     |
|------------|-------------------------------|-----------------------------------------|
| Stardust   | Harvested from anomalies      | Staking principal; DEX trading          |
| Plasma     | Earned as staking yield       | Ship upgrades; future crafting          |
| Crystals   | Reserved for future mechanics | Not yet issued                          |

Balances for each type are stored independently under
`DataKey::Balance(owner, ResourceType)`, so adding a new resource type
requires no migration of existing balances.

---

## 5. Configuration Reference

| Config field          | `.env.example` key          | Default | Updatable |
|-----------------------|-----------------------------|---------|-----------|
| `apy_basis_points`    | `STARDUST_APY_BPS`          | 500     | Yes (admin) |
| `daily_harvest_cap`   | `DAILY_HARVEST_CAP`         | 1 000   | Yes (admin) |
| `stardust_per_anomaly`| `STARDUST_PER_ANOMALY`      | 100     | No (redeploy) |
| `min_stake_duration`  | `MIN_STAKE_DURATION_LEDGERS`| 17 280  | No (redeploy) |

---

## 6. Event Reference

| Event topic        | Data fields                              | When emitted            |
|--------------------|------------------------------------------|-------------------------|
| `res_harv`         | `(ship_id, anomaly_index, amount)`       | Successful harvest      |
| `yld_claim`        | `(staked_amount, yield_amount, ledger)`  | Yield claimed / unstake |
