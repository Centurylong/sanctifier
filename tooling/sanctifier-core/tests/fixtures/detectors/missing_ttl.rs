use soroban_sdk::{Address, Env, Symbol};

struct Contract;

impl Contract {
    pub fn write_without_ttl(env: Env, key: Symbol, value: i128) {
        env.storage().persistent().set(&key, &value);
    }

    pub fn read_without_ttl(env: Env, owner: Address) -> i128 {
        env.storage().persistent().get(&owner).unwrap_or(0)
    }

    pub fn write_with_ttl(env: Env, key: Symbol, value: i128) {
        env.storage().persistent().set(&key, &value);
        env.storage().persistent().extend_ttl(&key, 100, 1000);
    }

    pub fn instance_with_ttl(env: Env, key: Symbol, value: i128) {
        env.storage().instance().set(&key, &value);
        env.storage().instance().extend_ttl(100, 1000);
    }
}
