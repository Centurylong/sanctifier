#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol};

// GALLERY: re-initialization
// Mapped finding: S001 (auth_gap) — state mutation with no guard/auth.

const ADMIN: Symbol = symbol_short!("ADMIN");

#[contract]
pub struct ReinitVulnerable;

#[contractimpl]
impl ReinitVulnerable {
    // VULN: no initialization guard — anyone can call initialize() again and
    // overwrite the admin. Mutates storage with no prior read and no auth.
    pub fn initialize(env: Env, admin: Address) {
        env.storage().instance().set(&ADMIN, &admin);
    }
}
