#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger, Env};

#[test]
fn test_timelock() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    
    // Set ledger timestamp to 100
    env.ledger().set_timestamp(100);
    
    client.initialize(&admin, &200);

    // Fast forward
    env.ledger().set_timestamp(201);
    
    // Should succeed
    client.execute();
}
