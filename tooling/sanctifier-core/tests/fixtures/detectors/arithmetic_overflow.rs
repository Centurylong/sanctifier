#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

// FIXTURE: arithmetic_overflow detector
// Unchecked +, -, and *= operations that could overflow/underflow.

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
}
