#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct UnusedContract;

#[contractimpl]
impl UnusedContract {
    pub fn ignore_me(env: Env, x: u32, y: u32) -> u32 {
        let z = 10;
        x
    }
}
