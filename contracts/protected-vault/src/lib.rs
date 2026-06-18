#![no_std]

use reentrancy_guard::ReentrancyGuard;
use soroban_sdk::{contract, contractimpl, symbol_short, Env, Symbol};

const BALANCE_KEY: Symbol = symbol_short!("BALANCE");

/// A simple vault contract that uses `reentrancy-guard` to protect its state-mutating functions.
#[contract]
pub struct ProtectedVault;

#[contractimpl]
impl ProtectedVault {
    /// Deposit `amount` into the vault.
    pub fn deposit(env: Env, amount: i128) {
        assert!(amount > 0, "amount must be positive");
        let guard = ReentrancyGuard::new(&env);
        guard.enter();
        let balance: i128 = env.storage().instance().get(&BALANCE_KEY).unwrap_or(0);
        env.storage()
            .instance()
            .set(&BALANCE_KEY, &(balance + amount));
        guard.exit();
    }

    /// Withdraw `amount` from the vault.
    pub fn withdraw(env: Env, amount: i128) {
        assert!(amount > 0, "amount must be positive");
        let guard = ReentrancyGuard::new(&env);
        guard.enter();
        let balance: i128 = env.storage().instance().get(&BALANCE_KEY).unwrap_or(0);
        assert!(balance >= amount, "insufficient balance");
        env.storage()
            .instance()
            .set(&BALANCE_KEY, &(balance - amount));
        guard.exit();
    }

    /// Return the current vault balance.
    pub fn balance(env: Env) -> i128 {
        env.storage().instance().get(&BALANCE_KEY).unwrap_or(0)
    }

    /// Simulates a reentrant withdrawal: holds the guard then tries to call `withdraw` again.
    /// In tests this demonstrates that the guard rejects the inner call with "reentrancy detected".
    pub fn reentrant_withdraw(env: Env, amount: i128) {
        let guard = ReentrancyGuard::new(&env);
        guard.enter();
        // The guard is already locked — this inner call should panic immediately.
        Self::withdraw(env.clone(), amount);
        guard.exit();
    }
}
