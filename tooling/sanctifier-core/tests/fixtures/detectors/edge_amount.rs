#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

// FIXTURE: edge_amount detector
// Token operations that skip amount > 0 and self-transfer (from != to) guards.

#[contract]
pub struct EdgeAmountContract;

#[contractimpl]
impl EdgeAmountContract {
    // Violation: no amount > 0 check and no from != to check.
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        let from_balance: i128 = env.storage().persistent().get(&from).unwrap();
        env.storage().persistent().set(&from, &(from_balance - amount));
        env.storage().persistent().set(&to, &amount);
    }

    // Violation: no amount > 0 check before minting.
    pub fn mint(env: Env, to: Address, amount: i128) {
        env.storage().persistent().set(&to, &amount);
    }

    // Violation: no amount > 0 check before burning.
    pub fn burn(env: Env, from: Address, amount: i128) {
        let balance: i128 = env.storage().persistent().get(&from).unwrap();
        env.storage().persistent().set(&from, &(balance - amount));
    }
}
