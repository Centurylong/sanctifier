#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol};

// GALLERY: missing TTL bump
// Mapped finding: S006 (unsafe_pattern) — planned detector.

#[contract]
pub struct MissingTtlVulnerable;

#[contractimpl]
impl MissingTtlVulnerable {
    // VULN: writes a persistent entry but never extends its TTL, so the entry
    // can expire and be archived, silently losing state.
    pub fn store(env: Env, caller: Address, key: Symbol, value: i128) {
        caller.require_auth();
        env.storage().persistent().set(&key, &value);
    }
}
