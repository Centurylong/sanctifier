#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol};

// GALLERY: CEI / reentrancy (fixed)
// Checks-Effects-Interactions: state is updated before the external call.

const PAID: Symbol = symbol_short!("PAID");

#[contract]
pub struct ReentrancyFixed;

#[contractimpl]
impl ReentrancyFixed {
    // FIX: write the new balance (effects) before publishing/calling out
    // (interactions), so a re-entrant call sees the already-debited balance.
    pub fn withdraw(env: Env, user: Address, amount: i128) {
        user.require_auth();
        let balance: i128 = env.storage().persistent().get(&user).unwrap_or(0);
        let remaining = balance.saturating_sub(amount);
        env.storage().persistent().set(&user, &remaining);
        env.storage().persistent().extend_ttl(&user, 100, 1000);
        env.events().publish((PAID,), (user.clone(), amount));
    }
}
