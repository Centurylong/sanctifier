#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

// FIXTURE: sep41_allowance_decrement detector
// SEP-41 requires transfer_from to reduce the spender's allowance by the
// transferred amount; reading it without writing back lets the same
// approval be spent repeatedly.

#[contract]
pub struct Sep41AllowanceDecrementContract;

#[contractimpl]
impl Sep41AllowanceDecrementContract {
    // Violation: reads the allowance to bound the transfer, never persists
    // the decremented value.
    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();
        let allowance = read_allowance(&env, &from, &spender);
        if allowance < amount {
            panic!("insufficient allowance");
        }
        move_balance(&env, &from, &to, amount);
    }

    // Violation: camelCase spelling of the entrypoint, same bug.
    pub fn transferFrom(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();
        let allowance = read_allowance(&env, &from, &spender);
        if allowance < amount {
            panic!("insufficient allowance");
        }
        move_balance(&env, &from, &to, amount);
    }

    // Safe: writes the decremented allowance back before returning.
    pub fn transfer_from_safe(
        env: Env,
        spender: Address,
        from: Address,
        to: Address,
        amount: i128,
    ) {
        spender.require_auth();
        let allowance = read_allowance(&env, &from, &spender);
        if allowance < amount {
            panic!("insufficient allowance");
        }
        write_allowance(&env, &from, &spender, allowance - amount);
        move_balance(&env, &from, &to, amount);
    }

    // Safe: decrements via a differently-named but still allowance-hinting
    // helper.
    pub fn transfer_from_via_spend(
        env: Env,
        spender: Address,
        from: Address,
        to: Address,
        amount: i128,
    ) {
        spender.require_auth();
        let allowance = read_allowance(&env, &from, &spender);
        if allowance < amount {
            panic!("insufficient allowance");
        }
        spend_allowance(&env, &from, &spender, amount);
        move_balance(&env, &from, &to, amount);
    }

    // Safe: no allowance read at all (e.g. an owner-only internal mover) —
    // out of scope for this detector.
    pub fn transfer_from_admin(env: Env, from: Address, to: Address, amount: i128) {
        move_balance(&env, &from, &to, amount);
    }

    // Safe: unrelated function, not named transfer_from.
    pub fn balance(env: Env, id: Address) -> i128 {
        read_balance(&env, &id)
    }
}
