#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

// GALLERY: allowance race (approve TOCTOU)
// Mapped finding: S006 (unsafe_pattern) — planned detector.

#[contract]
pub struct AllowanceRaceVulnerable;

#[contractimpl]
impl AllowanceRaceVulnerable {
    // VULN: approve() blindly overwrites the allowance. A spender can front-run
    // the change and spend the old allowance plus the new one (ERC20-style race).
    pub fn approve(env: Env, owner: Address, spender: Address, amount: i128) {
        owner.require_auth();
        env.storage()
            .persistent()
            .set(&(owner.clone(), spender.clone()), &amount);
        env.storage()
            .persistent()
            .extend_ttl(&(owner.clone(), spender.clone()), 100, 1000);
    }
}
