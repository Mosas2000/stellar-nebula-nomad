use soroban_sdk::{contracttype, Address};

/// Resource data structure for in-game tradeable resources.
#[derive(Clone)]
#[contracttype]
pub struct Resource {
    pub id: u64,
    pub owner: Address,
    pub resource_type: u32,
    pub quantity: u32,
}

    /// Pro-rated APY yield calculation.
    ///
    /// ```text
    /// yield = principal × apy_bps / 10_000 × elapsed_ledgers / LEDGERS_PER_YEAR
    /// ```
    ///
    /// Integer division truncates fractional cosmic essence — this is intentional
    /// to keep the contract deterministic and avoid rounding exploits.
    fn calculate_yield(stake: &StakeRecord, current_ledger: u32, apy_bps: u32) -> i128 {
        let elapsed = current_ledger.saturating_sub(stake.last_claim_ledger) as i128;
        if elapsed == 0 {
            return 0;
        }
        stake.amount * (apy_bps as i128) * elapsed / (BPS_DENOM * LEDGERS_PER_YEAR)
    }
}
