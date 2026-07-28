#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol};

// FIXTURE: SANCT_VISIBILITY detector
// Only the exposed, unauthenticated helper should be reported.

#[contract]
pub struct VisibilityContract;

#[contractimpl]
impl VisibilityContract {
    // Violation: helper-shaped public entrypoint delegates a state write.
    pub fn _set_balance(env: Env, who: Address, amount: i128) {
        write_balance(&env, &who, amount);
    }

    // Clean: the same helper shape is protected by authorization.
    pub fn _set_balance_authorized(env: Env, who: Address, amount: i128) {
        who.require_auth();
        write_balance(&env, &who, amount);
    }

    // Violation: authorization is conditional, so an unauthenticated path remains.
    pub fn _set_balance_conditionally(
        env: Env,
        who: Address,
        amount: i128,
        should_auth: bool,
    ) {
        if should_auth {
            who.require_auth();
        }
        write_balance(&env, &who, amount);
    }

    // Violation: aliased storage and try_update are still state mutation.
    pub fn helper_increment_balance(env: Env, who: Address, amount: i128) {
        let balances = env.storage().persistent();
        balances.try_update(&who, |balance: Option<i128>| -> Result<i128, ()> {
            Ok(balance.unwrap_or_default() + amount)
        });
    }

    // Clean: authentication performed by the local callee protects its write.
    pub fn _set_balance_via_guarded_helper(env: Env, who: Address, amount: i128) {
        authenticated_write_balance(&env, &who, amount);
    }

    // Violation: returning from a callee does not make the caller unreachable.
    pub fn _set_balance_after_validation(env: Env, who: Address, amount: i128) {
        validate();
        write_balance(&env, &who, amount);
    }

    // Violation: storage-handle arguments remain storage handles in the callee.
    pub fn helper_set_via_storage_alias(env: Env, who: Address, amount: i128) {
        let balances = env.storage().persistent();
        write_storage(&balances, &who, amount);
    }

    // Violation: qualified out-of-line storage helpers are conservative writes.
    pub fn helper_set_via_external_storage(env: Env, who: Address, amount: i128) {
        storage::write_external_balance(&env, &who, amount);
    }

    // Clean: a helper-shaped call name alone is not evidence of state mutation.
    pub fn _clear_buffer(env: Env) {
        clear_buffer(&env);
    }

    // Clean: an inner storage alias must not taint an earlier, shadowed local.
    pub fn helper_update_local_values(env: Env, values: LocalValues) {
        values.set(0, 1);
        if false {
            let values = env.storage().persistent();
            let _ = values;
        }
    }

    // Clean: a mandatory loop authenticates before its break exit.
    pub fn helper_set_after_loop_auth(env: Env, who: Address, amount: i128) {
        loop {
            who.require_auth();
            break;
        }
        write_balance(&env, &who, amount);
    }

    // Clean: an early caller return is not a callee return path.
    pub fn helper_set_after_early_return(
        env: Env,
        who: Address,
        amount: i128,
        done: bool,
    ) {
        if done {
            return;
        }
        authenticate(&who);
        write_balance(&env, &who, amount);
    }

    // Violation: an outer break remains reachable across a nested loop.
    pub fn helper_set_after_nested_loop(
        env: Env,
        who: Address,
        amount: i128,
        done: bool,
    ) {
        loop {
            if done {
                break;
            }
            loop {
                break;
            }
        }
        write_balance(&env, &who, amount);
    }

    // Clean: a normal public mutator is not an accidentally exposed helper.
    pub fn set_balance(env: Env, who: Address, amount: i128) {
        write_balance(&env, &who, amount);
    }

    // Clean: helper-shaped but read-only.
    pub fn _balance(env: Env, who: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&who)
            .unwrap_or_default()
    }

    // Clean: private helpers are not contract entrypoints.
    fn _clear_balance(env: Env, who: Address) {
        env.storage().persistent().remove(&who);
    }
}

impl VisibilityContract {
    // Clean: this impl is not exported through #[contractimpl].
    pub fn _set_config(env: Env, value: i128) {
        env.storage()
            .instance()
            .set(&Symbol::short("config"), &value);
    }
}

fn write_balance(env: &Env, who: &Address, amount: i128) {
    env.storage().persistent().set(who, &amount);
}

fn authenticated_write_balance(env: &Env, who: &Address, amount: i128) {
    who.require_auth();
    write_balance(env, who, amount);
}

fn validate() {
    return;
}

fn authenticate(who: &Address) {
    who.require_auth();
}

fn write_storage(storage: &PersistentStorage, who: &Address, amount: i128) {
    storage.set(who, &amount);
}

fn clear_buffer(_env: &Env) {}

pub struct LocalValues;

impl LocalValues {
    pub fn set(&self, _key: i128, _value: i128) {}
}

mod nested {
    use soroban_sdk::{contract, contractimpl, Env, Symbol};

    #[contract]
    pub struct NestedVisibilityContract;

    #[contractimpl]
    impl NestedVisibilityContract {
        // Violation: production inline modules are part of the contract surface.
        pub fn internal_set_flag(env: Env, enabled: bool) {
            env.storage()
                .instance()
                .set(&Symbol::short("enabled"), &enabled);
        }
    }
}
