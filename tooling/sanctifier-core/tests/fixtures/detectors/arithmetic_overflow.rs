#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

// FIXTURE: arithmetic_overflow detector
// Unchecked +, -, and *= operations that could overflow/underflow.
// Unsafe integer `as` casts that narrow width or change signedness.

#[contract]
pub struct ArithmeticContract;

#[contractimpl]
impl ArithmeticContract {
    pub fn deposit(_env: Env, balance: u64, amount: u64) -> u64 {
        balance + amount
    }

    pub fn withdraw(_env: Env, balance: u64, amount: u64) -> u64 {
        balance - amount
    }

    pub fn accrue(_env: Env, mut total: u128, rate: u128) -> u128 {
        total *= rate;
        total
    }

    pub fn truncate_amount(_env: Env, amount: i128) -> u32 {
        amount as u32
    }

    pub fn wrap_signed(_env: Env, amount: i64) -> u64 {
        amount as u64
    }

    pub fn widen_safely(_env: Env, amount: u32) -> u64 {
        amount as u64
    }

    pub fn checked_conversion(_env: Env, amount: i128) -> Result<u32, ()> {
        amount.try_into().map_err(|_| ())
    }
}
