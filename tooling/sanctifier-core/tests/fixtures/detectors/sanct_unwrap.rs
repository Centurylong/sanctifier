#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

// FIXTURE: sanct_unwrap detector
// Flags panic-prone unwrap/expect/default fallback calls inside #[contractimpl]
// entrypoints, but ignores test-only code and helper impls.

#[contract]
pub struct UnsafeUnwrapContract;

#[contractimpl]
impl UnsafeUnwrapContract {
    pub fn admin(env: Env) -> Address {
        env.storage().instance().get(&"admin").unwrap()
    }

    pub fn config(env: Env) -> Address {
        env.storage()
            .persistent()
            .get(&"config")
            .expect("config must exist")
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&("balance", id))
            .unwrap_or_default()
    }

    pub fn safe_balance(env: Env, id: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&("balance", id))
            .unwrap_or(0)
    }
}

impl UnsafeUnwrapContract {
    pub fn helper(env: Env) -> Address {
        env.storage().instance().get(&"helper").unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[contractimpl]
    impl UnsafeUnwrapContract {
        pub fn test_only(env: Env) -> Address {
            env.storage().instance().get(&"test").unwrap()
        }
    }
}
