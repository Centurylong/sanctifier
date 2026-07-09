use soroban_sdk::{contractimpl, Address, Env};

pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn set_balance(env: Env, user: Address, balance: i128) {
        env.storage().temporary().set(&("balance", user), &balance);
    }

    pub fn configure_admin(env: Env, admin: Address) {
        env.storage().temporary().set(&"admin", &admin);
    }

    pub fn cache_balance_preview(env: Env, user: Address, balance_preview: i128) {
        env.storage()
            .temporary()
            .set(&("cache_balance_preview", user), &balance_preview);
    }

    // sanctifier:ignore[SANCT_TEMPORARY_PERSISTENT_STORAGE]
    pub fn test_override_owner(env: Env, owner: Address) {
        env.storage().temporary().set(&"owner", &owner);
    }
}
