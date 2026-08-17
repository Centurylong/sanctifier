#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

// GALLERY: allowance race (fixed)
// Compare-and-set: the update only applies if the current value is as expected.

#[contract]
pub struct AllowanceRaceFixed;

#[contractimpl]
impl AllowanceRaceFixed {
    // FIX: require the caller to pass the allowance they think is current, and
    // only overwrite when it still matches — closing the front-running window.
    // Also takes an expiration_ledger (SEP-41) and extends the entry's TTL to
    // it, so the approval lapses instead of living forever.
    pub fn approve(
        env: Env,
        owner: Address,
        spender: Address,
        expected_current: i128,
        amount: i128,
        expiration_ledger: u32,
    ) {
        owner.require_auth();
        let stored: i128 = env
            .storage()
            .persistent()
            .get(&(owner.clone(), spender.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .extend_ttl(&(owner.clone(), spender.clone()), 100, 1000);
        if stored == expected_current {
            env.storage()
                .persistent()
                .set(&(owner.clone(), spender.clone()), &amount);
            env.storage().persistent().extend_ttl(
                &(owner.clone(), spender.clone()),
                100,
                expiration_ledger,
            );
        }
    }
}
