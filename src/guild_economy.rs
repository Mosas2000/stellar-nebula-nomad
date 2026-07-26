use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Vec};

use crate::alliance_manager::{get_alliance, get_alliance_treasury, AllianceKey};

// ─── Constants ─────────────────────────────────────────────────────────────

pub const PROPOSAL_DURATION_SECS: u64 = 86_400 * 3; // 3 days

// ─── Data Types ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct TreasuryProposal {
    pub proposal_id: u64,
    pub alliance_id: u64,
    pub proposer: Address,
    pub target_recipient: Address,
    pub amount: i128,
    pub votes_for: u32,
    pub votes_against: u32,
    pub expires_at: u64,
    pub executed: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum GuildEconomyKey {
    Proposal(u64, u64),        // (alliance_id, proposal_id) -> TreasuryProposal
    ProposalCount(u64),        // alliance_id -> u64
    MemberVoted(u64, u64, Address), // (alliance_id, proposal_id, player) -> bool
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum GuildEconomyError {
    ProposalNotFound = 1,
    AlreadyVoted = 2,
    ProposalExpired = 3,
    ProposalNotExpired = 4,
    InsufficientTreasuryFunds = 5,
    ProposalAlreadyExecuted = 6,
    NotAllianceMember = 7,
    ThresholdNotMet = 8,
}

// ─── Functions ────────────────────────────────────────────────────────────────

/// Create a proposal to spend/withdraw essence from the alliance treasury.
pub fn create_treasury_proposal(
    env: &Env,
    proposer: Address,
    alliance_id: u64,
    target_recipient: Address,
    amount: i128,
) -> Result<u64, GuildEconomyError> {
    proposer.require_auth();

    // Verify proposer is in this alliance
    let alliance = get_alliance(env, alliance_id).map_err(|_| GuildEconomyError::NotAllianceMember)?;
    let mut is_member = false;
    for member in alliance.members.iter() {
        if member == proposer {
            is_member = true;
            break;
        }
    }
    if !is_member {
        return Err(GuildEconomyError::NotAllianceMember);
    }

    // Verify treasury has enough funds
    let treasury_bal = get_alliance_treasury(env, alliance_id);
    if treasury_bal < amount {
        return Err(GuildEconomyError::InsufficientTreasuryFunds);
    }

    // Generate proposal ID
    let count_key = GuildEconomyKey::ProposalCount(alliance_id);
    let proposal_id = env.storage().persistent().get(&count_key).unwrap_or(0u64) + 1;
    env.storage().persistent().set(&count_key, &proposal_id);

    let expires_at = env.ledger().timestamp() + PROPOSAL_DURATION_SECS;

    let proposal = TreasuryProposal {
        proposal_id,
        alliance_id,
        proposer: proposer.clone(),
        target_recipient,
        amount,
        votes_for: 1, // Proposer votes FOR automatically
        votes_against: 0,
        expires_at,
        executed: false,
    };

    let prop_key = GuildEconomyKey::Proposal(alliance_id, proposal_id);
    env.storage().persistent().set(&prop_key, &proposal);

    // Record proposer's vote
    let vote_key = GuildEconomyKey::MemberVoted(alliance_id, proposal_id, proposer.clone());
    env.storage().persistent().set(&vote_key, &true);

    env.events().publish(
        (symbol_short!("guild_ec"), symbol_short!("prop_new")),
        (alliance_id, proposal_id, proposer, amount),
    );

    Ok(proposal_id)
}

/// Cast a vote on a treasury spending proposal.
pub fn vote_on_proposal(
    env: &Env,
    voter: Address,
    alliance_id: u64,
    proposal_id: u64,
    approve: bool,
) -> Result<(), GuildEconomyError> {
    voter.require_auth();

    // Verify voter is in alliance
    let alliance = get_alliance(env, alliance_id).map_err(|_| GuildEconomyError::NotAllianceMember)?;
    let mut is_member = false;
    for member in alliance.members.iter() {
        if member == voter {
            is_member = true;
            break;
        }
    }
    if !is_member {
        return Err(GuildEconomyError::NotAllianceMember);
    }

    let prop_key = GuildEconomyKey::Proposal(alliance_id, proposal_id);
    let mut proposal: TreasuryProposal = env
        .storage()
        .persistent()
        .get(&prop_key)
        .ok_or(GuildEconomyError::ProposalNotFound)?;

    if proposal.executed {
        return Err(GuildEconomyError::ProposalAlreadyExecuted);
    }

    if env.ledger().timestamp() > proposal.expires_at {
        return Err(GuildEconomyError::ProposalExpired);
    }

    let vote_key = GuildEconomyKey::MemberVoted(alliance_id, proposal_id, voter.clone());
    if env.storage().persistent().has(&vote_key) {
        return Err(GuildEconomyError::AlreadyVoted);
    }

    env.storage().persistent().set(&vote_key, &true);

    if approve {
        proposal.votes_for += 1;
    } else {
        proposal.votes_against += 1;
    }

    env.storage().persistent().set(&prop_key, &proposal);

    env.events().publish(
        (symbol_short!("guild_ec"), symbol_short!("prop_vote")),
        (alliance_id, proposal_id, voter, approve),
    );

    Ok(())
}

/// Execute a passed proposal, transferring funds from treasury.
pub fn execute_proposal(
    env: &Env,
    caller: Address,
    alliance_id: u64,
    proposal_id: u64,
) -> Result<(), GuildEconomyError> {
    caller.require_auth();

    let prop_key = GuildEconomyKey::Proposal(alliance_id, proposal_id);
    let mut proposal: TreasuryProposal = env
        .storage()
        .persistent()
        .get(&prop_key)
        .ok_or(GuildEconomyError::ProposalNotFound)?;

    if proposal.executed {
        return Err(GuildEconomyError::ProposalAlreadyExecuted);
    }

    // Check voting threshold
    let alliance = get_alliance(env, alliance_id).map_err(|_| GuildEconomyError::NotAllianceMember)?;
    let total_members = alliance.members.len();

    // Threshold calculation: votes_for / total_members >= threshold %
    // votes_for * 100 >= total_members * threshold
    let threshold = alliance.voting_threshold as u32;
    if (proposal.votes_for * 100) < (total_members * threshold) {
        // If not expired, keep waiting. If expired, reject execution.
        if env.ledger().timestamp() <= proposal.expires_at {
            return Err(GuildEconomyError::ThresholdNotMet);
        } else {
            return Err(GuildEconomyError::ProposalExpired);
        }
    }

    // Perform treasury reduction
    let treasury_key = AllianceKey::AllianceTreasury(alliance_id);
    let current_treasury = env
        .storage()
        .persistent()
        .get::<AllianceKey, i128>(&treasury_key)
        .unwrap_or(0);

    if current_treasury < proposal.amount {
        return Err(GuildEconomyError::InsufficientTreasuryFunds);
    }

    let new_treasury = current_treasury.saturating_sub(proposal.amount);
    env.storage().persistent().set(&treasury_key, &new_treasury);

    proposal.executed = true;
    env.storage().persistent().set(&prop_key, &proposal);

    // Emit treasury payout / transfer event
    env.events().publish(
        (symbol_short!("guild_ec"), symbol_short!("prop_exec")),
        (alliance_id, proposal_id, proposal.target_recipient, proposal.amount),
    );

    Ok(())
}

/// Get details of a treasury spending proposal.
pub fn get_proposal(
    env: &Env,
    alliance_id: u64,
    proposal_id: u64,
) -> Option<TreasuryProposal> {
    let prop_key = GuildEconomyKey::Proposal(alliance_id, proposal_id);
    env.storage().persistent().get(&prop_key)
}
