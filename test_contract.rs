use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct TestContract;

#[contractimpl]
impl TestContract {
    pub fn vulnerable_function(env: Env, amount: u64) -> u64 {
        // This will be flagged as arithmetic overflow
        amount + 1000000000000000000u64
    }
    
    pub fn missing_auth_function(env: Env, user: Address, amount: u64) {
        // This will be flagged as missing authentication
        env.storage().set(&user, &amount);
    }
}