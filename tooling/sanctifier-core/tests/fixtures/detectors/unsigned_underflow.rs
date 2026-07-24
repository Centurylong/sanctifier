#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

// FIXTURE: unsigned_underflow detector
// Unchecked subtraction on unsigned integers wraps past zero in release builds,
// silently turning `balance - amount` (when amount > balance) into a huge value.
// The detector flags the bare `-` / `-=` operator on unsigned-typed operands and
// leaves signed arithmetic and the checked_sub/saturating_sub forms alone.

#[contract]
pub struct Ledger;

#[contractimpl]
impl Ledger {
    // Violation: unsigned parameter subtracted with the bare `-` operator.
    pub fn withdraw(env: Env, balance: u64, amount: u64) -> u64 {
        balance - amount
    }

    // Violation: unsigned `-=` compound assignment on a local.
    pub fn spend(env: Env, cost: u128) -> u128 {
        let mut total: u128 = 1_000;
        total -= cost;
        total
    }

    // Safe: signed arithmetic is out of scope for this unsigned-specific rule.
    pub fn delta(env: Env, a: i128, b: i128) -> i128 {
        a - b
    }

    // Safe: saturating_sub clamps at zero instead of wrapping.
    pub fn withdraw_safe(env: Env, balance: u64, amount: u64) -> u64 {
        balance.saturating_sub(amount)
    }

    // Safe: checked_sub surfaces underflow as None.
    pub fn withdraw_checked(env: Env, balance: u64, amount: u64) -> Option<u64> {
        balance.checked_sub(amount)
    }
}
