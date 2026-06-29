#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol};

// GALLERY: missing TTL bump (fixed)
// Extends the persistent entry's TTL after writing it.

#[contract]
pub struct MissingTtlFixed;

#[contractimpl]
impl MissingTtlFixed {
    // FIX: bump the entry's TTL so it survives long enough to be read again.
    pub fn store(env: Env, caller: Address, key: Symbol, value: i128) {
        caller.require_auth();
        env.storage().persistent().set(&key, &value);
        env.storage().persistent().extend_ttl(&key, 100, 1000);
    }
}
