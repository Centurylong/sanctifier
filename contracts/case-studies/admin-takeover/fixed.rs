//! Case study repro — FIXED version.
//!
//! The same token, now with `admin.require_auth()` gating every privileged
//! entrypoint. `set_admin` and `mint` load the stored admin and require its
//! authorization before mutating state, closing the admin-takeover /
//! unauthenticated-mint hole. Sanctifier no longer reports S001 (auth_gap) on
//! `set_admin` or `mint`.
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

    /// FIXED: only the current admin (after authorization) can reassign admin.
    pub fn set_admin(env: Env, new_admin: Address) {
        if let Some(admin) = env.storage().instance().get::<_, Address>(&DataKey::Admin) {
            admin.require_auth();
            env.storage().instance().set(&DataKey::Admin, &new_admin);
        }
    }

    /// FIXED: only the admin (after authorization) can mint.
    pub fn mint(env: Env, to: Address, amount: i128) {
        if let Some(admin) = env.storage().instance().get::<_, Address>(&DataKey::Admin) {
            admin.require_auth();
            let current: i128 = env
                .storage()
                .persistent()
                .get(&DataKey::Balance(to.clone()))
                .unwrap_or(0);
            env.storage()
                .persistent()
                .set(&DataKey::Balance(to), &(current + amount));
        }
    }

    pub fn balance(env: Env, who: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(who))
            .unwrap_or(0)
    }
}
