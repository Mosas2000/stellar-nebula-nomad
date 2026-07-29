use soroban_sdk::{contracterror, Address, Env};

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SharedError {
    InvalidAddress = 1,
    MathOverflow = 2,
    Unauthorized = 3,
}

pub fn validate_address(_env: &Env, auth: Address) -> Result<(), SharedError> {
    auth.require_auth();
    Ok(())
}

pub fn calculate_yield(base: i128, multiplier: u32) -> Result<i128, SharedError> {
    let candidate = base.checked_mul(multiplier as i128).ok_or(SharedError::MathOverflow)?;
    Ok(candidate)
}

/// Require authorization for an address.
#[macro_export]
macro_rules! ensure_auth {
    ($addr:expr) => {
        $addr.require_auth();
    };
}

/// Fetch a value from instance storage or return a default fallback.
#[macro_export]
macro_rules! storage_get_default {
    ($env:expr, $key:expr, $default:expr) => {
        $env.storage().instance().get(&$key).unwrap_or($default)
    };
}

/// Set a value into instance storage.
#[macro_export]
macro_rules! storage_set {
    ($env:expr, $key:expr, $val:expr) => {
        $env.storage().instance().set(&$key, &$val);
    };
}

/// Check if a key exists in instance storage.
#[macro_export]
macro_rules! storage_has {
    ($env:expr, $key:expr) => {
        $env.storage().instance().has(&$key)
    };
}
