#![no_std]
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Symbol};

#[contract]
pub struct SensitiveGetterContract;

#[contractimpl]
impl SensitiveGetterContract {
    pub fn get_signing_key(env: Env) -> BytesN<32> {
        env.storage().instance().get(&DataKey::SigningKey).unwrap()
    }

    pub fn get_profile(env: Env) -> BytesN<32> {
        env.storage().persistent().get(&DataKey::Credentials).unwrap()
    }

    pub fn get_owner(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Owner).unwrap()
    }

    pub fn get_version(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&Symbol::short("version"))
            .unwrap_or(1)
    }

    pub fn set_signing_key(env: Env, key: BytesN<32>) {
        env.storage().instance().set(&DataKey::SigningKey, &key);
    }
}
