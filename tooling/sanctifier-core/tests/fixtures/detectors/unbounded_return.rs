use soroban_sdk::{contractimpl, Env, Address, Vec, Map};

pub struct Contract;

#[contractimpl]
impl Contract {
    // Vulnerable: returns an unbounded Vec
    pub fn get_all_users(env: Env) -> Vec<Address> {
        Vec::new(&env)
    }

    // Vulnerable: returns an unbounded Map
    pub fn get_all_balances(env: Env) -> Map<Address, i128> {
        Map::new(&env)
    }

    // Safe: paginated return (uses `start` and `limit`)
    pub fn get_users_paginated(env: Env, start: u32, limit: u32) -> Vec<Address> {
        Vec::new(&env)
    }

    // Safe: internal function, not an entrypoint
    fn fetch_users_internal(env: Env) -> Vec<Address> {
        Vec::new(&env)
    }

    // Safe: returns a scalar
    pub fn get_user_count(env: Env) -> u32 {
        0
    }
}
