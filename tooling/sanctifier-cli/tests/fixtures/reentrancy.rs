#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Address};

#[contract]
pub struct ReentrancyContract;

#[contractimpl]
impl ReentrancyContract {
    pub fn risky_call(env: Env, client: TokenClient, to: Address, amount: i128) {
        // Mutation
        env.storage().instance().set(&"status", &1u32);
        
        // External call
        client.transfer(&env.current_contract_address(), &to, &amount);
    }
}

pub struct TokenClient;
impl TokenClient {
    pub fn transfer(&self, from: &Address, to: &Address, amount: &i128) {}
}
