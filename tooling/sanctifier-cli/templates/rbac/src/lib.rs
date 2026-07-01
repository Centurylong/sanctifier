#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct RBACContract;

#[contractimpl]
impl RBACContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&soroban_sdk::symbol_short!("admin")) {
            panic!("already initialized");
        }
        env.storage().instance().set(&soroban_sdk::symbol_short!("admin"), &admin);
    }

    pub fn grant_role(env: Env, user: Address) {
        let admin: Address = env.storage().instance().get(&soroban_sdk::symbol_short!("admin")).unwrap();
        admin.require_auth();
        env.storage().persistent().set(&user, &true);
    }

    pub fn execute_restricted(env: Env, user: Address) {
        user.require_auth();
        let has_role: bool = env.storage().persistent().get(&user).unwrap_or(false);
        if !has_role {
            panic!("unauthorized");
        }
    }
}

#[cfg(test)]
mod test;
