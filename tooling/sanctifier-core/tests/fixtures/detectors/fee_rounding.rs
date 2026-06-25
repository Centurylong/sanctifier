#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

// FIXTURE: fee_rounding detector
// Fee/interest calculations that use integer division without a minimum-fee guard
// allow attackers to split transactions into micro-amounts so the computed fee
// rounds to zero, evading all charges.

#[contract]
pub struct FeeContract;

#[contractimpl]
impl FeeContract {
    // Violation: fee rounds to 0 for small amounts, no minimum-fee guard.
    pub fn charge_fee(env: Env, amount: i128, fee_bps: i128) -> i128 {
        let fee = amount * fee_bps / 10_000;
        amount - fee
    }

    // Violation: interest calculation also susceptible to rounding-to-zero.
    pub fn accrue_interest(env: Env, principal: i128, rate_bps: i128) -> i128 {
        let interest = principal * rate_bps / 1_000_000;
        principal + interest
    }

    // Safe: minimum-fee guard ensures at least 1 unit is charged.
    pub fn charge_fee_guarded(env: Env, amount: i128, fee_bps: i128) -> i128 {
        let mut fee = amount * fee_bps / 10_000;
        if fee == 0 && amount > 0 {
            fee = 1;
        }
        amount - fee
    }

    // Safe: .max(1) applied directly in the binding, preventing zero-fee.
    pub fn charge_fee_max(env: Env, amount: i128, fee_bps: i128) -> i128 {
        let fee = (amount * fee_bps / 10_000).max(1);
        amount - fee
    }
}
