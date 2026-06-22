#![no_std]
use soroban_sdk::{contracttype, Address};

// FIXTURE: ledger_size detector
// A #[contracttype] struct whose estimated size blows past the 64KB ledger
// entry limit (the fixed-size byte array dominates the estimate).

#[contracttype]
pub struct OversizedState {
    pub admin: Address,
    pub blob: [u8; 4096],
}

#[contracttype]
pub struct SmallState {
    pub admin: Address,
    pub counter: u64,
}
