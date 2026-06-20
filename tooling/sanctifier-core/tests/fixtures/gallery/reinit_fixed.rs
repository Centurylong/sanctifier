#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol};

// GALLERY: re-initialization (fixed)
// Reads existing state before writing, so initialize() is idempotent.

const ADMIN: Symbol = symbol_short!("ADMIN");

#[contract]
pub struct ReinitFixed;

#[contractimpl]
impl ReinitFixed {
    // FIX: read first and only set the admin when it is still unset.
    pub fn initialize(env: Env, admin: Address) {
        let existing: Option<Address> = env.storage().instance().get(&ADMIN);
        if existing.is_none() {
            env.storage().instance().set(&ADMIN, &admin);
        }
    }
}
