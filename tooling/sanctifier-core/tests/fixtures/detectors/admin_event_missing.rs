#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol};

// FIXTURE: admin_event_missing detector
//
// Admin/config-change functions must emit an on-chain event when they mutate
// storage. This fixture mixes violating functions with clean cases so the
// golden snapshot pins both true positives and intended exclusions.

#[contract]
pub struct AdminContract;

#[contractimpl]
impl AdminContract {
    // Violation 1: set_admin writes instance storage, no event.
    pub fn set_admin(env: Env, new_admin: Address) {
        env.storage()
            .instance()
            .set(&Symbol::short("admin"), &new_admin);
    }

    // Violation 2: pause writes persistent storage, no event.
    pub fn pause(env: Env) {
        env.storage()
            .persistent()
            .set(&Symbol::short("paused"), &true);
    }

    // Violation 3: upgrade removes instance storage, no event.
    pub fn upgrade(env: Env) {
        env.storage().instance().remove(&Symbol::short("wasm_hash"));
    }

    // Clean 1: update_config writes storage AND emits event via env.events().publish.
    pub fn update_config(env: Env, value: u32) {
        env.storage().instance().set(&Symbol::short("cfg"), &value);
        env.events()
            .publish((symbol_short!("admin"), symbol_short!("cfg_upd")), value);
    }

    // Clean 2: set_owner writes storage AND emits event via publish.
    pub fn set_owner(env: Env, owner: Address) {
        env.storage()
            .persistent()
            .set(&Symbol::short("owner"), &owner);
        env.events().publish(
            (symbol_short!("admin"), symbol_short!("owner_set")),
            owner.clone(),
        );
    }

    // Clean 3: configure only reads storage — no storage write, no violation.
    pub fn configure(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&Symbol::short("cfg"))
            .unwrap_or(0)
    }

    // Suppressed: migrate has a write but is opted out.
    // sanctifier:ignore[SANCT_ADMIN_EVENT_MISSING]
    pub fn migrate(env: Env) {
        env.storage().instance().set(&Symbol::short("v"), &2u32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // This admin-named function writes storage inside #[cfg(test)] — must NOT be flagged.
    fn set_admin_test_helper(env: &Env) {
        env.storage().instance().set(&Symbol::short("admin"), &true);
    }
}
