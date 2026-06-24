#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol};

// GALLERY: unchecked upgrade authorization (fixed)
// The privileged write is gated behind require_auth().

const ADMIN: Symbol = symbol_short!("ADMIN");

#[contract]
pub struct UpgradeAuthFixed;

#[contractimpl]
impl UpgradeAuthFixed {
    // FIX: require the current admin's authorization before reassigning it.
    pub fn set_upgrade_admin(env: Env, caller: Address, new_admin: Address) {
        caller.require_auth();
        env.storage().instance().set(&ADMIN, &new_admin);
        env.storage().instance().extend_ttl(100, 1000);
    }
}
