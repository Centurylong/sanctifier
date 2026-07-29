#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct TokenContract;

#[contractimpl]
impl TokenContract {
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        // Safe: require_auth in a public contract method binds to all its arguments.
        from.require_auth();
        Self::internal_transfer(env, from, to, amount);
    }
}

impl TokenContract {
    fn internal_transfer(env: Env, from: Address, to: Address, amount: i128) {
        // Unsafe: require_auth in an internal helper binds to the original caller's args, leaving `to` and `amount` unbound in this scope.
        from.require_auth();
    }

    fn safe_internal_transfer(env: Env, from: Address, to: Address, amount: i128) {
        // Safe: explicitly binding the helper's arguments.
        from.require_auth_for_args((to, amount).into_val(&env));
    }
}

// Unsafe: free-floating internal function using require_auth
fn helper(env: Env, from: Address) {
    from.require_auth();
}
