#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

// FIXTURE: panic_detection detector
// Uses panic!, unwrap(), and expect() inside contract functions.

#[contract]
pub struct PanicContract;

#[contractimpl]
impl PanicContract {
    pub fn must_have_admin(env: Env) -> Address {
        env.storage().instance().get(&"admin").unwrap()
    }

    pub fn must_init(env: Env) -> Address {
        env.storage().instance().get(&"init").expect("not initialized")
    }

    pub fn always_fail(_env: Env) {
        panic!("unreachable");
    }
}
