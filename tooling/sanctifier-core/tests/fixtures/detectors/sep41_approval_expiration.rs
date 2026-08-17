#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

// FIXTURE: sep41_approval_expiration detector
// SEP-41 requires approve to take an expiration_ledger and enforce it; a
// missing or unused parameter means the allowance never expires.

#[contract]
pub struct Sep41ApprovalExpirationContract;

#[contractimpl]
impl Sep41ApprovalExpirationContract {
    // Violation: no expiration_ledger parameter at all.
    pub fn approve(env: Env, from: Address, spender: Address, amount: i128) {
        from.require_auth();
        write_allowance(&env, &from, &spender, amount);
    }

    // Violation: takes expiration_ledger but never references it.
    pub fn approve_unused_expiry(
        env: Env,
        from: Address,
        spender: Address,
        amount: i128,
        expiration_ledger: u32,
    ) {
        from.require_auth();
        write_allowance(&env, &from, &spender, amount);
    }

    // Safe: expiration_ledger is used to extend the entry's TTL.
    pub fn approve_with_expiry(
        env: Env,
        from: Address,
        spender: Address,
        amount: i128,
        expiration_ledger: u32,
    ) {
        from.require_auth();
        write_allowance(&env, &from, &spender, amount);
        env.storage()
            .temporary()
            .extend_ttl(&allowance_key(&from, &spender), 0, expiration_ledger);
    }

    // Safe: unrelated function, not named approve.
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        move_balance(&env, &from, &to, amount);
    }
}
