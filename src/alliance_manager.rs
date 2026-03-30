use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, String, Vec};

// Alliance configuration constants
pub const MAX_MEMBERS_PER_ALLIANCE: u32 = 50;
pub const MIN_VOTING_THRESHOLD: u32 = 51; // 51% for decisions
pub const INITIAL_TREASURY: i128 = 0;

#[derive(Clone)]
#[contracttype]
pub enum AllianceKey {
    Alliance(u64),              // alliance_id -> Alliance
    AllianceCount,              // -> u64 (next alliance ID)
    MemberAlliance(Address),    // player -> alliance_id
    AllianceTreasury(u64),      // alliance_id -> i128
    MemberContribution(u64, Address), // (alliance_id, member) -> i128
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AllianceError {
    AllianceFull = 1,
    AllianceNotFound = 2,
    AlreadyInAlliance = 3,
    NotMember = 4,
    Unauthorized = 5,
    InvalidName = 6,
    InsufficientVotes = 7,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct Alliance {
    pub alliance_id: u64,
    pub name: String,
    pub founder: Address,
    pub created_at: u64,
    pub members: Vec<Address>,
    pub voting_threshold: u32,
    pub is_active: bool,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct MembershipRecord {
    pub alliance_id: u64,
    pub member: Address,
    pub joined_at: u64,
    pub contribution: i128,
}

/// Found a new alliance with initial treasury
pub fn found_alliance(
    env: &Env,
    founder: Address,
    name: String,
) -> Result<u64, AllianceError> {
    founder.require_auth();
    
    // Check if founder is already in an alliance
    if env.storage().persistent().has(&AllianceKey::MemberAlliance(founder.clone())) {
        return Err(AllianceError::AlreadyInAlliance);
    }
    
    if name.len() == 0 {
        return Err(AllianceError::InvalidName);
    }
    
    // Generate alliance ID
    let alliance_id = env
        .storage()
        .persistent()
        .get::<AllianceKey, u64>(&AllianceKey::AllianceCount)
        .unwrap_or(0);
    
    let current_time = env.ledger().timestamp();
    let mut members = Vec::new(env);
    members.push_back(founder.clone());
    
    let alliance = Alliance {
        alliance_id,
        name: name.clone(),
        founder: founder.clone(),
        created_at: current_time,
        members,
        voting_threshold: MIN_VOTING_THRESHOLD,
        is_active: true,
    };
    
    // Store alliance
    env.storage()
        .persistent()
        .set(&AllianceKey::Alliance(alliance_id), &alliance);
    
    // Update alliance count
    env.storage()
        .persistent()
        .set(&AllianceKey::AllianceCount, &(alliance_id + 1));
    
    // Set founder's alliance membership
    env.storage()
        .persistent()
        .set(&AllianceKey::MemberAlliance(founder.clone()), &alliance_id);
    
    // Initialize treasury
    env.storage()
        .persistent()
        .set(&AllianceKey::AllianceTreasury(alliance_id), &INITIAL_TREASURY);
    
    // Initialize founder contribution
    env.storage()
        .persistent()
        .set(&AllianceKey::MemberContribution(alliance_id, founder.clone()), &0i128);
    
    // Emit event
    env.events().publish(
        (symbol_short!("alliance"), symbol_short!("founded")),
        (alliance_id, name, founder),
    );
    
    Ok(alliance_id)
}

/// Join an existing alliance
pub fn join_alliance(
    env: &Env,
    alliance_id: u64,
    player: Address,
) -> Result<MembershipRecord, AllianceError> {
    player.require_auth();
    
    // Check if player is already in an alliance
    if env.storage().persistent().has(&AllianceKey::MemberAlliance(player.clone())) {
        return Err(AllianceError::AlreadyInAlliance);
    }
    
    // Get alliance
    let mut alliance = env
        .storage()
        .persistent()
        .get::<AllianceKey, Alliance>(&AllianceKey::Alliance(alliance_id))
        .ok_or(AllianceError::AllianceNotFound)?;
    
    // Check member limit
    if alliance.members.len() >= MAX_MEMBERS_PER_ALLIANCE.try_into().unwrap() {
        return Err(AllianceError::AllianceFull);
    }
    
    // Add member
    alliance.members.push_back(player.clone());
    
    // Update alliance
    env.storage()
        .persistent()
        .set(&AllianceKey::Alliance(alliance_id), &alliance);
    
    // Set player's alliance membership
    env.storage()
        .persistent()
        .set(&AllianceKey::MemberAlliance(player.clone()), &alliance_id);
    
    // Initialize member contribution
    env.storage()
        .persistent()
        .set(&AllianceKey::MemberContribution(alliance_id, player.clone()), &0i128);
    
    let current_time = env.ledger().timestamp();
    let membership = MembershipRecord {
        alliance_id,
        member: player.clone(),
        joined_at: current_time,
        contribution: 0,
    };
    
    // Emit event
    env.events().publish(
        (symbol_short!("alliance"), symbol_short!("joined")),
        (alliance_id, player),
    );
    
    Ok(membership)
}

/// Leave an alliance (revocable membership)
pub fn leave_alliance(
    env: &Env,
    player: Address,
) -> Result<(), AllianceError> {
    player.require_auth();
    
    // Get player's alliance
    let alliance_id = env
        .storage()
        .persistent()
        .get::<AllianceKey, u64>(&AllianceKey::MemberAlliance(player.clone()))
        .ok_or(AllianceError::NotMember)?;
    
    // Get alliance
    let mut alliance = env
        .storage()
        .persistent()
        .get::<AllianceKey, Alliance>(&AllianceKey::Alliance(alliance_id))
        .ok_or(AllianceError::AllianceNotFound)?;
    
    // Remove member from alliance
    let mut updated_members = Vec::new(env);
    for member in alliance.members.iter() {
        if member != player {
            updated_members.push_back(member);
        }
    }
    
    alliance.members = updated_members;
    
    // Update alliance
    env.storage()
        .persistent()
        .set(&AllianceKey::Alliance(alliance_id), &alliance);
    
    // Remove player's alliance membership
    env.storage()
        .persistent()
        .remove(&AllianceKey::MemberAlliance(player.clone()));
    
    // Emit event
    env.events().publish(
        (symbol_short!("alliance"), symbol_short!("left")),
        (alliance_id, player),
    );
    
    Ok(())
}

/// Contribute resources to alliance treasury
pub fn contribute_to_treasury(
    env: &Env,
    player: Address,
    amount: i128,
) -> Result<i128, AllianceError> {
    player.require_auth();
    
    if amount <= 0 {
        return Err(AllianceError::Unauthorized);
    }
    
    // Get player's alliance
    let alliance_id = env
        .storage()
        .persistent()
        .get::<AllianceKey, u64>(&AllianceKey::MemberAlliance(player.clone()))
        .ok_or(AllianceError::NotMember)?;
    
    // Update treasury
    let current_treasury = env
        .storage()
        .persistent()
        .get::<AllianceKey, i128>(&AllianceKey::AllianceTreasury(alliance_id))
        .unwrap_or(INITIAL_TREASURY);
    
    let new_treasury = current_treasury.saturating_add(amount);
    
    env.storage()
        .persistent()
        .set(&AllianceKey::AllianceTreasury(alliance_id), &new_treasury);
    
    // Update member contribution
    let current_contribution = env
        .storage()
        .persistent()
        .get::<AllianceKey, i128>(&AllianceKey::MemberContribution(alliance_id, player.clone()))
        .unwrap_or(0);
    
    let new_contribution = current_contribution.saturating_add(amount);
    
    env.storage()
        .persistent()
        .set(&AllianceKey::MemberContribution(alliance_id, player.clone()), &new_contribution);
    
    // Emit event
    env.events().publish(
        (symbol_short!("alliance"), symbol_short!("contrib")),
        (alliance_id, player, amount, new_treasury),
    );
    
    Ok(new_treasury)
}

/// Get alliance details
pub fn get_alliance(env: &Env, alliance_id: u64) -> Result<Alliance, AllianceError> {
    env.storage()
        .persistent()
        .get::<AllianceKey, Alliance>(&AllianceKey::Alliance(alliance_id))
        .ok_or(AllianceError::AllianceNotFound)
}

/// Get alliance treasury balance
pub fn get_alliance_treasury(env: &Env, alliance_id: u64) -> i128 {
    env.storage()
        .persistent()
        .get::<AllianceKey, i128>(&AllianceKey::AllianceTreasury(alliance_id))
        .unwrap_or(INITIAL_TREASURY)
}

/// Get member's contribution to alliance
pub fn get_member_contribution(env: &Env, alliance_id: u64, member: Address) -> i128 {
    env.storage()
        .persistent()
        .get::<AllianceKey, i128>(&AllianceKey::MemberContribution(alliance_id, member))
        .unwrap_or(0)
}

/// Get player's current alliance ID
pub fn get_player_alliance(env: &Env, player: Address) -> Option<u64> {
    env.storage()
        .persistent()
        .get::<AllianceKey, u64>(&AllianceKey::MemberAlliance(player))
}
