use soroban_sdk::{contractimpl, Address, Env};

pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn set_fee_rate(env: Env, fee_rate: u32) {
        env.storage().instance().set(&"fee_rate", &fee_rate);
    }

    pub fn update_oracle(env: Env, oracle: Address) {
        env.storage().instance().set(&"oracle", &oracle);
    }

    pub fn schedule_reserve_limit(env: Env, reserve_limit: u128, eta: u64) {
        env.storage()
            .instance()
            .set(&"pending_reserve_limit", &reserve_limit);
        env.storage().instance().set(&"reserve_limit_eta", &eta);
    }

    pub fn execute_reserve_limit(env: Env) {
        let pending_reserve_limit: u128 = env
            .storage()
            .instance()
            .get(&"pending_reserve_limit")
            .unwrap();
        env.storage()
            .instance()
            .set(&"reserve_limit", &pending_reserve_limit);
    }

    // sanctifier:ignore[SANCT_TIMELOCK_PARAMETER_CHANGE]
    pub fn set_rate_for_test(env: Env, rate: u32) {
        env.storage().instance().set(&"rate", &rate);
    }
}
