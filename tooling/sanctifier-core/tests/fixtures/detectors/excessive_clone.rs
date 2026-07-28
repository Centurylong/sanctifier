#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

// FIXTURE: excessive_clone detector
// Cloning the Soroban `Env` handle is gas-wasting copy-paste; passing `&env`
// is the idiomatic form. The detector flags `env.clone()` (and `self.env.clone()`)
// and leaves ordinary value clones (e.g. Address) alone.

#[contract]
pub struct Registry;

#[contractimpl]
impl Registry {
    // Violation: the Env handle is cloned to pass into a helper.
    pub fn record(env: Env, who: Address) {
        helper(env.clone(), who);
    }

    // Safe: cloning a domain value (Address) is out of scope.
    pub fn keep(env: Env, who: Address) -> Address {
        who.clone()
    }
}

fn helper(_env: Env, _who: Address) {}
