#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

// FIXTURE: shift_overflow detector
// Bit shifts whose amount may be >= the operand's bit width.
//   * unbounded variable amounts (<< and >>=)  -> flagged (warning)
//   * constant amount >= bit width             -> flagged (error)
//   * constant amount within range             -> ignored
//   * masked amount (`& 63`)                   -> ignored
//   * amount guarded by a comparison           -> ignored

#[contract]
pub struct ShiftContract;

#[contractimpl]
impl ShiftContract {
    pub fn pack(_env: Env, value: u64, amount: u32) -> u64 {
        value << amount
    }

    pub fn unpack(_env: Env, mut value: u128, amount: u32) -> u128 {
        value >>= amount;
        value
    }

    pub fn constant_overflow(_env: Env, value: u32) -> u32 {
        value << 40
    }

    pub fn constant_in_range(_env: Env, value: u64) -> u64 {
        value << 3
    }

    pub fn masked(_env: Env, value: u64, amount: u32) -> u64 {
        value << (amount & 63)
    }

    pub fn guarded(_env: Env, value: u64, amount: u32) -> u64 {
        if amount < 64 {
            value << amount
        } else {
            0
        }
    }
}
