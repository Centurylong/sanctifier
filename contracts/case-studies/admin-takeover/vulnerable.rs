//! Case study repro — VULNERABLE version.
//!
//! A minimal SEP-41-style token whose privileged entrypoints (`set_admin`,
//! `mint`) mutate state without calling `admin.require_auth()`. Any account can
//! therefore reassign the admin or mint tokens to itself — the "admin takeover"
//! / unauthenticated-mint class that has repeatedly appeared in Soroban audits.
//!
//! Sanctifier flags the missing authorization with `S001` (auth_gap).
#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contracttype]
pub enum DataKey {
    Admin,
    Balance(Address),
}

#[contract]
pub struct Token;

#[contractimpl]
impl Token {
    pub fn initialize(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// VULNERABLE: no `admin.require_auth()` — anyone can seize control.
    pub fn set_admin(env: Env, new_admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &new_admin);
    }

    /// VULNERABLE: no authorization — anyone can mint to any account.
    pub fn mint(env: Env, to: Address, amount: i128) {
        let current: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(to.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to), &(current + amount));
    }

    pub fn balance(env: Env, who: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(who))
            .unwrap_or(0)
    }
}
