#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

// FIXTURE: ledger_seconds detector
// A ledger sequence number is a block counter (~5s per ledger), not a wall
// clock. Adding a seconds-magnitude literal to `sequence()` is a unit mix-up:
// `sequence() + 86_400` is 86,400 *ledgers*, not one day. Seconds-based windows
// belong on `timestamp()`. The detector flags sequence()+seconds and leaves
// timestamp math and small ledger deltas alone.

#[contract]
pub struct Escrow;

#[contractimpl]
impl Escrow {
    // Violation: one-day window expressed in seconds added to a ledger number.
    pub fn deadline(env: Env) -> u32 {
        env.ledger().sequence() + 86400
    }

    // Safe: seconds window added to the real timestamp.
    pub fn deadline_ts(env: Env) -> u64 {
        env.ledger().timestamp() + 86400
    }

    // Safe: a small ledger delta is a genuine block count, not seconds.
    pub fn next_window(env: Env) -> u32 {
        env.ledger().sequence() + 10
    }
}
