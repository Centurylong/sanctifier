#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

// GALLERY: confused-deputy authorization (fixed)
// Authorizes the resource owner — the principal whose funds are moved.

#[contract]
pub struct ConfusedDeputyFixed;

#[contractimpl]
impl ConfusedDeputyFixed {
    // FIX: require_auth() on `owner`, the account whose balance is debited.
    pub fn claim_for(env: Env, owner: Address, to: Address, amount: i128) {
        owner.require_auth();
        let bal: i128 = env.storage().persistent().get(&owner).unwrap_or(0);
        env.storage().persistent().set(&owner, &bal.saturating_sub(amount));
        let to_bal: i128 = env.storage().persistent().get(&to).unwrap_or(0);
        env.storage().persistent().set(&to, &to_bal.saturating_add(amount));
    }
}
