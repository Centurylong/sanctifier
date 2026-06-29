#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

// FIXTURE: unused_variable detector
// A local binding is declared but never read.

#[contract]
pub struct UnusedVariableContract;

#[contractimpl]
impl UnusedVariableContract {
    pub fn compute(_env: Env, input: u64) -> u64 {
        let scratch = input * 2;
        let result = input + 1;
        result
    }
}
