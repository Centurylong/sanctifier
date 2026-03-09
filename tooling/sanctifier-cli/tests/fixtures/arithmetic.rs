#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct ArithmeticContract;

#[contractimpl]
impl ArithmeticContract {
    pub fn do_math(env: Env, a: u64, b: u64) -> u64 {
        let x = a + b;
        let y = x - 10;
        let z = y * 2;
        z / a
    }

    pub fn compound(env: Env, mut a: u64) {
        a += 10;
        a -= 5;
        a *= 2;
        a /= 3;
    }
}
