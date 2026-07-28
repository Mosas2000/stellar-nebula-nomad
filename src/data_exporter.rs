//! Bounded export of contract state for external consumers.
//!
use soroban_sdk::{contracterror, contracttype, Address, Bytes, Env, Vec};

use crate::player_profile::get_profile_by_owner;

const MAX_EXPORT_LIMIT: u32 = 200;

#[derive(Clone)]
#[contracttype]
pub enum ExportKey {
    Settings(u64),
    Registry,
    Session,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ExportError {
    ExportLimitExceeded = 1,
    NotOptedIn = 2,
    ProfileNotFound = 3,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ExportSettings {
    pub enabled: bool,
    pub compressed: bool,
    pub last_export_at: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ExportSession {
    pub cursor: u32,
    pub last_limit: u32,
    pub total_exports: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ExportRecord {
    pub player: Address,
    pub profile_id: u64,
    pub payload: Bytes,
    pub compressed: bool,
}

fn default_settings() -> ExportSettings {
    ExportSettings {
        enabled: false,
        compressed: false,
        last_export_at: 0,
    }
}

fn profile_id_for_player(env: &Env, player: &Address) -> Result<u64, ExportError> {
    let profile = get_profile_by_owner(env, player).map_err(|_| ExportError::ProfileNotFound)?;
    Ok(profile.id)
}

fn load_settings(env: &Env, profile_id: u64) -> ExportSettings {
    env.storage()
        .persistent()
        .get(&ExportKey::Settings(profile_id))
        .unwrap_or_else(default_settings)
}

fn store_settings(env: &Env, profile_id: u64, settings: &ExportSettings) {
    env.storage()
        .persistent()
        .set(&ExportKey::Settings(profile_id), settings);
}

fn load_registry(env: &Env) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&ExportKey::Registry)
        .unwrap_or_else(|| Vec::new(env))
}

fn store_registry(env: &Env, registry: &Vec<u64>) {
    env.storage().persistent().set(&ExportKey::Registry, registry);
}

fn load_session(env: &Env) -> ExportSession {
    env.storage()
        .instance()
        .get(&ExportKey::Session)
        .unwrap_or(ExportSession {
            cursor: 0,
            last_limit: 0,
            total_exports: 0,
        })
}

fn store_session(env: &Env, session: &ExportSession) {
    env.storage().instance().set(&ExportKey::Session, session);
}

fn append_bytes(target: &mut Bytes, data: &[u8]) {
    let mut i = 0usize;
    while i < data.len() {
        target.push_back(data[i]);
        i += 1;
    }
}

fn append_u64(target: &mut Bytes, mut value: u64) {
    let mut digits = [0u8; 20];
    let mut len = 0usize;

    if value == 0 {
        target.push_back(b'0');
        return;
    }

    while value > 0 {
        digits[len] = b'0' + (value % 10) as u8;
        value /= 10;
        len += 1;
    }

    while len > 0 {
        len -= 1;
        target.push_back(digits[len]);
    }
}

fn append_i128(target: &mut Bytes, value: i128) {
    if value < 0 {
        target.push_back(b'-');
        append_u128(target, (-value) as u128);
    } else {
        append_u128(target, value as u128);
    }
}

fn append_u128(target: &mut Bytes, mut value: u128) {
    let mut digits = [0u8; 39];
    let mut len = 0usize;

    if value == 0 {
        target.push_back(b'0');
        return;
    }

    while value > 0 {
        digits[len] = b'0' + (value % 10) as u8;
        value /= 10;
        len += 1;
    }

    while len > 0 {
        len -= 1;
        target.push_back(digits[len]);
    }
}

fn build_payload(
    env: &Env,
    player: &Address,
    profile_id: u64,
    settings: &ExportSettings,
) -> Result<Bytes, ExportError> {
    let profile = get_profile_by_owner(env, player).map_err(|_| ExportError::ProfileNotFound)?;

    let mut payload = Bytes::new(env);
    if settings.compressed {
        append_bytes(&mut payload, b"v1|");
        append_u64(&mut payload, profile_id);
        append_bytes(&mut payload, b"|");
        append_u64(&mut payload, profile.total_scans as u64);
        append_bytes(&mut payload, b"|");
        append_i128(&mut payload, profile.essence_earned);
        append_bytes(&mut payload, b"|");
        append_u64(&mut payload, if settings.compressed { 1 } else { 0 });
    } else {
        append_bytes(
            &mut payload,
            b"profile_id,scans,essence,compressed,last_updated\n",
        );
        append_u64(&mut payload, profile_id);
        append_bytes(&mut payload, b",");
        append_u64(&mut payload, profile.total_scans as u64);
        append_bytes(&mut payload, b",");
        append_i128(&mut payload, profile.essence_earned);
        append_bytes(&mut payload, b",");
        append_u64(&mut payload, if settings.compressed { 1 } else { 0 });
        append_bytes(&mut payload, b",");
        append_u64(&mut payload, profile.last_updated);
    }

    Ok(payload)
}

fn add_registry_profile(env: &Env, profile_id: u64) {
    let mut registry = load_registry(env);
    let mut i = 0u32;
    while i < registry.len() {
        if let Some(existing) = registry.get(i) {
            if existing == profile_id {
                return;
            }
        }
        i += 1;
    }
    registry.push_back(profile_id);
    store_registry(env, &registry);
}

fn remove_registry_profile(env: &Env, profile_id: u64) {
    let registry = load_registry(env);
    let mut updated = Vec::new(env);
    let mut i = 0u32;
    while i < registry.len() {
        if let Some(existing) = registry.get(i) {
            if existing != profile_id {
                updated.push_back(existing);
            }
        }
        i += 1;
    }
    store_registry(env, &updated);
}

pub fn set_export_opt_in(
    env: &Env,
    player: Address,
    enabled: bool,
) -> Result<ExportSettings, ExportError> {
    player.require_auth();
    let profile_id = profile_id_for_player(env, &player)?;

    let mut settings = load_settings(env, profile_id);
    settings.enabled = enabled;
    settings.last_export_at = env.ledger().timestamp();
    store_settings(env, profile_id, &settings);

    if enabled {
        add_registry_profile(env, profile_id);
    } else {
        remove_registry_profile(env, profile_id);
    }

    Ok(settings)
}

pub fn set_export_compression(
    env: &Env,
    player: Address,
    compressed: bool,
) -> Result<ExportSettings, ExportError> {
    player.require_auth();
    let profile_id = profile_id_for_player(env, &player)?;

    let mut settings = load_settings(env, profile_id);
    settings.compressed = compressed;
    store_settings(env, profile_id, &settings);
    Ok(settings)
}

pub fn get_export_settings(env: &Env, player: Address) -> ExportSettings {
    if let Ok(profile_id) = profile_id_for_player(env, &player) {
        load_settings(env, profile_id)
    } else {
        default_settings()
    }
}

pub fn export_player_data(env: &Env, player: Address) -> Result<Bytes, ExportError> {
    player.require_auth();
    let profile_id = profile_id_for_player(env, &player)?;
    let settings = load_settings(env, profile_id);
    if !settings.enabled {
        return Err(ExportError::NotOptedIn);
    }

    build_payload(env, &player, profile_id, &settings)
}

pub fn batch_export_players(env: &Env, limit: u32) -> Result<Vec<ExportRecord>, ExportError> {
    if limit == 0 || limit > MAX_EXPORT_LIMIT {
        return Err(ExportError::ExportLimitExceeded);
    }

    let registry = load_registry(env);
    let mut session = load_session(env);
    let mut exports = Vec::new(env);

    if registry.len() == 0 {
        session.cursor = 0;
        session.last_limit = limit;
        store_session(env, &session);
        return Ok(exports);
    }

    let start = session.cursor.min(registry.len());
    let mut index = start;
    let mut produced = 0u32;

    while index < registry.len() && produced < limit {
        if let Some(profile_id) = registry.get(index) {
            if let Some(profile) = env
                .storage()
                .persistent()
                .get::<crate::player_profile::ProfileKey, crate::player_profile::PlayerProfile>(
                    &crate::player_profile::ProfileKey::Profile(profile_id),
                )
            {
                let settings = load_settings(env, profile_id);
                if settings.enabled {
                    let payload = build_payload(env, &profile.owner, profile_id, &settings)?;
                    exports.push_back(ExportRecord {
                        player: profile.owner,
                        profile_id,
                        payload,
                        compressed: settings.compressed,
                    });
                    produced += 1;
                }
            }
        }
        index += 1;
    }

    session.cursor = if index >= registry.len() { 0 } else { index };
    session.last_limit = limit;
    session.total_exports = session.total_exports.saturating_add(exports.len() as u64);
    store_session(env, &session);

    Ok(exports)
}

pub fn get_export_session(env: &Env) -> ExportSession {
    load_session(env)
}
