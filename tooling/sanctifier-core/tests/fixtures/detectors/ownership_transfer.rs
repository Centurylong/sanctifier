use soroban_sdk::{contractimpl, Address, Env};

pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn transfer_ownership(env: Env, new_owner: Address) {
        env.storage().instance().set(&"owner", &new_owner);
    }

    pub fn update_admin(env: Env, new_admin: Address) {
        env.storage().instance().set(&"admin", &new_admin);
    }

    pub fn propose_owner(env: Env, new_owner: Address) {
        env.storage().instance().set(&"pending_owner", &new_owner);
    }

    pub fn accept_ownership(env: Env) {
        let pending_owner: Address = env.storage().instance().get(&"pending_owner").unwrap();
        env.storage().instance().set(&"owner", &pending_owner);
    }

    // sanctifier:ignore[SANCT_OWNERSHIP_TRANSFER]
    pub fn set_admin(env: Env, new_admin: Address) {
        env.storage().instance().set(&"admin", &new_admin);
    }
}
