#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Symbol};

#[contract]
pub struct DeprecatedContract;

#[contractimpl]
impl DeprecatedContract {
    pub fn old_stuff(env: Env) {
        let key = Symbol::new(&env, "key");
        env.put_contract_data(&key, &123);
        let _: i32 = env.get_contract_data(&key).unwrap();
        env.has_contract_data(&key);
        env.remove_contract_data(&key);
        let _ = env.get_contract_id();
    }
}
