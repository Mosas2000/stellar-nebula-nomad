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

