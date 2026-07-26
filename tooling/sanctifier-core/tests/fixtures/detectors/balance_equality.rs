#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

// FIXTURE: balance_equality detector
// Spends gated on exact balance/amount (in)equality instead of `>=`/`<=`.

#[contract]
pub struct BalanceEqualityContract;

#[contractimpl]
impl BalanceEqualityContract {
    // Violation: gates the withdrawal on `balance == amount`.
    pub fn withdraw(env: Env, from: Address, amount: i128) {
        let balance = get_balance(&env, from.clone());
        if balance == amount {
            do_withdraw(&env, from, amount);
        }
    }

    // Violation: `!=` form against a call that returns a balance.
    pub fn settle(env: Env, to: Address, payment: i128) {
        if get_balance(&env, to.clone()) != payment {
            panic!("mismatch");
        }
    }

    // OK: proper inequality gate — not flagged.
    pub fn safe_withdraw(env: Env, from: Address, amount: i128) {
        let balance = get_balance(&env, from.clone());
        if balance >= amount {
            do_withdraw(&env, from, amount);
        }
    }

    // OK: emptiness check against a literal — not flagged.
    pub fn guard(_env: Env, amount: i128) {
        if amount == 0 {
            panic!("zero");
        }
    }
}

fn get_balance(_env: &Env, _who: Address) -> i128 {
    0
}

fn do_withdraw(_env: &Env, _who: Address, _amount: i128) {}
