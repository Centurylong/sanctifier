use soroban_sdk::{Address, Env};

struct Contract;

impl Contract {
    pub fn get_balance(env: Env, user: Address) -> i128 {
        env.storage().persistent().set(&user, &0);
        0
    }

    pub fn preview_rewards(env: Env, user: Address) -> i128 {
        env.storage().temporary().remove(&user);
        0
    }

    pub fn quote_status(env: Env) -> bool {
        env.storage().instance().extend_ttl(100, 1000);
        true
    }

    pub fn set_balance(env: Env, user: Address, value: i128) {
        env.storage().persistent().set(&user, &value);
    }

    pub fn get_existing_balance(env: Env, user: Address) -> i128 {
        env.storage().persistent().get(&user).unwrap_or(0)
    }
}
