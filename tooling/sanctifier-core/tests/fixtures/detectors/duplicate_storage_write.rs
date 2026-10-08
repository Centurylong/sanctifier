#![no_std]

// FIXTURE: duplicate_storage_write detector

fn persist(env: Env, key: Symbol, value: i128, updated: i128) {
    env.storage().persistent().set(&key, &value);
    env.storage().persistent().set(&key, &value);
    env.storage().persistent().set(&key, &updated);
}
