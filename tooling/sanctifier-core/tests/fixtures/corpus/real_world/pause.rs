// Vendored from stellar/soroban-examples (Apache-2.0), path: pause/src/lib.rs
// Source: https://github.com/stellar/soroban-examples/blob/f0d194fb9cda924b981e48cdf2ce9e74aad107a2/pause/src/lib.rs
// Retrieved for issue #764 (real-world corpus expansion); unmodified except this header.

#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Env, Symbol};

const PAUSED: Symbol = symbol_short!("PAUSED");

#[contract]
pub struct Pause;

#[contractimpl]
impl Pause {
    pub fn paused(env: Env) -> bool {
        env.storage().instance().get(&PAUSED).unwrap_or_default()
    }

    pub fn set(env: Env, paused: bool) {
        env.storage().instance().set(&PAUSED, &paused);
    }
}

mod test;
