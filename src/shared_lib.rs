use soroban_sdk::{contracterror, contracttype, Address, Env};

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SharedError {
    InvalidAddress = 1,
    MathOverflow = 2,
}

pub fn validate_address(env: &Env, auth: Address) -> Result<(), SharedError> {
    auth.require_auth();
    Ok(())
}

pub fn calculate_yield(base: i128, multiplier: u32) -> Result<i128, SharedError> {
    let candidate = base.checked_mul(multiplier as i128).ok_or(SharedError::MathOverflow)?;
    Ok(candidate)
}
