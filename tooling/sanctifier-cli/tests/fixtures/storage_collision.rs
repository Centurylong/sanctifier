#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Symbol};

#[contract]
pub struct CollisionContract;

#[contractimpl]
impl CollisionContract {
    pub fn collide(env: Env) {
        let key = Symbol::new(&env, "admin");
        env.storage().instance().set(&key, &1);
        env.storage().persistent().set(&key, &2);
    }
}
