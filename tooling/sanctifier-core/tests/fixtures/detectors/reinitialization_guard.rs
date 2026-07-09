use soroban_sdk::{contractimpl, Address, Env};

pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn initialize(env: Env, admin: Address) {
        env.storage().instance().set(&"admin", &admin);
        env.storage().instance().set(&"initialized", &true);
    }

    pub fn init_config(env: Env, owner: Address, treasury: Address) {
        env.storage().instance().set(&"owner", &owner);
        env.storage().instance().set(&"treasury", &treasury);
    }

    pub fn initialize_guarded(env: Env, admin: Address) {
        if env.storage().instance().has(&"admin") {
            panic!("already initialized");
        }
        env.storage().instance().set(&"admin", &admin);
    }

    // sanctifier:ignore[SANCT_REINITIALIZATION_GUARD]
    pub fn init_for_test(env: Env, owner: Address) {
        env.storage().instance().set(&"owner", &owner);
    }
}
