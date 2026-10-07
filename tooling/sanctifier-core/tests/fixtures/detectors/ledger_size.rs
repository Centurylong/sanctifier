#![no_std]
use soroban_sdk::{contracttype, Address, BytesN};

// FIXTURE: ledger_size detector
// A supported fixed-size byte payload that stays comfortably below the 64KB
// ledger-entry limit. This guards against false positives in size estimation.

#[contracttype]
pub struct FixedBlobState {
    pub admin: Address,
    pub blob: BytesN<4096>,
}

#[contracttype]
pub struct SmallState {
    pub admin: Address,
    pub counter: u64,
}
