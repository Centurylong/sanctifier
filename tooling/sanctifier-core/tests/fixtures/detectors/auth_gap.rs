#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol};

// FIXTURE: auth_gap detector
// A public function mutates instance storage without calling require_auth().

#[contract]
pub struct AuthGapContract;

#[contractimpl]
impl AuthGapContract {
    // Violation: writes admin to storage with no authentication check.
    pub fn set_admin(env: Env, admin: Address) {
        env.storage().instance().set(&Symbol::short("admin"), &admin);
    }

    // Clean: reads first, then mutates after require_auth().
    pub fn rotate_admin(env: Env, new_admin: Address) {
        let current: Address = env.storage().instance().get(&Symbol::short("admin")).unwrap();
        current.require_auth();
        env.storage().instance().set(&Symbol::short("admin"), &new_admin);
    }
}
