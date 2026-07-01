#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct TimelockContract;

#[contractimpl]
impl TimelockContract {
    pub fn initialize(env: Env, admin: Address, unlock_time: u64) {
        if env.storage().instance().has(&soroban_sdk::symbol_short!("admin")) {
            panic!("already initialized");
        }
        env.storage().instance().set(&soroban_sdk::symbol_short!("admin"), &admin);
        env.storage().instance().set(&soroban_sdk::symbol_short!("time"), &unlock_time);
    }

    pub fn execute(env: Env) {
        let admin: Address = env.storage().instance().get(&soroban_sdk::symbol_short!("admin")).unwrap();
        admin.require_auth();

        let unlock_time: u64 = env.storage().instance().get(&soroban_sdk::symbol_short!("time")).unwrap();
        let current_time = env.ledger().timestamp();
        
        if current_time < unlock_time {
            panic!("timelock not expired");
        }
    }
}

#[cfg(test)]
mod test;
