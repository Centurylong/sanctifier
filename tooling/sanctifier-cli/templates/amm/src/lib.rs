#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct AMMContract;

#[contractimpl]
impl AMMContract {
    pub fn initialize(env: Env, token_a: Address, token_b: Address) {
        if env.storage().instance().has(&soroban_sdk::symbol_short!("token_a")) {
            panic!("already initialized");
        }
        env.storage().instance().set(&soroban_sdk::symbol_short!("token_a"), &token_a);
        env.storage().instance().set(&soroban_sdk::symbol_short!("token_b"), &token_b);
    }

    pub fn deposit(env: Env, from: Address, amount_a: i128, amount_b: i128) {
        from.require_auth();
        let current_a: i128 = env.storage().instance().get(&soroban_sdk::symbol_short!("reserve_a")).unwrap_or(0);
        let current_b: i128 = env.storage().instance().get(&soroban_sdk::symbol_short!("reserve_b")).unwrap_or(0);
        
        env.storage().instance().set(&soroban_sdk::symbol_short!("reserve_a"), &(current_a.checked_add(amount_a).expect("overflow")));
        env.storage().instance().set(&soroban_sdk::symbol_short!("reserve_b"), &(current_b.checked_add(amount_b).expect("overflow")));
    }
}

#[cfg(test)]
mod test;
