#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Env};

#[test]
fn test_deposit() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, AMMContract);
    let client = AMMContractClient::new(&env, &contract_id);

    let token_a = Address::generate(&env);
    let token_b = Address::generate(&env);
    client.initialize(&token_a, &token_b);

    let user = Address::generate(&env);
    client.deposit(&user, &100, &100);
}
