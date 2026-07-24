#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

// Fixture for the allowance_race (approve TOCTOU) detector.
//
// Exactly one function below is vulnerable: `approve` overwrites the stored
// allowance unconditionally from a caller-supplied amount. The others are the
// canonical safe shapes (delta semantics, compare-and-set) and an unrelated
// storage write, none of which should be flagged.

#[contract]
pub struct AllowanceRaceFixture;

#[contractimpl]
impl AllowanceRaceFixture {
    // VULN: blind overwrite of the allowance — the approve front-running race.
    pub fn approve(env: Env, owner: Address, spender: Address, amount: i128) {
        owner.require_auth();
        env.storage()
            .persistent()
            .set(&(owner.clone(), spender.clone()), &amount);
    }

    // SAFE: delta semantics — the written value is computed from the current
    // allowance, and the function name signals an increase.
    pub fn increase_allowance(env: Env, owner: Address, spender: Address, delta: i128) {
        owner.require_auth();
        let current: i128 = env
            .storage()
            .persistent()
            .get(&(owner.clone(), spender.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&(owner.clone(), spender.clone()), &(current + delta));
    }

    // SAFE: compare-and-set — the caller passes the allowance they expect to be
    // current, and the write only lands when it still matches.
    pub fn approve_checked(
        env: Env,
        owner: Address,
        spender: Address,
        expected_current: i128,
        amount: i128,
    ) {
        owner.require_auth();
        let stored: i128 = env
            .storage()
            .persistent()
            .get(&(owner.clone(), spender.clone()))
            .unwrap_or(0);
        if stored == expected_current {
            env.storage()
                .persistent()
                .set(&(owner.clone(), spender.clone()), &amount);
        }
    }

    // SAFE: unrelated storage write (a balance), not an allowance.
    pub fn set_balance(env: Env, who: Address, amount: i128) {
        env.storage().persistent().set(&who, &amount);
    }
}
