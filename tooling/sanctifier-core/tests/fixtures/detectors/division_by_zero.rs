#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

// FIXTURE: division_by_zero detector
// Division/modulo by a non-constant value that isn't proven non-zero first
// panics on Soroban's host if that value happens to be zero at runtime.

#[contract]
pub struct DivisionByZeroContract;

#[contractimpl]
impl DivisionByZeroContract {
    // Violation: `count` could be zero, no guard before the division.
    pub fn average(env: Env, total: i128, count: i128) -> i128 {
        total / count
    }

    // Violation: modulo by a possibly-zero variable.
    pub fn pick_winner(env: Env, seed: u64, players: u32) -> u32 {
        (seed as u32) % players
    }

    // Safe: constant denominator can never be zero.
    pub fn to_bps(env: Env, amount: i128) -> i128 {
        amount / 10_000
    }

    // Safe: sibling early-return guard proves `count` non-zero before use.
    pub fn average_guarded(env: Env, total: i128, count: i128) -> i128 {
        if count == 0 {
            return 0;
        }
        total / count
    }

    // Safe: division only reachable through the `count != 0` branch.
    pub fn average_branch_guarded(env: Env, total: i128, count: i128) -> i128 {
        if count != 0 {
            total / count
        } else {
            0
        }
    }

    // Safe: checked_div never panics.
    pub fn average_checked(env: Env, total: i128, count: i128) -> i128 {
        total.checked_div(count).unwrap_or(0)
    }
}
