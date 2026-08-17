#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

// FIXTURE: vesting_schedule detector
//   * end - start with no guard                        -> flagged
//   * end - start guarded by a diverging sibling `if`   -> ignored
//   * end - start guarded by a wrapping `if end > start` -> ignored
//   * end - start guarded by a bare `assert!`           -> ignored
//   * unrelated subtraction                             -> ignored

#[contract]
pub struct VestingContract;

#[contractimpl]
impl VestingContract {
    pub fn unguarded(_env: Env, start_ledger: u32, end_ledger: u32, amount: i128) -> i128 {
        let duration = end_ledger - start_ledger;
        amount / (duration as i128)
    }

    pub fn early_return_guard(_env: Env, start_ledger: u32, end_ledger: u32) -> u32 {
        if end_ledger <= start_ledger {
            return 0;
        }
        end_ledger - start_ledger
    }

    pub fn wrapping_guard(_env: Env, start_ledger: u32, end_ledger: u32) -> u32 {
        if end_ledger > start_ledger {
            end_ledger - start_ledger
        } else {
            0
        }
    }

    pub fn assert_guard(_env: Env, start_ledger: u32, end_ledger: u32) -> u32 {
        assert!(end_ledger > start_ledger, "invalid schedule");
        end_ledger - start_ledger
    }

    pub fn unrelated(_env: Env, a: i128, b: i128) -> i128 {
        a - b
    }
}
