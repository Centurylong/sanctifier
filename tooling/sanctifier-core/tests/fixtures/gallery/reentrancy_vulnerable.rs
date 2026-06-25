#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol};

// GALLERY: CEI / reentrancy
// Mapped finding: S006 (unsafe_pattern) — planned detector.

const PAID: Symbol = symbol_short!("PAID");

#[contract]
pub struct ReentrancyVulnerable;

#[contractimpl]
impl ReentrancyVulnerable {
    // VULN: interaction (the external/event call) happens BEFORE the balance is
    // updated, so a re-entrant call observes the stale balance and double-spends.
    pub fn withdraw(env: Env, user: Address, amount: i128) {
        user.require_auth();
        let balance: i128 = env.storage().persistent().get(&user).unwrap_or(0);
        env.events().publish((PAID,), (user.clone(), amount));
        let remaining = balance.saturating_sub(amount);
        env.storage().persistent().set(&user, &remaining);
        env.storage().persistent().extend_ttl(&user, 100, 1000);
    }
}
