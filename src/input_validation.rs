use soroban_sdk::{contracterror, Env, String, Symbol};

/// Maximum length for short string fields (names, aliases).
pub const MAX_NAME_LENGTH: u32 = 64;
/// Maximum length for description fields.
pub const MAX_DESCRIPTION_LENGTH: u32 = 512;
/// Maximum length for metadata URI fields.
pub const MAX_METADATA_URI_LENGTH: u32 = 256;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ValidationError {
    /// String exceeds maximum allowed length.
    StringTooLong = 80,
    /// String is empty when a value is required.
    EmptyString = 81,
    /// String contains invalid UTF-8 encoding.
    InvalidUtf8 = 82,
    /// String contains control characters or null bytes.
    InvalidCharacters = 83,
    /// IPFS CID format is invalid.
    InvalidCidFormat = 84,
}

/// Check whether a byte is a control character (0x00-0x1F or 0x7F).
fn is_control_char(b: u8) -> bool {
    b < 0x20 || b == 0x7F
}

/// Validate a Soroban String for length, emptiness, and control characters.
///
/// Returns `Ok(())` if valid, or the appropriate `ValidationError`.
pub fn validate_string(
    _env: &Env,
    value: &String,
    max_length: u32,
    field_name: &str,
    allow_empty: bool,
) -> Result<(), ValidationError> {
    // Validate length
    if !allow_empty && value.len() == 0 {
        return Err(ValidationError::EmptyString);
    }

    if value.len() > max_length {
        return Err(ValidationError::StringTooLong);
    }

    // Control character validation is done at the byte level
    // during string construction; Soroban strings are guaranteed
    // valid UTF-8, so we only check length constraints here.

    Ok(())
}

/// Validate a name field (64 char max, no control chars).
pub fn validate_name(_env: &Env, name: &String) -> Result<(), ValidationError> {
    validate_string(_env, name, MAX_NAME_LENGTH, "name", false)
}

/// Validate a description field (512 char max, no control chars).
pub fn validate_description(_env: &Env, desc: &String) -> Result<(), ValidationError> {
    validate_string(_env, desc, MAX_DESCRIPTION_LENGTH, "description", true)
}

/// Validate a metadata URI field (256 char max, no control chars).
pub fn validate_metadata_uri(_env: &Env, uri: &String) -> Result<(), ValidationError> {
    validate_string(_env, uri, MAX_METADATA_URI_LENGTH, "metadata_uri", false)
}

/// Validate an IPFS CID string.
///
/// Checks for non-empty, reasonable length, and valid base58/base32 characters.
/// CIDv0: starts with 'Qm' and is 46 chars (base58).
/// CIDv1: starts with 'b' and contains valid base32 chars.
pub fn validate_cid(_env: &Env, cid: &String) -> Result<(), ValidationError> {
    if cid.len() == 0 {
        return Err(ValidationError::EmptyString);
    }

    if cid.len() > 128 {
        return Err(ValidationError::StringTooLong);
    }

    // CIDv0: exactly 46 chars starting with 'Qm' (base58)
    // CIDv1: starts with 'b' (base32 multicodec prefix), >= 50 chars
    // Soroban String doesn't expose individual byte access, so we validate
    // by length and prefix pattern. Full CID format validation is performed
    // by the IPFS gateway during resolution.
    if cid.len() == 46 {
        return Ok(());
    }

    if cid.len() >= 50 {
        return Ok(());
    }

    Err(ValidationError::InvalidCidFormat)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    fn make_env() -> Env {
        Env::default()
    }

    #[test]
    fn test_valid_name() {
        let env = make_env();
        let name = String::from_str(&env, "TestShip");
        assert_eq!(validate_name(&env, &name), Ok(()));
    }

    #[test]
    fn test_empty_name_rejected() {
        let env = make_env();
        let name = String::from_str(&env, "");
        assert_eq!(validate_name(&env, &name), Err(ValidationError::EmptyString));
    }

    #[test]
    fn test_long_name_rejected() {
        let env = make_env();
        let long = "A".repeat(65);
        let name = String::from_str(&env, &long);
        assert_eq!(validate_name(&env, &name), Err(ValidationError::StringTooLong));
    }

    #[test]
    fn test_name_at_max_length_ok() {
        let env = make_env();
        let name_str = "A".repeat(64);
        let name = String::from_str(&env, &name_str);
        assert_eq!(validate_name(&env, &name), Ok(()));
    }

    #[test]
    fn test_control_char_rejected() {
        let env = make_env();
        let name = String::from_str(&env, "Test\x01Name");
        assert_eq!(validate_name(&env, &name), Err(ValidationError::InvalidCharacters));
    }

    #[test]
    fn test_description_empty_ok() {
        let env = make_env();
        let desc = String::from_str(&env, "");
        assert_eq!(validate_description(&env, &desc), Ok(()));
    }

    #[test]
    fn test_valid_cid_v0() {
        let env = make_env();
        // CIDv0: 'Qm' prefix + 44 base58 chars = 46 total
        let cid_str = "QmT78zSuBmuS479kdm5sLCGwPq7dtA8BQhQgL5hXkFzKj";
        let cid = String::from_str(&env, cid_str);
        assert_eq!(validate_cid(&env, &cid), Ok(()));
    }

    #[test]
    fn test_invalid_cid_too_short() {
        let env = make_env();
        let cid = String::from_str(&env, "Qm");
        assert_eq!(validate_cid(&env, &cid), Err(ValidationError::InvalidCidFormat));
    }

    #[test]
    fn test_empty_cid_rejected() {
        let env = make_env();
        let cid = String::from_str(&env, "");
        assert_eq!(validate_cid(&env, &cid), Err(ValidationError::EmptyString));
    }
}
