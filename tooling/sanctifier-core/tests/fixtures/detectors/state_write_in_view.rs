#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol};

// FIXTURE: state_write_in_view detector
//
// Getter/view-named functions are expected to be read-only. This fixture mixes
// violating getters with several patterns that must NOT be flagged so the
// golden snapshot pins both the true positives and the intended-mutation
// exclusions.

#[contract]
pub struct ViewWriteContract;

#[contractimpl]
impl ViewWriteContract {
    // Violation: a getter that writes to persistent storage.
    pub fn get_balance(env: Env, who: Address) -> i128 {
        env.storage().persistent().set(&who, &0i128);
        0
    }

    // Violation: `_of` suffix getter that removes storage.
    pub fn allowance_of(env: Env, owner: Address) -> i128 {
        env.storage().persistent().remove(&owner);
        0
    }

    // Clean: a getter that only extends TTL on the read path (intended mutation).
    pub fn get_config(env: Env) -> u32 {
        let value: u32 = env.storage().instance().get(&Symbol::short("cfg")).unwrap();
        env.storage().instance().extend_ttl(100, 100);
        value
    }

    // Clean: explicit opt-out for an intentional lazy-initialization write.
    pub fn get_or_init_nonce(env: Env) -> u32 {
        // sanctifier:ignore[SANCT_STATE_WRITE_IN_VIEW]
        env.storage().instance().set(&Symbol::short("nonce"), &1u32);
        1
    }

    // Clean: a genuinely read-only getter.
    pub fn get_owner(env: Env) -> Address {
        env.storage().instance().get(&Symbol::short("owner")).unwrap()
    }

    // Clean: a mutating function whose name does not imply a getter.
    pub fn set_owner(env: Env, owner: Address) {
        env.storage().instance().set(&Symbol::short("owner"), &owner);
    }
}
