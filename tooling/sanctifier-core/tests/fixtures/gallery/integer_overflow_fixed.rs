#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

// GALLERY: integer overflow (fixed)
// Uses checked arithmetic so overflow cannot wrap silently.

#[contract]
pub struct IntegerOverflowFixed;

#[contractimpl]
impl IntegerOverflowFixed {
    // FIX: checked_add returns None on overflow; fall back to the old balance.
    pub fn add_reward(env: Env, user: Address, amount: i128) {
        let balance: i128 = env.storage().persistent().get(&user).unwrap_or(0);
        let new_balance = balance.checked_add(amount).unwrap_or(balance);
        env.storage().persistent().set(&user, &new_balance);
        env.storage().persistent().extend_ttl(&user, 100, 1000);
    }
}
