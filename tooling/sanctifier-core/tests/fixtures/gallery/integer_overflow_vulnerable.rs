#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

// GALLERY: integer overflow
// Mapped finding: S003 (arithmetic_overflow).

#[contract]
pub struct IntegerOverflowVulnerable;

#[contractimpl]
impl IntegerOverflowVulnerable {
    // VULN: unchecked addition can overflow i128 and wrap around.
    pub fn add_reward(env: Env, user: Address, amount: i128) {
        let balance: i128 = env.storage().persistent().get(&user).unwrap_or(0);
        let new_balance = balance + amount;
        env.storage().persistent().set(&user, &new_balance);
        env.storage().persistent().extend_ttl(&user, 100, 1000);
    }
}
