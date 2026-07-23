#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Symbol};

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    // VULN: getter aborts the read if the price was never set.
    pub fn get_price(env: Env, asset: Symbol) -> i128 {
        env.storage().persistent().get(&asset).unwrap()
    }

    // VULN: raw indexing panics on an out-of-bounds access.
    pub fn get_holder(holders: [u64; 4], idx: usize) -> u64 {
        holders[idx]
    }

    // OK: mutating entrypoint, not treated as a view function.
    pub fn set_price(env: Env, asset: Symbol, price: i128) {
        env.storage().persistent().set(&asset, &price);
    }

    // OK: getter returns Option instead of panicking.
    pub fn get_owner(env: Env) -> Option<Symbol> {
        env.storage().instance().get(&Symbol::new(&env, "owner"))
    }
}
