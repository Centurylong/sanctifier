//! Fixture for the `nonce_monotonicity` detector.
//!
//! One handler accepts any nonce greater than the stored value (skipping gaps);
//! the other requires exactly the next nonce.

use soroban_sdk::{contractimpl, Env};

pub struct VulnerableContract;

#[contractimpl]
impl VulnerableContract {
    pub fn execute(env: Env, provided_nonce: u64) {
        let stored_nonce: u64 = env.storage().instance().get(&"nonce").unwrap_or(0);

        // FLAGGED: this rejects reuse but accepts arbitrary gaps.
        if provided_nonce <= stored_nonce {
            panic!("nonce already used");
        }

        env.storage().instance().set(&"nonce", &provided_nonce);
    }
}

pub struct StrictContract;

#[contractimpl]
impl StrictContract {
    pub fn execute(env: Env, provided_nonce: u64) {
        let stored_nonce: u64 = env.storage().instance().get(&"nonce").unwrap_or(0);

        // NOT FLAGGED: only the immediately following nonce is accepted.
        if provided_nonce != stored_nonce + 1 {
            panic!("nonce must increase by exactly one");
        }

        env.storage().instance().set(&"nonce", &provided_nonce);
    }
}
