#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env};

#[contract]
pub struct EventCompletenessContract;

#[contractimpl]
impl EventCompletenessContract {
    pub fn set_balance(env: Env, who: Address, amount: i128) {
        env.storage().persistent().set(&who, &amount);
    }

    pub fn revoke(env: Env, who: Address) {
        env.storage().persistent().remove(&who);
    }

    pub fn set_balance_with_event(env: Env, who: Address, amount: i128) {
        env.storage().persistent().set(&who, &amount);
        env.events()
            .publish((symbol_short!("balance"), who), amount);
    }

    pub fn balance(env: Env, who: Address) -> Option<i128> {
        env.storage().persistent().get(&who)
    }

    fn reset_internal(env: Env, who: Address) {
        env.storage().persistent().remove(&who);
    }
}

struct Helper;
impl Helper {
    pub fn write(env: Env, who: Address) {
        env.storage().persistent().set(&who, &0_i128);
    }
}
