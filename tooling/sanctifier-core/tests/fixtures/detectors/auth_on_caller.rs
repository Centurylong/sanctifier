//! Fixture for the `auth_on_caller` detector.
//!
//! Confused-deputy shape: the caller signs, but somebody else's state moves.

use soroban_sdk::{contractimpl, Address, Env};

pub struct Vault;

#[contractimpl]
impl Vault {
    /// FLAGGED: `caller` authorizes, but the balance written is keyed by
    /// `owner`. The caller has consented to a change to another account.
    pub fn withdraw(env: Env, caller: Address, owner: Address, amount: i128) {
        caller.require_auth();
        let balance = Self::balance_of(&env, &owner) - amount;
        env.storage().persistent().set(&owner, &balance);
    }

    /// FLAGGED: the same mistake with neutral parameter names, to show the
    /// rule is structural and not keyed off the word "caller".
    pub fn sweep(env: Env, a: Address, b: Address) {
        a.require_auth();
        env.storage().instance().set(&b, &0i128);
    }

    /// NOT FLAGGED: the owner whose state changes is the one that authorizes.
    pub fn withdraw_correct(env: Env, caller: Address, owner: Address, amount: i128) {
        owner.require_auth();
        let balance = Self::balance_of(&env, &owner) - amount;
        env.storage().persistent().set(&owner, &balance);
        let _ = caller;
    }

    /// NOT FLAGGED: both parties authorize, so neither is a confused deputy.
    pub fn transfer_both_signed(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        to.require_auth();
        env.storage().persistent().set(&from, &amount);
    }

    /// NOT FLAGGED: `require_auth_for_args` counts as authorization too.
    pub fn withdraw_bound(env: Env, caller: Address, owner: Address, amount: i128) {
        owner.require_auth_for_args((amount,).into_val(&env));
        env.storage().persistent().set(&owner, &amount);
        let _ = caller;
    }

    /// NOT FLAGGED: a single address parameter has no other owner to confuse
    /// it with, and an entrypoint with no auth at all is `auth_gap`'s finding.
    pub fn deposit(env: Env, owner: Address, amount: i128) {
        env.storage().persistent().set(&owner, &amount);
    }

    fn balance_of(_env: &Env, _who: &Address) -> i128 {
        0
    }
}
