use crate::resource_minter::{AssetId, ResourceKey};
use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env};

/// Gift expiry: ~48 hours at 5-second ledger close time.
const GIFT_EXPIRY_LEDGERS: u32 = 34_560;

/// Maximum gifts a single sender can create per session (burst limit).
const MAX_GIFTS_PER_SESSION: u32 = 10;

// ── Storage Keys ──────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum GiftKey {
    GiftCounter,
    Gift(u64),
    SenderSessionCount(Address),
}

// ── Error Codes ───────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum GiftError {
    ZeroAmount = 1,
    InsufficientBalance = 2,
    GiftNotFound = 3,
    GiftExpired = 4,
    NotReceiver = 5,
    GiftAlreadyClaimed = 6,
    SelfGift = 7,
    BurstLimitExceeded = 8,
}

// ── Data Structures ───────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub struct Gift {
    pub id: u64,
    pub sender: Address,
    pub receiver: Address,
    pub resource: AssetId,
    pub amount: i128,
    pub created_ledger: u32,
    pub claimed: bool,
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn next_gift_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&GiftKey::GiftCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage()
        .instance()
        .set(&GiftKey::GiftCounter, &next);
    next
}

fn increment_sender_burst(env: &Env, sender: &Address) -> Result<(), GiftError> {
    let key = GiftKey::SenderSessionCount(sender.clone());
    let count: u32 = env.storage().instance().get(&key).unwrap_or(0);
    if count >= MAX_GIFTS_PER_SESSION {
        return Err(GiftError::BurstLimitExceeded);
    }
    env.storage().instance().set(&key, &(count + 1));
    Ok(())
}

// ── Public Functions ──────────────────────────────────────────────────────

/// Send a resource gift to another player.
///
/// Debits the sender's resource balance and creates a pending gift record.
/// The gift expires after ~48 hours if not accepted. No refunds on expiry
/// (resources are burned as a deliberate anti-spam design).
pub fn send_gift(
    env: &Env,
    sender: Address,
    receiver: Address,
    resource: AssetId,
    amount: i128,
) -> Result<u64, GiftError> {
    sender.require_auth();

    if sender == receiver {
        return Err(GiftError::SelfGift);
    }
    if amount <= 0 {
        return Err(GiftError::ZeroAmount);
    }

    increment_sender_burst(env, &sender)?;

    let balance_key = ResourceKey::ResourceBalance(sender.clone(), resource.clone());
    let balance: u32 = env.storage().instance().get(&balance_key).unwrap_or(0);
    if (balance as i128) < amount {
        return Err(GiftError::InsufficientBalance);
    }

    env.storage()
        .instance()
        .set(&balance_key, &(balance - amount as u32));

    let gift_id = next_gift_id(env);
    let gift = Gift {
        id: gift_id,
        sender: sender.clone(),
        receiver: receiver.clone(),
        resource: resource.clone(),
        amount,
        created_ledger: env.ledger().sequence(),
        claimed: false,
    };

    env.storage()
        .persistent()
        .set(&GiftKey::Gift(gift_id), &gift);

    env.events().publish(
        (symbol_short!("gift"), symbol_short!("sent")),
        (gift_id, sender, receiver, resource, amount),
    );

    Ok(gift_id)
}

/// Accept a pending gift and credit the resources to the receiver's balance.
///
/// The receiver must authorize the call. Gifts expire after ~48 hours.
pub fn accept_gift(env: &Env, receiver: Address, gift_id: u64) -> Result<(), GiftError> {
    receiver.require_auth();

    let mut gift: Gift = env
        .storage()
        .persistent()
        .get(&GiftKey::Gift(gift_id))
        .ok_or(GiftError::GiftNotFound)?;

    if gift.claimed {
        return Err(GiftError::GiftAlreadyClaimed);
    }
    if gift.receiver != receiver {
        return Err(GiftError::NotReceiver);
    }

    let age = env.ledger().sequence() - gift.created_ledger;
    if age > GIFT_EXPIRY_LEDGERS {
        return Err(GiftError::GiftExpired);
    }

    let balance_key = ResourceKey::ResourceBalance(receiver.clone(), gift.resource.clone());
    let balance: u32 = env.storage().instance().get(&balance_key).unwrap_or(0);
    env.storage()
        .instance()
        .set(&balance_key, &(balance + gift.amount as u32));

    gift.claimed = true;
    env.storage()
        .persistent()
        .set(&GiftKey::Gift(gift_id), &gift);

    env.events().publish(
        (symbol_short!("gift"), symbol_short!("accept")),
        (gift_id, receiver),
    );

    Ok(())
}

/// Read a gift record by ID.
pub fn get_gift(env: &Env, gift_id: u64) -> Option<Gift> {
    env.storage().persistent().get(&GiftKey::Gift(gift_id))
}
