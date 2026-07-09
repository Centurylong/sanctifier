#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

// FIXTURE: ledger_seconds_confusion detector
// Ledger sequence counts and wall-clock seconds must not be mixed directly.

#[contract]
pub struct TimeMathContract;

#[contractimpl]
impl TimeMathContract {
    // Violation: adds seconds directly to a ledger sequence.
    pub fn vesting_end_ledger(_env: Env, current_ledger: u32, cliff_seconds: u32) -> u32 {
        current_ledger + cliff_seconds
    }

    // Violation: passes a seconds-style variable to an API expecting ledgers.
    pub fn bump_ttl(env: Env, ttl_seconds: u32) {
        env.storage().instance().extend_ttl(100, ttl_seconds);
    }

    // Clean: ledger-only math is already in ledger units.
    pub fn expiry_ledger(_env: Env, current_ledger: u32, extra_ledgers: u32) -> u32 {
        current_ledger + extra_ledgers
    }

    // Clean: wall-clock timestamp math stays in seconds/timestamps.
    pub fn unlock_time(_env: Env, now_timestamp: u64, duration_seconds: u64) -> u64 {
        now_timestamp + duration_seconds
    }
}
