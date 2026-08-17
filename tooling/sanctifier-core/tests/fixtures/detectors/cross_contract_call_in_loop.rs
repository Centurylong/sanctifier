#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec};

// FIXTURE: cross_contract_call_in_loop detector
// A cross-contract call issued from inside a loop scales its budget cost with
// the loop's iteration count.

#[contract]
pub struct CrossContractCallInLoopContract;

#[contractimpl]
impl CrossContractCallInLoopContract {
    // Violation: invoke_contract called once per loop iteration.
    pub fn broadcast(env: Env, targets: Vec<Address>) {
        for target in targets.iter() {
            env.invoke_contract::<()>(&target, &Symbol::new(&env, "ping"), Vec::new(&env));
        }
    }

    // Violation: client method call chained inside a while loop.
    pub fn drain(env: Env, token: Address, mut recipients: Vec<Address>) {
        while let Some(to) = recipients.pop_front() {
            TokenClient::new(&env, &token).transfer(&env.current_contract_address(), &to, &1);
        }
    }

    // Safe: cross-contract call outside any loop.
    pub fn ping_once(env: Env, target: Address) {
        env.invoke_contract::<()>(&target, &Symbol::new(&env, "ping"), Vec::new(&env));
    }

    // Safe: loop body has no cross-contract call.
    pub fn sum(env: Env, values: Vec<i128>) -> i128 {
        let mut total: i128 = 0;
        for v in values.iter() {
            total += v;
        }
        total
    }
}
