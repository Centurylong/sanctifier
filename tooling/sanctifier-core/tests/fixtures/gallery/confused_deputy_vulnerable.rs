#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

// GALLERY: confused-deputy authorization
// Mapped finding: S001 (auth_gap) family — planned refinement; the wrong
// principal is authorized, which a presence-only auth check cannot catch yet.

#[contract]
pub struct ConfusedDeputyVulnerable;

#[contractimpl]
impl ConfusedDeputyVulnerable {
    // VULN: authorizes the *caller* but moves the *owner's* funds. The auth
    // check is present but on the wrong principal, so anyone can drain `owner`.
    pub fn claim_for(env: Env, caller: Address, owner: Address, to: Address, amount: i128) {
        caller.require_auth();
        let bal: i128 = env.storage().persistent().get(&owner).unwrap_or(0);
        env.storage().persistent().set(&owner, &bal.saturating_sub(amount));
        env.storage().persistent().extend_ttl(&owner, 100, 1000);
        let to_bal: i128 = env.storage().persistent().get(&to).unwrap_or(0);
        env.storage().persistent().set(&to, &to_bal.saturating_add(amount));
        env.storage().persistent().extend_ttl(&to, 100, 1000);
    }
}
